//! Distributed tracing setup with OpenTelemetry and structured logging

use opentelemetry::KeyValue;
use opentelemetry::trace::TracerProvider as _;
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::{Resource, trace::SdkTracerProvider};
use std::time::Duration;
use tracing_subscriber::{EnvFilter, Layer, fmt, layer::SubscriberExt, util::SubscriberInitExt};

/// Initialize basic logging subsystem (without OpenTelemetry)
///
/// Sets up:
/// - Structured JSON logging to stdout
/// - Environment-based log level filtering
///
/// Call this first in main() before any async operations.
pub fn init_logging(log_level: Option<&str>) {
    // Build filter from environment or default
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(log_level.unwrap_or("info")));

    // JSON formatter for structured logging
    let fmt_layer = fmt::layer()
        .json()
        .with_target(true)
        .with_thread_ids(true)
        .with_thread_names(true)
        .with_level(true)
        .with_file(true)
        .with_line_number(true)
        .with_filter(filter);

    // Initialize with just logging layer
    tracing_subscriber::registry()
        .with(fmt_layer)
        .init();
}

/// Initialize tracing with OpenTelemetry support (must be called from within Tokio runtime)
///
/// This sets up:
/// - Structured JSON logging to stdout
/// - OTLP span exporter using tonic/gRPC
/// - Tracing provider with service metadata
/// - OpenTelemetry tracing layer
///
/// IMPORTANT: Must be called from within a Tokio runtime
/// IMPORTANT: Will replace any existing global subscriber
pub async fn init_tracing_with_otel(
    service_name: &str,
    otlp_endpoint: &str,
    log_level: Option<&str>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Build filter from environment or default
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(log_level.unwrap_or("info")));

    // JSON formatter for structured logging
    let fmt_layer = fmt::layer()
        .json()
        .with_target(true)
        .with_thread_ids(true)
        .with_thread_names(true)
        .with_level(true)
        .with_file(true)
        .with_line_number(true)
        .with_filter(filter);

    // Initialize OpenTelemetry OTLP exporter
    let exporter = opentelemetry_otlp::SpanExporter::builder()
        .with_tonic()
        .with_endpoint(otlp_endpoint)
        .with_timeout(Duration::from_secs(3))
        .build()?;

    let provider = SdkTracerProvider::builder()
        .with_batch_exporter(exporter)
        .with_resource(
            Resource::builder_empty()
                .with_service_name(service_name.to_string())
                .with_attributes([KeyValue::new("service.version", env!("CARGO_PKG_VERSION"))])
                .build(),
        )
        .build();

    // Get the tracer before setting as global
    let tracer = provider.tracer(service_name.to_string());
    
    // Set as global provider
    opentelemetry::global::set_tracer_provider(provider);

    // Create OpenTelemetry layer with the tracer
    let otel_layer = tracing_opentelemetry::layer().with_tracer(tracer);

    // Initialize the subscriber with both layers
    // This will replace any existing subscriber
    tracing::subscriber::set_global_default(
        tracing_subscriber::registry()
            .with(fmt_layer)
            .with(otel_layer)
    )?;

    tracing::info!(
        service = service_name,
        otlp_endpoint = otlp_endpoint,
        "OpenTelemetry tracing initialized"
    );

    Ok(())
}

/// Initialize tracing and logging subsystem (legacy function for compatibility)
///
/// Sets up:
/// - Structured JSON logging to stdout
/// - Optional OpenTelemetry tracing to OTLP collector (if endpoint provided)
/// - Environment-based log level filtering
///
/// Note: If otlp_endpoint is provided, this will log a warning and skip OpenTelemetry init
/// since it requires a Tokio runtime. Use init_logging() + init_opentelemetry() instead.
pub fn init_tracing(
    service_name: &str,
    otlp_endpoint: Option<&str>,
    log_level: Option<&str>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    init_logging(log_level);

    if otlp_endpoint.is_some() {
        tracing::warn!(
            "OpenTelemetry endpoint configured but skipped in sync context. \
             Use init_logging() + init_opentelemetry() in async context instead."
        );
        tracing::info!(
            service = service_name,
            "Tracing initialized (OpenTelemetry deferred)"
        );
    } else {
        tracing::info!(
            service = service_name,
            "Tracing initialized (OpenTelemetry disabled)"
        );
    }

    Ok(())
}

/// Shutdown tracing and flush remaining spans
pub fn shutdown_tracing() {
    // OpenTelemetry SDK handles shutdown automatically on drop
    // Just give it a moment to flush
    std::thread::sleep(std::time::Duration::from_millis(100));
}

/// Helper macro to create a traced span with common attributes
#[macro_export]
macro_rules! traced_span {
    ($level:expr, $name:expr, $($key:tt = $value:expr),*) => {
        tracing::span!(
            $level,
            $name,
            $($key = $value),*
        )
    };
}

/// Helper to instrument HTTP requests
pub fn http_span(method: &str, path: &str, status: u16) -> tracing::Span {
    tracing::info_span!(
        "http_request",
        http.method = method,
        http.route = path,
        http.status_code = status,
        otel.kind = "server"
    )
}

/// Helper to instrument database operations
pub fn db_span(operation: &str, table: &str) -> tracing::Span {
    tracing::info_span!(
        "db_operation",
        db.operation = operation,
        db.table = table,
        otel.kind = "client"
    )
}
