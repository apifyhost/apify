//! Network service related (listener creation, service startup)

use super::app_state::AppState;
use super::config::ListenerConfig;
use super::handler::handle_request;
use super::hyper::server::conn::http1;
use super::hyper::service::service_fn;
use super::tokio::net::TcpListener;
use super::{Arc, hyper_util::rt::TokioIo, tokio};
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
    database_config: Option<super::config::DatabaseConfig>,
    openapi_configs: Vec<super::config::OpenAPIConfig>,
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
        println!("Thread {} bound to http://{}", thread_id, addr);

        // Create application state
        println!("Thread {} creating AppState...", thread_id);
        let state = match AppState::new_with_crud(
            listener_config.routes,
            database_config,
            openapi_configs,
        ).await {
            Ok(s) => {
                println!("Thread {} AppState created successfully", thread_id);
                Arc::new(s)
            }
            Err(e) => {
                eprintln!("Thread {} failed to create AppState: {}", thread_id, e);
                return Err(format!("Thread {} AppState creation failed: {}", thread_id, e).into());
            }
        };

        println!("Thread {} entering accept loop", thread_id);
        // Continuously accept and handle connections
        loop {
            match listener.accept().await {
                Ok((stream, _)) => {
                    if let Err(e) = stream.set_nodelay(true) {
                        eprintln!("Thread {} set_nodelay error: {}", thread_id, e);
                        continue;
                    }
                    let io = TokioIo::new(stream);
                    let state_clone = Arc::clone(&state);
                    // Handle connection asynchronously
                    tokio::task::spawn(async move {
                        let service = service_fn(move |req| handle_request(req, Arc::clone(&state_clone)));
                        if let Err(err) = http1::Builder::new()
                            .keep_alive(true)
                            .serve_connection(io, service)
                            .await
                        {
                            eprintln!("Thread {} connection handling error: {:?}", thread_id, err);
                        }
                    });
                }
                Err(e) => {
                    eprintln!("Thread {} accept error: {}", thread_id, e);
                    continue;
                }
            }
        }
        #[allow(unreachable_code)]
        {
            use std::time::Duration;
            loop { tokio::time::sleep(Duration::from_secs(3600)).await; }
        }
    })
}
