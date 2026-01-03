//! Configuration file parsing and structure definitions

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::net::SocketAddr;

/// Top-level configuration structure
#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub listeners: Option<Vec<ListenerConfig>>,
    pub apis: Option<Vec<ApiConfig>>, // Global APIs configuration
    #[serde(alias = "control-plane")]
    pub control_plane: Option<ControlPlaneConfig>,
    pub auth: Option<Vec<Authenticator>>, // Unified authentication configuration
    pub datasource: Option<std::collections::HashMap<String, DatabaseSettings>>, // Global datasources
    pub modules: Option<GlobalModulesConfig>, // Global modules (tracing, metrics, etc.)
    pub log_level: Option<String>,            // Global log level (trace, debug, info, warn, error)
}

/// Control Plane configuration
#[derive(Debug, Deserialize, Clone)]
pub struct ControlPlaneConfig {
    pub listen: ControlPlaneListenConfig,
    pub database: DatabaseSettings,
}

/// Control Plane listen configuration
#[derive(Debug, Deserialize, Clone)]
pub struct ControlPlaneListenConfig {
    pub ip: String,
    pub port: u16,
}

/// Authenticator Enum (Polymorphic)
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(tag = "type")]
pub enum Authenticator {
    #[serde(rename = "api-key")]
    ApiKey(ApiKeyAuthenticator),
    #[serde(rename = "oidc")]
    Oidc(OidcAuthenticator),
}

/// API Key Authenticator
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ApiKeyAuthenticator {
    pub name: String,
    pub enabled: Option<bool>,
    pub config: ApiKeyConfig,
}

/// API Key Configuration
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ApiKeyConfig {
    pub source: Option<ApiKeySource>, // "header" or "query"
    pub key_name: Option<String>,     // default "X-Api-Key"
    pub consumers: Vec<ConsumerConfig>,
}

/// API Key Source
#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ApiKeySource {
    Header,
    Query,
}

/// OIDC Authenticator
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct OidcAuthenticator {
    pub name: String,
    pub enabled: Option<bool>,
    pub config: OidcConfig,
}

/// OIDC Configuration
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct OidcConfig {
    pub issuer: String,
    pub client_id: Option<String>,
    pub client_secret: Option<String>,
    pub audience: Option<String>,
    pub introspection: Option<bool>,
}

/// Global modules configuration
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct GlobalModulesConfig {
    pub tracing: Option<TracingConfig>,
    pub metrics: Option<MetricsConfig>,
    pub openapi_docs: Option<OpenApiDocsConfig>,
    pub access_log: Option<AccessLogConfig>,
}

/// Access Log module configuration
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct AccessLogConfig {
    pub enabled: Option<bool>,
    pub path: Option<String>, // Path to log file (default: "logs/access.log")
    pub format: Option<String>, // "json" or "text" (default: "json")
    pub headers: Option<Vec<String>>, // List of headers to log (case-insensitive)
    pub query: Option<bool>,  // Log query parameters
    pub body: Option<bool>,   // Log request body (if available as JSON)
    pub cookies: Option<bool>, // Log cookies
}

/// Tracing module configuration
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct TracingConfig {
    pub enabled: Option<bool>,         // Enable tracing (OpenTelemetry)
    pub otlp_endpoint: Option<String>, // OpenTelemetry collector endpoint
}

/// Metrics module configuration
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct MetricsConfig {
    pub enabled: Option<bool>, // Enable Prometheus metrics endpoint
    pub port: Option<u16>,     // Port for metrics endpoint (default: 9090)
}

/// OpenAPI Docs module configuration
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct OpenApiDocsConfig {
    pub enabled: Option<bool>, // Enable OpenAPI docs server
    pub port: Option<u16>,     // Port for OpenAPI docs (Swagger UI)
}

/// Database configuration structure - supports multiple named datasources
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DatabaseConfig {
    pub datasource: std::collections::HashMap<String, DatabaseSettings>,
}

/// Database settings for a single datasource
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DatabaseSettings {
    pub driver: String,
    pub host: Option<String>,
    pub port: Option<u16>,
    pub user: Option<String>,
    pub password: Option<String>,
    pub database: String,
    pub ssl_mode: Option<String>,
    pub max_pool_size: Option<usize>,
}

/// OpenAPI configuration structure
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct OpenAPIConfig {
    pub openapi: OpenAPISettings,
}

/// OpenAPI settings
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct OpenAPISettings {
    pub spec: Value,
    pub validation: Option<ValidationConfig>,
}

/// Listener configuration (port, IP, routes, etc.)
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ListenerConfig {
    pub name: Option<String>, // Listener name for reference
    pub port: u16,
    pub ip: String,
    pub protocol: String,
    pub routes: Option<Vec<RouteConfig>>, // Legacy routes support
    pub modules: Option<ModulesConfig>, // Listener-level fallback modules (internal, not OpenAPI)
    pub consumers: Option<Vec<ConsumerConfig>>, // Authentication consumers
}

/// Route configuration (name and matching rules)
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct RouteConfig {
    pub name: String,
    pub matches: Vec<MatchRule>,
}

/// Route matching rules (path, method, etc.)
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct MatchRule {
    pub path: PathMatch,
    pub method: Option<String>,
}

/// Path matching rules (prefix matching)
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct PathMatch {
    pub path_prefix: String,
}

/// Validation configuration for OpenAPI
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ValidationConfig {
    pub strict_mode: Option<bool>,
    pub validate_response_body: Option<bool>,
}

/// Phase modules configuration (kept outside OpenAPI to preserve spec compliance)
#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct ModulesConfig {
    pub access: Option<Vec<String>>,  // e.g., ["auth_header", "jwt"]
    pub rewrite: Option<Vec<String>>, // e.g., ["prefix_strip:/api"] (future)
}

/// API configuration (top-level)
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ApiConfig {
    pub path: String,
    pub datasource: Option<String>,
    pub listeners: Option<Vec<String>>, // List of listener names
    pub modules: Option<ModulesConfig>,
    pub access_log: Option<AccessLogConfig>,
}


#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ConsumerConfig {
    pub name: String,
    pub keys: Vec<String>, // API keys bound to this consumer
                           // Future: rate limits, roles, metadata, etc.
}

impl Config {
    /// Read and parse configuration from file
    // Updated error type to include Send + Sync
    pub fn from_file(path: &str) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let content =
            fs::read_to_string(path).map_err(|e| format!("Failed to read config file: {}", e))?;

        // Expand environment variables in format ${VAR:default}
        let expanded = expand_env_vars(&content);

        let config = serde_yaml::from_str(&expanded)
            .map_err(|e| format!("Failed to parse config file: {}", e))?;
        Ok(config)
    }
}

/// Expand environment variables in config content
/// Supports ${VAR:default} syntax
fn expand_env_vars(content: &str) -> String {
    let mut result = content.to_string();

    // Regex pattern: ${VAR:default} or ${VAR}
    let re = regex::Regex::new(r"\$\{([^:}]+)(?::([^}]*))?\}").unwrap();

    loop {
        let mut changed = false;
        let new_result = re
            .replace_all(&result, |caps: &regex::Captures| {
                let var_name = &caps[1];
                let default_val = caps.get(2).map(|m| m.as_str()).unwrap_or("");

                let expanded_value =
                    std::env::var(var_name).unwrap_or_else(|_| default_val.to_string());
                tracing::debug!(
                    var = %var_name,
                    value = %expanded_value,
                    from_env = std::env::var(var_name).is_ok(),
                    "Expanding environment variable"
                );

                changed = true;
                expanded_value
            })
            .to_string();

        if !changed {
            break;
        }
        result = new_result;
    }

    result
}

impl DatabaseConfig {
    /// Read and parse database configuration from file
    pub fn from_file(path: &str) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let content = fs::read_to_string(path)
            .map_err(|e| format!("Failed to read database config file: {}", e))?;
        let config = serde_yaml::from_str(&content)
            .map_err(|e| format!("Failed to parse database config file: {}", e))?;
        Ok(config)
    }
}

impl OpenAPIConfig {
    /// Read and parse OpenAPI configuration from file
    pub fn from_file(path: &str) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let content = fs::read_to_string(path)
            .map_err(|e| format!("Failed to read OpenAPI config file: {}", e))?;

        // Expand environment variables
        let expanded = expand_env_vars(&content);

        let config = serde_yaml::from_str(&expanded)
            .map_err(|e| format!("Failed to parse OpenAPI config file: {}", e))?;
        Ok(config)
    }
}

impl ListenerConfig {
    /// Convert to SocketAddr
    // Updated error type to include Send + Sync
    pub fn to_socket_addr(&self) -> Result<SocketAddr, Box<dyn std::error::Error + Send + Sync>> {
        let addr = format!("{}:{}", self.ip, self.port)
            .parse()
            .map_err(|e| format!("Invalid address format: {}", e))?;
        Ok(addr)
    }
}
