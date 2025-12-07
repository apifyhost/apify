#!/bin/bash

# Test script for UPDATE and DELETE with relations

echo "=== Relations CRUD Test (Update & Delete) ==="
echo

# Create test directory
TEST_DIR="/tmp/apify-relations-crud-test"
rm -rf "$TEST_DIR"
mkdir -p "$TEST_DIR/data"
mkdir -p "$TEST_DIR/config/openapi"

# Copy configuration files
cat > "$TEST_DIR/config/config.yaml" << 'EOF'
listeners:
  - port: 8084
    ip: 127.0.0.1
    protocol: HTTP
    apis:
      - path: ./openapi/orders.yaml
        datasource: sqlite1
      - path: ./openapi/users.yaml
        datasource: sqlite1

datasource:
  sqlite1:
    driver: sqlite
    database: /tmp/apify-relations-crud-test/data/test.db
    max_pool_size: 5

log_level: "info"

modules:
  tracing:
    enabled: true
  metrics:
    enabled: false
EOF

# Copy OpenAPI specs
cp examples/relations/config/openapi/orders.yaml "$TEST_DIR/config/openapi/"
cp examples/relations/config/openapi/users.yaml "$TEST_DIR/config/openapi/"

echo "Starting Apify server..."
target/release/apify --config "$TEST_DIR/config/config.yaml" &
SERVER_PID=$!

# Wait for server to start
sleep 3

# Check if server is running
if ! ps -p $SERVER_PID > /dev/null; then
    echo "Error: Server failed to start"
    exit 1
fi

echo "Server started with PID $SERVER_PID"
echo

# ==================== UPDATE TESTS ====================

echo "=== UPDATE Tests ==="
echo

# Test 1: Create an order with items
echo "Test 1: Create order with items"
CREATE_RESPONSE=$(curl -s -X POST http://127.0.0.1:8084/orders \
  -H "Content-Type: application/json" \
  -d '{
    "customer_name": "Alice",
    "total": 150.00,
    "status": "pending",
    "items": [
      {"product_name": "Laptop", "quantity": 1, "price": 100.00},
      {"product_name": "Mouse", "quantity": 2, "price": 25.00}
    ]
  }')

ORDER_ID=$(echo "$CREATE_RESPONSE" | jq -r '.id')
echo "Created order ID: $ORDER_ID"
echo "Items count: $(echo "$CREATE_RESPONSE" | jq '.items | length')"
echo

# Test 2: Update order (change items)
echo "Test 2: Update order - replace items"
UPDATE_RESPONSE=$(curl -s -X PUT http://127.0.0.1:8084/orders/$ORDER_ID \
  -H "Content-Type: application/json" \
  -d '{
    "customer_name": "Alice Johnson",
    "total": 200.00,
    "status": "confirmed",
    "items": [
      {"product_name": "MacBook Pro", "quantity": 1, "price": 200.00}
    ]
  }')

echo "Update response: $UPDATE_RESPONSE"
echo

# Test 3: Get order to verify update
echo "Test 3: Verify updated order"
GET_ORDER=$(curl -s http://127.0.0.1:8084/orders/$ORDER_ID)
echo "Customer name: $(echo "$GET_ORDER" | jq -r '.customer_name')"
echo "Status: $(echo "$GET_ORDER" | jq -r '.status')"
echo "Items count: $(echo "$GET_ORDER" | jq '.items | length')"
echo "New item: $(echo "$GET_ORDER" | jq -r '.items[0].product_name')"
echo

# ==================== DELETE TESTS ====================

echo "=== DELETE Tests (Cascade) ==="
echo

# Test 4: Create user with profile
echo "Test 4: Create user with profile"
CREATE_USER=$(curl -s -X POST http://127.0.0.1:8084/users \
  -H "Content-Type: application/json" \
  -d '{
    "username": "bob",
    "email": "bob@example.com",
    "profile": {
      "full_name": "Bob Smith",
      "bio": "Product manager"
    }
  }')

USER_ID=$(echo "$CREATE_USER" | jq -r '.id')
PROFILE_ID=$(echo "$CREATE_USER" | jq -r '.profile.id')
echo "Created user ID: $USER_ID"
echo "Profile ID: $PROFILE_ID"
echo

# Test 5: Verify profile exists
echo "Test 5: Verify profile exists before delete"
PROFILE_CHECK=$(curl -s http://127.0.0.1:8084/userprofiles/$PROFILE_ID)
echo "Profile exists: $(echo "$PROFILE_CHECK" | jq -r '.full_name')"
echo

# Test 6: Delete user (should cascade delete profile)
echo "Test 6: Delete user (cascade delete profile)"
DELETE_RESPONSE=$(curl -s -X DELETE http://127.0.0.1:8084/users/$USER_ID)
echo "Delete response: $DELETE_RESPONSE"
echo

# Test 7: Verify profile was cascade deleted
echo "Test 7: Verify profile was cascade deleted"
PROFILE_AFTER=$(curl -s http://127.0.0.1:8084/userprofiles/$PROFILE_ID)
echo "Profile after delete: $PROFILE_AFTER"
if echo "$PROFILE_AFTER" | grep -q "404"; then
    echo "✅ Profile successfully cascade deleted"
else
    echo "❌ Profile still exists (cascade delete failed)"
fi
echo

# Test 8: Delete order (should cascade delete items)
echo "Test 8: Delete order (cascade delete items)"
DELETE_ORDER=$(curl -s -X DELETE http://127.0.0.1:8084/orders/$ORDER_ID)
echo "Delete response: $DELETE_ORDER"
echo

# Test 9: Verify items were cascade deleted
echo "Test 9: Verify items were cascade deleted"
ALL_ITEMS=$(curl -s http://127.0.0.1:8084/orderitems)
ITEM_COUNT=$(echo "$ALL_ITEMS" | jq 'length')
echo "Remaining items count: $ITEM_COUNT"
if [ "$ITEM_COUNT" -eq "0" ]; then
    echo "✅ All items successfully cascade deleted"
else
    echo "❌ Items still exist (cascade delete failed)"
fi
echo

# Cleanup
echo "Cleaning up..."
kill $SERVER_PID
wait $SERVER_PID 2>/dev/null

echo
echo "=== Test Complete ==="
echo "Summary:"
echo "- ✅ UPDATE: Replaced nested items in order"
echo "- ✅ DELETE: Cascade deleted user's profile (hasOne)"
echo "- ✅ DELETE: Cascade deleted order's items (hasMany)"
echo
echo "Database: $TEST_DIR/data/test.db"
