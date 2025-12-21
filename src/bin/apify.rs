//! Application entry point, responsible for parsing CLI args, loading config, and starting services

use apify::{
    app_state::OpenApiStateConfig,
    config::{ApiRef, Config, OpenAPIConfig},
    modules::{metrics::init_metrics, tracing::shutdown_tracing},
    server::{start_docs_server, start_listener},
    startup::{RuntimeInitData, build_runtime, init_database, setup_logging},
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

    /// Enable Control Plane
    #[arg(long)]
    control_plane: bool,

    /// Enable Data Plane (default is true, unless --control-plane is set and this is not explicitly set)
    #[arg(long)]
    data_plane: bool,
}

fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Parse command-line arguments
    let cli = Cli::parse();

    // Determine modes
    // If neither is specified, default to Data Plane only (backward compatibility)
    // If only control_plane is specified, run only CP.
    // If only data_plane is specified, run only DP.
    // If both, run both.
    let (run_cp, run_dp) = if cli.control_plane || cli.data_plane {
        (cli.control_plane, cli.data_plane)
    } else {
        // Default: Data Plane only
        (false, true)
    };

    // Load main configuration from specified file path
    let config = Config::from_file(&cli.config)?;

    // Setup logging
    let (tracing_enabled, otlp_endpoint, log_level) = setup_logging(&config)?;

    // Initialize Metadata DB connection and load configs
    let rt_init = build_runtime()?;

    let (control_plane_db, db_openapi_configs, db_auth_config, db_datasources, db_listeners): RuntimeInitData =
        rt_init.block_on(async {
            let db = init_database(&config).await.map_err(|e| e.to_string())?;

            // If Control Plane is enabled, initialize schema and start server
            if run_cp {
                if let Some(cp_config) = config.control_plane.clone() {
                    tracing::info!("Starting Control Plane Server");
                    // Initialize metadata schema
                    db.initialize_schema(apify::control_plane::get_metadata_schemas())
                        .await
                        .map_err(|e| e.to_string())?;

                    let db_clone = db.clone();
                    tokio::spawn(async move {
                        if let Err(e) = apify::control_plane::start_control_plane_server(cp_config, db_clone).await {
                            tracing::error!("Control Plane Server failed: {}", e);
                            std::process::exit(1);
                        }
                    });
                } else {
                    tracing::warn!("Control Plane enabled but configuration missing");
                }
            }

            if !run_dp {
                // If only CP is running, we just need to keep the runtime alive.
                // But we are inside block_on, so we return empty data and handle the wait outside.
                return Ok::<_, String>((None, std::collections::HashMap::new(), None, None, None));
            }

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

            Ok::<_, String>((Some(db), api_configs, auth_configs, datasources, listeners))
        })?;

    tracing::info!(
        config_file = %cli.config,
        "Configuration loaded successfully"
    );

    // If Data Plane is NOT enabled, we just wait for CP
    if !run_dp {
        if run_cp {
            tracing::info!("Running in Control Plane only mode");

            // Let's handle signals
            let (tx, rx) = std::sync::mpsc::channel();
            ctrlc::set_handler(move || {
                tx.send(()).expect("Could not send signal on channel.");
            })
            .expect("Error setting Ctrl-C handler");

            tracing::info!("Waiting for Ctrl-C...");
            rx.recv().expect("Could not receive from channel.");
            tracing::info!("Shutting down...");
            return Ok(());
        } else {
            // Neither?
            tracing::warn!("Neither Data Plane nor Control Plane enabled. Exiting.");
            return Ok(());
        }
    }

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
            otlp_endpoint.clone()
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
            tracing::info!(
                port = metrics_port,
                "Metrics endpoint will start with OpenTelemetry tracing"
            );
        } else {
            tracing::info!(port = metrics_port, "Metrics endpoint started");
        }
    }

    let mut listeners = if let Some(mut l) = config.listeners {
        if let Some(db_l) = db_listeners {
            l.extend(db_l);
        }
        Some(l)
    } else {
        db_listeners
    };

    // Distribute global APIs to listeners
    if let Some(global_apis) = &config.apis
        && let Some(listeners_vec) = &mut listeners
    {
        for api_config in global_apis {
            let api_ref = ApiRef::WithConfig {
                path: api_config.path.clone(),
                modules: api_config.modules.clone(),
                datasource: api_config.datasource.clone(),
                access_log: api_config.access_log.clone(),
            };

            if let Some(target_listeners) = &api_config.listeners {
                for listener_name in target_listeners {
                    let mut found = false;
                    for listener in listeners_vec.iter_mut() {
                        if let Some(name) = &listener.name
                            && name == listener_name
                        {
                            if listener.apis.is_none() {
                                listener.apis = Some(Vec::new());
                            }
                            listener.apis.as_mut().unwrap().push(api_ref.clone());
                            found = true;
                        }
                    }
                    if !found {
                        tracing::warn!(
                            "Listener '{}' not found for API '{}'",
                            listener_name,
                            api_config.path
                        );
                    }
                }
            } else {
                tracing::warn!("Global API {} has no listeners configured", api_config.path);
            }
        }
    }

    for (listener_idx, listener_config) in listeners.clone().into_iter().flatten().enumerate() {
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
        }) && listener_idx == 0
        {
            let listener_config_clone = listener_config.clone();
            let datasources_clone = datasources.clone();
            let openapi_configs_clone = openapi_configs.clone();
            let auth_config_clone = auth_config.clone();
            let access_log_config = config.modules.as_ref().and_then(|m| m.access_log.clone());
            let access_log_config_clone = access_log_config.clone();

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

    // Track running listeners to avoid duplicates
    let mut running_listeners: std::collections::HashSet<String> = std::collections::HashSet::new();
    if let Some(l_list) = &listeners {
        for l in l_list {
            let key = format!("{}:{}", l.ip, l.port);
            running_listeners.insert(key);
        }
    }

    tracing::info!("Entering main supervisor loop");

    loop {
        // Check for new listeners if Control Plane DB is available
        if let Some(db) = &control_plane_db {
            let new_listeners = rt_init.block_on(async {
                apify::control_plane::load_listeners(db).await
            });

            match new_listeners {
                Ok(Some(list)) => {
                    for listener_config in list {
                        let key = format!("{}:{}", listener_config.ip, listener_config.port);
                        if !running_listeners.contains(&key) {
                            tracing::info!(key = %key, "Found new listener configuration, spawning...");

                            // Prepare config clones
                            let datasources_clone = datasources.clone();
                            let auth_config_clone = auth_config.clone();
                            let access_log_config = config.modules.as_ref().and_then(|m| m.access_log.clone());
                            let control_plane_db_clone = control_plane_db.clone();

                            // Resolve OpenAPI configs for this listener
                            let api_configs_map = rt_init.block_on(async {
                                apify::control_plane::load_api_configs(db).await.unwrap_or_default()
                            });

                            let mut openapi_configs = Vec::new();
                            if let Some(api_refs) = &listener_config.apis {
                                for api_ref in api_refs {
                                    match api_ref {
                                        ApiRef::Path(p) => {
                                            // Check if in DB first
                                            if let Some(db_config) = api_configs_map.get(p) {
                                                openapi_configs.push(db_config.clone());
                                            } else {
                                                let api_path = config_dir.join(p);
                                                if let Ok(openapi_config) = OpenAPIConfig::from_file(&api_path.to_string_lossy()) {
                                                    openapi_configs.push(OpenApiStateConfig {
                                                        config: openapi_config,
                                                        modules: None,
                                                        datasource: None,
                                                        access_log: None,
                                                    });
                                                }
                                            }
                                        }
                                        ApiRef::WithConfig { path, modules, datasource, access_log } => {
                                            // Check if in DB first
                                            if let Some(db_config) = api_configs_map.get(path) {
                                                let mut config = db_config.clone();
                                                if datasource.is_some() { config.datasource = datasource.clone(); }
                                                if modules.is_some() { config.modules = modules.clone(); }
                                                if access_log.is_some() { config.access_log = access_log.clone(); }
                                                openapi_configs.push(config);
                                            } else {
                                                let api_path = config_dir.join(path);
                                                if let Ok(openapi_config) = OpenAPIConfig::from_file(&api_path.to_string_lossy()) {
                                                    let ds_info = datasource.clone();
                                                    openapi_configs.push(OpenApiStateConfig {
                                                        config: openapi_config,
                                                        modules: modules.clone(),
                                                        datasource: ds_info,
                                                        access_log: access_log.clone(),
                                                    });
                                                }
                                            }
                                        }
                                    }
                                }
                            }

                            // Spawn threads
                            for thread_id in 0..num_threads {
                                let l_clone = listener_config.clone();
                                let ds_clone = datasources_clone.clone();
                                let oa_clone = openapi_configs.clone();
                                let ac_clone = auth_config_clone.clone();
                                let al_clone = access_log_config.clone();
                                let cp_clone = control_plane_db_clone.clone();

                                thread::spawn(move || {
                                     let _ = start_listener(l_clone, thread_id, ds_clone, oa_clone, ac_clone, al_clone, cp_clone);
                                });
                            }

                            running_listeners.insert(key);
                        }
                    }
                }
                Ok(None) => {}
                Err(e) => {
                    tracing::warn!("Failed to load listeners in supervisor loop: {}", e);
                }
            }
        }

        // Sleep
        std::thread::sleep(std::time::Duration::from_secs(5));
    }

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
                init_logging(log_level.as_deref());
                tracing::error!(
                    "Failed to initialize OpenTelemetry: {}, falling back to basic logging",
                    e
                );
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
