//! Configuration file parsing and structure definitions

use serde::Deserialize;
use serde_json::Value;
use std::fs;
use std::net::SocketAddr;

/// Top-level configuration structure
#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub listeners: Vec<ListenerConfig>,
}

/// Database configuration structure
#[derive(Debug, Deserialize, Clone)]
pub struct DatabaseConfig {
    pub database: DatabaseSettings,
}

/// Database settings
#[derive(Debug, Deserialize, Clone)]
pub struct DatabaseSettings {
    pub driver: Option<String>,
    pub host: String,
    pub port: u16,
    pub user: String,
    pub password: String,
    pub database: String,
    pub ssl_mode: Option<String>,
    pub max_pool_size: Option<usize>,
}

/// OpenAPI configuration structure
#[derive(Debug, Deserialize, Clone)]
pub struct OpenAPIConfig {
    pub openapi: OpenAPISettings,
}

/// OpenAPI settings
#[derive(Debug, Deserialize, Clone)]
pub struct OpenAPISettings {
    pub spec: Value,
    pub validation: Option<ValidationConfig>,
}

/// Listener configuration (port, IP, routes, etc.)
#[derive(Debug, Deserialize, Clone)]
pub struct ListenerConfig {
    pub port: u16,
    pub ip: String,
    pub protocol: String,
    pub apis: Option<Vec<ApiRef>>, // API file paths or objects with modules
    pub routes: Option<Vec<RouteConfig>>, // Legacy routes support
    pub modules: Option<ModulesConfig>, // Listener-level fallback modules (internal, not OpenAPI)
}

/// Route configuration (name and matching rules)
#[derive(Debug, Deserialize, Clone)]
pub struct RouteConfig {
    pub name: String,
    pub matches: Vec<MatchRule>,
}

/// Route matching rules (path, method, etc.)
#[derive(Debug, Deserialize, Clone)]
pub struct MatchRule {
    pub path: PathMatch,
    pub method: Option<String>,
}

/// Path matching rules (prefix matching)
#[derive(Debug, Deserialize, Clone)]
pub struct PathMatch {
    pub path_prefix: String,
}


/// Validation configuration for OpenAPI
#[derive(Debug, Deserialize, Clone)]
pub struct ValidationConfig {
    pub strict_mode: Option<bool>,
    pub validate_request_body: Option<bool>,
    pub validate_response_body: Option<bool>,
}

/// Phase modules configuration (kept outside OpenAPI to preserve spec compliance)
#[derive(Debug, Deserialize, Clone, Default)]
pub struct ModulesConfig {
    pub access: Option<Vec<String>>,  // e.g., ["auth_header", "jwt"]
    pub rewrite: Option<Vec<String>>, // e.g., ["prefix_strip:/api"] (future)
}

/// API reference in listener: path string or object with path + per-API modules
#[derive(Debug, Deserialize, Clone)]
#[serde(untagged)]
pub enum ApiRef {
    Path(String),
    WithModules { path: String, modules: Option<ModulesConfig> },
}

impl Config {
    /// Read and parse configuration from file
    // Updated error type to include Send + Sync
    pub fn from_file(path: &str) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let content =
            fs::read_to_string(path).map_err(|e| format!("Failed to read config file: {}", e))?;
        let config = serde_yaml::from_str(&content)
            .map_err(|e| format!("Failed to parse config file: {}", e))?;
        Ok(config)
    }
}

impl DatabaseConfig {
    /// Read and parse database configuration from file
    pub fn from_file(path: &str) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let content =
            fs::read_to_string(path).map_err(|e| format!("Failed to read database config file: {}", e))?;
        let config = serde_yaml::from_str(&content)
            .map_err(|e| format!("Failed to parse database config file: {}", e))?;
        Ok(config)
    }
}

impl OpenAPIConfig {
    /// Read and parse OpenAPI configuration from file
    pub fn from_file(path: &str) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let content =
            fs::read_to_string(path).map_err(|e| format!("Failed to read OpenAPI config file: {}", e))?;
        let config = serde_yaml::from_str(&content)
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
