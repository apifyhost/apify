use crate::database::DatabaseManager;
use http_body_util::Full;
use hyper::body::Bytes;
use hyper::{Response, StatusCode};
use serde_json::Value;
use std::collections::HashMap;

pub async fn handle_import_request(
    req: hyper::Request<hyper::body::Incoming>,
    db: &DatabaseManager,
) -> Result<Response<Full<Bytes>>, Box<dyn std::error::Error + Send + Sync>> {
    let (parts, body) = req.into_parts();
    let method = parts.method;

    if method != hyper::Method::POST {
        return Ok(Response::builder()
            .status(StatusCode::METHOD_NOT_ALLOWED)
            .body(Full::new(Bytes::from("Method Not Allowed")))?);
    }

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
            data.insert("name".to_string(), Value::String(name.clone()));
            data.insert("type".to_string(), Value::String(ds_config.driver.clone()));
            data.insert("config".to_string(), Value::String(config_str));
            data.insert(
                "updated_at".to_string(),
                Value::Number(serde_json::Number::from(updated_at)),
            );

            if let Err(e) = db.insert("_meta_datasources", data.clone()).await {
                // Try update if insert fails (likely due to unique name)
                // Note: This is a simple retry strategy. Ideally we should check existence first.
                tracing::warn!("Failed to insert datasource, trying update: {}", e);

                let mut where_clause = HashMap::new();
                where_clause.insert("name".to_string(), Value::String(name));

                // Remove ID from data to avoid changing it
                let mut update_data = data;
                update_data.remove("id");

                if let Err(e) = db
                    .update("_meta_datasources", update_data, where_clause)
                    .await
                {
                    tracing::warn!("Failed to update datasource: {}", e);
                }
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
    if let Some(listeners) = config.listeners {
        for listener in &listeners {
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
                "created_at".to_string(),
                Value::Number(serde_json::Number::from(updated_at)),
            );
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
    if let Some(apis) = config.apis {
        for api_config in apis {
            let path = api_config.path;
            // modules removed from ApiConfig
            let datasource = api_config.datasource;
            let listeners = api_config.listeners;

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
                    data.insert("name".to_string(), Value::String(name.clone()));
                    data.insert("version".to_string(), Value::String(version));
                    data.insert(
                        "spec".to_string(),
                        Value::String(serde_json::to_string(&spec_value)?),
                    );
                    if let Some(ds) = datasource {
                        data.insert("datasource_name".to_string(), Value::String(ds));
                    }
                    // modules removed
                    if let Some(l) = listeners {
                        data.insert(
                            "listeners".to_string(),
                            Value::String(serde_json::to_string(&l)?),
                        );
                    }
                    data.insert(
                        "created_at".to_string(),
                        Value::Number(serde_json::Number::from(updated_at)),
                    );

                    if let Err(e) = db.insert("_meta_api_configs", data.clone()).await {
                        tracing::warn!("Failed to insert API config, trying update: {}", e);

                        let mut where_clause = HashMap::new();
                        where_clause.insert("name".to_string(), Value::String(name));

                        // Remove ID and created_at from update
                        let mut update_data = data;
                        update_data.remove("id");
                        update_data.remove("created_at");

                        if let Err(e) = db
                            .update("_meta_api_configs", update_data, where_clause)
                            .await
                        {
                            tracing::warn!("Failed to update API config: {}", e);
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!("Failed to read API spec file {}: {}", path, e);
                }
            }
        }
    }

    Ok(Response::builder()
        .status(StatusCode::OK)
        .body(Full::new(Bytes::from("Imported")))?)
}
