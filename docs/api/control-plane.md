# Control Plane API

The Control Plane API allows you to manage the Apify instance programmatically.

## Endpoints

### Health Check
`GET /health`
Returns the health status of the service.

### Reload Configuration
`POST /_/reload`
Triggers a hot reload of the configuration files without restarting the server.

### Metrics
`GET /metrics`
Exposes Prometheus metrics (if enabled).

### OpenAPI Documentation
If the `openapi_docs` module is enabled (default port 4001):

*   `GET /docs`: Swagger UI interface.
*   `GET /openapi.json`: Aggregated OpenAPI 3.0 specification for all configured APIs.

## Authentication

If `control_plane.admin_key` is configured in `config.yaml`, all requests to the Control Plane API (typically under `/apify/admin/`) must include the authentication header:

`X-API-KEY: <your-admin-key>`
