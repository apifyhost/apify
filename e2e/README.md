# E2E Tests

End-to-end tests for Apify with both quick smoke tests (bash/curl) and comprehensive test suites (Go/Ginkgo).

## Prerequisites

- **For Quick Tests**: curl, bash
- **For Comprehensive Tests**: Go 1.21+, Docker
- **Optional**: Ginkgo CLI for enhanced test output

## Installation

Install dependencies:

```bash
make deps
```

Or manually:

```bash
go mod download
go install github.com/onsi/ginkgo/v2/ginkgo@latest
```

## Running Tests

### Quick Smoke Test (Recommended for CI/Local)

Fast health check using curl:

```bash
# Quick smoke test (no Go required)
make test-quick

# Or directly
./test.sh quick
```

### Comprehensive Tests (Go/Ginkgo)

Full test suite with detailed assertions:

```bash
# Start the service first
docker compose up -d apify-sqlite

# Run all tests
make test

# Run with verbose output
make test-verbose

# Run specific test suites
make test-crud           # CRUD operations only
make test-observability  # Observability/metrics only
```

### Using the Unified Test Script

The `test.sh` script supports multiple modes:

```bash
./test.sh              # All Go tests (default)
./test.sh quick        # Quick smoke test
./test.sh observability # Observability tests only
./test.sh crud         # CRUD tests only
./test.sh help         # Show usage
```

# Or directly
ginkgo -v --progress
```

### Using go test

```bash
go test -v
```

## Configuration

Tests can be configured via environment variables:

- `BASE_URL`: URL of the Apify service (default: `http://localhost:3000`)
- `API_KEY`: API key for authentication (default: `e2e-test-key-001`)
- `METRICS_PORT`: Port for Prometheus metrics endpoint (default: `9090`)

Example:

```bash
BASE_URL=http://localhost:8080 API_KEY=my-key METRICS_PORT=9090 go test -v
```

## Test Structure

The tests are organized using Ginkgo's BDD-style syntax:

- **Describe**: Groups related tests
- **Context**: Describes different scenarios
- **It**: Individual test cases
- **BeforeEach**: Setup before each test
- **Ordered**: Runs tests in order (used for CRUD operations)

### Test Suites

1. **Health Check**: Verifies service availability
2. **Authentication**: Tests API key validation
3. **CRUD Operations**: Complete create, read, update, delete flow
4. **Observability**: Prometheus metrics, structured logging, tracing
   - Metrics endpoint availability
   - HTTP request metrics
   - Database query metrics
   - Metric labels and histograms
   - Performance under load
5. **Large Payload Handling**: Tests with large data
6. **Content-Type Validation**: Ensures proper header handling

## CI/CD Integration

Tests are automatically run in GitHub Actions for:

- SQLite database backend
- PostgreSQL database backend

The workflow:
1. Builds Docker image
2. Runs E2E tests with SQLite
3. Runs E2E tests with PostgreSQL
4. Security scanning
5. Publishes release images

## Makefile Targets

- `make help`: Show available targets
- `make deps`: Install dependencies
- `make test`: Run tests
- `make test-verbose`: Run tests with detailed output
- `make test-crud`: Run CRUD tests only
- `make test-observability`: Run observability tests only
- `make test-all`: Run all tests with full configuration
- `make clean`: Clean test artifacts

## Troubleshooting

### Service not ready

If tests fail with connection errors, ensure:

1. The Apify service is running
2. The service has completed initialization
3. The correct port is exposed (default: 3000)

Check service logs:

```bash
docker logs apify-sqlite
```

### Database issues

For PostgreSQL tests, ensure the database service is healthy:

```bash
docker compose ps
```

### Test cache

If tests behave unexpectedly, clean the test cache:

```bash
make clean
```

## Writing New Tests

Example test structure:

```go
var _ = Describe("My Feature", func() {
    var client *http.Client
    
    BeforeEach(func() {
        client = &http.Client{Timeout: 10 * time.Second}
    })
    
    Context("when condition is met", func() {
        It("should do something", func() {
            resp, err := client.Get(baseURL + "/endpoint")
            Expect(err).NotTo(HaveOccurred())
            Expect(resp.StatusCode).To(Equal(http.StatusOK))
        })
    })
})
```

## Reference

- [Ginkgo Documentation](https://onsi.github.io/ginkgo/)
- [Gomega Matchers](https://onsi.github.io/gomega/)
