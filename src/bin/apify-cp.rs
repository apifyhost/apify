//! Control Plane application entry point

use apify::{
    app_state::OpenApiStateConfig,
    config::{Config, OpenAPIConfig},
    server::start_docs_server,
    startup::{build_runtime, init_database, setup_logging},
};
use clap::Parser;
use std::path::Path;

/// Apify Control Plane Server
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
    let rt = build_runtime()?;

    rt.block_on(async {
        // Initialize Database
        let db = init_database(&config).await?;

        // Start Docs Server if configured
        let docs_config = config
            .modules
            .as_ref()
            .and_then(|m| m.openapi_docs.as_ref());

        if let Some(docs_c) = docs_config {
            if docs_c.enabled.unwrap_or(false) {
                if let Some(port) = docs_c.port {
                    tracing::info!("Docs server enabled on port {}", port);

                    // 1. Listeners
                    let db_listeners = apify::control_plane::load_listeners(&db).await.ok().flatten();
                    let mut listeners = config.listeners.clone().unwrap_or_default();
                    if let Some(dbl) = db_listeners {
                        listeners.extend(dbl);
                    }

                    // Use the first listener as reference, OR invoke with a dummy config
                    let target_listener = listeners.first().cloned().unwrap_or_else(|| {
                       apify::config::ListenerConfig {
                           name: Some("default-docs".to_string()),
                           port: 0, // Not used
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
                            // If we have a dummy listener, we might miss static configs that are bound to specific names
                            // But usually, static configs are bound to "main-listener" etc.
                            // If there are NO listeners, then static APIs bound to a listener won't match anyway.
                            
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
                            // If we are in "dummy listener" mode, we should perhaps include ALL APIs?
                            // Or just stick to the logic: Docs server reflects what's on a "Target Listener".
                            // But people usually want "All APIs".
                            // For now, let's keep strict matching. If user hasn't created a listener, they haven't "deployed" the API.
                            
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
                    let db_clone = db.clone();

                    let _ = std::thread::spawn(move || {
                        if let Err(e) = start_docs_server(
                            port,
                            target_listener_clone,
                            Some(datasources),
                            openapi_configs,
                            auth_config,
                            access_log,
                            Some(db_clone),
                        ) {
                            tracing::error!("Docs server failed: {}", e);
                        }
                    });
                }
            }
        }

        if let Some(cp_config) = config.control_plane {
            tracing::info!("Starting Control Plane Server");

            // Initialize metadata schema
            db.initialize_schema(apify::control_plane::get_metadata_schemas())
                .await
                .map_err(|e| e.to_string())?;

            apify::control_plane::start_control_plane_server(cp_config, db).await?;
            Ok(())
        } else {
            Err("Control plane configuration missing in config file".into())
        }
    })
}
