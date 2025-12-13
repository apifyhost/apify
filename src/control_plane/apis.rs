use super::models::ApiConfigRecord;
use crate::app_state::OpenApiStateConfig;
use crate::config::{ModulesConfig, OpenAPIConfig};
use crate::database::DatabaseManager;
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
            let spec = payload.get("spec").ok_or("Missing spec")?;
            let datasource_name = payload.get("datasource_name").and_then(|v| v.as_str());
            let modules_config = payload.get("modules_config");

            let id = uuid::Uuid::new_v4().to_string();
            let created_at = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_secs() as i64;

            let mut data = HashMap::new();
            data.insert("id".to_string(), Value::String(id.clone()));
            data.insert("name".to_string(), Value::String(name.to_string()));
            data.insert("version".to_string(), Value::String(version.to_string()));
            data.insert("spec".to_string(), Value::String(spec.to_string()));
            if let Some(ds) = datasource_name {
                data.insert("datasource_name".to_string(), Value::String(ds.to_string()));
            }
            if let Some(mc) = modules_config {
                data.insert("modules_config".to_string(), Value::String(mc.to_string()));
            }
            data.insert(
                "created_at".to_string(),
                Value::Number(serde_json::Number::from(created_at)),
            );

            db.insert("_meta_api_configs", data).await?;

            // Extract schemas from spec and initialize them in the DB
            let schemas =
                crate::schema_generator::SchemaGenerator::extract_schemas_from_openapi(spec)?;
            db.initialize_schema(schemas).await?;

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
