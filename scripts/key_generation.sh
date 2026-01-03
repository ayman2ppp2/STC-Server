#!/usr/bin/env bash
set -euo pipefail

# -------------------------
# Directory structure
# -------------------------
BASE_DIR="stc-pki"
ROOT_DIR="$BASE_DIR/root"
SERVER_DIR="$BASE_DIR/server"
B64_DIR="$BASE_DIR/base64"

SERVER_CN="api.stc.gov.sd"
RENDER_DOMAIN="stc-server.onrender.com"

mkdir -p "$ROOT_DIR" "$SERVER_DIR" "$B64_DIR"

# -------------------------
# 1. Generate Root CA (offline)
# -------------------------
if [[ ! -f "$ROOT_DIR/stc_root_ca.key" ]]; then
  echo "==> Generating STC Root CA (offline)"
  openssl genpkey -algorithm RSA \
    -out "$ROOT_DIR/stc_root_ca.key" \
    -pkeyopt rsa_keygen_bits:4096
  chmod 600 "$ROOT_DIR/stc_root_ca.key"

  openssl req -x509 -new -key "$ROOT_DIR/stc_root_ca.key" \
    -out "$ROOT_DIR/stc_root_ca.crt" \
    -days 3650 -sha256 \
    -subj "/C=SD/O=STC/CN=STC Root CA"

  echo "Root CA created."
else
  echo "Root CA already exists, skipping."
fi

# -------------------------
# 2. Create server OpenSSL config
# -------------------------
cat > "$SERVER_DIR/server.cnf" <<EOF
[req]
prompt = no
default_md = sha256
distinguished_name = dn
req_extensions = req_ext

[dn]
C = SD
O = STC
CN = $SERVER_CN

[req_ext]
subjectAltName = @alt_names

[alt_names]
DNS.1 = $SERVER_CN
DNS.2 = $RENDER_DOMAIN
EOF

# -------------------------
# 3. Generate server private key
# -------------------------
echo "==> Generating server private key"
openssl genpkey -algorithm RSA \
  -out "$SERVER_DIR/server.key" \
  -pkeyopt rsa_keygen_bits:2048
chmod 600 "$SERVER_DIR/server.key"

# -------------------------
# 4. Generate server CSR
# -------------------------
echo "==> Generating server CSR"
openssl req -new \
  -key "$SERVER_DIR/server.key" \
  -out "$SERVER_DIR/server.csr" \
  -config "$SERVER_DIR/server.cnf"

# -------------------------
# 5. Sign server certificate with Root CA
# -------------------------
echo "==> Signing server certificate with Root CA"
openssl x509 -req \
  -in "$SERVER_DIR/server.csr" \
  -CA "$ROOT_DIR/stc_root_ca.crt" \
  -CAkey "$ROOT_DIR/stc_root_ca.key" \
  -CAcreateserial \
  -out "$SERVER_DIR/server.crt" \
  -days 825 -sha256 \
  -extensions req_ext \
  -extfile "$SERVER_DIR/server.cnf"

# -------------------------
# 6. Base64 encode server key & cert for Render env vars
# -------------------------
echo "==> Converting server key and cert to Base64 for environment variables"
base64 -w 0 "$SERVER_DIR/server.key" > "$B64_DIR/server.key.b64"
base64 -w 0 "$SERVER_DIR/server.crt" > "$B64_DIR/server.crt.b64"
base64 -w 0 "$ROOT_DIR/stc_root_ca.crt" > "$B64_DIR/stc_root_ca.crt.b64"




echo
echo "==> DONE"
echo
echo "Files generated:"
echo "Root CA key:       $ROOT_DIR/stc_root_ca.key  (keep offline!)"
echo "Root CA cert:      $ROOT_DIR/stc_root_ca.crt"
echo "Server key:        $SERVER_DIR/server.key"
echo "Server cert:       $SERVER_DIR/server.crt"
echo "Server key Base64: $B64_DIR/server.key.b64"
echo "Server cert Base64:$B64_DIR/server.crt.b64"
echo "Root CA cert Base64:$B64_DIR/stc_root_ca.crt.b64"
echo
echo "You can now set these Base64 contents as environment variables in Render:"
echo "SERVER_KEY_B64, SERVER_CERT_B64, STC_CA_CERT_B64"
