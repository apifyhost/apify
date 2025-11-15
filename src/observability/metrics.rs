//! Prometheus metrics for HTTP requests, database operations, and system resources

use lazy_static::lazy_static;
use prometheus::{
    CounterVec, Encoder, HistogramVec, IntGauge, TextEncoder, register_counter_vec,
    register_histogram_vec, register_int_gauge,
};
use std::time::Instant;

lazy_static! {
    /// HTTP request counter by method, path, and status code
    pub static ref HTTP_REQUESTS_TOTAL: CounterVec = register_counter_vec!(
        "apify_http_requests_total",
        "Total number of HTTP requests",
        &["method", "path", "status"]
    )
    .unwrap();

    /// HTTP request duration histogram in seconds
    pub static ref HTTP_REQUEST_DURATION: HistogramVec = register_histogram_vec!(
        "apify_http_request_duration_seconds",
        "HTTP request duration in seconds",
        &["method", "path", "status"],
        vec![0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0]
    )
    .unwrap();

    /// Database query counter by operation type
    pub static ref DB_QUERIES_TOTAL: CounterVec = register_counter_vec!(
        "apify_db_queries_total",
        "Total number of database queries",
        &["operation", "table", "status"]
    )
    .unwrap();

    /// Database query duration histogram in seconds
    pub static ref DB_QUERY_DURATION: HistogramVec = register_histogram_vec!(
        "apify_db_query_duration_seconds",
        "Database query duration in seconds",
        &["operation", "table"],
        vec![0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0]
    )
    .unwrap();

    /// Active connections gauge
    pub static ref ACTIVE_CONNECTIONS: IntGauge = register_int_gauge!(
        "apify_active_connections",
        "Number of currently active HTTP connections"
    )
    .unwrap();

    /// Worker threads gauge
    pub static ref WORKER_THREADS: IntGauge = register_int_gauge!(
        "apify_worker_threads",
        "Number of worker threads"
    )
    .unwrap();
}

/// Request metrics tracker - auto-records duration on drop
pub struct RequestMetrics {
    method: String,
    path: String,
    start: Instant,
}

impl RequestMetrics {
    /// Start tracking a new HTTP request
    pub fn new(method: impl Into<String>, path: impl Into<String>) -> Self {
        ACTIVE_CONNECTIONS.inc();
        Self {
            method: method.into(),
            path: path.into(),
            start: Instant::now(),
        }
    }

    /// Record the request completion with status code
    pub fn record(self, status: u16) {
        let duration = self.start.elapsed().as_secs_f64();
        let status_str = status.to_string();

        HTTP_REQUESTS_TOTAL
            .with_label_values(&[&self.method, &self.path, &status_str])
            .inc();

        HTTP_REQUEST_DURATION
            .with_label_values(&[&self.method, &self.path, &status_str])
            .observe(duration);
    }
}

impl Drop for RequestMetrics {
    fn drop(&mut self) {
        ACTIVE_CONNECTIONS.dec();
    }
}

/// Database operation metrics tracker
pub struct DbMetrics {
    operation: String,
    table: String,
    start: Instant,
}

impl DbMetrics {
    /// Start tracking a database operation
    pub fn new(operation: impl Into<String>, table: impl Into<String>) -> Self {
        Self {
            operation: operation.into(),
            table: table.into(),
            start: Instant::now(),
        }
    }

    /// Record the operation completion with status
    pub fn record(self, status: &str) {
        let duration = self.start.elapsed().as_secs_f64();

        DB_QUERIES_TOTAL
            .with_label_values(&[self.operation.as_str(), self.table.as_str(), status])
            .inc();

        DB_QUERY_DURATION
            .with_label_values(&[self.operation.as_str(), self.table.as_str()])
            .observe(duration);
    }
}

/// Export metrics in Prometheus text format
pub fn export_metrics() -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let encoder = TextEncoder::new();
    let metric_families = prometheus::gather();
    let mut buffer = Vec::new();
    encoder.encode(&metric_families, &mut buffer)?;
    Ok(String::from_utf8(buffer)?)
}

/// Initialize metrics with static configuration
pub fn init_metrics(worker_threads: usize) {
    WORKER_THREADS.set(worker_threads as i64);
}
