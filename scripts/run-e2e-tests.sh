#!/bin/bash

# E2E Test Runner for Apify
# This script runs E2E tests against both SQLite and PostgreSQL backends

set -e

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

print_header() {
    echo -e "\n${GREEN}=== $1 ===${NC}\n"
}

print_error() {
    echo -e "${RED}❌ $1${NC}"
}

print_success() {
    echo -e "${GREEN}✅ $1${NC}"
}

print_info() {
    echo -e "${YELLOW}ℹ️  $1${NC}"
}

# Check if Go is installed
if ! command -v go &> /dev/null; then
    print_error "Go is not installed. Please install Go 1.21 or higher."
    exit 1
fi

# Check if Docker is installed
if ! command -v docker &> /dev/null; then
    print_error "Docker is not installed. Please install Docker."
    exit 1
fi

cd "$(dirname "$0")/.."

# Install dependencies
print_header "Installing Dependencies"
cd e2e-tests
if [ ! -f go.sum ]; then
    print_info "Downloading Go modules..."
    go mod download
fi

if ! command -v ginkgo &> /dev/null; then
    print_info "Installing Ginkgo CLI..."
    go install github.com/onsi/ginkgo/v2/ginkgo@latest
fi

cd ..

# Test SQLite
print_header "Testing with SQLite"
print_info "Starting Apify with SQLite..."

docker compose up -d apify-sqlite
sleep 5

# Wait for health check
print_info "Waiting for service to be ready..."
for i in {1..30}; do
    if curl -f http://localhost:3000/healthz &> /dev/null; then
        print_success "Service is ready"
        break
    fi
    if [ $i -eq 30 ]; then
        print_error "Service failed to start"
        docker compose logs apify-sqlite
        docker compose down
        exit 1
    fi
    sleep 1
done

print_info "Running tests..."
cd e2e-tests
if BASE_URL=http://localhost:3000 API_KEY=e2e-test-key-001 go test -v; then
    print_success "SQLite tests passed"
    SQLITE_RESULT=0
else
    print_error "SQLite tests failed"
    SQLITE_RESULT=1
fi
cd ..

print_info "Stopping SQLite container..."
docker compose stop apify-sqlite

# Test PostgreSQL
print_header "Testing with PostgreSQL"
print_info "Starting Apify with PostgreSQL..."

docker compose up -d postgres apify-postgres
sleep 5

# Wait for health check
print_info "Waiting for service to be ready..."
for i in {1..30}; do
    if curl -f http://localhost:3000/healthz &> /dev/null; then
        print_success "Service is ready"
        break
    fi
    if [ $i -eq 30 ]; then
        print_error "Service failed to start"
        docker compose logs apify-postgres
        docker compose down
        exit 1
    fi
    sleep 1
done

print_info "Running tests..."
cd e2e-tests
if BASE_URL=http://localhost:3000 API_KEY=e2e-test-key-001 go test -v; then
    print_success "PostgreSQL tests passed"
    POSTGRES_RESULT=0
else
    print_error "PostgreSQL tests failed"
    POSTGRES_RESULT=1
fi
cd ..

# Cleanup
print_header "Cleanup"
docker compose down

# Summary
print_header "Test Summary"
if [ $SQLITE_RESULT -eq 0 ]; then
    print_success "SQLite tests: PASSED"
else
    print_error "SQLite tests: FAILED"
fi

if [ $POSTGRES_RESULT -eq 0 ]; then
    print_success "PostgreSQL tests: PASSED"
else
    print_error "PostgreSQL tests: FAILED"
fi

# Exit with error if any tests failed
if [ $SQLITE_RESULT -ne 0 ] || [ $POSTGRES_RESULT -ne 0 ]; then
    print_error "Some tests failed"
    exit 1
fi

print_success "All tests passed!"
