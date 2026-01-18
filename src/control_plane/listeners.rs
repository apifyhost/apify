use crate::database::DatabaseManager;
use http_body_util::Full;
use hyper::body::Bytes;
use hyper::{Response, StatusCode};
use serde_json::Value;
use std::collections::HashMap;

pub async fn load_listeners(
    db: &DatabaseManager,
) -> Result<Option<Vec<crate::config::ListenerConfig>>, Box<dyn std::error::Error + Send + Sync>> {
    let records = db.select("_meta_listeners", None, None, None, None).await?;

    if records.is_empty() {
        return Ok(None);
    }

    let mut listeners = Vec::new();
    for record in records {
        if let Some(config_val) = record.get("config")
            && let Some(config_str) = config_val.as_str()
        {
            let listener: crate::config::ListenerConfig = serde_json::from_str(config_str)?;
            listeners.push(listener);
        }
    }

    Ok(Some(listeners))
}

pub async fn handle_listeners_request(
    req: hyper::Request<hyper::body::Incoming>,
    db: &DatabaseManager,
) -> Result<Response<Full<Bytes>>, Box<dyn std::error::Error + Send + Sync>> {
    let (parts, body) = req.into_parts();
    let method = parts.method;

    match method {
        hyper::Method::GET => {
            let records = db.select("_meta_listeners", None, None, None, None).await?;
            let json = serde_json::to_string(&records)?;
            Ok(Response::builder()
                .status(StatusCode::OK)
                .header("Content-Type", "application/json")
                .body(Full::new(Bytes::from(json)))?)
        }
        hyper::Method::POST => {
            let body_bytes = http_body_util::BodyExt::collect(body).await?.to_bytes();
            // Validate that it parses as ListenerConfig
            let config: crate::config::ListenerConfig = serde_json::from_slice(&body_bytes)?;

            // Check if a listener with conflicting IP and port already exists
            // Rules:
            // 1. If new listener is 0.0.0.0:port, check if ANY IP is using this port
            // 2. If new listener is specific IP:port, check if 0.0.0.0:port OR same IP:port exists
            let mut where_clause = HashMap::new();
            where_clause.insert(
                "port".to_string(),
                Value::Number(serde_json::Number::from(config.port)),
            );

            let existing = db
                .select("_meta_listeners", None, Some(where_clause), None, None)
                .await?;

            if !existing.is_empty() {
                for record in existing {
                    if let Some(config_str) = record.get("config").and_then(|v| v.as_str()) {
                        if let Ok(existing_config) =
                            serde_json::from_str::<crate::config::ListenerConfig>(config_str)
                        {
                            // Check for conflicts
                            let conflict = if config.ip == "0.0.0.0" {
                                // New listener wants to bind to all interfaces on this port
                                // This conflicts with ANY existing listener on this port
                                true
                            } else if existing_config.ip == "0.0.0.0" {
                                // Existing listener is bound to all interfaces
                                // This conflicts with any specific IP on this port
                                true
                            } else {
                                // Both are specific IPs, only conflict if IPs match
                                existing_config.ip == config.ip
                            };

                            if conflict {
                                return Ok(Response::builder()
                                    .status(StatusCode::CONFLICT)
                                    .header("Content-Type", "application/json")
                                    .body(Full::new(Bytes::from(
                                        serde_json::json!({
                                            "error": format!(
                                                "Listener port {} conflicts with existing listener {}:{}",
                                                config.port, existing_config.ip, existing_config.port
                                            )
                                        }).to_string(),
                                    )))?);
                            }
                        }
                    }
                }
            }

            let config_str = serde_json::to_string(&config)?;

            let id = uuid::Uuid::new_v4().to_string();
            let created_at = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_secs() as i64;

            let mut data = HashMap::new();
            data.insert("id".to_string(), Value::String(id.clone()));
            data.insert("config".to_string(), Value::String(config_str));
            data.insert(
                "port".to_string(),
                Value::Number(serde_json::Number::from(config.port)),
            );
            data.insert(
                "created_at".to_string(),
                Value::Number(serde_json::Number::from(created_at)),
            );
            data.insert(
                "updated_at".to_string(),
                Value::Number(serde_json::Number::from(created_at)),
            );

            db.insert("_meta_listeners", data).await?;

            Ok(Response::builder()
                .status(StatusCode::CREATED)
                .header("Content-Type", "application/json")
                .body(Full::new(Bytes::from(
                    serde_json::json!({"id": id}).to_string(),
                )))?)
        }
        _ => Ok(Response::builder()
            .status(StatusCode::METHOD_NOT_ALLOWED)
            .body(Full::new(Bytes::from("Method Not Allowed")))?),
    }
}
