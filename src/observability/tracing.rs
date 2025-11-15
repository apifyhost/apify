//! Distributed tracing setup with OpenTelemetry and structured logging

use opentelemetry::KeyValue;
use opentelemetry::trace::TracerProvider as _;
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::{Resource, trace::SdkTracerProvider};
use std::time::Duration;
use tracing_subscriber::{EnvFilter, Layer, fmt, layer::SubscriberExt, util::SubscriberInitExt};

/// Initialize tracing and logging subsystem
///
/// Sets up:
/// - Structured JSON logging to stdout
/// - Optional OpenTelemetry tracing to OTLP collector
/// - Environment-based log level filtering
pub fn init_tracing(
    service_name: &str,
    otlp_endpoint: Option<&str>,
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

    // Initialize OpenTelemetry tracer if endpoint provided
    if let Some(endpoint) = otlp_endpoint {
        let exporter = opentelemetry_otlp::SpanExporter::builder()
            .with_tonic()
            .with_endpoint(endpoint)
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

        opentelemetry::global::set_tracer_provider(provider.clone());

        let telemetry_layer =
            tracing_opentelemetry::layer().with_tracer(provider.tracer(service_name.to_string()));

        // Combine layers: structured logging + OpenTelemetry
        tracing_subscriber::registry()
            .with(fmt_layer)
            .with(telemetry_layer)
            .init();

        tracing::info!(
            service = service_name,
            otlp_endpoint = endpoint,
            "Tracing initialized with OpenTelemetry"
        );
    } else {
        // Just structured logging without OpenTelemetry
        tracing_subscriber::registry().with(fmt_layer).init();

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
