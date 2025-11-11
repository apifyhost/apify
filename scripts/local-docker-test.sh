#!/bin/bash
# Local Docker Build and Test Script
# Run this before pushing to verify everything works

set -e

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

log_info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

log_step() {
    echo -e "${BLUE}[STEP]${NC} $1"
}

# Cleanup function
cleanup() {
    log_info "Cleaning up..."
    docker compose down -v 2>/dev/null || true
    docker rm -f apify-sqlite-test apify-postgres-test 2>/dev/null || true
}

# Set trap for cleanup
trap cleanup EXIT

log_step "Starting local Docker build and test"
echo ""

# Step 1: Build image
log_step "1/5 Building Docker image..."
docker build -t apify:local-test .

if [ $? -eq 0 ]; then
    log_info "✓ Docker build successful"
else
    log_error "✗ Docker build failed"
    exit 1
fi

echo ""

# Step 2: Check image size
log_step "2/5 Checking image size..."
IMAGE_SIZE=$(docker images apify:local-test --format "{{.Size}}")
log_info "Image size: $IMAGE_SIZE"

echo ""

# Step 3: Test with SQLite
log_step "3/5 Running E2E tests with SQLite..."

# Start container
docker run -d \
    --name apify-sqlite-test \
    -p 3000:3000 \
    -v $(pwd)/e2e/config-sqlite.yaml:/app/config/config.yaml:ro \
    -v $(pwd)/e2e/e2e-api.yaml:/app/config/e2e-api.yaml:ro \
    -v $(pwd)/data:/app/data \
    apify:local-test

# Wait for service
log_info "Waiting for service to start..."
sleep 3

# Run tests
cd e2e-tests
if BASE_URL=http://localhost:3000 API_KEY=e2e-test-key-001 go test -v; then
    log_info "✓ SQLite E2E tests passed"
    SQLITE_TEST=true
else
    log_error "✗ SQLite E2E tests failed"
    docker logs apify-sqlite-test
    SQLITE_TEST=false
fi
cd ..

# Cleanup SQLite container
docker stop apify-sqlite-test >/dev/null 2>&1
docker rm apify-sqlite-test >/dev/null 2>&1

echo ""

# Step 4: Test with PostgreSQL
log_step "4/5 Running E2E tests with PostgreSQL..."

# Start PostgreSQL
docker run -d \
    --name postgres-test \
    -e POSTGRES_USER=apify \
    -e POSTGRES_PASSWORD=apify_test_password \
    -e POSTGRES_DB=apify_e2e \
    -p 5432:5432 \
    postgres:16-alpine

# Wait for PostgreSQL
log_info "Waiting for PostgreSQL to start..."
sleep 5

# Start Apify with PostgreSQL
docker run -d \
    --name apify-postgres-test \
    --network host \
    -v $(pwd)/e2e/config-postgres.yaml:/app/config/config.yaml:ro \
    -v $(pwd)/e2e/e2e-api.yaml:/app/config/e2e-api.yaml:ro \
    apify:local-test

# Wait for service
log_info "Waiting for service to start..."
sleep 3

# Run tests
cd e2e-tests
if BASE_URL=http://localhost:3000 API_KEY=e2e-test-key-001 go test -v; then
    log_info "✓ PostgreSQL E2E tests passed"
    POSTGRES_TEST=true
else
    log_error "✗ PostgreSQL E2E tests failed"
    docker logs apify-postgres-test
    POSTGRES_TEST=false
fi
cd ..

# Cleanup PostgreSQL containers
docker stop apify-postgres-test postgres-test >/dev/null 2>&1
docker rm apify-postgres-test postgres-test >/dev/null 2>&1

echo ""

# Step 5: Security scan (optional)
log_step "5/5 Running security scan (optional)..."
if command -v trivy &> /dev/null; then
    trivy image --severity HIGH,CRITICAL apify:local-test
    log_info "✓ Security scan completed"
else
    log_info "⊘ Trivy not installed, skipping security scan"
    log_info "  Install: https://github.com/aquasecurity/trivy"
fi

echo ""
echo "======================================"
log_step "Test Summary"
echo "======================================"

if [ "$SQLITE_TEST" = true ]; then
    echo -e "${GREEN}✓${NC} SQLite tests: PASSED"
else
    echo -e "${RED}✗${NC} SQLite tests: FAILED"
fi

if [ "$POSTGRES_TEST" = true ]; then
    echo -e "${GREEN}✓${NC} PostgreSQL tests: PASSED"
else
    echo -e "${RED}✗${NC} PostgreSQL tests: FAILED"
fi

echo "======================================"

if [ "$SQLITE_TEST" = true ] && [ "$POSTGRES_TEST" = true ]; then
    log_info "All tests passed! ✓ Ready to push."
    exit 0
else
    log_error "Some tests failed. Please fix before pushing."
    exit 1
fi
