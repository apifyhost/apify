//! All-in-one entry point that starts both Control Plane and Data Plane

use apify::{
    app_state::OpenApiStateConfig,
    config::{Config, OpenAPIConfig},
    modules::metrics::init_metrics,
    server::{ServerContext, start_docs_server, start_listener},
    startup::{build_runtime, init_database, setup_logging},
};
use clap::Parser;
use std::path::Path;
use std::thread;

/// Apify All-in-One Server (Control Plane + Data Plane)
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
    let config_path = Path::new(&cli.config);
    let config_dir = config_path.parent().unwrap_or_else(|| Path::new("."));

    // Load main configuration from specified file path
    let config = Config::from_file(&cli.config)?;

    // Setup logging
    let (_tracing_enabled, _otlp_endpoint, _log_level) = setup_logging(&config)?;

    // Initialize Runtime
    let rt_init = build_runtime()?;

    let (control_plane_db, db_openapi_configs, db_auth_config, db_datasources, db_listeners) =
        rt_init.block_on(async {
            let db = init_database(&config).await.map_err(|e| e.to_string())?;

            // Initialize Control Plane schema
            if let Some(cp_config) = config.control_plane.clone() {
                tracing::info!("Starting Control Plane Server");
                // Initialize metadata schema
                db.initialize_schema(apify::control_plane::get_metadata_schemas())
                    .await
                    .map_err(|e| e.to_string())?;

                let db_clone = db.clone();
                let cp_config_for_server = cp_config.clone();
                tokio::spawn(async move {
                    if let Err(e) = apify::control_plane::start_control_plane_server(
                        cp_config_for_server,
                        db_clone,
                    )
                    .await
                    {
                        tracing::error!("Control Plane Server failed: {}", e);
                        std::process::exit(1);
                    }
                });

                // Start Docs Server if configured
                let docs_config = config
                    .modules
                    .as_ref()
                    .and_then(|m| m.openapi_docs.as_ref());

                if let Some(docs_c) = docs_config
                    && docs_c.enabled.unwrap_or(false)
                    && let Some(port) = docs_c.port
                {
                    tracing::info!("Docs server enabled on port {}", port);

                    // 1. Listeners
                    let db_listeners = apify::control_plane::load_listeners(&db)
                        .await
                        .ok()
                        .flatten();
                    let mut listeners_cfg = config.listeners.clone().unwrap_or_default();
                    if let Some(dbl) = db_listeners {
                        listeners_cfg.extend(dbl);
                    }

                    // Use the first listener as reference
                    let target_listener = listeners_cfg.first().cloned().unwrap_or_else(|| {
                        apify::config::ListenerConfig {
                            name: Some("default-docs".to_string()),
                            port: 0,
                            ip: "0.0.0.0".to_string(),
                            protocol: "http".to_string(),
                            routes: None,
                            modules: None,
                            consumers: None,
                        }
                    });

                    let target_listener_clone = target_listener.clone();
                    // 2. OpenAPI Configs
                    let mut openapi_configs = Vec::new();

                    // A. Static
                    if let Some(global_apis) = &config.apis {
                        for api_config in global_apis {
                            if let Some(target_listeners) = &api_config.listeners
                                && let Some(lname) = &target_listener_clone.name
                                && target_listeners.contains(lname)
                            {
                                let api_path = config_dir.join(&api_config.path);
                                if let Ok(openapi_config) =
                                    OpenAPIConfig::from_file(&api_path.to_string_lossy())
                                {
                                    openapi_configs.push(OpenApiStateConfig {
                                        config: openapi_config,
                                        modules: api_config.modules.clone(),
                                        datasource: api_config.datasource.clone(),
                                        access_log: api_config.access_log.clone(),
                                        listeners: Some(target_listeners.clone()),
                                    });
                                }
                            }
                        }
                    }

                    // B. Dynamic (load from DB)
                    if let Ok(db_apis) = apify::control_plane::load_api_configs(&db).await {
                        for ctx in db_apis.values() {
                            if let Some(target_listeners) = &ctx.listeners
                                && let Some(lname) = &target_listener_clone.name
                                && target_listeners.contains(lname)
                            {
                                openapi_configs.push(ctx.clone());
                            }
                        }
                    }

                    // 3. Datasources
                    let mut datasources = config.datasource.clone().unwrap_or_default();
                    if let Ok(Some(db_ds)) = apify::control_plane::load_datasources(&db).await {
                        datasources.extend(db_ds);
                    }

                    // 4. Auth
                    let mut auth_config = config.auth.clone();
                    if let Ok(Some(db_auth)) = apify::control_plane::load_auth_configs(&db).await {
                        if let Some(existing) = &mut auth_config {
                            existing.extend(db_auth);
                        } else {
                            auth_config = Some(db_auth);
                        }
                    }

                    let access_log = config.modules.as_ref().and_then(|m| m.access_log.clone());
                    let db_for_docs = db.clone();
                    let cp_config_for_docs = Some(cp_config.clone());

                    let _ = std::thread::spawn(move || {
                        let context = ServerContext {
                            datasources: Some(datasources),
                            openapi_configs,
                            auth_config,
                            access_log_config: access_log,
                            control_plane_db: Some(db_for_docs),
                            control_plane_config: cp_config_for_docs,
                        };

                        if let Err(e) = start_docs_server(port, target_listener_clone, context) {
                            tracing::error!("Docs server failed: {}", e);
                        }
                    });
                }
            } else {
                tracing::warn!("Control Plane enabled but configuration missing");
            }

            tracing::info!("Starting in Data Plane mode");
            // Load configs from DB
            let api_configs = match apify::control_plane::load_api_configs(&db).await {
                Ok(configs) => {
                    tracing::info!(count = configs.len(), "Loaded API configs from Metadata DB");
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
        "Configuration loaded successfully, starting Data Plane"
    );

    // Initialize metrics
    let num_threads: usize = std::env::var("APIFY_THREADS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(2); // default 2

    tracing::info!(worker_threads = num_threads, "Initializing worker threads");
    init_metrics(num_threads);

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

    // Collect all listeners
    let mut listeners = config.listeners.clone().unwrap_or_default();
    if let Some(db_l) = db_listeners {
        listeners.extend(db_l);
    }

    // Build merged API configs with module and datasource info for DP
    let mut merged_api_configs = std::collections::HashMap::new();
    for (k, v) in &db_openapi_configs {
        merged_api_configs.insert(k.clone(), v.clone());
    }
    // Config file APIs already included in db_openapi_configs via load_api_configs

    if listeners.is_empty() {
        // If CP is running, keep the process alive (user can Ctrl-C to exit)
        if config.control_plane.is_some() {
            tracing::info!("No Data Plane listeners configured. Running Control Plane only mode");

            // Handle signals for graceful shutdown
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
            tracing::warn!("No Data Plane listeners and no Control Plane configured. Exiting.");
            return Ok(());
        }
    }

    // Start listener threads
    let num_listeners = listeners.len();
    let mut handles = vec![];

    for (thread_id, listener_config) in listeners.into_iter().enumerate() {
        let listener_name = listener_config
            .name
            .clone()
            .unwrap_or_else(|| format!("listener-{}", thread_id));

        // Merge all configs for this listener thread
        let mut openapi_configs = Vec::new();
        for api_cfg in merged_api_configs.values() {
            if let Some(ref listeners_for_api) = api_cfg.listeners
                && listeners_for_api.contains(&listener_name)
            {
                openapi_configs.push(api_cfg.clone());
            }
        }

        let datasources_clone = datasources.clone();
        let auth_config_clone = db_auth_config.clone();
        let access_log_config = config.modules.as_ref().and_then(|m| m.access_log.clone());
        let control_plane_db_clone = control_plane_db.clone();
        let control_plane_config = config.control_plane.clone();

        let handle = thread::spawn(move || {
            let context = ServerContext {
                datasources: datasources_clone,
                openapi_configs,
                auth_config: auth_config_clone,
                access_log_config,
                control_plane_db: control_plane_db_clone,
                control_plane_config,
            };

            if let Err(e) = start_listener(listener_config, thread_id, context) {
                tracing::error!("Listener '{}' failed: {}", listener_name, e);
            }
        });

        handles.push(handle);
    }

    tracing::info!("Started {} listener threads", num_listeners);

    // Wait for all threads
    for handle in handles {
        let _ = handle.join();
    }

    Ok(())
}
