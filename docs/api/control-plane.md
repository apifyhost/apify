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
