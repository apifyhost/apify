//! Application entry point, responsible for parsing CLI args, loading config, and starting services

use apify::{
    config::{ApiRef, Config, OpenAPIConfig},
    observability::{init_metrics, init_tracing, shutdown_tracing},
    server::start_listener,
};
use clap::Parser;
use std::path::Path;
use std::thread;

/// Configurable HTTP server with route matching
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Path to the configuration file (YAML format)
    #[arg(short, long, default_value = "config.yaml")]
    config: String,
}

fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Parse command-line arguments
    let cli = Cli::parse();

    // Load main configuration from specified file path
    let config = Config::from_file(&cli.config)?;

    // Initialize observability (logging, tracing, metrics)
    let otlp_endpoint = config
        .observability
        .as_ref()
        .and_then(|o| o.otlp_endpoint.as_deref());
    let log_level = config
        .observability
        .as_ref()
        .and_then(|o| o.log_level.as_deref());

    init_tracing("apify", otlp_endpoint, log_level)?;

    tracing::info!(
        config_file = %cli.config,
        "Configuration loaded successfully"
    );

    // Use datasources from config if available
    let datasources = config.datasource.clone();
    if let Some(ref ds) = datasources {
        tracing::info!(datasource_count = ds.len(), "Datasources configured");
    }

    // Use consumers from config (global or listener-level)
    let global_consumers = config.consumers.clone().unwrap_or_default();

    // Start worker threads (multiple threads per listener, sharing port via SO_REUSEPORT)
    // Allow override via APIFY_THREADS env var (useful for tests)
    let num_threads: usize = std::env::var("APIFY_THREADS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(2); // default 2

    tracing::info!(worker_threads = num_threads, "Initializing worker threads");
    init_metrics(num_threads);

    let mut handles: Vec<thread::JoinHandle<Result<(), Box<dyn std::error::Error + Send + Sync>>>> =
        Vec::new();

    let config_dir = Path::new(&cli.config).parent().unwrap_or(Path::new("."));

    // Start metrics server if enabled
    let metrics_enabled = config
        .observability
        .as_ref()
        .and_then(|o| o.metrics_enabled)
        .unwrap_or(true); // Default enabled
    let metrics_port = config
        .observability
        .as_ref()
        .and_then(|o| o.metrics_port)
        .unwrap_or(9090);

    if metrics_enabled {
        let metrics_handle = thread::spawn(
            move || -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
                start_metrics_server(metrics_port)?;
                Ok(())
            },
        );
        handles.push(metrics_handle);
        tracing::info!(port = metrics_port, "Metrics endpoint started");
    }

    for (listener_idx, listener_config) in config.listeners.into_iter().enumerate() {
        // Merge global consumers with listener-specific consumers
        let mut all_consumers = global_consumers.clone();
        if let Some(ref listener_consumers) = listener_config.consumers {
            all_consumers.extend(listener_consumers.clone());
        }

        // Load OpenAPI configurations for this listener with datasource info
        let mut openapi_configs = Vec::new();
        if let Some(api_refs) = &listener_config.apis {
            for api_ref in api_refs {
                match api_ref {
                    ApiRef::Path(p) => {
                        let api_path = config_dir.join(p);
                        match OpenAPIConfig::from_file(&api_path.to_string_lossy()) {
                            Ok(openapi_config) => {
                                tracing::info!(path = %p, "OpenAPI config loaded");
                                openapi_configs.push((openapi_config, None, None));
                            }
                            Err(e) => {
                                tracing::error!(path = %p, error = %e, "Failed to load OpenAPI config")
                            }
                        }
                    }
                    ApiRef::WithConfig {
                        path,
                        modules,
                        datasource,
                    } => {
                        let api_path = config_dir.join(path);
                        match OpenAPIConfig::from_file(&api_path.to_string_lossy()) {
                            Ok(openapi_config) => {
                                let ds_info = if let Some(ds_name) = datasource {
                                    tracing::info!(
                                        path = %path,
                                        datasource = %ds_name,
                                        "OpenAPI config loaded with datasource"
                                    );
                                    Some(ds_name.clone())
                                } else {
                                    tracing::info!(path = %path, "OpenAPI config loaded");
                                    None
                                };
                                openapi_configs.push((openapi_config, modules.clone(), ds_info));
                            }
                            Err(e) => {
                                tracing::error!(path = %path, error = %e, "Failed to load OpenAPI config")
                            }
                        }
                    }
                }
            }
        }

        for thread_id in 0..num_threads {
            let listener_config_clone = listener_config.clone();
            let datasources_clone = datasources.clone();
            let openapi_configs_clone = openapi_configs.clone();
            let consumers_clone = all_consumers.clone();
            let handle = thread::spawn(
                move || -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
                    tracing::info!(
                        thread_id = thread_id,
                        listener_idx = listener_idx,
                        port = listener_config_clone.port,
                        "Starting worker thread"
                    );
                    start_listener(
                        listener_config_clone,
                        thread_id,
                        datasources_clone,
                        openapi_configs_clone,
                        consumers_clone,
                    )?;
                    Ok(())
                },
            );
            handles.push(handle);
        }
    }

    // Wait for all threads to complete
    for (idx, handle) in handles.into_iter().enumerate() {
        match handle.join() {
            Ok(Ok(())) => {
                tracing::info!(thread_idx = idx, "Thread exited normally");
            }
            Ok(Err(e)) => {
                tracing::error!(thread_idx = idx, error = %e, "Thread execution error");
            }
            Err(e) => {
                tracing::error!(thread_idx = idx, error = ?e, "Thread panicked");
            }
        }
    }

    tracing::info!("All threads exited, shutting down");
    shutdown_tracing();

    Ok(())
}

/// Start metrics HTTP server
fn start_metrics_server(port: u16) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use apify::{
        http_body_util::Full,
        hyper::{
            Request, Response, StatusCode, body::Bytes, server::conn::http1, service::service_fn,
        },
        hyper_util::rt::TokioIo,
        observability::export_metrics,
        tokio::{self, net::TcpListener},
    };

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;

    rt.block_on(async {
        let addr: std::net::SocketAddr = format!("0.0.0.0:{}", port).parse()?;
        let listener = TcpListener::bind(addr).await?;
        tracing::info!(address = %addr, "Metrics server listening");

        loop {
            let (stream, _) = listener.accept().await?;
            let io = TokioIo::new(stream);

            tokio::task::spawn(async move {
                let service = service_fn(|_req: Request<hyper::body::Incoming>| async {
                    match export_metrics() {
                        Ok(body) => Ok::<_, hyper::Error>(
                            Response::builder()
                                .status(StatusCode::OK)
                                .header("Content-Type", "text/plain; version=0.0.4")
                                .body(Full::new(Bytes::from(body)))
                                .unwrap(),
                        ),
                        Err(e) => {
                            tracing::error!(error = %e, "Failed to export metrics");
                            Ok(Response::builder()
                                .status(StatusCode::INTERNAL_SERVER_ERROR)
                                .body(Full::new(Bytes::from("Error exporting metrics")))
                                .unwrap())
                        }
                    }
                });

                if let Err(err) = http1::Builder::new().serve_connection(io, service).await {
                    tracing::error!(error = ?err, "Metrics connection error");
                }
            });
        }
    })
}
