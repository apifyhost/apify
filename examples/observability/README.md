# Observability Example

Full observability stack with metrics, tracing, and dashboards.

## Features

- ✅ Prometheus metrics
- ✅ Grafana dashboards
- ✅ Jaeger distributed tracing
- ✅ OpenTelemetry integration
- ✅ Pre-configured dashboards

## Quick Start

```bash
# From repository root
./quickstart.sh observability

# Wait for all services to start (~30 seconds)
```

## Access Points

- **API**: http://localhost:3000
- **Prometheus**: http://localhost:9091
- **Grafana**: http://localhost:3001 (admin/admin)
- **Jaeger**: http://localhost:16686
- **PostgreSQL**: localhost:5432

## Grafana Dashboards

1. Access Grafana at http://localhost:3001
2. Login with username `admin`, password `admin`
3. Navigate to Dashboards → Apify Metrics
4. View pre-configured panels:
   - Request rate
   - Response time
   - Error rate
   - Database connections

## Distributed Tracing

1. Access Jaeger UI at http://localhost:16686
2. Select service "apify"
3. View request traces with detailed spans
4. Analyze performance bottlenecks

## Metrics Endpoints

```bash
# Apify metrics
curl http://localhost:9090/metrics

# Prometheus query example
curl 'http://localhost:9091/api/v1/query?query=apify_http_requests_total'
```

## Configuration

- **Prometheus**: `prometheus.yml`
- **Grafana Datasources**: `grafana/datasources/`
- **Grafana Dashboards**: `grafana/dashboards/`
- **OTLP Endpoint**: Configured in `config/config.yaml`

## Stop and Clean

```bash
./quickstart.sh observability stop    # Stop services
./quickstart.sh observability clean   # Stop and remove data
```

## Configuration

- **Main Config**: `config/config.yaml`
- **Resources**: `config/resource.yaml` (listeners, auth, datasources)
- **OpenAPI Spec**: `config/openapi/items.yaml`
