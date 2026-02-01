use crate::config::Config;
use crate::database::DatabaseManager;
use crate::modules::tracing::init_tracing;
use std::collections::HashMap;

pub type RuntimeInitData = (
    Option<DatabaseManager>,
    HashMap<String, crate::app_state::OpenApiStateConfig>,
    Option<Vec<crate::config::Authenticator>>,
    Option<HashMap<String, crate::config::DatabaseSettings>>,
    Option<Vec<crate::config::ListenerConfig>>,
);

pub type LoggingSetup = (bool, Option<String>, Option<String>);

pub fn setup_logging(
    config: &Config,
) -> Result<LoggingSetup, Box<dyn std::error::Error + Send + Sync>> {
    let tracing_config = config.modules.as_ref().and_then(|m| m.tracing.as_ref());
    let tracing_enabled = tracing_config.and_then(|t| t.enabled).unwrap_or(true);
    let otlp_endpoint = tracing_config.and_then(|t| t.otlp_endpoint.as_deref());
    let log_level = config.log_level.as_deref();

    if tracing_enabled && otlp_endpoint.is_some() {
        eprintln!("Deferring tracing initialization to Tokio runtime (OpenTelemetry enabled)");
    } else {
        init_tracing("apify", None, log_level)?;
    }

    Ok((
        tracing_enabled,
        otlp_endpoint.map(|s| s.to_string()),
        log_level.map(|s| s.to_string()),
    ))
}

pub fn build_runtime() -> Result<tokio::runtime::Runtime, std::io::Error> {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
}

pub async fn init_database(
    config: &Config,
) -> Result<DatabaseManager, Box<dyn std::error::Error + Send + Sync>> {
    let db_config = if let Some(cp_config) = &config.control_plane {
        let s = &cp_config.database;
        let url = if s.driver == "sqlite" {
            format!("sqlite:{}", s.database)
        } else {
            let mut url = "postgres://".to_string();
            if let Some(user) = &s.user {
                url.push_str(user);
                if let Some(pass) = &s.password {
                    url.push(':');
                    url.push_str(pass);
                }
                url.push('@');
            }
            if let Some(host) = &s.host {
                url.push_str(host);
            }
            if let Some(port) = s.port {
                url.push(':');
                url.push_str(&port.to_string());
            }
            url.push('/');
            url.push_str(&s.database);
            url
        };
        crate::database::DatabaseRuntimeConfig {
            driver: s.driver.clone(),
            url,
            max_size: s.max_pool_size.unwrap_or(20) as u32,
        }
    } else {
        crate::database::DatabaseRuntimeConfig::sqlite_default()
    };

    DatabaseManager::new(db_config).await.map_err(|e| e.into())
}
