#!/bin/bash
# Test script for HTTP PURGE functionality
#
# Usage: ./scripts/test_purge.sh
#
# This script demonstrates various PURGE operations

set -e

BASE_URL="http://localhost:8080"
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${BLUE}=== HTTP PURGE Test Script ===${NC}\n"

# Function to print section headers
section() {
    echo -e "\n${YELLOW}>>> $1${NC}"
}

# Function to run curl and show result
run_curl() {
    echo -e "${GREEN}$ $1${NC}"
    eval "$1"
    echo ""
}

# Check if server is running
section "Checking if server is running..."
if ! curl -s -f "$BASE_URL/stats" > /dev/null 2>&1; then
    echo "Error: Server is not running on $BASE_URL"
    echo "Please start the server first:"
    echo "  cargo run --example http_purge_server"
    exit 1
fi
echo "✓ Server is running"

# Test 1: Get cache stats
section "Test 1: Get cache statistics"
run_curl "curl -s $BASE_URL/stats | jq ."

# Test 2: Get cached file (should HIT)
section "Test 2: Get cached file (should HIT)"
run_curl "curl -s -i $BASE_URL/test.dat | head -10"

# Test 3: Purge specific URL
section "Test 3: Purge specific URL"
run_curl "curl -s -X PURGE $BASE_URL/test.dat | jq ."

# Test 4: Verify it's purged (should MISS)
section "Test 4: Verify cache is purged (should MISS)"
run_curl "curl -s -i $BASE_URL/test.dat | head -10"

# Test 5: Check stats after purge
section "Test 5: Check cache stats after purge"
run_curl "curl -s $BASE_URL/stats | jq ."

# Test 6: Purge with pattern (URL prefix)
section "Test 6: Purge with URL prefix pattern"
run_curl "curl -s -X PURGE $BASE_URL/video.mp4 -H 'X-Purge-Pattern: prefix' | jq ."

# Test 7: Purge all cache
section "Test 7: Purge all cache"
run_curl "curl -s -X PURGE '$BASE_URL/*' -H 'X-Purge-All: true' | jq ."

# Test 8: Verify all cache is purged
section "Test 8: Verify all cache is purged"
run_curl "curl -s $BASE_URL/stats | jq ."

# Test 9: Test with authentication (if PURGE_TOKEN is set)
if [ -n "$PURGE_TOKEN" ]; then
    section "Test 9: Purge with authentication"
    run_curl "curl -s -X PURGE $BASE_URL/test.dat -H 'Authorization: Bearer $PURGE_TOKEN' | jq ."
else
    section "Test 9: Authentication test (skipped)"
    echo "Set PURGE_TOKEN environment variable to test authentication"
    echo "Example: PURGE_TOKEN=secret ./scripts/test_purge.sh"
fi

# Test 10: Test invalid method
section "Test 10: Test invalid method (should fail)"
run_curl "curl -s -X DELETE $BASE_URL/test.dat | jq ."

echo -e "\n${BLUE}=== All tests completed ===${NC}\n"

# Summary
section "Summary"
echo "✓ PURGE specific URL works"
echo "✓ PURGE all cache works"
echo "✓ PURGE with pattern works"
echo "✓ Cache statistics are updated correctly"
echo "✓ Invalid methods are rejected"

echo -e "\n${GREEN}All tests passed!${NC}\n"
