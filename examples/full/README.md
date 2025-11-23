# Full Example

Complete Apify setup with all features enabled. **Used for E2E testing.**

## Features

- ✅ All APIs (basic + OAuth)
- ✅ Multiple databases (PostgreSQL + SQLite)
- ✅ OAuth/OIDC with Keycloak
- ✅ Full observability stack
- ✅ Prometheus + Grafana + Jaeger
- ✅ Metrics and distributed tracing

## Quick Start

```bash
# From repository root
./quickstart.sh full

# Wait for all services (~90 seconds)
```

## Access Points

### APIs
- **PostgreSQL API**: http://localhost:3000
- **SQLite API**: http://localhost:3001

### Authentication
- **Keycloak Admin**: http://localhost:8080 (admin/admin)

### Observability
- **Prometheus**: http://localhost:9090
- **Grafana**: http://localhost:3002 (admin/admin)
- **Jaeger**: http://localhost:16686

### Metrics
- **PostgreSQL Metrics**: http://localhost:9091/metrics
- **SQLite Metrics**: http://localhost:9092/metrics

## E2E Testing

This configuration is used by the E2E test suite:

```bash
# From e2e directory
cd e2e

# Run all tests
./test.sh go

# Run quick smoke test
./test.sh quick
```

## Configuration

This example combines all features:
- Both `items.yaml` and `items_oauth.yaml` APIs
- PostgreSQL and SQLite datasources
- OAuth providers configuration
- Full observability setup
- Debug logging enabled

## Important Notes

- **External Network**: This example uses `apify_default` external network for testing
- **Resource Usage**: Requires significant memory (~4GB) for all services
- **Startup Time**: First startup takes 1-2 minutes for all services to be ready
- **Debug Mode**: `RUST_LOG=debug` enabled for detailed logging

## Stop and Clean

```bash
./quickstart.sh full stop    # Stop services
./quickstart.sh full clean   # Stop and remove data + network
```

## CI/CD Usage

This configuration is used in GitHub Actions:

```yaml
# .github/workflows/docker.yml references this example
working-directory: examples/full
```
