#!/bin/bash
# Run all integration tests
# Usage: ./scripts/tests/run_all.sh [BASE_URL]

set -e

BASE_URL="${1:-http://localhost:8080}"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

echo "========================================"
echo "  STC Server Integration Tests"
echo "========================================"
echo ""
echo "Base URL: $BASE_URL"
echo ""

# Export BASE_URL for child scripts
export BASE_URL

# Check server is running
echo "Checking if server is running..."
if ! curl -s --max-time 2 "$BASE_URL/health_check" > /dev/null 2>&1; then
    echo "❌ ERROR: Server not responding at $BASE_URL"
    echo "   Start server with: cargo run"
    exit 1
fi
echo "✅ Server is running"
echo ""

# Run tests
echo "========================================"
echo "  1. Testing Clearance Endpoint"
echo "========================================"
bash "$SCRIPT_DIR/test_clearance.sh"
echo ""

echo "========================================"
echo "  2. Testing Reporting Endpoint"
echo "========================================"
bash "$SCRIPT_DIR/test_reporting.sh"
echo ""

echo "========================================"
echo "  3. Testing Enrollment Flow"
echo "========================================"
bash "$SCRIPT_DIR/test_enrollment.sh"
echo ""

echo "========================================"
echo "  All Tests Complete"
echo "========================================"
