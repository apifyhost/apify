#!/bin/bash
set -e

# Colors
GREEN='\033[0;32m'
NC='\033[0m'

log() {
    echo -e "${GREEN}[CP-TEST]${NC} $1"
}

# Cleanup function
cleanup() {
    log "Stopping servers..."
    if [ -n "$CP_PID" ]; then kill $CP_PID 2>/dev/null || true; fi
    if [ -n "$DP_PID" ]; then kill $DP_PID 2>/dev/null || true; fi
}
trap cleanup EXIT

# Ensure postgres is running
log "Starting dependencies (Postgres)..."
docker-compose -f examples/full/docker-compose.yml up -d postgres

# Wait for postgres to be ready
log "Waiting for Postgres..."
sleep 5

# Clean up old databases
log "Cleaning up old databases..."
rm -f control_plane.sqlite apify.sqlite examples/full/data/apify.sqlite

# Prepare local resource config
log "Preparing local resource config..."
# Adjust Postgres connection for localhost
# Adjust file paths to be relative to project root
sed 's/host: postgres/host: localhost/' examples/full/config/resource.yaml | \
sed 's/port: 5432/port: 5433/' | \
sed 's|\./openapi/|examples/full/config/openapi/|' > examples/full/config/resource.local.yaml

# Start Control Plane
log "Starting Control Plane..."
cargo run -- --config examples/full/config/config.cp.yaml --control-plane &
CP_PID=$!

# Wait for CP
log "Waiting for Control Plane (port 4000)..."
max_attempts=30
attempt=0
while ! nc -z localhost 4000; do   
  sleep 1
  attempt=$((attempt + 1))
  if [ $attempt -ge $max_attempts ]; then
      log "Control Plane failed to start"
      exit 1
  fi
done

# Import resources
log "Importing resources..."
curl -f -X POST --data-binary @examples/full/config/resource.local.yaml http://localhost:4000/_meta/import
echo ""

# Start Data Plane
log "Starting Data Plane..."
cargo run -- --config examples/full/config/config.yaml &
DP_PID=$!

# Wait for DP
log "Waiting for Data Plane (port 3000)..."
attempt=0
while ! nc -z localhost 3000; do   
  sleep 1
  attempt=$((attempt + 1))
  if [ $attempt -ge $max_attempts ]; then
      log "Data Plane failed to start"
      exit 1
  fi
done

# Run tests
log "Running E2E tests..."
# We use 'quick' mode first to verify basic connectivity
./e2e/test.sh quick

# Then run the full suite if needed, or just the relevant parts
# ./e2e/test.sh go
