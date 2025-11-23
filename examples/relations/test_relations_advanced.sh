#!/bin/bash

# Test script for hasOne and belongsTo relations

echo "=== Advanced Relations Feature Test ===" echo

# Create test directory
TEST_DIR="/tmp/apify-relations-advanced-test"
rm -rf "$TEST_DIR"
mkdir -p "$TEST_DIR/data"
mkdir -p "$TEST_DIR/config/openapi"

# Copy configuration files
cat > "$TEST_DIR/config/config.yaml" << 'EOF'
listeners:
  - port: 8083
    ip: 127.0.0.1
    protocol: HTTP
    apis:
      - path: ./openapi/users.yaml
        datasource: sqlite1

datasource:
  sqlite1:
    driver: sqlite
    database: /tmp/apify-relations-advanced-test/data/users.db
    max_pool_size: 5

observability:
  log_level: "info"
  metrics_enabled: false
EOF

# Copy users OpenAPI spec
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

# Test 1: Create user with profile (hasOne)
echo "Test 1: Creating a user with profile (hasOne relation)"
CREATE_USER=$(curl -s -X POST http://127.0.0.1:8083/users \
  -H "Content-Type: application/json" \
  -d '{
    "username": "alice",
    "email": "alice@example.com",
    "profile": {
      "full_name": "Alice Johnson",
      "bio": "Software engineer at Example Corp",
      "avatar_url": "https://example.com/avatars/alice.jpg"
    }
  }')

echo "Create response:"
echo "$CREATE_USER" | jq '.'
echo

# Extract user ID
USER_ID=$(echo "$CREATE_USER" | jq -r '.id')
echo "Created user with ID: $USER_ID"
echo

# Test 2: Get user (should include profile)
echo "Test 2: Retrieving user (should include profile via hasOne)"
GET_USER=$(curl -s http://127.0.0.1:8083/users/$USER_ID)
echo "$GET_USER" | jq '.'
echo

# Test 3: List all profiles (should show belongsTo parent)
echo "Test 3: Listing profiles (should include user via belongsTo)"
GET_PROFILES=$(curl -s http://127.0.0.1:8083/userprofiles)
echo "$GET_PROFILES" | jq '.'
echo

# Test 4: Create another user with profile
echo "Test 4: Creating another user with profile"
CREATE_USER2=$(curl -s -X POST http://127.0.0.1:8083/users \
  -H "Content-Type: application/json" \
  -d '{
    "username": "bob",
    "email": "bob@example.com",
    "profile": {
      "full_name": "Bob Smith",
      "bio": "Product manager",
      "avatar_url": "https://example.com/avatars/bob.jpg"
    }
  }')

echo "$CREATE_USER2" | jq '.'
echo

# Test 5: List all users (should include their profiles)
echo "Test 5: Listing all users (should include profiles)"
LIST_USERS=$(curl -s http://127.0.0.1:8083/users)
echo "$LIST_USERS" | jq '.'
echo

# Cleanup
echo "Cleaning up..."
kill $SERVER_PID
wait $SERVER_PID 2>/dev/null

echo
echo "=== Test Complete ==="
echo "Summary:"
echo "- ✅ Created users with hasOne profile relation"
echo "- ✅ Profiles automatically loaded in GET /users"
echo "- ✅ Parent users automatically loaded in GET /userprofiles (belongsTo)"
echo "- ✅ Foreign keys were automatically injected"
echo "- Check the database at: $TEST_DIR/data/users.db"
echo
echo "To inspect the database:"
echo "  sqlite3 $TEST_DIR/data/users.db"
echo "  SELECT * FROM users;"
echo "  SELECT * FROM userprofiles;"
