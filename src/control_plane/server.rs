use crate::database::DatabaseManager;
use http_body_util::Full;
use hyper::body::Bytes;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper_util::rt::TokioIo;
use std::net::SocketAddr;
use std::sync::Arc;
use std::path::PathBuf;
use tokio::net::TcpListener;

use super::apis::handle_apis_request;
use super::auth::handle_auth_request;
use super::datasources::handle_datasources_request;
use super::import::handle_import_request;
use super::listeners::handle_listeners_request;

/// Serve static files from admin dashboard
async fn serve_static_file(
    path: &str,
) -> Result<hyper::Response<Full<Bytes>>, Box<dyn std::error::Error + Send + Sync>> {
    // Remove /admin/ prefix and get file path
    let file_path = path.strip_prefix("/admin/").unwrap_or("index.html");
    let file_path = if file_path.is_empty() || file_path == "/" {
        "index.html"
    } else {
        file_path
    };

    // Build full path to static files
    let static_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/admin");
    let full_path = static_dir.join(file_path);

    // Security check: prevent directory traversal
    if !full_path.starts_with(&static_dir) {
        return Ok(hyper::Response::builder()
            .status(hyper::StatusCode::FORBIDDEN)
            .body(Full::new(Bytes::from("Forbidden")))?);
    }

    // Read file
    match tokio::fs::read(&full_path).await {
        Ok(content) => {
            // Determine content type
            let content_type = mime_guess::from_path(&full_path)
                .first_or_octet_stream()
                .to_string();

            Ok(hyper::Response::builder()
                .status(hyper::StatusCode::OK)
                .header("Content-Type", content_type)
                .header("Cache-Control", "public, max-age=3600")
                .body(Full::new(Bytes::from(content)))?)
        }
        Err(_) => {
            // If file not found and it's not an API route, serve index.html for SPA routing
            if !path.starts_with("/apify/") {
                let index_path = static_dir.join("index.html");
                if let Ok(content) = tokio::fs::read(&index_path).await {
                    return Ok(hyper::Response::builder()
                        .status(hyper::StatusCode::OK)
                        .header("Content-Type", "text/html; charset=utf-8")
                        .body(Full::new(Bytes::from(content)))?);
                }
            }

            Ok(hyper::Response::builder()
                .status(hyper::StatusCode::NOT_FOUND)
                .body(Full::new(Bytes::from("Not Found")))?)
        }
    }
}

pub async fn handle_control_plane_request(
    req: hyper::Request<hyper::body::Incoming>,
    db: &DatabaseManager,
    config: &crate::config::ControlPlaneConfig,
) -> Result<hyper::Response<Full<Bytes>>, Box<dyn std::error::Error + Send + Sync>> {
    let path = req.uri().path().to_string();
    tracing::info!("Control Plane Request: {} {}", req.method(), path);

    // Serve admin dashboard static files
    if path.starts_with("/admin") || path == "/" {
        return serve_static_file(&path).await;
    }

    // Authentication Check for API endpoints only
    if path.starts_with("/apify/admin/") {
        if let Some(admin_key) = &config.admin_key {
            let authorized = if let Some(api_key_header) = req.headers().get("X-API-KEY") {
                if let Ok(api_key) = api_key_header.to_str() {
                    api_key == admin_key
                } else {
                    false
                }
            } else {
                false
            };

            if !authorized {
                tracing::warn!("Unauthorized access attempt to Control Plane");
                return Ok(hyper::Response::builder()
                    .status(hyper::StatusCode::UNAUTHORIZED)
                    .body(Full::new(Bytes::from("Unauthorized")))?);
            }
        }
    }

    if path == "/apify/admin/apis" {
        let res = handle_apis_request(req, db).await;
        if let Ok(ref r) = res {
            tracing::info!("API Request handled, status: {}", r.status());
        }
        res
    } else if path == "/apify/admin/listeners" {
        handle_listeners_request(req, db).await
    } else if path.starts_with("/admin") {
        // Fallback for admin routes (SPA routing)
        serve_static_file(&path).await
    } else if path == "/apify/admin/datasources" {
        handle_datasources_request(req, db).await
    } else if path == "/apify/admin/auth" {
        handle_auth_request(req, db).await
    } else if path == "/apify/admin/import" {
        handle_import_request(req, db).await
    } else {
        Ok(hyper::Response::builder()
            .status(hyper::StatusCode::NOT_FOUND)
            .body(Full::new(Bytes::from("Not Found")))?)
    }
}

pub async fn start_control_plane_server(
    config: crate::config::ControlPlaneConfig,
    db: DatabaseManager,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let addr: SocketAddr = format!("{}:{}", config.listen.ip, config.listen.port).parse()?;
    let listener = TcpListener::bind(addr).await?;
    tracing::info!("Control Plane listening on {}", addr);

    let db = Arc::new(db);
    let config = Arc::new(config);

    loop {
        let (stream, _) = listener.accept().await?;
        let io = TokioIo::new(stream);
        let db_clone = db.clone();
        let config_clone = config.clone();

        tokio::task::spawn(async move {
            if let Err(err) = http1::Builder::new()
                .serve_connection(
                    io,
                    service_fn(move |req| {
                        let db = db_clone.clone();
                        let config = config_clone.clone();
                        async move {
                            match handle_control_plane_request(req, &db, &config).await {
                                Ok(res) => Ok::<_, hyper::Error>(res),
                                Err(e) => {
                                    tracing::error!("Internal server error: {}", e);
                                    let res = hyper::Response::builder()
                                        .status(hyper::StatusCode::INTERNAL_SERVER_ERROR)
                                        .body(Full::new(Bytes::from(format!(
                                            "Internal Server Error: {}",
                                            e
                                        ))))
                                        .unwrap();
                                    Ok(res)
                                }
                            }
                        }
                    }),
                )
                .await
            {
                tracing::error!("Error serving connection: {:?}", err);
            }
        });
    }
}
