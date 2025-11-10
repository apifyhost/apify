# E2E Tests

End-to-end tests for the Apify framework using Go and Ginkgo/Gomega.

## Prerequisites

- Go 1.21 or higher
- Docker (for running the Apify service)
- Ginkgo CLI (optional, for enhanced test output)

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

### Quick Start

```bash
# Start the Apify service (in another terminal)
docker compose up -d

# Run tests with default settings
make test
```

### Using Ginkgo CLI

```bash
# Run with verbose output and progress
make test-verbose

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

Example:

```bash
BASE_URL=http://localhost:8080 API_KEY=my-key go test -v
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
4. **Large Payload Handling**: Tests with large data
5. **Content-Type Validation**: Ensures proper header handling

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
