//! Network service related (listener creation, service startup)

use super::app_state::AppState;
use super::config::ListenerConfig;
use super::handler::handle_request;
use super::hyper::server::conn::http1;
use super::hyper::service::service_fn;
use super::tokio::net::TcpListener;
use super::{Arc, hyper_util::rt::TokioIo, tokio};
use crate::app_state::AppStateConfig;
use arc_swap::ArcSwap;
use socket2::{Domain, Socket, Type};
use std::error::Error;
use std::net::{SocketAddr, TcpListener as StdTcpListener};

/// Create TCP listener with SO_REUSEPORT support
pub fn create_reuse_port_socket(
    addr: SocketAddr,
) -> Result<TcpListener, Box<dyn Error + Send + Sync>> {
    let socket = Socket::new(Domain::IPV4, Type::STREAM, None)
        .map_err(|e| format!("Failed to create socket: {}", e))?;

    // Enable port reuse and address reuse
    socket
        .set_reuse_port(true)
        .map_err(|e| format!("Failed to set SO_REUSEPORT: {}", e))?;
    socket
        .set_reuse_address(true)
        .map_err(|e| format!("Failed to set SO_REUSEADDR: {}", e))?;

    socket
        .bind(&addr.into())
        .map_err(|e| format!("Failed to bind to address: {}", e))?;
    socket
        .listen(1024)
        .map_err(|e| format!("Failed to listen on socket: {}", e))?;

    // Convert to tokio's non-blocking TcpListener
    let std_listener = StdTcpListener::from(socket);
    std_listener
        .set_nonblocking(true)
        .map_err(|e| format!("Failed to set non-blocking mode: {}", e))?;
    let tokio_listener = TcpListener::from_std(std_listener)
        .map_err(|e| format!("Failed to convert to tokio listener: {}", e))?;

    Ok(tokio_listener)
}

/// Start listener service (runs independently in each thread with current_thread runtime)
pub fn start_listener(
    listener_config: ListenerConfig,
    thread_id: usize,
    datasources: Option<std::collections::HashMap<String, super::config::DatabaseSettings>>,
    openapi_configs: Vec<super::app_state::OpenApiStateConfig>,
    auth_config: Option<Vec<super::config::Authenticator>>,
    access_log_config: Option<super::config::AccessLogConfig>,
    control_plane_db: Option<super::database::DatabaseManager>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    // Critical: Create single-threaded runtime using new_current_thread
    let rt = tokio::runtime::Builder::new_current_thread() // <-- Restored critical line
        .enable_all()
        .build()
        .map_err(|e| format!("Failed to build current_thread runtime: {}", e))?;

    // Run the async event loop on the dedicated single thread
    rt.block_on(async move {
        // <-- Restored critical pattern
        let addr = listener_config.to_socket_addr()?;
        let listener = create_reuse_port_socket(addr)?;
        tracing::info!("Thread {} bound to http://{}", thread_id, addr);

        // Clone configs for poller
        let initial_datasources = datasources.clone();
        let initial_auth = auth_config.clone();
        let initial_access_log = access_log_config.clone();
        let db_for_poller = control_plane_db.clone();
        let port = listener_config.port;

        // Create application state
        tracing::info!("Thread {} creating AppState...", thread_id);
        let state = match AppState::new_with_crud(crate::app_state::AppStateConfig {
            routes: listener_config.routes.clone(),
            datasources,
            openapi_configs,
            listener_modules: listener_config.modules.clone(),
            auth_config,
            public_url: None,
            access_log_config,
            control_plane_db,
        })
        .await
        {
            Ok(s) => {
                tracing::info!("Thread {} AppState created successfully", thread_id);
                Arc::new(ArcSwap::from_pointee(s))
            }
            Err(e) => {
                tracing::error!("Thread {} failed to create AppState: {}", thread_id, e);
                return Err(format!("Thread {} AppState creation failed: {}", thread_id, e).into());
            }
        };

        // Spawn poller if DB is available
        if let Some(db) = db_for_poller {
            let state_swap = Arc::clone(&state);
            tokio::spawn(async move {
                use std::time::Duration;
                let poll_interval = std::env::var("APIFY_CONFIG_POLL_INTERVAL")
                    .ok()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(10);

                loop {
                    tokio::time::sleep(Duration::from_secs(poll_interval)).await;

                    // 1. Load Listeners
                    let listeners = match crate::control_plane::load_listeners(&db).await {
                        Ok(l) => l,
                        Err(e) => {
                            tracing::error!("Failed to reload listeners: {}", e);
                            continue;
                        }
                    };

                    // Find config for this port
                    let new_listener_config = match listeners
                        .unwrap_or_default()
                        .into_iter()
                        .find(|l| l.port == port)
                    {
                        Some(l) => l,
                        None => {
                            // Listener removed? We can't really shut down the thread easily from here without more logic.
                            // For now, just ignore or log warning.
                            // tracing::warn!("Listener for port {} not found in DB", port);
                            continue;
                        }
                    };

                    // 2. Load Datasources
                    let new_datasources = match crate::control_plane::load_datasources(&db).await {
                        Ok(d) => d,
                        Err(e) => {
                            tracing::error!("Failed to reload datasources: {}", e);
                            continue;
                        }
                    };
                    let mut final_datasources = initial_datasources.clone().unwrap_or_default();
                    final_datasources.extend(new_datasources.unwrap_or_default());

                    // 3. Load Auth
                    let new_auth = match crate::control_plane::load_auth_configs(&db).await {
                        Ok(a) => a,
                        Err(e) => {
                            tracing::error!("Failed to reload auth: {}", e);
                            continue;
                        }
                    };
                    let mut final_auth = initial_auth.clone().unwrap_or_default();
                    final_auth.extend(new_auth.unwrap_or_default());

                    // 4. Load API Configs
                    let api_configs_map = match crate::control_plane::load_api_configs(&db).await {
                        Ok(c) => c,
                        Err(e) => {
                            tracing::error!("Failed to reload api configs: {}", e);
                            continue;
                        }
                    };

                    // 5. Construct OpenApiStateConfig list based on api_configs_map and listener name
                    let mut new_openapi_configs = Vec::new();
                    for cfg in api_configs_map.values() {
                        if let Some(target_listeners) = &cfg.listeners
                            && let Some(lname) = &new_listener_config.name
                            && target_listeners.contains(lname)
                        {
                            new_openapi_configs.push(cfg.clone());
                        }
                    }

                    // 6. Create new AppState
                    let new_state = match AppState::new_with_crud(AppStateConfig {
                        routes: new_listener_config.routes,
                        datasources: Some(final_datasources),
                        openapi_configs: new_openapi_configs,
                        listener_modules: new_listener_config.modules,
                        auth_config: Some(final_auth),
                        public_url: None,
                        access_log_config: initial_access_log.clone(),
                        control_plane_db: Some(db.clone()),
                    })
                    .await
                    {
                        Ok(s) => s,
                        Err(e) => {
                            tracing::error!("Failed to create new AppState: {}", e);
                            continue;
                        }
                    };

                    // 7. Swap
                    state_swap.store(Arc::new(new_state));
                    // tracing::info!("Configuration reloaded for port {}", port);
                }
            });
        }

        tracing::info!("Thread {} entering accept loop", thread_id);
        // Continuously accept and handle connections
        loop {
            match listener.accept().await {
                Ok((stream, remote_addr)) => {
                    if let Err(e) = stream.set_nodelay(true) {
                        tracing::warn!("Thread {} set_nodelay error: {}", thread_id, e);
                        continue;
                    }
                    let io = TokioIo::new(stream);
                    let state_clone = Arc::clone(&state);
                    // Handle connection asynchronously
                    tokio::task::spawn(async move {
                        let service = service_fn(move |mut req| {
                            req.extensions_mut().insert(remote_addr);
                            handle_request(req, Arc::clone(&state_clone))
                        });
                        if let Err(err) = http1::Builder::new()
                            .keep_alive(true)
                            .serve_connection(io, service)
                            .await
                        {
                            tracing::error!(
                                "Thread {} connection handling error: {:?}",
                                thread_id,
                                err
                            );
                        }
                    });
                }
                Err(e) => {
                    tracing::error!("Thread {} accept error: {}", thread_id, e);
                    continue;
                }
            }
        }
        #[allow(unreachable_code)]
        {
            use std::time::Duration;
            loop {
                tokio::time::sleep(Duration::from_secs(3600)).await;
            }
        }
    })
}

/// Start documentation server (runs independently)
pub fn start_docs_server(
    port: u16,
    listener_config: ListenerConfig,
    datasources: Option<std::collections::HashMap<String, super::config::DatabaseSettings>>,
    openapi_configs: Vec<super::app_state::OpenApiStateConfig>,
    auth_config: Option<Vec<super::config::Authenticator>>,
    access_log_config: Option<super::config::AccessLogConfig>,
    control_plane_db: Option<super::database::DatabaseManager>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| format!("Failed to build docs runtime: {}", e))?;

    rt.block_on(async move {
        // Clone configs for poller
        let initial_datasources = datasources.clone();
        let initial_auth = auth_config.clone();
        let initial_openapi = openapi_configs.clone(); // Static configs
        let initial_access_log = access_log_config.clone();
        let db_for_poller = control_plane_db.clone();
        let listener_port = listener_config.port; // Port of the MAIN listener, not docs port

        // Create initial application state
        let state = match AppState::new_with_crud(AppStateConfig {
            routes: listener_config.routes.clone(),
            datasources: datasources.clone(),
            openapi_configs,
            listener_modules: listener_config.modules.clone(),
            auth_config,
            public_url: Some(format!("http://localhost:{}", listener_port)),
            access_log_config,
            control_plane_db,
        })
        .await
        {
            Ok(s) => Arc::new(ArcSwap::from_pointee(s)),
            Err(e) => return Err(format!("Docs server AppState creation failed: {}", e).into()),
        };

        // Spawn poller if DB is available (Logic copied from start_listener)
        if let Some(db) = db_for_poller {
            let state_swap = Arc::clone(&state);
            let mut current_listener_name = listener_config.name.clone();

            tokio::spawn(async move {
                use std::time::Duration;
                let poll_interval = std::env::var("APIFY_CONFIG_POLL_INTERVAL")
                    .ok()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(10);

                loop {
                    tokio::time::sleep(Duration::from_secs(poll_interval)).await;

                    // 1. Load Listeners
                    let listeners = match crate::control_plane::load_listeners(&db).await {
                        Ok(l) => l,
                        Err(e) => {
                            tracing::error!("Docs poller: Failed to reload listeners: {}", e);
                            continue;
                        }
                    };

                    let listeners_list = listeners.unwrap_or_default();

                    // Determine which listener config to use
                    // If we are currently using "default-docs" (dummy), try to switch to the first real listener
                    // Otherwise, try to find our current listener by name/port
                    let new_listener_config = if current_listener_name.as_deref() == Some("default-docs") {
                         if let Some(first) = listeners_list.first() {
                             tracing::info!("Docs server: Switching from default-docs to listener '{}'", first.name.as_deref().unwrap_or("unnamed"));
                             current_listener_name = first.name.clone();
                             first.clone()
                         } else {
                             // Stay on dummy
                             crate::config::ListenerConfig {
                                name: Some("default-docs".to_string()),
                                port: 0,
                                ip: "0.0.0.0".to_string(),
                                protocol: "http".to_string(),
                                routes: None,
                                modules: None,
                                consumers: None,
                            }
                         }
                    } else {
                        // Find by name if possible, else port
                        match listeners_list.iter().find(|l| l.name == current_listener_name) {
                            Some(l) => l.clone(),
                            None => {
                                // Fallback: find by port? Or if deleted, revert to dummy?
                                // Let's simplify: if current listener is gone, revert to dummy
                                tracing::warn!("Docs server: Current listener '{:?}' not found, reverting to default-docs", current_listener_name);
                                current_listener_name = Some("default-docs".to_string());
                                crate::config::ListenerConfig {
                                    name: Some("default-docs".to_string()),
                                    port: 0,
                                    ip: "0.0.0.0".to_string(),
                                    protocol: "http".to_string(),
                                    routes: None,
                                    modules: None,
                                    consumers: None,
                                }
                            }
                        }
                    };

                    let listener_port = new_listener_config.port;

                    // 2. Load Datasources
                    let new_datasources = match crate::control_plane::load_datasources(&db).await {
                        Ok(d) => d,
                        Err(e) => {
                            tracing::error!("Docs poller: Failed to reload datasources: {}", e);
                            continue;
                        }
                    };
                    let mut final_datasources = initial_datasources.clone().unwrap_or_default();
                    final_datasources.extend(new_datasources.unwrap_or_default());

                    // 3. Load Auth
                    let new_auth = match crate::control_plane::load_auth_configs(&db).await {
                        Ok(a) => a,
                        Err(e) => {
                            tracing::error!("Docs poller: Failed to reload auth: {}", e);
                            continue;
                        }
                    };
                    let mut final_auth = initial_auth.clone().unwrap_or_default();
                    final_auth.extend(new_auth.unwrap_or_default());

                    // 4. Load API Configs
                    let api_configs_map = match crate::control_plane::load_api_configs(&db).await {
                        Ok(c) => c,
                        Err(e) => {
                            tracing::error!("Docs poller: Failed to reload api configs: {}", e);
                            continue;
                        }
                    };

                    // 5. Construct OpenApiStateConfig list
                    // Start with static configs
                    let mut new_openapi_configs = initial_openapi.clone();
                    // Add dynamic configs that match this listener
                    for cfg in api_configs_map.values() {
                        if let Some(target_listeners) = &cfg.listeners
                            && let Some(lname) = &new_listener_config.name
                            && target_listeners.contains(lname)
                        {
                            new_openapi_configs.push(cfg.clone());
                        }
                    }

                    // 6. Create new AppState
                    let new_state = match AppState::new_with_crud(AppStateConfig {
                        routes: new_listener_config.routes,
                        datasources: Some(final_datasources),
                        openapi_configs: new_openapi_configs,
                        listener_modules: new_listener_config.modules,
                        auth_config: Some(final_auth),
                        public_url: Some(format!("http://localhost:{}", listener_port)),
                        access_log_config: initial_access_log.clone(),
                        control_plane_db: Some(db.clone()),
                    })
                    .await
                    {
                        Ok(s) => s,
                        Err(e) => {
                            tracing::error!("Docs poller: Failed to create new AppState: {}", e);
                            continue;
                        }
                    };

                    // 7. Swap
                    state_swap.store(Arc::new(new_state));
                }
            });
        }

        let addr: SocketAddr = format!("0.0.0.0:{}", port).parse()?;
        let listener = create_reuse_port_socket(addr)?;
        tracing::info!("Docs server listening on http://{}", addr);

        loop {
            match listener.accept().await {
                Ok((stream, _)) => {
                    let io = TokioIo::new(stream);
                    let state_clone = Arc::clone(&state);
                    tokio::task::spawn(async move {
                        let service = service_fn(move |req| {
                            crate::modules::openapi_docs::handle_docs_request(
                                req,
                                state_clone.load().clone(), // Access via ArcSwap load()
                            )
                        });
                        if let Err(err) = http1::Builder::new().serve_connection(io, service).await
                        {
                            tracing::error!("Docs connection error: {:?}", err);
                        }
                    });
                }
                Err(e) => {
                    tracing::error!("Docs accept error: {}", e);
                    continue;
                }
            }
        }
        #[allow(unreachable_code)]
        Ok(())
    })
}
