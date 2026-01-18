use super::models::AuthConfigRecord;
use crate::config::Authenticator;
use crate::database::DatabaseManager;
use http_body_util::Full;
use hyper::body::Bytes;
use hyper::{Response, StatusCode};
use serde_json::Value;
use std::collections::HashMap;

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

pub async fn handle_auth_request(
    req: hyper::Request<hyper::body::Incoming>,
    db: &DatabaseManager,
) -> Result<Response<Full<Bytes>>, Box<dyn std::error::Error + Send + Sync>> {
    let (parts, body) = req.into_parts();
    let method = parts.method;

    match method {
        hyper::Method::GET => {
            let records = db
                .select("_meta_auth_configs", None, None, None, None)
                .await?;
            let json = serde_json::to_string(&records)?;
            Ok(Response::builder()
                .status(StatusCode::OK)
                .header("Content-Type", "application/json")
                .body(Full::new(Bytes::from(json)))?)
        }
        hyper::Method::POST => {
            let body_bytes = http_body_util::BodyExt::collect(body).await?.to_bytes();
            // Validate that it parses as Authenticator
            let auth_config: Authenticator = serde_json::from_slice(&body_bytes)?;

            // Extract name from auth config
            let auth_name = match &auth_config {
                Authenticator::ApiKey(config) => &config.name,
                Authenticator::Oidc(config) => &config.name,
            };

            // Check if auth config with same name already exists
            let records = db
                .select("_meta_auth_configs", None, None, None, None)
                .await?;

            for record in records {
                if let Ok(existing_auth_record) = serde_json::from_value::<AuthConfigRecord>(record)
                {
                    if let Ok(existing_auth) =
                        serde_json::from_str::<Authenticator>(&existing_auth_record.config)
                    {
                        let existing_name = match &existing_auth {
                            Authenticator::ApiKey(config) => &config.name,
                            Authenticator::Oidc(config) => &config.name,
                        };

                        if existing_name == auth_name {
                            return Ok(Response::builder()
                                .status(StatusCode::CONFLICT)
                                .header("Content-Type", "application/json")
                                .body(Full::new(Bytes::from(
                                    serde_json::json!({
                                        "error": format!("Auth config with name '{}' already exists", auth_name)
                                    }).to_string(),
                                )))?);
                        }
                    }
                }
            }

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
