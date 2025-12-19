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
