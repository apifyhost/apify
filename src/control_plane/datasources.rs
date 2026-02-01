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
    let path = parts.uri.path();

    // Parse ID from path if present (e.g., /apify/admin/datasources/{id})
    let id = if path.starts_with("/apify/admin/datasources/") {
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
                if let Some(obj) = record.as_object_mut() {
                    if let Some(config_str) = obj
                        .remove("config")
                        .and_then(|v| v.as_str().map(|s| s.to_string()))
                        && let Ok(config_json) = serde_json::from_str::<Value>(&config_str)
                        && let Some(config_obj) = config_json.as_object()
                    {
                        for (k, v) in config_obj {
                            if !obj.contains_key(k) {
                                obj.insert(k.clone(), v.clone());
                            }
                            if k == "user" {
                                obj.insert("username".to_string(), v.clone());
                            }
                        }
                    }

                    if !obj.contains_key("db_type") {
                        let val = obj
                            .get("type")
                            .or_else(|| obj.get("driver"))
                            .cloned()
                            .unwrap_or(Value::Null);
                        obj.insert("db_type".to_string(), val);
                    }
                }
                record
            };

            if let Some(id) = id {
                // Get specific datasource by ID
                let mut where_clause = HashMap::new();
                where_clause.insert("id".to_string(), Value::String(id.clone()));

                let records = db
                    .select("_meta_datasources", None, Some(where_clause), None, None)
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
                // List all datasources
                let records = db
                    .select("_meta_datasources", None, None, None, None)
                    .await?;

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
                // Update specific datasource by ID
                let body_bytes = http_body_util::BodyExt::collect(body).await?.to_bytes();
                let payload: Value = serde_json::from_slice(&body_bytes)?;

                let name = payload
                    .get("name")
                    .and_then(|v| v.as_str())
                    .ok_or("Missing name")?;

                // Handle flat structure from frontend
                let config = if let Some(c) = payload.get("config") {
                    c.clone()
                } else {
                    let driver = payload.get("db_type").or(payload.get("driver"));
                    let driver = driver.and_then(|v| v.as_str()).ok_or("Missing db_type")?;
                    let database = payload
                        .get("database")
                        .and_then(|v| v.as_str())
                        .ok_or("Missing database")?;

                    let mut obj = serde_json::Map::new();
                    obj.insert("driver".to_string(), Value::String(driver.to_string()));
                    obj.insert("database".to_string(), Value::String(database.to_string()));

                    if let Some(host) = payload.get("host") {
                        obj.insert("host".to_string(), host.clone());
                    }
                    if let Some(port) = payload.get("port") {
                        obj.insert("port".to_string(), port.clone());
                    }
                    if let Some(user) = payload.get("username").or(payload.get("user")) {
                        obj.insert("user".to_string(), user.clone());
                    }
                    if let Some(password) = payload.get("password") {
                        obj.insert("password".to_string(), password.clone());
                    }
                    if let Some(ssl_mode) = payload.get("ssl_mode") {
                        obj.insert("ssl_mode".to_string(), ssl_mode.clone());
                    }
                    if let Some(max_pool_size) = payload.get("max_pool_size") {
                        obj.insert("max_pool_size".to_string(), max_pool_size.clone());
                    }

                    Value::Object(obj)
                };

                // Check if datasource exists
                let mut where_clause = HashMap::new();
                where_clause.insert("id".to_string(), Value::String(id.clone()));

                let existing = db
                    .select(
                        "_meta_datasources",
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

                // Check if another datasource with the same name exists (excluding current)
                let mut name_where = HashMap::new();
                name_where.insert("name".to_string(), Value::String(name.to_string()));

                let name_records = db
                    .select("_meta_datasources", None, Some(name_where), None, None)
                    .await?;

                for record in name_records {
                    if let Some(record_id) = record.get("id").and_then(|v| v.as_str())
                        && record_id != id
                    {
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
                }

                // Validate config
                let ds_config: DatabaseSettings = serde_json::from_value(config.clone())?;
                let config_str = serde_json::to_string(&config)?;

                let updated_at = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)?
                    .as_secs() as i64;

                let mut data = HashMap::new();
                data.insert("name".to_string(), Value::String(name.to_string()));
                data.insert("type".to_string(), Value::String(ds_config.driver));
                data.insert("config".to_string(), Value::String(config_str));
                data.insert(
                    "updated_at".to_string(),
                    Value::Number(serde_json::Number::from(updated_at)),
                );

                db.update("_meta_datasources", data, where_clause).await?;

                Ok(Response::builder()
                    .status(StatusCode::OK)
                    .header("Content-Type", "application/json")
                    .body(Full::new(Bytes::from(
                        serde_json::json!({"id": id}).to_string(),
                    )))?)
            } else {
                Ok(Response::builder()
                    .status(StatusCode::BAD_REQUEST)
                    .body(Full::new(Bytes::from("Missing datasource ID")))?)
            }
        }
        hyper::Method::DELETE => {
            if let Some(id) = id {
                // Delete specific datasource by ID
                let mut where_clause = HashMap::new();
                where_clause.insert("id".to_string(), Value::String(id.clone()));

                let existing = db
                    .select(
                        "_meta_datasources",
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

                db.delete("_meta_datasources", where_clause).await?;

                Ok(Response::builder()
                    .status(StatusCode::NO_CONTENT)
                    .body(Full::new(Bytes::from("")))?)
            } else {
                Ok(Response::builder()
                    .status(StatusCode::BAD_REQUEST)
                    .body(Full::new(Bytes::from("Missing datasource ID")))?)
            }
        }
        hyper::Method::POST => {
            let body_bytes = http_body_util::BodyExt::collect(body).await?.to_bytes();
            let payload: Value = serde_json::from_slice(&body_bytes)?;

            let name = payload
                .get("name")
                .and_then(|v| v.as_str())
                .ok_or("Missing name")?;

            // Handle flat structure from frontend
            let config = if let Some(c) = payload.get("config") {
                c.clone()
            } else {
                let driver = payload.get("db_type").or(payload.get("driver"));
                let driver = driver.and_then(|v| v.as_str()).ok_or("Missing db_type")?;
                let database = payload
                    .get("database")
                    .and_then(|v| v.as_str())
                    .ok_or("Missing database")?;

                let mut obj = serde_json::Map::new();
                obj.insert("driver".to_string(), Value::String(driver.to_string()));
                obj.insert("database".to_string(), Value::String(database.to_string()));

                if let Some(host) = payload.get("host") {
                    obj.insert("host".to_string(), host.clone());
                }
                if let Some(port) = payload.get("port") {
                    obj.insert("port".to_string(), port.clone());
                }
                if let Some(user) = payload.get("username").or(payload.get("user")) {
                    obj.insert("user".to_string(), user.clone());
                }
                if let Some(password) = payload.get("password") {
                    obj.insert("password".to_string(), password.clone());
                }
                if let Some(ssl_mode) = payload.get("ssl_mode") {
                    obj.insert("ssl_mode".to_string(), ssl_mode.clone());
                }
                if let Some(max_pool_size) = payload.get("max_pool_size") {
                    obj.insert("max_pool_size".to_string(), max_pool_size.clone());
                }

                Value::Object(obj)
            };

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
            let config_str = serde_json::to_string(&config)?;

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
