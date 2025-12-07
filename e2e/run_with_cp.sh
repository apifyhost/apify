#!/bin/bash
set -e

# Build the project
echo "Building apify..."
cargo build

# Paths
BINARY="./target/debug/apify"
CONFIG="e2e/test_config.yaml"
DB_FILE="apify.sqlite"

# Clean up previous DB
rm -f $DB_FILE

# Create a minimal config file for the test
echo "Creating test config..."
cat > $CONFIG <<EOF
listeners:
  - port: 3005
    ip: 127.0.0.1
    protocol: http
    routes: [] # No static routes, will load from DB
auth:
  - type: api-key
    name: default-key
    enabled: true
    config:
      source: header
      key_name: X-Api-Key
      consumers:
        - name: test-user
          keys: ["e2e-test-key-001"]
datasource:
  default:
    driver: sqlite
    database: $DB_FILE
modules:
  tracing:
    enabled: false
access_log:
  enabled: true
  path: "logs/access.log"
  format: "json"
  body: true
EOF

# 1. Start Control Plane
echo "Starting Control Plane..."
export APIFY_THREADS=1
APIFY_DB_URL="sqlite:$DB_FILE" $BINARY --config $CONFIG --control-plane > apify.log 2>&1 &
CP_PID=$!

# Wait for CP to be ready
echo "Waiting for Control Plane..."
sleep 5

# 2. Import OpenAPI Spec
echo "Importing OpenAPI Specs..."

import_spec() {
    local file=$1
    local name=$2
    echo "Importing $file as $name..."
    PAYLOAD=$(ruby -r yaml -r json -e 'begin
      data = YAML.load(ARGF.read)
      spec = data["openapi"]["spec"]
      payload = {
        "name" => "'"$name"'",
        "version" => "1.0.0",
        "spec" => spec
      }
      puts JSON.generate(payload)
    rescue => e
      STDERR.puts "Error: #{e}"
      exit 1
    end' < "$file")
    curl -v -X POST http://127.0.0.1:3005/_meta/apis \
        -H "Content-Type: application/json" \
        -d "$PAYLOAD"
    echo ""
}

import_spec "examples/basic/config/openapi/items.yaml" "items"
import_spec "examples/relations/config/openapi/orders.yaml" "orders"
import_spec "examples/relations/config/openapi/users.yaml" "users"
import_spec "examples/oauth/config/openapi/items_oauth.yaml" "items_oauth"

# 3. Stop Control Plane


# 3. Stop Control Plane
echo "Stopping Control Plane..."
kill $CP_PID
wait $CP_PID || true
sleep 5 # Give OS time to release the port

# 4. Start Data Plane
echo "Starting Data Plane..."
APIFY_DB_URL="sqlite:$DB_FILE" $BINARY --config $CONFIG >> apify.log 2>&1 &
DP_PID=$!

# Wait for DP to be ready
echo "Waiting for Data Plane..."
sleep 5

# Debug: Check if /orders exists
echo "Checking /orders endpoint..."
curl -v http://127.0.0.1:3005/orders || true
echo ""

# 5. Run Tests
echo "Running Tests..."
export BASE_URL="http://127.0.0.1:3005"
export API_KEY="e2e-test-key-001"
export PATH=$PATH:$(go env GOPATH)/bin

# Run the existing test runner
set +e
./e2e/test.sh relations
TEST_EXIT_CODE=$?
set -e

# 6. Cleanup
echo "Cleaning up..."
kill $DP_PID
rm $CONFIG
# rm $DB_FILE # Optional: keep for debugging

echo "=== APIFY LOGS ==="
cat apify.log
echo "=================="

exit $TEST_EXIT_CODE
