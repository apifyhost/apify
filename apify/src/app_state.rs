//! Application state management and route matching logic

use super::api_generator::APIGenerator;
use super::config::{DatabaseConfig, MatchRule, OpenAPIConfig, RouteConfig};
use super::crud_handler::CRUDHandler;
use super::database::DatabaseManager;
use super::hyper::Method;
use super::schema_generator::SchemaGenerator;
use std::collections::HashMap;
use std::sync::Arc;

/// Shared application state (route configurations and CRUD handlers)
#[derive(Debug, Clone)]
pub struct AppState {
    routes: Vec<RouteConfig>,
    route_responses: HashMap<String, String>, // route name -> response string
    pub crud_handler: Option<Arc<CRUDHandler>>,
}

impl AppState {
    /// Create new application state
    pub fn new(routes: Vec<RouteConfig>) -> Self {
        let mut route_responses = HashMap::new();
        // Generate fixed responses for each route (hello + route name)
        for route in &routes {
            route_responses.insert(route.name.clone(), format!("hello {}", route.name));
        }
        Self {
            routes,
            route_responses,
            crud_handler: None,
        }
    }

    /// Create new application state with CRUD support
    pub async fn new_with_crud(
        routes: Option<Vec<RouteConfig>>,
        _database_config: Option<DatabaseConfig>,
        openapi_configs: Vec<OpenAPIConfig>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let routes = routes.unwrap_or_default();
        let mut route_responses = HashMap::new();
        // Generate fixed responses for each route (hello + route name)
        for route in &routes {
            route_responses.insert(route.name.clone(), format!("hello {}", route.name));
        }

        // For now, always use SQLite backend regardless of provided database config
        let crud_handler = {
            if !openapi_configs.is_empty() {
                let db_cfg = crate::database::DatabaseConfig::sqlite_default();
                let db_manager = DatabaseManager::new(db_cfg).await?;
                
                // Extract table schemas from all OpenAPI specs
                let mut all_schemas = Vec::new();
                for openapi_config in &openapi_configs {
                    let schemas = SchemaGenerator::extract_schemas_from_openapi(&openapi_config.openapi.spec)?;
                    all_schemas.extend(schemas);
                }
                
                // Initialize database schema with extracted table definitions
                if !all_schemas.is_empty() {
                    eprintln!("Initializing database with {} table schemas", all_schemas.len());
                    db_manager.initialize_schema(all_schemas).await?;
                } else {
                    eprintln!("Warning: No table schemas found in OpenAPI configurations");
                }
                
                // Merge all OpenAPI specs into one - deep merge for paths
                let mut merged_spec = serde_json::Map::new();
                let mut merged_paths = serde_json::Map::new();
                
                for openapi_config in &openapi_configs {
                    if let Some(spec_obj) = openapi_config.openapi.spec.as_object() {
                        for (key, value) in spec_obj {
                            if key == "paths" {
                                // Deep merge paths from all specs
                                if let Some(paths_obj) = value.as_object() {
                                    for (path_key, path_value) in paths_obj {
                                        merged_paths.insert(path_key.clone(), path_value.clone());
                                    }
                                }
                            } else {
                                // For other keys, just use the last value
                                merged_spec.insert(key.clone(), value.clone());
                            }
                        }
                    }
                }
                
                // Add merged paths to the spec
                merged_spec.insert("paths".to_string(), serde_json::Value::Object(merged_paths));
                
                let merged_value = serde_json::Value::Object(merged_spec);
                let api_generator = APIGenerator::new(merged_value)?;
                Some(Arc::new(CRUDHandler::new(db_manager, api_generator)))
            } else { None }
        };

        Ok(Self {
            routes,
            route_responses,
            crud_handler,
        })
    }

    /// Match route based on request path and method
    pub fn match_route(&self, path: &str, method: &Method) -> Option<&String> {
        for route in &self.routes {
            if self.matches_route(route, path, method) {
                return self.route_responses.get(&route.name);
            }
        }
        None
    }

    /// Check if a single route matches the request
    fn matches_route(&self, route: &RouteConfig, path: &str, method: &Method) -> bool {
        route
            .matches
            .iter()
            .any(|rule| self.matches_rule(rule, path, method))
    }

    /// Check if a single match rule matches the request
    fn matches_rule(&self, rule: &MatchRule, path: &str, method: &Method) -> bool {
        // 1. Path prefix matching
        let path_matches = path.starts_with(&rule.path.path_prefix);
        if !path_matches {
            return false;
        }

        // 2. Method matching (matches all if no method specified)
        let method_matches = rule
            .method
            .as_ref()
            .is_none_or(|rule_method| method.as_str() == rule_method);

        path_matches && method_matches
    }
}
