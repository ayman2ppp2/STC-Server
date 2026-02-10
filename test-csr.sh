#!/bin/bash

# 1. Use absolute path (avoid ~ in variables if possible)
KEY_PATH="/home/ayman/programing/Rust/STC-Server/stc-pki/base64/server.key.b64"
TOKEN="5003:c5cc9ce0-3913-45a8-b076-02fed692c4ca"
URL="http://localhost:8080/enroll"

# 2. Safety Check: Does the file exist?
if [ ! -f "$KEY_PATH" ]; then
    echo "Error: Key file not found at $KEY_PATH"
    exit 1
fi

echo "Decoding key and generating CSR..."

# 3. Generate CSR
# We decode the key to a variable first to ensure it's valid
RAW_KEY=$(base64 -d "$KEY_PATH")

CSR_BASE64=$(openssl req -new \
  -key <(echo "$RAW_KEY") \
  -outform DER \
  -subj "/C=US/ST=State/L=City/O=Organization/OU=IT/CN=yourdomain.com/serialNumber=5003" | base64 -w 0)

# 4. Final Safety: Check if CSR_BASE64 is empty
if [ -z "$CSR_BASE64" ]; then
    echo "Error: Failed to generate CSR"
    exit 1
fi

# 5. Build JSON and POST
echo "Sending Base64-encoded DER CSR to $URL..."

jq -n \
  --arg csr "$CSR_BASE64" \
  --arg token "$TOKEN" \
  '{csr: $csr, token: $token}' | \
  curl -X POST "$URL" \
  -H "Content-Type: application/json" \
  -d @-