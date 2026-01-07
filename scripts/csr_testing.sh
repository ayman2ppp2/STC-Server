#!/usr/bin/env bash
set -euo pipefail
#"https://stc-server.onrender.com/enroll"
ENROLL_URL="http://localhost:8080/enroll"
TMP_DIR="$(mktemp -d)"

KEY_FILE="$TMP_DIR/client.key"
CSR_FILE="$TMP_DIR/client.csr"

RAND_HEX=$(openssl rand -hex 6)

SUBJECT="/C=SD/ST=Khartoum/L=Khartoum/O=TestBusiness-${RAND_HEX}/OU=Enrollment/CN=client-${RAND_HEX}"

openssl genrsa -out "$KEY_FILE" 2048

openssl req -new \
  -key "$KEY_FILE" \
  -out "$CSR_FILE" \
  -subj "$SUBJECT"


JSON_PAYLOAD=$(jq -n --arg csr "$(cat "$CSR_FILE")" '{csr:$csr}')

echo "$JSON_PAYLOAD" | jq .

curl -v -X POST "$ENROLL_URL" \
  -H "Content-Type: application/json" \
  --data-binary "$JSON_PAYLOAD"

rm -rf "$TMP_DIR"

