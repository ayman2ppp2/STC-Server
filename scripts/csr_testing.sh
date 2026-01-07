#!/usr/bin/env bash
set -euo pipefail

ENROLL_URL="https://stc-server.onrender.com/enroll"
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



JSON_PAYLOAD=$(printf '{"csr_base64":"%s"}' "$CSR_FILE")

curl -v -X POST "$ENROLL_URL" \
  -H "Content-Type: application/json" \
  --data-binary "$JSON_PAYLOAD"

rm -rf "$TMP_DIR"

