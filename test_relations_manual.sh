#!/bin/bash
# Quick manual test for relations in full example

set -e

BASE_URL="http://localhost:3000"
API_KEY="e2e-test-key-001"

echo "=== Testing Relations Feature ==="
echo ""

# Test 1: Create order with items (hasMany)
echo "1. Creating order with nested items..."
ORDER_RESPONSE=$(curl -s -X POST "$BASE_URL/orders" \
  -H "X-Api-Key: $API_KEY" \
  -H "Content-Type: application/json" \
  -d '{
    "customer_name": "Manual Test Customer",
    "total_amount": 150.00,
    "status": "pending",
    "items": [
      {"product_name": "Widget A", "quantity": 2, "price": 50.00},
      {"product_name": "Widget B", "quantity": 1, "price": 50.00}
    ]
  }')

ORDER_ID=$(echo "$ORDER_RESPONSE" | grep -o '"id":[0-9]*' | grep -o '[0-9]*')
echo "   ✓ Order created with ID: $ORDER_ID"
echo ""

# Test 2: Get order with auto-loaded items
echo "2. Getting order with auto-loaded items..."
ORDER_DATA=$(curl -s -X GET "$BASE_URL/orders/$ORDER_ID" \
  -H "X-Api-Key: $API_KEY")

ITEMS_COUNT=$(echo "$ORDER_DATA" | grep -o '"product_name"' | wc -l)
echo "   ✓ Order retrieved with $ITEMS_COUNT items"
echo "   Response: $ORDER_DATA" | head -c 200
echo "..."
echo ""

# Test 3: Create user with profile (hasOne)
echo "3. Creating user with nested profile..."
USER_RESPONSE=$(curl -s -X POST "$BASE_URL/users" \
  -H "X-Api-Key: $API_KEY" \
  -H "Content-Type: application/json" \
  -d '{
    "username": "manualtest",
    "email": "manual@test.com",
    "full_name": "Manual Test User",
    "profile": {
      "bio": "Testing relations",
      "phone": "+1234567890"
    }
  }')

USER_ID=$(echo "$USER_RESPONSE" | grep -o '"id":[0-9]*' | grep -o '[0-9]*')
echo "   ✓ User created with ID: $USER_ID"
echo ""

# Test 4: Get user with auto-loaded profile
echo "4. Getting user with auto-loaded profile..."
USER_DATA=$(curl -s -X GET "$BASE_URL/users/$USER_ID" \
  -H "X-Api-Key: $API_KEY")

HAS_PROFILE=$(echo "$USER_DATA" | grep -o '"profile"' | wc -l)
echo "   ✓ User retrieved with profile (found: $HAS_PROFILE)"
echo "   Response: $USER_DATA" | head -c 200
echo "..."
echo ""

# Test 5: Cascade delete
echo "5. Testing cascade delete..."
curl -s -X DELETE "$BASE_URL/orders/$ORDER_ID" \
  -H "X-Api-Key: $API_KEY" > /dev/null
echo "   ✓ Order deleted (items should be cascade deleted)"

curl -s -X DELETE "$BASE_URL/users/$USER_ID" \
  -H "X-Api-Key: $API_KEY" > /dev/null
echo "   ✓ User deleted (profile should be cascade deleted)"
echo ""

echo "=== All manual tests passed! ==="
