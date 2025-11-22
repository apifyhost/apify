# Configuration Guide

This directory contains the main configuration files for Apify.

## File Structure

```
config/
├── config.yaml          # Main configuration file
└── openapi/
    └── items.yaml       # OpenAPI schema with table definitions
```

## Main Configuration (config.yaml)

The main configuration file includes:

### Listeners
- Port and IP binding
- API endpoints and their associated datasources

### Consumers (Authentication)
- API key-based authentication
- Consumer names and their keys

### Datasources
- Database connections (SQLite, PostgreSQL)
- Connection pooling settings

### Observability (Optional)
- Log level configuration
- OpenTelemetry tracing endpoint
- Prometheus metrics settings

## OpenAPI Schema (openapi/items.yaml)

Defines:
- API endpoints and operations
- Table schemas (x-table-schemas)
- Request/response formats
- **Authentication requirements** (via OpenAPI `security` and `components.securitySchemes`)
  - Standards-compliant security definitions
  - Legacy `x-modules` still supported for backward compatibility

## Usage

### Local Development

```bash
# Run with default config
cargo run --release

# Run with specific config
cargo run --release -- -c config/config.yaml
```

### Docker

```bash
# Uses e2e configs for testing
docker compose up apify-sqlite
docker compose up apify-postgres
```

## Configuration Examples

### SQLite Database

```yaml
datasource:
  default:
    driver: sqlite
    database: ./apify.sqlite
    max_pool_size: 5
```

### PostgreSQL Database

```yaml
datasource:
  default:
    driver: postgres
    host: localhost
    port: 5432
    user: postgres
    password: postgres
    database: apify_db
    ssl_mode: prefer
    max_pool_size: 10
```

### Multiple Datasources

```yaml
datasource:
  users_db:
    driver: sqlite
    database: ./users.sqlite
  orders_db:
    driver: postgres
    host: localhost
    # ...

listeners:
  - port: 3000
    apis:
      - path: ./openapi/users.yaml
        datasource: users_db
      - path: ./openapi/orders.yaml
        datasource: orders_db
```

### Authentication with OpenAPI Security Schemes

Apify supports standard OpenAPI 3.0 security schemes:

```yaml
# In your OpenAPI spec (e.g., openapi/items.yaml)
openapi:
  spec:
    openapi: "3.0.0"
    
    # Define security schemes
    components:
      securitySchemes:
        ApiKeyAuth:
          type: apiKey
          in: header
          name: X-Api-Key
    
    # Apply globally to all operations
    security:
      - ApiKeyAuth: []
    
    paths:
      /items:
        get:
          # Inherits global security
          summary: "List items"
        post:
          # Can override with operation-level security
          security:
            - ApiKeyAuth: []
          summary: "Create item"
      /public:
        get:
          # Disable auth for specific operation
          security: []
          summary: "Public endpoint"
```

**Migration Note:** Legacy `x-modules: access: ["key_auth"]` syntax is still supported for backward compatibility, but using standard OpenAPI security schemes is recommended.

## Environment Variables

- `RUST_LOG`: Override log level (e.g., `RUST_LOG=debug`)
- `APIFY_THREADS`: Number of worker threads (default: CPU cores)

## See Also

- [Main README](../README.md)
- [E2E Test Configs](../e2e/)
- [Observability Guide](../observability/README.md)
