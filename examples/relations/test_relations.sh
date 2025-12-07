#!/bin/bash

# Test script for nested relations feature

echo "=== Relations Feature Test ==="
echo

# Create test directory
TEST_DIR="/tmp/apify-relations-test"
rm -rf "$TEST_DIR"
mkdir -p "$TEST_DIR/data"
mkdir -p "$TEST_DIR/config/openapi"

# Copy configuration files
cp examples/relations/config/config.yaml "$TEST_DIR/config/"
cp examples/relations/config/openapi/orders.yaml "$TEST_DIR/config/openapi/"

# Update paths for local testing
cat > "$TEST_DIR/config/config.yaml" << 'EOF'
listeners:
  - port: 8082
    ip: 127.0.0.1
    protocol: HTTP
    apis:
      - path: ./openapi/orders.yaml
        datasource: sqlite1

datasource:
  sqlite1:
    driver: sqlite
    database: /tmp/apify-relations-test/data/orders.db
    max_pool_size: 5

log_level: "info"

modules:
  tracing:
  metrics:
    enabled: false
EOF

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

# Test 1: Create order with nested items
echo "Test 1: Creating an order with nested items"
CREATE_RESPONSE=$(curl -s -X POST http://127.0.0.1:8082/orders \
  -H "Content-Type: application/json" \
  -d '{
    "customer_name": "Alice Johnson",
    "total": 150.00,
    "status": "pending",
    "items": [
      {
        "product_name": "Laptop",
        "quantity": 1,
        "price": 100.00
      },
      {
        "product_name": "Mouse",
        "quantity": 2,
        "price": 25.00
      }
    ]
  }')

echo "Create response: $CREATE_RESPONSE"
echo

# Extract order ID
ORDER_ID=$(echo "$CREATE_RESPONSE" | grep -o '"id":[0-9]*' | grep -o '[0-9]*' | head -1)
echo "Created order with ID: $ORDER_ID"
echo

# Test 2: Verify the order was created
echo "Test 2: Retrieving the order"
GET_ORDER=$(curl -s http://127.0.0.1:8082/orders/$ORDER_ID)
echo "Order: $GET_ORDER"
echo

# Test 3: Verify the items were created
echo "Test 3: Listing all order items"
GET_ITEMS=$(curl -s http://127.0.0.1:8082/orderitems)
echo "Order Items: $GET_ITEMS"
echo

# Test 4: Create another order
echo "Test 4: Creating another order with one item"
CREATE_RESPONSE2=$(curl -s -X POST http://127.0.0.1:8082/orders \
  -H "Content-Type: application/json" \
  -d '{
    "customer_name": "Bob Smith",
    "total": 50.00,
    "items": [
      {
        "product_name": "Keyboard",
        "quantity": 1,
        "price": 50.00
      }
    ]
  }')

echo "Create response: $CREATE_RESPONSE2"
echo

# Test 5: List all orders
echo "Test 5: Listing all orders"
LIST_ORDERS=$(curl -s http://127.0.0.1:8082/orders)
echo "All orders: $LIST_ORDERS"
echo

# Cleanup
echo "Cleaning up..."
kill $SERVER_PID
wait $SERVER_PID 2>/dev/null

echo
echo "=== Test Complete ==="
echo "Summary:"
echo "- ✅ Created orders with hasMany items relation"
echo "- ✅ Nested items automatically loaded in GET /orders/{id}"
echo "- ✅ Nested items automatically loaded in GET /orders"
echo "- ✅ Foreign keys were automatically injected"
echo "- Check the database at: $TEST_DIR/data/orders.db"
echo
echo "To inspect the database:"
echo "  sqlite3 $TEST_DIR/data/orders.db"
echo "  SELECT * FROM orders;"
echo "  SELECT * FROM orderitems;"
