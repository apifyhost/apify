use super::models::DatasourceConfigRecord;
use crate::config::DatabaseSettings;
use crate::database::DatabaseManager;
use http_body_util::Full;
use hyper::body::Bytes;
use hyper::{Response, StatusCode};
use serde_json::Value;
use std::collections::HashMap;

pub async fn load_datasources(
    db: &DatabaseManager,
) -> Result<Option<HashMap<String, DatabaseSettings>>, Box<dyn std::error::Error + Send + Sync>> {
    let records = db
        .select("_meta_datasources", None, None, None, None)
        .await?;

    let mut datasources = HashMap::new();
    for record in records {
        let ds_record: DatasourceConfigRecord = serde_json::from_value(record)?;
        let ds_config: DatabaseSettings = serde_json::from_str(&ds_record.config)?;
        datasources.insert(ds_record.name, ds_config);
    }

    if datasources.is_empty() {
        Ok(None)
    } else {
        Ok(Some(datasources))
    }
}

pub async fn handle_datasources_request(
    req: hyper::Request<hyper::body::Incoming>,
    db: &DatabaseManager,
) -> Result<Response<Full<Bytes>>, Box<dyn std::error::Error + Send + Sync>> {
    let (parts, body) = req.into_parts();
    let method = parts.method;

    match method {
        hyper::Method::GET => {
            let records = db
                .select("_meta_datasources", None, None, None, None)
                .await?;
            let json = serde_json::to_string(&records)?;
            Ok(Response::builder()
                .status(StatusCode::OK)
                .header("Content-Type", "application/json")
                .body(Full::new(Bytes::from(json)))?)
        }
        hyper::Method::POST => {
            let body_bytes = http_body_util::BodyExt::collect(body).await?.to_bytes();
            let payload: Value = serde_json::from_slice(&body_bytes)?;

            let name = payload
                .get("name")
                .and_then(|v| v.as_str())
                .ok_or("Missing name")?;
            let config = payload.get("config").ok_or("Missing config")?;

            // Check if datasource with same name already exists
            let mut where_clause = HashMap::new();
            where_clause.insert("name".to_string(), Value::String(name.to_string()));

            let existing = db
                .select("_meta_datasources", None, Some(where_clause), None, None)
                .await?;

            if !existing.is_empty() {
                return Ok(Response::builder()
                    .status(StatusCode::CONFLICT)
                    .header("Content-Type", "application/json")
                    .body(Full::new(Bytes::from(
                        serde_json::json!({
                            "error": format!("Datasource with name '{}' already exists", name)
                        })
                        .to_string(),
                    )))?);
            }

            // Validate config
            let ds_config: DatabaseSettings = serde_json::from_value(config.clone())?;
            let config_str = serde_json::to_string(config)?;

            let id = uuid::Uuid::new_v4().to_string();
            let updated_at = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_secs() as i64;

            let mut data = HashMap::new();
            data.insert("id".to_string(), Value::String(id.clone()));
            data.insert("name".to_string(), Value::String(name.to_string()));
            data.insert("type".to_string(), Value::String(ds_config.driver));
            data.insert("config".to_string(), Value::String(config_str));
            data.insert(
                "updated_at".to_string(),
                Value::Number(serde_json::Number::from(updated_at)),
            );

            db.insert("_meta_datasources", data).await?;

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
