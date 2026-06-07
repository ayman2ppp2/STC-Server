#!/bin/bash
# Test enrollment flow - token generation and device enrollment

set -e

BASE_URL="${BASE_URL:-http://localhost:8080}"
KEY_PATH="${KEY_PATH:-./test-key.pem}"

echo "=== STC Server Enrollment Test ==="
echo "Base URL: $BASE_URL"
echo ""

# Test 1: Generate token for a registered taxpayer
echo "1. Generating enrollment token..."
echo "   Note: taxpayer TIN must exist in taxpayers table"
echo ""

# Replace with actual TIN from your taxpayers table
TAXPAYER_TIN="${TAXPAYER_TIN:-100011}"
TAXPAYER_PASSWORD="${TAXPAYER_PASSWORD:-password}"

RESPONSE=$(curl -s -X POST "$BASE_URL/e-invoicing/token" \
  -H "Content-Type: application/json" \
  -d "{
    \"tin\": \"$TAXPAYER_TIN\",
    \"password\": \"$TAXPAYER_PASSWORD\"
  }")

echo "$RESPONSE" | jq '.'
TOKEN=$(echo "$RESPONSE" | jq -r '.data.token')
echo ""

if [ "$TOKEN" = "null" ] || [ -z "$TOKEN" ]; then
    echo "ERROR: Failed to get token. Check if taxpayer exists."
    exit 1
fi

echo "   Token received: ${TOKEN:0:50}..."
echo ""

# Test 2: Enroll device with CSR
echo "2. Enrolling device..."
echo "   Note: You need a valid CSR with:"
echo "   - SerialNumber field = device UUID"
echo "   - OrganizationName field = TIN"
echo ""

# Check if key file exists for CSR generation
if [ ! -f "$KEY_PATH" ]; then
    echo "   Creating test key..."
    openssl genrsa -out "$KEY_PATH" 2048 2>/dev/null
fi

# Generate CSR with proper fields
DEVICE_UUID=$(uuidgen 2>/dev/null || echo "550e8400-e29b-41d4-a716-446655440000")
CSR_DER=$(openssl req -new \
    -key "$KEY_PATH" \
    -outform DER \
    -subj "/O=$TAXPAYER_TIN/serialNumber=$DEVICE_UUID" 2>/dev/null | base64 -w 0)

echo "   Device UUID: $DEVICE_UUID"
echo "   CSR generated (base64 DER)"
echo ""

ENROLL_RESPONSE=$(curl -s -X POST "$BASE_URL/prod/enrollment/enroll" \
    -H "Content-Type: application/json" \
    -d "{
        \"token\": \"$TOKEN\",
        \"csr\": \"$CSR_DER\"
    }")

echo "$ENROLL_RESPONSE" | jq '.'
echo ""

# Check if enrollment succeeded
SUCCESS=$(echo "$ENROLL_RESPONSE" | jq -r '.success')
if [ "$SUCCESS" = "true" ]; then
    echo "✅ Device enrolled successfully!"
    echo "   Certificate received (save from response)"
else
    echo "❌ Enrollment failed. Check error details above."
fi

echo ""
echo "=== Test Complete ==="
