# Apify QuickStart

Quick start guide to run Apify with PostgreSQL backend.

## Prerequisites

- Docker and Docker Compose installed
- Port 3000 and 5432 available

## Quick Start

1. Start the services:
```bash
docker compose up -d
```

2. Wait for services to be ready (about 10-15 seconds):
```bash
docker compose ps
```

3. Test the API:
```bash
# Health check
curl http://localhost:3000/healthz

# Create an item (with API key authentication)
curl -X POST http://localhost:3000/items \
  -H "Content-Type: application/json" \
  -H "X-Api-Key: dev-key-123" \
  -d '{"name": "Test Item", "description": "My first item", "price": 99.99}'

# List items
curl -H "X-Api-Key: dev-key-123" http://localhost:3000/items
```

## Configuration

- **config/config.yaml**: Main configuration file
  - API endpoints and datasources
  - API key authentication
  - Logging and metrics settings

- **config/openapi/items.yaml**: OpenAPI specification
  - Defines the `/items` CRUD endpoints
  - Schema validation rules

## API Key Authentication

The default API keys are configured in `config/config.yaml`:
- `dev-key-123`
- `my-api-key-001`

Include the API key in requests using the `X-Api-Key` header.

## Metrics

Prometheus metrics are available at:
```
http://localhost:9090/metrics
```

## Stop Services

```bash
docker compose down
```

To remove all data:
```bash
docker compose down -v
```

## Next Steps

- Modify `config/openapi/items.yaml` to define your own API schemas
- Update `config/config.yaml` to add more datasources or consumers
- Check the [main documentation](https://github.com/apifyhost/apify) for advanced features

## Troubleshooting

**Service not starting?**
```bash
docker compose logs apify
```

**Database connection issues?**
```bash
docker compose logs postgres
```

**Reset everything:**
```bash
docker compose down -v
docker compose up -d
```
