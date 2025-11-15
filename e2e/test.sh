#!/bin/bash
# Unified E2E Test Runner
# Supports both quick bash tests and comprehensive Go tests

set -e

# Configuration
BASE_URL="${BASE_URL:-http://localhost:3000}"
METRICS_PORT="${METRICS_PORT:-9090}"
API_KEY="${API_KEY:-e2e-test-key-001}"
TEST_MODE="${1:-go}"  # go, quick, or observability

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

log_info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

log_warning() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

log_header() {
    echo -e "${BLUE}=== $1 ===${NC}"
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

# Quick smoke test using curl
run_quick_test() {
    log_header "Quick Smoke Test"
    
    # Test health endpoint
    log_info "Testing /healthz endpoint..."
    if curl -sf "$BASE_URL/healthz" > /dev/null; then
        echo "  ✅ Health check passed"
    else
        echo "  ❌ Health check failed"
        return 1
    fi
    
    # Test authentication
    log_info "Testing authentication..."
    status=$(curl -s -o /dev/null -w "%{http_code}" "$BASE_URL/items")
    if [ "$status" = "401" ]; then
        echo "  ✅ Authentication working (401 without key)"
    else
        echo "  ❌ Expected 401, got $status"
        return 1
    fi
    
    # Test with valid API key
    log_info "Testing with valid API key..."
    status=$(curl -s -o /dev/null -w "%{http_code}" -H "X-Api-Key: $API_KEY" "$BASE_URL/items")
    if [ "$status" = "200" ]; then
        echo "  ✅ API key authentication passed"
    else
        echo "  ❌ Expected 200, got $status"
        return 1
    fi
    
    # Test metrics endpoint
    log_info "Testing metrics endpoint..."
    if curl -sf "http://localhost:$METRICS_PORT/metrics" > /dev/null 2>&1; then
        echo "  ✅ Metrics endpoint available"
    else
        log_warning "  ⚠️  Metrics endpoint not available (may be expected)"
    fi
    
    echo ""
    log_info "Quick smoke test completed successfully!"
    return 0
}

# Run comprehensive Go tests
run_go_tests() {
    log_header "Running Comprehensive E2E Tests (Go/Ginkgo)"
    
    cd "$(dirname "$0")"
    
    # Check if Go is installed
    if ! command -v go &> /dev/null; then
        log_error "Go is not installed. Please install Go 1.21 or higher."
        return 1
    fi
    
    # Check if Ginkgo is installed
    if ! command -v ginkgo &> /dev/null; then
        log_warning "Ginkgo CLI not found. Installing..."
        go install github.com/onsi/ginkgo/v2/ginkgo@latest
    fi
    
    # Set environment variables
    export BASE_URL
    export METRICS_PORT
    export API_KEY
    
    # Run tests
    if [ "$TEST_MODE" = "observability" ]; then
        log_info "Running observability tests only..."
        ginkgo -v --focus="Observability"
    elif [ "$TEST_MODE" = "crud" ]; then
        log_info "Running CRUD tests only..."
        ginkgo -v --focus="CRUD"
    else
        log_info "Running all tests..."
        ginkgo -v
    fi
}

# Show usage
show_usage() {
    cat << EOF
Usage: $0 [MODE]

Unified E2E test runner for Apify

MODES:
  go              Run comprehensive Go/Ginkgo tests (default)
  quick           Run quick smoke tests using curl
  observability   Run observability tests only
  crud            Run CRUD tests only

ENVIRONMENT VARIABLES:
  BASE_URL        Service URL (default: http://localhost:3000)
  METRICS_PORT    Metrics endpoint port (default: 9090)
  API_KEY         API key for authentication (default: e2e-test-key-001)

EXAMPLES:
  $0              # Run all Go tests
  $0 quick        # Quick smoke test
  $0 observability # Run only observability tests
  BASE_URL=http://localhost:3001 $0 quick

For more options, use the Makefile:
  make test               # Run tests with go test
  make test-verbose       # Run tests with detailed output
  make test-observability # Run observability tests
  make test-crud          # Run CRUD tests
  make help               # Show all available targets
EOF
}

# Main execution
main() {
    log_header "Apify E2E Test Runner"
    echo "Configuration:"
    echo "  BASE_URL: $BASE_URL"
    echo "  METRICS_PORT: $METRICS_PORT"
    echo "  API_KEY: ${API_KEY:0:15}..."
    echo "  MODE: $TEST_MODE"
    echo ""
    
    # Check if help is requested
    if [ "$TEST_MODE" = "help" ] || [ "$TEST_MODE" = "-h" ] || [ "$TEST_MODE" = "--help" ]; then
        show_usage
        exit 0
    fi
    
    # Wait for service to be ready
    wait_for_service || exit 1
    echo ""
    
    # Run appropriate test mode
    case "$TEST_MODE" in
        quick)
            run_quick_test
            ;;
        go|observability|crud)
            run_go_tests
            ;;
        *)
            log_error "Unknown test mode: $TEST_MODE"
            echo ""
            show_usage
            exit 1
            ;;
    esac
}

main "$@"
