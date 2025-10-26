//! Application state management and route matching logic

use super::config::{MatchRule, RouteConfig, DatabaseConfig, OpenAPIConfig};
use super::database::DatabaseManager;
use super::api_generator::APIGenerator;
use super::crud_handler::CRUDHandler;
use super::hyper::Method;
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
    pub fn new_with_crud(
        routes: Vec<RouteConfig>,
        database_config: Option<DatabaseConfig>,
        openapi_config: Option<OpenAPIConfig>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let mut route_responses = HashMap::new();
        // Generate fixed responses for each route (hello + route name)
        for route in &routes {
            route_responses.insert(route.name.clone(), format!("hello {}", route.name));
        }

        let crud_handler = if let (Some(db_config), Some(openapi_config)) = (database_config, openapi_config) {
            let db_config_converted = crate::database::DatabaseConfig {
                host: db_config.host,
                port: db_config.port,
                user: db_config.user,
                password: db_config.password,
                database: db_config.database,
                ssl_mode: db_config.ssl_mode.unwrap_or_else(|| "prefer".to_string()),
                max_size: db_config.max_pool_size.unwrap_or(10),
            };
            let db_manager = DatabaseManager::new(db_config_converted)?;
            let api_generator = APIGenerator::new(openapi_config.spec)?;
            Some(Arc::new(CRUDHandler::new(db_manager, api_generator)))
        } else {
            None
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
