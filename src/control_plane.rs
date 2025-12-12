use crate::app_state::OpenApiStateConfig;
use crate::config::{Authenticator, DatabaseSettings, ModulesConfig, OpenAPIConfig};
use crate::database::DatabaseManager;
use crate::schema_generator::{ColumnDefinition, TableSchema};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::net::SocketAddr;
use tokio::net::TcpListener;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use std::sync::Arc;
use hyper_util::rt::TokioIo;

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
                    name: "datasource_name".to_string(),
                    column_type: "TEXT".to_string(),
                    nullable: true,
                    primary_key: false,
                    unique: false,
                    auto_increment: false,
                    default_value: None,
                    auto_field: false,
                },
                ColumnDefinition {
                    name: "modules_config".to_string(),
                    column_type: "TEXT".to_string(), // JSON string
                    nullable: true,
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
            table_name: "_meta_datasources".to_string(),
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
                    name: "type".to_string(),
                    column_type: "TEXT".to_string(),
                    nullable: false,
                    primary_key: false,
                    unique: false,
                    auto_increment: false,
                    default_value: None,
                    auto_field: false,
                },
                ColumnDefinition {
                    name: "config".to_string(),
                    column_type: "TEXT".to_string(), // JSON string (DatabaseSettings)
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
        TableSchema {
            table_name: "_meta_listeners".to_string(),
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
                    name: "port".to_string(),
                    column_type: "INTEGER".to_string(),
                    nullable: false,
                    primary_key: false,
                    unique: false,
                    auto_increment: false,
                    default_value: None,
                    auto_field: false,
                },
                ColumnDefinition {
                    name: "config".to_string(),
                    column_type: "TEXT".to_string(), // JSON string (ListenerConfig)
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
    pub datasource_name: Option<String>,
    pub modules_config: Option<String>,
    pub created_at: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DatasourceConfigRecord {
    pub id: String,
    pub name: String,
    pub config: String,
    pub updated_at: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AuthConfigRecord {
    pub id: String,
    pub config: String,
    pub updated_at: i64,
}

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

pub async fn load_listeners(
    db: &DatabaseManager,
) -> Result<Option<Vec<crate::config::ListenerConfig>>, Box<dyn std::error::Error + Send + Sync>> {
    let records = db
        .select("_meta_listeners", None, None, None, None)
        .await?;

    if records.is_empty() {
        return Ok(None);
    }

    let mut listeners = Vec::new();
    for record in records {
        if let Some(config_val) = record.get("config") {
            if let Some(config_str) = config_val.as_str() {
                let listener: crate::config::ListenerConfig = serde_json::from_str(config_str)?;
                listeners.push(listener);
            }
        }
    }

    Ok(Some(listeners))
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

                return Ok(hyper::Response::builder()
                    .status(hyper::StatusCode::CREATED)
                    .header("Content-Type", "application/json")
                    .body(http_body_util::Full::new(hyper::body::Bytes::from(
                        serde_json::json!({"id": id}).to_string(),
                    )))?);
            }
            _ => {}
        }
    } else if path == "/_meta/datasources" {
        match method {
            hyper::Method::GET => {
                let records = db
                    .select("_meta_datasources", None, None, None, None)
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
                let config = payload.get("config").ok_or("Missing config")?;

                // Validate config
                let _ds_config: DatabaseSettings = serde_json::from_value(config.clone())?;
                let config_str = serde_json::to_string(config)?;

                let id = uuid::Uuid::new_v4().to_string();
                let updated_at = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)?
                    .as_secs() as i64;

                let mut data = HashMap::new();
                data.insert("id".to_string(), Value::String(id.clone()));
                data.insert("name".to_string(), Value::String(name.to_string()));
                data.insert("config".to_string(), Value::String(config_str));
                data.insert(
                    "updated_at".to_string(),
                    Value::Number(serde_json::Number::from(updated_at)),
                );

                db.insert("_meta_datasources", data).await?;

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
    } else if path == "/_meta/import" && method == hyper::Method::POST {
        let body_bytes = http_body_util::BodyExt::collect(body).await?.to_bytes();
        let config: crate::config::Config = serde_yaml::from_slice(&body_bytes)?;

        // Import Datasources
        if let Some(datasources) = config.datasource {
            for (name, ds_config) in datasources {
                let id = uuid::Uuid::new_v4().to_string();
                let config_str = serde_json::to_string(&ds_config)?;
                let updated_at = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)?
                    .as_secs() as i64;

                let mut data = HashMap::new();
                data.insert("id".to_string(), Value::String(id));
                data.insert("name".to_string(), Value::String(name));
                data.insert("type".to_string(), Value::String(ds_config.driver.clone()));
                data.insert("config".to_string(), Value::String(config_str));
                data.insert(
                    "updated_at".to_string(),
                    Value::Number(serde_json::Number::from(updated_at)),
                );

                if let Err(e) = db.insert("_meta_datasources", data).await {
                    tracing::warn!("Failed to import datasource: {}", e);
                }
            }
        }

        // Import Auth
        if let Some(auths) = config.auth {
            for auth in auths {
                let id = uuid::Uuid::new_v4().to_string();
                let config_str = serde_json::to_string(&auth)?;
                let updated_at = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)?
                    .as_secs() as i64;

                let mut data = HashMap::new();
                data.insert("id".to_string(), Value::String(id));
                data.insert("config".to_string(), Value::String(config_str));
                data.insert(
                    "updated_at".to_string(),
                    Value::Number(serde_json::Number::from(updated_at)),
                );

                if let Err(e) = db.insert("_meta_auth_configs", data).await {
                    tracing::warn!("Failed to import auth config: {}", e);
                }
            }
        }

        // Import Listeners
        if let Some(listeners) = &config.listeners {
            for listener in listeners {
                let id = uuid::Uuid::new_v4().to_string();
                let config_str = serde_json::to_string(listener)?;
                let updated_at = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)?
                    .as_secs() as i64;

                let mut data = HashMap::new();
                data.insert("id".to_string(), Value::String(id));
                data.insert(
                    "port".to_string(),
                    Value::Number(serde_json::Number::from(listener.port)),
                );
                data.insert("config".to_string(), Value::String(config_str));
                data.insert(
                    "updated_at".to_string(),
                    Value::Number(serde_json::Number::from(updated_at)),
                );

                if let Err(e) = db.insert("_meta_listeners", data).await {
                    tracing::warn!("Failed to import listener: {}", e);
                }
            }
        }

        // Import APIs
        if let Some(listeners) = config.listeners {
            for listener in listeners {
                if let Some(apis) = listener.apis {
                    for api_ref in apis {
                        let (path, modules, datasource, _access_log) = match api_ref {
                            crate::config::ApiRef::Path(p) => (p, None, None, None),
                            crate::config::ApiRef::WithConfig {
                                path,
                                modules,
                                datasource,
                                access_log,
                            } => (path, modules, datasource, access_log),
                        };

                        match std::fs::read_to_string(&path) {
                            Ok(spec_content) => {
                                let spec_value = if let Ok(api_config) =
                                    serde_yaml::from_str::<crate::config::OpenAPIConfig>(&spec_content)
                                {
                                    api_config.openapi.spec
                                } else if let Ok(val) = serde_yaml::from_str::<Value>(&spec_content) {
                                    val
                                } else {
                                    tracing::warn!("Failed to parse API spec: {}", path);
                                    continue;
                                };

                                let name = path.clone();
                                let version = "1.0.0".to_string();
                                let id = uuid::Uuid::new_v4().to_string();
                                let updated_at = std::time::SystemTime::now()
                                    .duration_since(std::time::UNIX_EPOCH)?
                                    .as_secs() as i64;

                                let mut data = HashMap::new();
                                data.insert("id".to_string(), Value::String(id));
                                data.insert("name".to_string(), Value::String(name));
                                data.insert("version".to_string(), Value::String(version));
                                data.insert(
                                    "spec".to_string(),
                                    Value::String(serde_json::to_string(&spec_value)?),
                                );
                                if let Some(ds) = datasource {
                                    data.insert("datasource_name".to_string(), Value::String(ds));
                                }
                                if let Some(m) = modules {
                                    data.insert(
                                        "modules_config".to_string(),
                                        Value::String(serde_json::to_string(&m)?),
                                    );
                                }
                                data.insert(
                                    "created_at".to_string(),
                                    Value::Number(serde_json::Number::from(updated_at)),
                                );

                                if let Err(e) = db.insert("_meta_api_configs", data).await {
                                    tracing::warn!("Failed to import API config: {}", e);
                                }
                            }
                            Err(e) => {
                                tracing::warn!("Failed to read API spec file {}: {}", path, e);
                            }
                        }
                    }
                }
            }
        }

        return Ok(hyper::Response::builder()
            .status(hyper::StatusCode::OK)
            .body(http_body_util::Full::new(hyper::body::Bytes::from(
                "Imported",
            )))?);
    }

    Ok(hyper::Response::builder()
        .status(hyper::StatusCode::NOT_FOUND)
        .body(http_body_util::Full::new(hyper::body::Bytes::from(
            "Not Found",
        )))?)
}

pub async fn start_control_plane_server(
    config: crate::config::ControlPlaneConfig,
    db: DatabaseManager,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let addr: SocketAddr = format!("{}:{}", config.listen.ip, config.listen.port).parse()?;
    let listener = TcpListener::bind(addr).await?;
    tracing::info!("Control Plane listening on {}", addr);

    let db = Arc::new(db);

    loop {
        let (stream, _) = listener.accept().await?;
        let io = TokioIo::new(stream);
        let db_clone = db.clone();

        tokio::task::spawn(async move {
            if let Err(err) = http1::Builder::new()
                .serve_connection(io, service_fn(move |req| {
                    let db = db_clone.clone();
                    async move {
                        match handle_control_plane_request(req, &db).await {
                            Ok(res) => Ok::<_, hyper::Error>(res),
                            Err(e) => {
                                tracing::error!("Internal server error: {}", e);
                                let res = hyper::Response::builder()
                                    .status(hyper::StatusCode::INTERNAL_SERVER_ERROR)
                                    .body(http_body_util::Full::new(hyper::body::Bytes::from(
                                        format!("Internal Server Error: {}", e),
                                    )))
                                    .unwrap();
                                Ok(res)
                            }
                        }
                    }
                }))
                .await
            {
                tracing::error!("Error serving connection: {:?}", err);
            }
        });
    }
}
