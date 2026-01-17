use crate::database::DatabaseManager;
use http_body_util::Full;
use hyper::body::Bytes;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper_util::rt::TokioIo;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;

use super::apis::handle_apis_request;
use super::auth::handle_auth_request;
use super::datasources::handle_datasources_request;
use super::import::handle_import_request;
use super::listeners::handle_listeners_request;

pub async fn handle_control_plane_request(
    req: hyper::Request<hyper::body::Incoming>,
    db: &DatabaseManager,
    config: &crate::config::ControlPlaneConfig,
) -> Result<hyper::Response<Full<Bytes>>, Box<dyn std::error::Error + Send + Sync>> {
    let path = req.uri().path().to_string();
    tracing::info!("Control Plane Request: {} {}", req.method(), path);

    // Authentication Check
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

    if path == "/_meta/apis" {
        let res = handle_apis_request(req, db).await;
        if let Ok(ref r) = res {
            tracing::info!("API Request handled, status: {}", r.status());
        }
        res
    } else if path == "/_meta/listeners" {
        handle_listeners_request(req, db).await
    } else if path == "/_meta/datasources" {
        handle_datasources_request(req, db).await
    } else if path == "/_meta/auth" {
        handle_auth_request(req, db).await
    } else if path == "/_meta/import" {
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
