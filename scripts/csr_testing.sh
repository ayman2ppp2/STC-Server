#!/usr/bin/env bash
set -euo pipefail

BASE_URL="${BASE_URL:-http://localhost:8080}"
TAXPAYER_TIN="${TAXPAYER_TIN:-100011}"
TMP_DIR="$(mktemp -d)"

cleanup() {
  rm -rf "$TMP_DIR"
}
trap cleanup EXIT

KEY_FILE="$TMP_DIR/device.key"

if command -v uuidgen >/dev/null 2>&1; then
  DEVICE_UUID="$(uuidgen)"
else
  DEVICE_UUID="550e8400-e29b-41d4-a716-446655440000"
fi

echo "=== STC CSR Enrollment Helper ==="
echo "Base URL: $BASE_URL"
echo "Taxpayer TIN: $TAXPAYER_TIN"
echo "Device UUID: $DEVICE_UUID"
echo ""

echo "1. Generating enrollment token..."
TOKEN_RESPONSE=$(curl -s -X POST "$BASE_URL/onboard" \
  -H "Content-Type: application/json" \
  -d "{\"name\":\"CSR Test\",\"email\":\"csr-test@example.com\",\"company_id\":\"$TAXPAYER_TIN\"}")

echo "$TOKEN_RESPONSE" | jq '.' 2>/dev/null || echo "$TOKEN_RESPONSE"

if command -v jq >/dev/null 2>&1; then
  TOKEN="$(echo "$TOKEN_RESPONSE" | jq -r '.token // empty')"
else
  TOKEN="$(printf '%s' "$TOKEN_RESPONSE" | sed -n 's/.*"token"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p')"
fi

if [[ -z "$TOKEN" ]]; then
  echo "ERROR: Failed to extract enrollment token. Check taxpayer seed data and server logs."
  exit 1
fi

echo ""
echo "2. Generating DER CSR..."
openssl genrsa -out "$KEY_FILE" 2048 >/dev/null 2>&1
CSR_DER=$(openssl req -new \
  -key "$KEY_FILE" \
  -outform DER \
  -subj "/O=$TAXPAYER_TIN/serialNumber=$DEVICE_UUID" 2>/dev/null | base64 -w 0)

echo "CSR generated."
echo ""

echo "3. Enrolling device..."
ENROLL_RESPONSE=$(curl -s -X POST "$BASE_URL/enroll" \
  -H "Content-Type: application/json" \
  -d "{\"token\":\"$TOKEN\",\"csr\":\"$CSR_DER\"}")

echo "$ENROLL_RESPONSE" | jq '.' 2>/dev/null || echo "$ENROLL_RESPONSE"

if command -v jq >/dev/null 2>&1; then
  SUCCESS="$(echo "$ENROLL_RESPONSE" | jq -r '.success // false')"
  if [[ "$SUCCESS" == "true" ]]; then
    echo ""
    echo "Device enrolled successfully."
  else
    echo ""
    echo "Enrollment failed."
    exit 1
  fi
fi
