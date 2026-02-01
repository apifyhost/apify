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
    let path = parts.uri.path();

    // Parse ID from path if present (e.g., /apify/admin/listeners/{id})
    let id = if path.starts_with("/apify/admin/listeners/") {
        let segments: Vec<&str> = path.split('/').collect();
        if segments.len() > 4 {
            Some(segments[4].to_string())
        } else {
            None
        }
    } else {
        None
    };

    match method {
        hyper::Method::GET => {
            let transform_record = |mut record: Value| -> Value {
                if let Some(obj) = record.as_object_mut()
                    && let Some(config_str) = obj
                        .remove("config")
                        .and_then(|v| v.as_str().map(|s| s.to_string()))
                    && let Ok(config_json) = serde_json::from_str::<Value>(&config_str)
                    && let Some(config_obj) = config_json.as_object()
                {
                    for (k, v) in config_obj {
                        if !obj.contains_key(k) {
                            obj.insert(k.clone(), v.clone());
                        }
                    }
                }
                record
            };

            if let Some(id) = id {
                // Get specific listener by ID
                let mut where_clause = HashMap::new();
                where_clause.insert("id".to_string(), Value::String(id.clone()));

                let records = db
                    .select("_meta_listeners", None, Some(where_clause), None, None)
                    .await?;

                if records.is_empty() {
                    Ok(Response::builder()
                        .status(StatusCode::NOT_FOUND)
                        .body(Full::new(Bytes::from("Not Found")))?)
                } else {
                    let record = transform_record(records[0].clone());
                    let json = serde_json::to_string(&record)?;
                    Ok(Response::builder()
                        .status(StatusCode::OK)
                        .header("Content-Type", "application/json")
                        .body(Full::new(Bytes::from(json)))?)
                }
            } else {
                // List all listeners
                let records = db.select("_meta_listeners", None, None, None, None).await?;
                let transformed_records: Vec<Value> =
                    records.into_iter().map(transform_record).collect();
                let json = serde_json::to_string(&transformed_records)?;
                Ok(Response::builder()
                    .status(StatusCode::OK)
                    .header("Content-Type", "application/json")
                    .body(Full::new(Bytes::from(json)))?)
            }
        }
        hyper::Method::PUT => {
            if let Some(id) = id {
                // Update specific listener by ID
                let body_bytes = http_body_util::BodyExt::collect(body).await?.to_bytes();
                let config: crate::config::ListenerConfig =
                    match serde_json::from_slice(&body_bytes) {
                        Ok(c) => c,
                        Err(e) => {
                            tracing::error!("Failed to deserialize ListenerConfig: {}", e);
                            return Ok(Response::builder()
                                .status(StatusCode::BAD_REQUEST)
                                .body(Full::new(Bytes::from(format!("Invalid config: {}", e))))?);
                        }
                    };

                // Check if listener exists
                let mut where_clause = HashMap::new();
                where_clause.insert("id".to_string(), Value::String(id.clone()));

                let existing = db
                    .select(
                        "_meta_listeners",
                        None,
                        Some(where_clause.clone()),
                        None,
                        None,
                    )
                    .await?;

                if existing.is_empty() {
                    return Ok(Response::builder()
                        .status(StatusCode::NOT_FOUND)
                        .body(Full::new(Bytes::from("Not Found")))?);
                }

                // Check for port conflicts with other listeners
                let mut port_where = HashMap::new();
                port_where.insert(
                    "port".to_string(),
                    Value::Number(serde_json::Number::from(config.port)),
                );

                let port_records = db
                    .select("_meta_listeners", None, Some(port_where), None, None)
                    .await?;

                for record in port_records {
                    if let Some(record_id) = record.get("id").and_then(|v| v.as_str())
                        && record_id != id
                        && let Some(config_str) = record.get("config").and_then(|v| v.as_str())
                        && let Ok(existing_config) =
                            serde_json::from_str::<crate::config::ListenerConfig>(config_str)
                    {
                        let conflict = config.ip == "0.0.0.0"
                            || existing_config.ip == "0.0.0.0"
                            || existing_config.ip == config.ip;

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

                let config_str = serde_json::to_string(&config)?;
                let updated_at = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)?
                    .as_secs() as i64;

                let mut data = HashMap::new();
                data.insert("config".to_string(), Value::String(config_str));
                data.insert(
                    "port".to_string(),
                    Value::Number(serde_json::Number::from(config.port)),
                );
                data.insert(
                    "updated_at".to_string(),
                    Value::Number(serde_json::Number::from(updated_at)),
                );

                db.update("_meta_listeners", data, where_clause)
                    .await
                    .map_err(|e| {
                        tracing::error!("Failed to update listener: {:?}", e);
                        e
                    })?;

                Ok(Response::builder()
                    .status(StatusCode::OK)
                    .header("Content-Type", "application/json")
                    .body(Full::new(Bytes::from(
                        serde_json::json!({"id": id}).to_string(),
                    )))?)
            } else {
                Ok(Response::builder()
                    .status(StatusCode::BAD_REQUEST)
                    .body(Full::new(Bytes::from("Missing listener ID")))?)
            }
        }
        hyper::Method::DELETE => {
            if let Some(id) = id {
                // Delete specific listener by ID
                let mut where_clause = HashMap::new();
                where_clause.insert("id".to_string(), Value::String(id.clone()));

                let existing = db
                    .select(
                        "_meta_listeners",
                        None,
                        Some(where_clause.clone()),
                        None,
                        None,
                    )
                    .await?;

                if existing.is_empty() {
                    return Ok(Response::builder()
                        .status(StatusCode::NOT_FOUND)
                        .body(Full::new(Bytes::from("Not Found")))?);
                }

                db.delete("_meta_listeners", where_clause).await?;

                Ok(Response::builder()
                    .status(StatusCode::NO_CONTENT)
                    .body(Full::new(Bytes::from("")))?)
            } else {
                Ok(Response::builder()
                    .status(StatusCode::BAD_REQUEST)
                    .body(Full::new(Bytes::from("Missing listener ID")))?)
            }
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
                    if let Some(config_str) = record.get("config").and_then(|v| v.as_str())
                        && let Ok(existing_config) =
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
