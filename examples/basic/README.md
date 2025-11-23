# Basic Example

The simplest Apify setup with PostgreSQL backend.

## Features

- ✅ Basic CRUD API
- ✅ API key authentication
- ✅ PostgreSQL database
- ✅ Prometheus metrics

## Quick Start

```bash
# From repository root
./quickstart.sh basic

# Or directly from this directory
docker compose up
```

## Access Points

- **API**: http://localhost:3000
- **Health**: http://localhost:3000/healthz
- **Metrics**: http://localhost:9090/metrics
- **PostgreSQL**: localhost:5432 (user: apify, password: apify_password)

## Testing the API

```bash
# Health check
curl http://localhost:3000/healthz

# List items (requires API key)
curl -H "X-Api-Key: demo-key-123" http://localhost:3000/items

# Create an item
curl -X POST http://localhost:3000/items \
  -H "X-Api-Key: demo-key-123" \
  -H "Content-Type: application/json" \
  -d '{"name": "Test Item", "description": "A test item"}'
```

## Configuration

- **API Keys**: Defined in `config/config.yaml` under `consumers`
- **Database**: PostgreSQL connection settings in `datasource` section
- **OpenAPI Spec**: `config/openapi/items.yaml`

## Stop and Clean

```bash
./quickstart.sh basic stop    # Stop services
./quickstart.sh basic clean   # Stop and remove data
```
