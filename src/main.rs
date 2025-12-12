//! Application entry point, responsible for parsing CLI args, loading config, and starting services

use apify::{
    app_state::OpenApiStateConfig,
    config::{ApiRef, Config, OpenAPIConfig},
    modules::{
        metrics::init_metrics,
        tracing::{init_tracing, shutdown_tracing},
    },
    server::{start_docs_server, start_listener},
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

    /// Enable Control Plane mode
    #[arg(long)]
    control_plane: bool,
}

fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Parse command-line arguments
    let cli = Cli::parse();

    // Load main configuration from specified file path
    let config = Config::from_file(&cli.config)?;

    // Initialize Metadata DB connection and load configs
    let rt_init = apify::tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;

    type RuntimeInitData = (
        Option<apify::database::DatabaseManager>,
        std::collections::HashMap<String, apify::app_state::OpenApiStateConfig>,
        Option<Vec<apify::config::Authenticator>>,
        Option<std::collections::HashMap<String, apify::config::DatabaseSettings>>,
        Option<Vec<apify::config::ListenerConfig>>,
    );

    let (control_plane_db, db_openapi_configs, db_auth_config, db_datasources, db_listeners): RuntimeInitData =
        rt_init.block_on(async {
            let db_config = if let Some(cp_config) = &config.control_plane {
                let s = &cp_config.database;
                let url = if s.driver == "sqlite" {
                    format!("sqlite:{}", s.database)
                } else {
                    let mut url = "postgres://".to_string();
                    if let Some(user) = &s.user {
                        url.push_str(user);
                        if let Some(pass) = &s.password {
                            url.push(':');
                            url.push_str(pass);
                        }
                        url.push('@');
                    }
                    if let Some(host) = &s.host {
                        url.push_str(host);
                    }
                    if let Some(port) = s.port {
                        url.push(':');
                        url.push_str(&port.to_string());
                    }
                    url.push('/');
                    url.push_str(&s.database);
                    url
                };
                apify::database::DatabaseRuntimeConfig {
                    driver: s.driver.clone(),
                    url,
                    max_size: s.max_pool_size.unwrap_or(5) as u32,
                }
            } else {
                apify::database::DatabaseRuntimeConfig::sqlite_default()
            };

            let db = apify::database::DatabaseManager::new(db_config)
                .await
                .map_err(|e| e.to_string())?;

            if cli.control_plane {
                tracing::info!("Starting in Control Plane mode");
                // Initialize metadata schema
                db.initialize_schema(apify::control_plane::get_metadata_schemas())
                    .await
                    .map_err(|e| e.to_string())?;
                Ok::<_, String>((Some(db), std::collections::HashMap::new(), None, None, None))
            } else {
                tracing::info!("Starting in Data Plane mode");
                // Load configs from DB
                let api_configs = match apify::control_plane::load_api_configs(&db).await {
                    Ok(configs) => {
                        tracing::info!(
                            count = configs.len(),
                            "Loaded API configs from Metadata DB"
                        );
                        configs
                    }
                    Err(e) => {
                        tracing::warn!("Failed to load API configs from DB: {}", e);
                        std::collections::HashMap::new()
                    }
                };

                let auth_configs = match apify::control_plane::load_auth_configs(&db).await {
                    Ok(configs) => {
                        if let Some(c) = &configs {
                            tracing::info!(count = c.len(), "Loaded Auth configs from Metadata DB");
                        }
                        configs
                    }
                    Err(e) => {
                        tracing::warn!("Failed to load Auth configs from DB: {}", e);
                        None
                    }
                };

                let datasources = match apify::control_plane::load_datasources(&db).await {
                    Ok(ds) => {
                        if let Some(d) = &ds {
                            tracing::info!(count = d.len(), "Loaded Datasources from Metadata DB");
                        }
                        ds
                    }
                    Err(e) => {
                        tracing::warn!("Failed to load Datasources from DB: {}", e);
                        None
                    }
                };

                let listeners = match apify::control_plane::load_listeners(&db).await {
                    Ok(l) => {
                        if let Some(list) = &l {
                            tracing::info!(count = list.len(), "Loaded Listeners from Metadata DB");
                        }
                        l
                    }
                    Err(e) => {
                        tracing::warn!("Failed to load Listeners from DB: {}", e);
                        None
                    }
                };

                Ok::<_, String>((None, api_configs, auth_configs, datasources, listeners))
            }
        })?;

    // Get global modules configuration
    let tracing_config = config.modules.as_ref().and_then(|m| m.tracing.as_ref());
    let tracing_enabled = tracing_config.and_then(|t| t.enabled).unwrap_or(true);

    if cli.control_plane {
        if let Some(cp_config) = config.control_plane {
            if let Some(db) = control_plane_db {
                // Initialize tracing for control plane (simple logging)
                init_tracing("apify-cp", None, config.log_level.as_deref())?;
                tracing::info!("Starting Control Plane Server");
                rt_init.block_on(apify::control_plane::start_control_plane_server(
                    cp_config, db,
                ))?;
                return Ok(());
            }
        } else {
            return Err("Control plane configuration missing".into());
        }
    }

    let otlp_endpoint = tracing_config.and_then(|t| t.otlp_endpoint.as_deref());
    let log_level = config.log_level.as_deref();

    // If OpenTelemetry is configured AND enabled, we need to defer ALL tracing initialization
    // to the metrics server thread (which has Tokio runtime)
    // Otherwise, initialize basic logging here
    if tracing_enabled && otlp_endpoint.is_some() {
        // Print to stderr since tracing isn't initialized yet
        eprintln!("Deferring tracing initialization to Tokio runtime (OpenTelemetry enabled)");
    } else {
        init_tracing("apify", None, log_level)?;
    }

    tracing::info!(
        config_file = %cli.config,
        "Configuration loaded successfully"
    );

    // Use datasources from config if available, merge with DB datasources
    let mut datasources_map = config.datasource.clone().unwrap_or_default();
    if let Some(db_ds) = db_datasources {
        for (name, ds) in db_ds {
            datasources_map.insert(name, ds);
        }
    }
    let datasources = if datasources_map.is_empty() {
        None
    } else {
        Some(datasources_map)
    };

    if let Some(ref ds) = datasources {
        tracing::info!(datasource_count = ds.len(), "Datasources configured");
    }

    // Use auth config
    let mut auth_config = config.auth.clone();
    if let Some(db_auth) = db_auth_config {
        if let Some(existing) = &mut auth_config {
            existing.extend(db_auth);
        } else {
            auth_config = Some(db_auth);
        }
    }

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
        .modules
        .as_ref()
        .and_then(|m| m.metrics.as_ref())
        .and_then(|metrics| metrics.enabled)
        .unwrap_or(true); // Default enabled
    let metrics_port = config
        .modules
        .as_ref()
        .and_then(|m| m.metrics.as_ref())
        .and_then(|metrics| metrics.port)
        .unwrap_or(9090);

    if metrics_enabled {
        let otlp_endpoint_for_thread = if tracing_enabled {
            otlp_endpoint.map(|s| s.to_string())
        } else {
            None
        };
        let log_level_for_thread = log_level.map(|s| s.to_string());
        let metrics_handle = thread::spawn(
            move || -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
                start_metrics_server(metrics_port, otlp_endpoint_for_thread, log_level_for_thread)?;
                Ok(())
            },
        );
        handles.push(metrics_handle);

        if tracing_enabled && otlp_endpoint.is_some() {
            eprintln!(
                "Metrics endpoint will start on port {} with OpenTelemetry tracing",
                metrics_port
            );
        } else {
            tracing::info!(port = metrics_port, "Metrics endpoint started");
        }
    }

    let listeners = config.listeners.or(db_listeners);
    for (listener_idx, listener_config) in listeners.into_iter().flatten().enumerate() {
        let auth_config_clone = auth_config.clone();

        // Load OpenAPI configurations for this listener with datasource info
        let mut openapi_configs = Vec::new();
        if let Some(api_refs) = &listener_config.apis {
            for api_ref in api_refs {
                match api_ref {
                    ApiRef::Path(p) => {
                        // Check if in DB first
                        if let Some(db_config) = db_openapi_configs.get(p) {
                            tracing::info!(path = %p, "OpenAPI config loaded from DB");
                            openapi_configs.push(db_config.clone());
                        } else {
                            let api_path = config_dir.join(p);
                            match OpenAPIConfig::from_file(&api_path.to_string_lossy()) {
                                Ok(openapi_config) => {
                                    tracing::info!(path = %p, "OpenAPI config loaded");
                                    openapi_configs.push(OpenApiStateConfig {
                                        config: openapi_config,
                                        modules: None,
                                        datasource: None,
                                        access_log: None,
                                    });
                                }
                                Err(e) => {
                                    tracing::error!(path = %p, error = %e, "Failed to load OpenAPI config")
                                }
                            }
                        }
                    }
                    ApiRef::WithConfig {
                        path,
                        modules,
                        datasource,
                        access_log,
                    } => {
                        // Check if in DB first
                        if let Some(db_config) = db_openapi_configs.get(path) {
                            tracing::info!(path = %path, "OpenAPI config loaded from DB");
                            let mut config = db_config.clone();
                            if datasource.is_some() {
                                config.datasource = datasource.clone();
                            }
                            if modules.is_some() {
                                config.modules = modules.clone();
                            }
                            if access_log.is_some() {
                                config.access_log = access_log.clone();
                            }
                            openapi_configs.push(config);
                        } else {
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
                                    openapi_configs.push(OpenApiStateConfig {
                                        config: openapi_config,
                                        modules: modules.clone(),
                                        datasource: ds_info,
                                        access_log: access_log.clone(),
                                    });
                                }
                                Err(e) => {
                                    tracing::error!(path = %path, error = %e, "Failed to load OpenAPI config")
                                }
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
            let auth_config_clone = auth_config_clone.clone();
            let access_log_config = config.modules.as_ref().and_then(|m| m.access_log.clone());
            let access_log_config_clone = access_log_config.clone();
            let control_plane_db_clone = control_plane_db.clone();

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
                        auth_config_clone,
                        access_log_config_clone,
                        control_plane_db_clone,
                    )?;
                    Ok(())
                },
            );
            handles.push(handle);
        }

        // Start docs server if configured
        let docs_config = config
            .modules
            .as_ref()
            .and_then(|m| m.openapi_docs.as_ref());
        if let Some(docs_port) = docs_config.and_then(|c| {
            if c.enabled.unwrap_or(false) {
                c.port
            } else {
                None
            }
        }) {
            // We need to construct a minimal AppState for the docs server
            // For now, we can reuse the logic from start_listener but simplified,
            // or better yet, we need access to the AppState created inside start_listener.
            // However, AppState is created per-thread inside start_listener.
            // To support a separate docs server that needs access to the generated OpenAPI spec (which is in AppState -> CRUDHandler),
            // we should probably create the AppState ONCE here in main (or at least the CRUDHandler part) and pass it down.
            // But AppState creation is async and involves DB connection.

            // Alternative: The docs server needs to know the OpenAPI spec.
            // The spec is loaded from files in main().
            // We can reconstruct the AppState or just the relevant parts for the docs server.
            // Actually, the docs server only needs the OpenAPI spec JSON.
            // We can pass the loaded openapi_configs to the docs server and let it build a minimal state or just serve the JSON.

            // Let's modify start_docs_server to take openapi_configs directly or build a minimal state.
            // But wait, start_docs_server takes Arc<AppState>.
            // And AppState::new_with_crud is what builds the merged spec.

            // To avoid refactoring everything, let's spawn the docs server thread
            // and inside it, create a dedicated AppState just for serving docs.
            // This means double DB connection initialization if we are not careful,
            // but for docs we might not need DB connection if we only serve the JSON?
            // AppState::new_with_crud DOES initialize DB.

            // Optimization: Refactor AppState creation to separate Spec generation from DB init?
            // For now, let's just create a separate AppState for the docs server.
            // It might be slightly inefficient (extra DB pool) but it's robust.

            let listener_config_clone = listener_config.clone();
            let datasources_clone = datasources.clone();
            let openapi_configs_clone = openapi_configs.clone();
            let auth_config_clone = auth_config_clone.clone();
            let access_log_config = config.modules.as_ref().and_then(|m| m.access_log.clone());
            let access_log_config_clone = access_log_config.clone();

            // Only start docs server once (e.g. for the first listener, or globally?)
            // The user asked for "separate port", implying one global docs port.
            // But config structure has listeners.
            // If we have multiple listeners, do we have one docs port for all?
            // The config.docs_port is global.
            // So we should start it only once.
            if listener_idx == 0 {
                let handle = thread::spawn(
                    move || -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
                        tracing::info!(port = docs_port, "Starting docs server");

                        // Create a runtime for AppState creation
                        let rt = tokio::runtime::Builder::new_current_thread()
                            .enable_all()
                            .build()?;

                        let state = rt.block_on(async {
                            apify::app_state::AppState::new_with_crud(
                                apify::app_state::AppStateConfig {
                                    routes: listener_config_clone.routes,
                                    datasources: datasources_clone,
                                    openapi_configs: openapi_configs_clone,
                                    listener_modules: listener_config_clone.modules,
                                    auth_config: auth_config_clone,
                                    public_url: Some(format!(
                                        "http://localhost:{}",
                                        listener_config_clone.port
                                    )),
                                    access_log_config: access_log_config_clone,
                                    control_plane_db: None,
                                },
                            )
                            .await
                        })?;

                        start_docs_server(docs_port, std::sync::Arc::new(state))?;
                        Ok(())
                    },
                );
                handles.push(handle);
            }
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
fn start_metrics_server(
    port: u16,
    otlp_endpoint: Option<String>,
    log_level: Option<String>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use apify::{
        http_body_util::Full,
        hyper::{
            Request, Response, StatusCode, body::Bytes, server::conn::http1, service::service_fn,
        },
        hyper_util::rt::TokioIo,
        modules::{
            metrics::export_metrics,
            tracing::{init_logging, init_tracing_with_otel},
        },
        tokio::{self, net::TcpListener},
    };

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;

    rt.block_on(async {
        // Initialize tracing now that we're in Tokio runtime
        if let Some(ref endpoint) = otlp_endpoint {
            // Initialize with OpenTelemetry support
            if let Err(e) = init_tracing_with_otel("apify", endpoint, log_level.as_deref()).await {
                // Fallback to basic logging
                eprintln!(
                    "Failed to initialize OpenTelemetry: {}, falling back to basic logging",
                    e
                );
                init_logging(log_level.as_deref());
            }
        } else {
            // Just basic logging (shouldn't reach here if main() already initialized)
            init_logging(log_level.as_deref());
        }

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
