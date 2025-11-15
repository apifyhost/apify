#!/bin/bash
# Quick test script for observability E2E tests

set -e

echo "=== Apify Observability E2E Test Script ==="
echo ""

# Configuration
BASE_URL="${BASE_URL:-http://localhost:3000}"
METRICS_PORT="${METRICS_PORT:-9090}"
API_KEY="${API_KEY:-e2e-test-key-001}"

echo "Configuration:"
echo "  BASE_URL: $BASE_URL"
echo "  METRICS_PORT: $METRICS_PORT"
echo "  API_KEY: $API_KEY"
echo ""

# Check if service is running
echo "Checking if Apify service is running..."
if ! curl -sf "$BASE_URL/healthz" > /dev/null 2>&1; then
    echo "❌ Apify service is not running at $BASE_URL"
    echo "   Please start the service first:"
    echo "   docker compose up -d apify-sqlite"
    exit 1
fi
echo "✅ Apify service is running"
echo ""

# Check if metrics endpoint is available
echo "Checking if metrics endpoint is available..."
if ! curl -sf "http://localhost:$METRICS_PORT/metrics" > /dev/null 2>&1; then
    echo "⚠️  Metrics endpoint is not available at port $METRICS_PORT"
    echo "   This may be expected if observability is not configured"
else
    echo "✅ Metrics endpoint is available"
fi
echo ""

# Run observability tests
echo "Running observability E2E tests..."
echo ""

cd "$(dirname "$0")"

export BASE_URL
export METRICS_PORT
export API_KEY

# Run only observability tests
ginkgo -v --focus="Observability" || {
    echo ""
    echo "❌ Tests failed!"
    echo ""
    echo "Troubleshooting tips:"
    echo "  1. Check if Apify is running with observability enabled"
    echo "  2. Verify metrics port is correct (default: 9090)"
    echo "  3. Check docker logs: docker compose logs apify-sqlite"
    exit 1
}

echo ""
echo "✅ All observability tests passed!"
