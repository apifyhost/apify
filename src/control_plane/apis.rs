use super::models::ApiConfigRecord;
use super::models::DatasourceConfigRecord;
use crate::app_state::OpenApiStateConfig;
use crate::config::DatabaseSettings;
use crate::config::{ModulesConfig, OpenAPIConfig};
use crate::database::DatabaseManager;
use crate::database::DatabaseRuntimeConfig;
use http_body_util::Full;
use hyper::body::Bytes;
use hyper::{Response, StatusCode};
use serde_json::Value;
use std::collections::HashMap;

pub async fn load_api_configs(
    db: &DatabaseManager,
) -> Result<HashMap<String, OpenApiStateConfig>, Box<dyn std::error::Error + Send + Sync>> {
    let records = db
        .select("_meta_api_configs", None, None, None, None)
        .await?;

    let mut configs = HashMap::new();
    for record in records {
        let api_record: ApiConfigRecord = serde_json::from_value(record)?;
        let spec: Value = serde_json::from_str(&api_record.spec)?;

        let modules = if let Some(m_str) = api_record.modules_config {
            if m_str.trim().is_empty() {
                None
            } else {
                Some(serde_json::from_str::<ModulesConfig>(&m_str)?)
            }
        } else {
            None
        };

        let datasource_name = api_record
            .datasource_name
            .filter(|ds| !ds.trim().is_empty());

        let listeners = if let Some(l_str) = api_record.listeners {
            if l_str.trim().is_empty() {
                None
            } else {
                Some(serde_json::from_str::<Vec<String>>(&l_str)?)
            }
        } else {
            None
        };

        configs.insert(
            api_record.name,
            OpenApiStateConfig {
                config: OpenAPIConfig {
                    openapi: crate::config::OpenAPISettings {
                        spec,
                        validation: None,
                    },
                },
                modules,
                datasource: datasource_name,
                access_log: None,
                listeners,
            },
        );
    }
    Ok(configs)
}

pub async fn handle_apis_request(
    req: hyper::Request<hyper::body::Incoming>,
    db: &DatabaseManager,
) -> Result<Response<Full<Bytes>>, Box<dyn std::error::Error + Send + Sync>> {
    let (parts, body) = req.into_parts();
    let method = parts.method;

    match method {
        hyper::Method::GET => {
            let records = db
                .select("_meta_api_configs", None, None, None, None)
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
            let version = payload
                .get("version")
                .and_then(|v| v.as_str())
                .ok_or("Missing version")?;

            let spec_content = if let Some(s) = payload.get("spec") {
                if s.is_string() {
                    s.as_str().unwrap().to_string()
                } else {
                    s.to_string()
                }
            } else if let Some(p) = payload.get("path").and_then(|v| v.as_str()) {
                tokio::fs::read_to_string(p)
                    .await
                    .map_err(|e| format!("Failed to read spec file: {}", e))?
            } else {
                return Err("Missing spec or path".into());
            };

            let datasource_name = payload.get("datasource_name").and_then(|v| v.as_str());
            let modules_config = payload.get("modules_config");

            // Parse spec to check for embedded listeners configuration
            let spec_value: Value = serde_yaml::from_str(&spec_content)
                .or_else(|_| serde_json::from_str(&spec_content))
                .unwrap_or(Value::Null);

            // Handle listeners association
            let listeners_from_payload = payload.get("listeners").and_then(|v| v.as_array());
            let listeners_from_spec = spec_value.get("listeners").and_then(|v| v.as_array());

            let mut target_listeners = Vec::new();
            if let Some(list) = listeners_from_payload {
                for l in list {
                    if let Some(s) = l.as_str() {
                        target_listeners.push(s.to_string());
                    }
                }
            }
            if let Some(list) = listeners_from_spec {
                for l in list {
                    if let Some(s) = l.as_str() {
                        target_listeners.push(s.to_string());
                    }
                }
            }
            // Deduplicate
            target_listeners.sort();
            target_listeners.dedup();

            let id = uuid::Uuid::new_v4().to_string();
            let created_at = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_secs() as i64;

            // Check if API with same name and version already exists
            let records = db
                .select("_meta_api_configs", None, None, None, None)
                .await?;

            for record in records {
                if let Ok(api_record) = serde_json::from_value::<ApiConfigRecord>(record) {
                    if api_record.name == name && api_record.version == version {
                        return Ok(Response::builder()
                            .status(StatusCode::CONFLICT)
                            .header("Content-Type", "application/json")
                            .body(Full::new(Bytes::from(
                                serde_json::json!({
                                    "error": format!("API with name '{}' and version '{}' already exists", name, version)
                                }).to_string(),
                            )))?);
                    }
                }
            }

            // Extract schemas from spec and initialize them in the DB
            let spec_value: serde_json::Value = if let Ok(v) = serde_json::from_str(&spec_content) {
                v
            } else {
                serde_yaml::from_str(&spec_content)
                    .map_err(|e| format!("Failed to parse spec as JSON or YAML: {}", e))?
            };

            let mut data = HashMap::new();
            data.insert("id".to_string(), Value::String(id.clone()));
            data.insert("name".to_string(), Value::String(name.to_string()));
            data.insert("version".to_string(), Value::String(version.to_string()));
            // Store normalized JSON spec
            data.insert(
                "spec".to_string(),
                Value::String(serde_json::to_string(&spec_value)?),
            );
            if let Some(ds) = datasource_name {
                data.insert("datasource_name".to_string(), Value::String(ds.to_string()));
            }
            if let Some(mc) = modules_config {
                data.insert("modules_config".to_string(), Value::String(mc.to_string()));
            }
            if !target_listeners.is_empty() {
                data.insert(
                    "listeners".to_string(),
                    Value::String(serde_json::to_string(&target_listeners)?),
                );
            }
            data.insert(
                "created_at".to_string(),
                Value::Number(serde_json::Number::from(created_at)),
            );

            db.insert("_meta_api_configs", data.clone()).await?;

            let schemas = crate::schema_generator::SchemaGenerator::extract_schemas_from_openapi(
                &spec_value,
            )?;

            if let Some(ds_name) = datasource_name {
                let mut where_clause = HashMap::new();
                where_clause.insert("name".to_string(), Value::String(ds_name.to_string()));
                let records = db
                    .select("_meta_datasources", None, Some(where_clause), None, None)
                    .await?;

                if let Some(record) = records.first() {
                    let ds_record: DatasourceConfigRecord = serde_json::from_value(record.clone())?;
                    let ds_settings: DatabaseSettings = serde_json::from_str(&ds_record.config)?;

                    let url = if ds_settings.driver == "postgres" {
                        format!(
                            "postgres://{}:{}@{}:{}/{}",
                            ds_settings.user.unwrap_or_default(),
                            ds_settings.password.unwrap_or_default(),
                            ds_settings.host.unwrap_or("localhost".to_string()),
                            ds_settings.port.unwrap_or(5432),
                            ds_settings.database
                        )
                    } else {
                        format!("sqlite:{}", ds_settings.database)
                    };

                    let config = DatabaseRuntimeConfig {
                        driver: ds_settings.driver,
                        url,
                        max_size: ds_settings.max_pool_size.unwrap_or(10) as u32,
                    };

                    let target_db = DatabaseManager::new(config).await?;
                    target_db.initialize_schema(schemas).await?;
                } else {
                    tracing::warn!(
                        "Datasource '{}' not found, skipping schema initialization",
                        ds_name
                    );
                }
            } else {
                db.initialize_schema(schemas).await?;
            }

            Ok(Response::builder()
                .status(StatusCode::OK)
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
