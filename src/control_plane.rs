use crate::app_state::OpenApiStateConfig;
use crate::config::{Authenticator, OpenAPIConfig};
use crate::database::DatabaseManager;
use crate::schema_generator::{ColumnDefinition, TableSchema};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

pub fn get_metadata_schemas() -> Vec<TableSchema> {
    vec![
        TableSchema {
            table_name: "_meta_api_configs".to_string(),
            columns: vec![
                ColumnDefinition {
                    name: "id".to_string(),
                    column_type: "TEXT".to_string(), // UUID
                    nullable: false,
                    primary_key: true,
                    unique: true,
                    auto_increment: false,
                    default_value: None,
                    auto_field: false,
                },
                ColumnDefinition {
                    name: "name".to_string(),
                    column_type: "TEXT".to_string(),
                    nullable: false,
                    primary_key: false,
                    unique: true,
                    auto_increment: false,
                    default_value: None,
                    auto_field: false,
                },
                ColumnDefinition {
                    name: "version".to_string(),
                    column_type: "TEXT".to_string(),
                    nullable: false,
                    primary_key: false,
                    unique: false,
                    auto_increment: false,
                    default_value: None,
                    auto_field: false,
                },
                ColumnDefinition {
                    name: "spec".to_string(),
                    column_type: "TEXT".to_string(), // JSON string
                    nullable: false,
                    primary_key: false,
                    unique: false,
                    auto_increment: false,
                    default_value: None,
                    auto_field: false,
                },
                ColumnDefinition {
                    name: "created_at".to_string(),
                    column_type: "INTEGER".to_string(), // Timestamp
                    nullable: false,
                    primary_key: false,
                    unique: false,
                    auto_increment: false,
                    default_value: None,
                    auto_field: false,
                },
            ],
            indexes: vec![],
            relations: vec![],
        },
        TableSchema {
            table_name: "_meta_auth_configs".to_string(),
            columns: vec![
                ColumnDefinition {
                    name: "id".to_string(),
                    column_type: "TEXT".to_string(),
                    nullable: false,
                    primary_key: true,
                    unique: true,
                    auto_increment: false,
                    default_value: None,
                    auto_field: false,
                },
                ColumnDefinition {
                    name: "config".to_string(),
                    column_type: "TEXT".to_string(), // JSON string
                    nullable: false,
                    primary_key: false,
                    unique: false,
                    auto_increment: false,
                    default_value: None,
                    auto_field: false,
                },
                ColumnDefinition {
                    name: "updated_at".to_string(),
                    column_type: "INTEGER".to_string(),
                    nullable: false,
                    primary_key: false,
                    unique: false,
                    auto_increment: false,
                    default_value: None,
                    auto_field: false,
                },
            ],
            indexes: vec![],
            relations: vec![],
        },
    ]
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ApiConfigRecord {
    pub id: String,
    pub name: String,
    pub version: String,
    pub spec: String,
    pub created_at: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AuthConfigRecord {
    pub id: String,
    pub config: String,
    pub updated_at: i64,
}

pub async fn load_api_configs(
    db: &DatabaseManager,
) -> Result<Vec<OpenApiStateConfig>, Box<dyn std::error::Error + Send + Sync>> {
    let records = db
        .select("_meta_api_configs", None, None, None, None)
        .await?;

    let mut configs = Vec::new();
    for record in records {
        let api_record: ApiConfigRecord = serde_json::from_value(record)?;
        let spec: Value = serde_json::from_str(&api_record.spec)?;

        configs.push(OpenApiStateConfig {
            config: OpenAPIConfig {
                openapi: crate::config::OpenAPISettings {
                    spec,
                    validation: None,
                },
            },
            modules: None, // TODO: Store modules config in DB
            datasource: None,
            access_log: None,
        });
    }
    Ok(configs)
}

pub async fn load_auth_configs(
    db: &DatabaseManager,
) -> Result<Option<Vec<Authenticator>>, Box<dyn std::error::Error + Send + Sync>> {
    let records = db
        .select("_meta_auth_configs", None, None, None, None)
        .await?;

    let mut authenticators = Vec::new();
    for record in records {
        let auth_record: AuthConfigRecord = serde_json::from_value(record)?;
        let auth_config: Authenticator = serde_json::from_str(&auth_record.config)?;
        authenticators.push(auth_config);
    }

    if authenticators.is_empty() {
        Ok(None)
    } else {
        Ok(Some(authenticators))
    }
}

pub async fn handle_control_plane_request(
    req: hyper::Request<hyper::body::Incoming>,
    db: &DatabaseManager,
) -> Result<
    hyper::Response<http_body_util::Full<hyper::body::Bytes>>,
    Box<dyn std::error::Error + Send + Sync>,
> {
    let (parts, body) = req.into_parts();
    let method = parts.method;
    let path = parts.uri.path();

    if path == "/_meta/apis" {
        match method {
            hyper::Method::GET => {
                let records = db
                    .select("_meta_api_configs", None, None, None, None)
                    .await?;
                let json = serde_json::to_string(&records)?;
                return Ok(hyper::Response::builder()
                    .status(hyper::StatusCode::OK)
                    .header("Content-Type", "application/json")
                    .body(http_body_util::Full::new(hyper::body::Bytes::from(json)))?);
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

                let id = uuid::Uuid::new_v4().to_string();
                let created_at = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)?
                    .as_secs() as i64;

                let mut data = HashMap::new();
                data.insert("id".to_string(), Value::String(id.clone()));
                data.insert("name".to_string(), Value::String(name.to_string()));
                data.insert("version".to_string(), Value::String(version.to_string()));
                data.insert("spec".to_string(), Value::String(spec.to_string()));
                data.insert(
                    "created_at".to_string(),
                    Value::Number(serde_json::Number::from(created_at)),
                );

                db.insert("_meta_api_configs", data).await?;

                // Extract schemas from spec and initialize them in the DB
                let schemas =
                    crate::schema_generator::SchemaGenerator::extract_schemas_from_openapi(spec)?;
                db.initialize_schema(schemas).await?;

                return Ok(hyper::Response::builder()
                    .status(hyper::StatusCode::CREATED)
                    .header("Content-Type", "application/json")
                    .body(http_body_util::Full::new(hyper::body::Bytes::from(
                        serde_json::json!({"id": id}).to_string(),
                    )))?);
            }
            _ => {}
        }
    } else if path == "/_meta/auth" {
        match method {
            hyper::Method::GET => {
                let records = db
                    .select("_meta_auth_configs", None, None, None, None)
                    .await?;
                let json = serde_json::to_string(&records)?;
                return Ok(hyper::Response::builder()
                    .status(hyper::StatusCode::OK)
                    .header("Content-Type", "application/json")
                    .body(http_body_util::Full::new(hyper::body::Bytes::from(json)))?);
            }
            hyper::Method::POST => {
                let body_bytes = http_body_util::BodyExt::collect(body).await?.to_bytes();
                // Validate that it parses as Authenticator
                let auth_config: Authenticator = serde_json::from_slice(&body_bytes)?;
                // Store as string
                let config_str = serde_json::to_string(&auth_config)?;

                let id = uuid::Uuid::new_v4().to_string();
                let updated_at = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)?
                    .as_secs() as i64;

                let mut data = HashMap::new();
                data.insert("id".to_string(), Value::String(id.clone()));
                data.insert("config".to_string(), Value::String(config_str));
                data.insert(
                    "updated_at".to_string(),
                    Value::Number(serde_json::Number::from(updated_at)),
                );

                db.insert("_meta_auth_configs", data).await?;

                return Ok(hyper::Response::builder()
                    .status(hyper::StatusCode::CREATED)
                    .header("Content-Type", "application/json")
                    .body(http_body_util::Full::new(hyper::body::Bytes::from(
                        serde_json::json!({"id": id}).to_string(),
                    )))?);
            }
            _ => {}
        }
    }

    Ok(hyper::Response::builder()
        .status(hyper::StatusCode::NOT_FOUND)
        .body(http_body_util::Full::new(hyper::body::Bytes::from(
            "Not Found",
        )))?)
}
