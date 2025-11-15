# Apify Observability

Complete observability stack for Apify with structured logging, distributed tracing, and metrics.

## Features

### 📝 Structured Logging
- **JSON format**: Machine-readable logs with structured fields
- **Log levels**: trace, debug, info, warn, error
- **Context propagation**: Thread IDs, request IDs, span IDs
- **File & line numbers**: Easy debugging with source location

### 🔍 Distributed Tracing (OpenTelemetry)
- **OTLP exporter**: Sends traces to Jaeger/Tempo/etc.
- **Automatic instrumentation**: HTTP requests, database queries
- **Context propagation**: Trace IDs across services
- **Span attributes**: Method, path, status, duration

### 📊 Prometheus Metrics
- **HTTP metrics**:
  - `apify_http_requests_total` - Request counter by method, path, status
  - `apify_http_request_duration_seconds` - Request duration histogram
  - `apify_active_connections` - Current active connections gauge
- **Database metrics**:
  - `apify_db_queries_total` - Query counter by operation, table, status
  - `apify_db_query_duration_seconds` - Query duration histogram
- **System metrics**:
  - `apify_worker_threads` - Number of worker threads

## Quick Start

### 1. Configuration

Add observability section to your `config.yaml`:

```yaml
listeners:
  - port: 3000
    ip: "0.0.0.0"
    protocol: "http"
    apis:
      - path: "openapi/items.yaml"

observability:
  log_level: "info"                          # trace|debug|info|warn|error
  otlp_endpoint: "http://localhost:4317"     # OpenTelemetry collector
  metrics_enabled: true                       # Enable Prometheus metrics
  metrics_port: 9090                          # Metrics HTTP port
```

### 2. Run with Docker Compose

Start the complete observability stack:

```bash
docker compose up -d
```

This starts:
- **Apify**: Your application (ports 3000, 9090)
- **Jaeger**: Tracing UI at http://localhost:16686
- **Prometheus**: Metrics storage at http://localhost:9092
- **Grafana**: Dashboards at http://localhost:3002 (admin/admin)

### 3. Access UIs

| Service | URL | Purpose |
|---------|-----|---------|
| Apify API | http://localhost:3000 | Main application |
| Metrics | http://localhost:9090/metrics | Prometheus metrics endpoint |
| Jaeger | http://localhost:16686 | Distributed tracing UI |
| Prometheus | http://localhost:9092 | Metrics query UI |
| Grafana | http://localhost:3002 | Dashboards (admin/admin) |

## Log Examples

### Structured JSON Logs

```json
{
  "timestamp": "2025-11-15T10:30:45.123456Z",
  "level": "INFO",
  "fields": {
    "message": "Configuration loaded successfully",
    "config_file": "config.yaml"
  },
  "target": "apify::main",
  "thread_id": "ThreadId(1)",
  "thread_name": "main"
}
```

### HTTP Request Log

```json
{
  "timestamp": "2025-11-15T10:30:50.456789Z",
  "level": "INFO",
  "fields": {
    "message": "http_request",
    "http.method": "GET",
    "http.route": "/items/42",
    "http.status_code": 200,
    "otel.kind": "server"
  },
  "span": {
    "trace_id": "3f7a2b1c4d5e6f7a8b9c0d1e2f3a4b5c",
    "span_id": "1234567890abcdef"
  }
}
```

## Tracing

### Viewing Traces

1. Open Jaeger UI: http://localhost:16686
2. Select service: `apify`
3. Click "Find Traces"
4. View request flows, timings, and errors

### Trace Context

Each trace includes:
- **Trace ID**: Unique identifier for the entire request
- **Span ID**: Unique identifier for each operation
- **Parent Span**: Links operations together
- **Attributes**: HTTP method, path, status, DB table, etc.

### Custom Spans

Add custom spans in your code:

```rust
use tracing::{info_span, instrument};

#[instrument]
async fn process_order(order_id: u64) {
    let span = info_span!("validate_order", order_id = order_id);
    let _enter = span.enter();
    
    // Your code here
    tracing::info!("Order validated");
}
```

## Metrics

### Querying Metrics

Access raw metrics:
```bash
curl http://localhost:9090/metrics
```

### Prometheus Queries

**Request rate**:
```promql
rate(apify_http_requests_total[1m])
```

**95th percentile latency**:
```promql
histogram_quantile(0.95, rate(apify_http_request_duration_seconds_bucket[5m]))
```

**Error rate**:
```promql
rate(apify_http_requests_total{status=~"5.."}[1m])
```

**DB query rate by table**:
```promql
rate(apify_db_queries_total[1m])
```

### Grafana Dashboards

Pre-configured dashboard includes:
- **HTTP Request Rate**: Requests per second
- **Request Duration**: p95, p99 latency
- **Active Connections**: Current connection count
- **Worker Threads**: Thread pool size
- **Database Query Rate**: Queries per second

## Environment Variables

Override configuration with environment variables:

```bash
# Log level (overrides config)
RUST_LOG=debug

# Worker threads (useful for testing)
APIFY_THREADS=4

# OpenTelemetry endpoint
OTEL_EXPORTER_OTLP_ENDPOINT=http://collector:4317
```

## Production Deployment

### Recommended Settings

```yaml
observability:
  log_level: "info"                              # Or "warn" for production
  otlp_endpoint: "http://otel-collector:4317"    # Your OTLP collector
  metrics_enabled: true
  metrics_port: 9090
```

### Log Aggregation

For production, send logs to:
- **Elasticsearch + Kibana**: Use Filebeat or Fluentd
- **Loki + Grafana**: Native Grafana integration
- **CloudWatch/Stackdriver**: Cloud-native solutions

### Distributed Tracing

Send traces to:
- **Jaeger**: Self-hosted or SaaS
- **Tempo**: Grafana's tracing backend
- **Datadog/New Relic**: APM platforms
- **AWS X-Ray/Google Cloud Trace**: Cloud providers

### Metrics Storage

Options for long-term metrics:
- **Prometheus**: Self-hosted with remote write
- **Thanos/Cortex**: Scalable Prometheus
- **Grafana Cloud**: Managed Prometheus
- **Datadog/New Relic**: APM platforms

## Troubleshooting

### No traces appearing

1. Check OTLP endpoint is accessible:
   ```bash
   curl http://localhost:4317
   ```

2. Verify configuration:
   ```yaml
   observability:
     otlp_endpoint: "http://localhost:4317"  # Must be accessible
   ```

3. Check logs for connection errors:
   ```bash
   docker compose logs apify-sqlite | grep -i otel
   ```

### Metrics not showing

1. Verify metrics endpoint:
   ```bash
   curl http://localhost:9090/metrics
   ```

2. Check Prometheus targets:
   - Open http://localhost:9092/targets
   - Ensure apify targets are "UP"

3. Verify Prometheus config:
   ```yaml
   scrape_configs:
     - job_name: 'apify-sqlite'
       static_configs:
         - targets: ['apify-sqlite:9090']
   ```

### High cardinality warnings

Metrics with too many unique label combinations:

**Problem**: Path contains IDs: `/items/123`, `/items/456`, ...

**Solution**: Use path patterns in labels:
```rust
// Instead of: /items/123
// Use: /items/:id
```

The metrics already normalize paths using route patterns.

## Best Practices

### 1. Log Levels

- **trace**: Very detailed, for debugging specific issues
- **debug**: Detailed information, development only
- **info**: General information, production default
- **warn**: Warning messages, potential issues
- **error**: Error messages, failures

### 2. Span Design

- Keep spans focused on single operations
- Add relevant attributes (IDs, types, counts)
- Use consistent naming conventions
- Don't create spans for very fast operations (<1ms)

### 3. Metric Labels

- Keep cardinality low (< 10 values per label)
- Use static labels (method, status, table)
- Avoid dynamic labels (IDs, timestamps, user names)
- Normalize paths to route patterns

### 4. Sampling

For high-traffic production:

```yaml
observability:
  log_level: "warn"           # Reduce log volume
  # Use sampled tracing in OTLP collector config
```

Configure OTLP collector to sample:
```yaml
processors:
  probabilistic_sampler:
    sampling_percentage: 10  # Sample 10% of traces
```

## Architecture

```
┌──────────────┐
│   Apify      │
│              │
│ ┌──────────┐ │     ┌─────────────┐
│ │  Logging │─┼────▶│   stdout    │──┐
│ └──────────┘ │     └─────────────┘  │
│              │                        │
│ ┌──────────┐ │     ┌─────────────┐  │    ┌──────────────┐
│ │ Tracing  │─┼────▶│ OTLP (gRPC) │──┼───▶│    Jaeger    │
│ └──────────┘ │     └─────────────┘  │    └──────────────┘
│              │                        │
│ ┌──────────┐ │     ┌─────────────┐  │    ┌──────────────┐
│ │ Metrics  │─┼────▶│  HTTP:9090  │──┼───▶│  Prometheus  │
│ └──────────┘ │     └─────────────┘  │    └──────────────┘
└──────────────┘                       │
                                       │    ┌──────────────┐
                                       └───▶│   Grafana    │
                                            └──────────────┘
```

## Performance Impact

Observability overhead (approximate):

- **Logging**: < 1% CPU, minimal memory
- **Tracing**: 2-5% CPU, ~10MB memory per 1000 spans
- **Metrics**: < 1% CPU, ~1MB memory per 1000 series

**Total overhead**: ~5-10% in typical scenarios

To minimize impact:
- Use sampling for tracing
- Reduce log level in production
- Limit metric label cardinality

## License

Same as Apify project.
