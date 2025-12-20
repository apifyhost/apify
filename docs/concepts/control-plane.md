# Control Plane

The Control Plane (CP) is responsible for managing the configuration of the Apify instance. It provides a REST API to dynamically add, update, or remove APIs and datasources without restarting the server.

## API Reference

### Import API

**Endpoint:** `POST /_meta/apis`

Import a single OpenAPI specification.

```bash
curl -X POST http://127.0.0.1:4000/_meta/apis \
  -H "Content-Type: application/json" \
  -d '{
    "name": "demo-api",
    "version": "1.0.0",
    "spec": { ... }
  }'
```

### Bulk Import

**Endpoint:** `POST /_meta/import`

Import a complete configuration file.

```bash
curl -X POST http://127.0.0.1:4000/_meta/import \
  --data-binary @config.yaml
```
