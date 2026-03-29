#!/bin/bash
# Test invoice submission to reporting endpoint with sandbox mode

set -e

BASE_URL="${BASE_URL:-http://localhost:8080}"

# Sample invoice UUID
UUID="8d487816-70b8-4ade-a618-9d620b73814a"

# Sample invoice hash (SHA256 base64 encoded)
INVOICE_HASH="ITLDpoQ8InzLMDaYuK8prsmRRjs/cLHX91STO4SVMvU="

# Minimal test invoice (base64 encoded UBL invoice)
INVOICE_B64="PD94bWwgdmVyc2lvbj0iMS4wIiBlbmNvZGluZz0iVVRGLTgiPz48SW52b2ljZSB4bWxucz0idXJuOm9hc2lzOm5hbWVzOnNwZWNpZmljYXRpb246dWJsOnNjaGVtYTp4c2Q6SW52b2ljZS0yIiB4bWxuczpjYmM9InVybjpvYXNpczpuYW1lczpzcGVjaWZpY2F0aW9uOnVibDpzY2hlbWE6eHNkOkNvbW1vbkJhc2ljQ29tcG9uZW50cy0yIiB4bWxuczpleHQ9InVybjpvYXNpczpuYW1lczpzcGVjaWZpY2F0aW9uOnVibDpzY2hlbWE6eHNkOkNvbW1vbkV4dGVuc2lvbkNvbXBvbmVudHMtMiI+PGNjYzpQcm9maWxlSUQ+cmVwb3J0aW5nOjEuMDwvY2JjOlByb2ZpbGVJRD48Y2JjOklEPnNhbXBsZS1pbnZvaWNlPC9jYmM6SUQ+PGNjYjpJc3N1ZWRDYXRlPjIwMjQtMDEtMDFUMDA6MDA6MDBaPC9jYmM6SXNzdWVkRGF0ZT48Y2JjOkludm9pY2VUeXBlQ29kZSBjb2RlPSJzb2xlcyIvPjxjYmM6TG9jYWxLZXlDb2RlPmE8L2NiYzpMb2NhbEtleUNvZGU+PGNhYzpBdGNhY3R1YWxTZXR0bGVkQWNjb3VudGluZ1Bvc3RhbEFkZHJlc3M+PGNhYzpTdHJlZXROYW1lPlRlc3QgU3RyZWV0PC9jYWM6U3RyZWV0TmFtZT48Y2FjOlN0cmVldE5hbWU+PC9hY2M6U3RyZWV0TmFtZT48Y2FjOkJ1aWxkaW5nTnVtYmVyPjEyMzQ8L2NhYzpCdWlsZGluZ051bWJlcj48Y2FjOkNpdHlTdWJEaXZpc2lvbk5hbWU+PC9hY2M6Q2l0eVN1YkRpdmlzaW9uTmFtZT48Y2FjOkNpdHlOYW1lPlRlc3QgQ2l0eTwvY2FjOkNpdHlOYW1lPj48Y2FjOkNvdW50cnk+PENCBDpJZGVudGlmaWNhdGlvbkNvZGU+U0E8L0NCBDpJZGVudGlmaWNhdGlvbkNvZGU+PC9hY2M6Q291bnRyeT48L2NhYzpBY3R1YWxTZXR0bGVkQWNjb3VudGluZ1Bvc3RhbEFkZHJlc3M+PGNhYzpCdXllckFjY291bnRpbmc+PGNhYzpQYXJ0eT48Y2FjOlBhcnR5SWRlbnRpZmljYXRpb24+PGNhYzpJRD4xMjM0NTY3ODkwPC9jYWM6SUQ+PC9hY2M6UGFydHlJZGVudGlmaWNhdGlvbj48L2NhYzpQYXJ0eT48L2NhYzpCdXllckFjY291bnRpbmc+PC9JbnZvaWNlPg=="

echo "=== STC Server Reporting Test ==="
echo "Base URL: $BASE_URL"
echo ""

# Test 1: Health check
echo "1. Testing health check..."
curl -s "$BASE_URL/health_check" && echo ""
echo ""

# Test 2: Reporting with sandbox mode
echo "2. Testing reporting endpoint (sandbox mode)..."
echo "   Sandbox mode skips: UUID check, PIH chain, database persistence"
echo ""

RESPONSE=$(curl -s -X POST "$BASE_URL/reporting" \
  -H "Content-Type: application/json" \
  -H "X-Sandbox-Mode: true" \
  -d "{
    \"invoice_hash\": \"$INVOICE_HASH\",
    \"uuid\": \"$UUID\",
    \"invoice\": \"$INVOICE_B64\"
  }")

echo "$RESPONSE" | jq '.' 2>/dev/null || echo "$RESPONSE"
echo ""

# Test 3: Reporting without sandbox (requires full validation)
echo "3. Testing reporting endpoint (production mode)..."
echo "   Production mode validates: UUID, PIH chain, TINs, signatures"
echo ""

RESPONSE=$(curl -s -X POST "$BASE_URL/reporting" \
  -H "Content-Type: application/json" \
  -d "{
    \"invoice_hash\": \"$INVOICE_HASH\",
    \"uuid\": \"$UUID\",
    \"invoice\": \"$INVOICE_B64\"
  }")

echo "$RESPONSE" | jq '.' 2>/dev/null || echo "$RESPONSE"
echo ""

echo "=== Test Complete ==="
