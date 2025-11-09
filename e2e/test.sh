#!/bin/bash
# E2E Test Script
# Tests both SQLite and PostgreSQL configurations

set -e

BASE_URL="${BASE_URL:-http://localhost:3000}"
API_KEY="${API_KEY:-e2e-test-key-001}"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Test counter
TESTS_RUN=0
TESTS_PASSED=0
TESTS_FAILED=0

log_info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

log_test() {
    echo -e "${YELLOW}[TEST]${NC} $1"
}

# Wait for service to be ready
wait_for_service() {
    log_info "Waiting for service at $BASE_URL..."
    max_attempts=30
    attempt=0
    
    while [ $attempt -lt $max_attempts ]; do
        if curl -s -f "$BASE_URL/healthz" > /dev/null 2>&1; then
            log_info "Service is ready!"
            return 0
        fi
        attempt=$((attempt + 1))
        sleep 1
    done
    
    log_error "Service did not become ready in time"
    return 1
}

# Test function
run_test() {
    local test_name="$1"
    local expected_status="$2"
    shift 2
    local curl_args=("$@")
    
    TESTS_RUN=$((TESTS_RUN + 1))
    log_test "$test_name"
    
    response=$(curl -s -w "\n%{http_code}" "${curl_args[@]}")
    status_code=$(echo "$response" | tail -n1)
    body=$(echo "$response" | sed '$d')
    
    if [ "$status_code" = "$expected_status" ]; then
        log_info "✓ PASS (Status: $status_code)"
        TESTS_PASSED=$((TESTS_PASSED + 1))
        echo "$body"
        return 0
    else
        log_error "✗ FAIL (Expected: $expected_status, Got: $status_code)"
        TESTS_FAILED=$((TESTS_FAILED + 1))
        echo "$body"
        return 1
    fi
}

# Main test suite
main() {
    log_info "Starting E2E Tests"
    log_info "Base URL: $BASE_URL"
    log_info "API Key: ${API_KEY:0:10}..."
    
    # Wait for service
    wait_for_service || exit 1
    
    echo ""
    log_info "=== Running E2E Tests ==="
    echo ""
    
    # Test 1: Health check
    run_test "Health check" "200" \
        -X GET "$BASE_URL/healthz"
    
    # Test 2: Unauthorized access (no API key)
    run_test "Unauthorized access without API key" "401" \
        -X GET "$BASE_URL/items"
    
    # Test 3: List items (empty)
    run_test "List items (empty)" "200" \
        -H "X-Api-Key: $API_KEY" \
        -X GET "$BASE_URL/items"
    
    # Test 4: Create item
    create_response=$(run_test "Create new item" "200" \
        -H "X-Api-Key: $API_KEY" \
        -H "Content-Type: application/json" \
        -X POST "$BASE_URL/items" \
        -d '{"name":"Test Item","description":"E2E test item","price":99.99}')
    
    # Test 5: List items (should have 1)
    list_response=$(run_test "List items (should have 1)" "200" \
        -H "X-Api-Key: $API_KEY" \
        -X GET "$BASE_URL/items")
    
    # Extract item ID from list response
    ITEM_ID=$(echo "$list_response" | grep -o '"id":[0-9]*' | head -1 | grep -o '[0-9]*')
    
    if [ -z "$ITEM_ID" ]; then
        log_error "Failed to extract item ID from response"
        ITEM_ID=1  # Fallback
    else
        log_info "Extracted Item ID: $ITEM_ID"
    fi
    
    # Test 6: Get specific item
    run_test "Get item by ID" "200" \
        -H "X-Api-Key: $API_KEY" \
        -X GET "$BASE_URL/items/$ITEM_ID"
    
    # Test 7: Update item
    run_test "Update item" "200" \
        -H "X-Api-Key: $API_KEY" \
        -H "Content-Type: application/json" \
        -X PUT "$BASE_URL/items/$ITEM_ID" \
        -d '{"name":"Updated Item","price":149.99}'
    
    # Test 8: Verify update
    updated_response=$(run_test "Verify item update" "200" \
        -H "X-Api-Key: $API_KEY" \
        -X GET "$BASE_URL/items/$ITEM_ID")
    
    if echo "$updated_response" | grep -q "Updated Item"; then
        log_info "✓ Item name was updated correctly"
    else
        log_error "✗ Item name was not updated"
        TESTS_FAILED=$((TESTS_FAILED + 1))
    fi
    
    # Test 9: Create second item
    run_test "Create second item" "200" \
        -H "X-Api-Key: $API_KEY" \
        -H "Content-Type: application/json" \
        -X POST "$BASE_URL/items" \
        -d '{"name":"Second Item","price":49.99}'
    
    # Test 10: List all items (should have 2)
    list_all=$(run_test "List all items (should have 2)" "200" \
        -H "X-Api-Key: $API_KEY" \
        -X GET "$BASE_URL/items")
    
    item_count=$(echo "$list_all" | grep -o '"id":' | wc -l | tr -d ' ')
    if [ "$item_count" -ge 2 ]; then
        log_info "✓ Found $item_count items as expected"
    else
        log_error "✗ Expected at least 2 items, found $item_count"
        TESTS_FAILED=$((TESTS_FAILED + 1))
    fi
    
    # Test 11: Delete item
    run_test "Delete item" "200" \
        -H "X-Api-Key: $API_KEY" \
        -X DELETE "$BASE_URL/items/$ITEM_ID"
    
    # Test 12: Verify deletion (should return 404)
    run_test "Verify item deletion (404)" "404" \
        -H "X-Api-Key: $API_KEY" \
        -X GET "$BASE_URL/items/$ITEM_ID"
    
    # Test 13: Invalid API key
    run_test "Invalid API key" "401" \
        -H "X-Api-Key: invalid-key" \
        -X GET "$BASE_URL/items"
    
    # Test 14: Large payload test
    run_test "Create item with large description" "200" \
        -H "X-Api-Key: $API_KEY" \
        -H "Content-Type: application/json" \
        -X POST "$BASE_URL/items" \
        -d "{\"name\":\"Large Item\",\"description\":\"$(printf 'x%.0s' {1..1000})\"}"
    
    echo ""
    log_info "=== Test Summary ==="
    echo "Total Tests: $TESTS_RUN"
    echo -e "${GREEN}Passed: $TESTS_PASSED${NC}"
    echo -e "${RED}Failed: $TESTS_FAILED${NC}"
    
    if [ $TESTS_FAILED -eq 0 ]; then
        log_info "All tests passed! ✓"
        exit 0
    else
        log_error "Some tests failed!"
        exit 1
    fi
}

main "$@"
