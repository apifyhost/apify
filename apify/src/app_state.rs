//! Application state management and route matching logic

use super::api_generator::APIGenerator;
use super::config::{
    ConsumerConfig, DatabaseConfig, MatchRule, ModulesConfig, OpenAPIConfig, RouteConfig,
};
use super::crud_handler::CRUDHandler;
use super::database::DatabaseManager;
use super::hyper::Method;
use super::schema_generator::SchemaGenerator;
use std::collections::HashMap;
use std::sync::Arc;

/// Shared application state (route configurations and CRUD handlers)
#[derive(Clone)]
pub struct AppState {
    routes: Vec<RouteConfig>,
    route_responses: HashMap<String, String>, // route name -> response string
    pub crud_handler: Option<Arc<CRUDHandler>>,
    pub modules: crate::modules::ModuleRegistry,
    pub route_modules: HashMap<String, crate::modules::ModuleRegistry>, // path_pattern -> modules
    pub operation_modules: HashMap<String, crate::modules::ModuleRegistry>, // "METHOD path_pattern" -> modules
    consumers: HashMap<String, ConsumerConfig>,                             // name -> config
    key_to_consumer: HashMap<String, String>, // api_key -> consumer name
    // database gating flags (like key_auth): operation > route > listener
    pub allow_database_listener: bool,
    pub route_allow_database: HashMap<String, bool>, // path_pattern -> allow database
    pub operation_allow_database: HashMap<String, bool>, // "METHOD path_pattern" -> allow database
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
            modules: Default::default(),
            route_modules: HashMap::new(),
            operation_modules: HashMap::new(),
            consumers: HashMap::new(),
            key_to_consumer: HashMap::new(),
            allow_database_listener: false,
            route_allow_database: HashMap::new(),
            operation_allow_database: HashMap::new(),
        }
    }

    /// Create new application state with CRUD support
    pub async fn new_with_crud(
        routes: Option<Vec<RouteConfig>>,
    database_config: Option<DatabaseConfig>,
        openapi_configs: Vec<(OpenAPIConfig, Option<ModulesConfig>)>,
        listener_modules: Option<ModulesConfig>,
        consumers: Vec<ConsumerConfig>,
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
                // Build runtime config from database_config if provided (shared across APIs)
                let db_cfg = if let Some(db_conf) = &database_config {
                    // Build URL from components (only postgres/sqlite supported)
                    let driver = db_conf.database.driver.clone().unwrap_or_else(|| "sqlite".into());
                    let max_size = db_conf.database.max_pool_size.unwrap_or(10) as u32;
                    let url = if driver == "postgres" {
                        format!(
                            "postgres://{}:{}@{}:{}/{}",
                            db_conf.database.user,
                            db_conf.database.password,
                            db_conf.database.host,
                            db_conf.database.port,
                            db_conf.database.database
                        )
                    } else {
                        // For sqlite treat 'database' as filename
                        let path = &db_conf.database.database;
                        if path == ":memory:" { "sqlite::memory:".into() } else { format!("sqlite:{}", path) }
                    };
                    crate::database::DatabaseRuntimeConfig { driver, url, max_size }
                } else {
                    crate::database::DatabaseRuntimeConfig::sqlite_default()
                };
                let db_manager = DatabaseManager::new(db_cfg).await?;

                // Extract table schemas from all OpenAPI specs
                let mut all_schemas = Vec::new();
                for (openapi_config, _) in &openapi_configs {
                    let schemas = SchemaGenerator::extract_schemas_from_openapi(
                        &openapi_config.openapi.spec,
                    )?;
                    all_schemas.extend(schemas);
                }

                // Initialize database schema with extracted table definitions
                // Only run schema initialization if database operations include "init_schemas" (opt-in)
                let should_init = database_config
                    .as_ref()
                    .and_then(|c| c.database.operations.as_ref())
                    .map(|ops| ops.iter().any(|o| o == "init_schemas"))
                    .unwrap_or(false);
                if should_init {
                    if !all_schemas.is_empty() {
                        eprintln!(
                            "Initializing database with {} table schemas",
                            all_schemas.len()
                        );
                        db_manager.initialize_schema(all_schemas).await?;
                    } else {
                        eprintln!("Warning: No table schemas found in OpenAPI configurations");
                    }
                } else {
                    eprintln!("Skipping schema initialization (init_schemas not in operations)");
                }

                // Merge all OpenAPI specs into one - deep merge for paths
                let mut merged_spec = serde_json::Map::new();
                let mut merged_paths = serde_json::Map::new();

                for (openapi_config, _) in &openapi_configs {
                    if let Some(spec_obj) = openapi_config.openapi.spec.as_object() {
                        for (key, value) in spec_obj {
                            if key == "paths" {
                                if let Some(paths_obj) = value.as_object() {
                                    for (path_key, path_value) in paths_obj {
                                        merged_paths.insert(path_key.clone(), path_value.clone());
                                    }
                                }
                            } else {
                                merged_spec.insert(key.clone(), value.clone());
                            }
                        }
                    }
                }

                // Add merged paths to the spec
                merged_spec.insert("paths".to_string(), serde_json::Value::Object(merged_paths));

                let merged_value = serde_json::Value::Object(merged_spec);
                let api_generator = APIGenerator::new(merged_value.clone())?;
                Some(Arc::new(CRUDHandler::new(db_manager, api_generator)))
            } else {
                None
            }
        };

        // Build listener-level fallback module registry and database flag
        let mut modules_registry = crate::modules::ModuleRegistry::new();
        let mut allow_database_listener = false;
        if let Some(cfg) = listener_modules {
            if cfg.access.as_ref().is_some_and(|v| v.iter().any(|m| m == "database")) {
                allow_database_listener = true;
            }
            modules_registry = apply_modules_cfg(modules_registry, cfg);
        }

        // Build per-route module registries from per-API modules, and db flags
        let mut route_modules: HashMap<String, crate::modules::ModuleRegistry> = HashMap::new();
        let mut route_allow_database: HashMap<String, bool> = HashMap::new();
        for (openapi_config, per_api_modules) in &openapi_configs {
            if let Some(cfg) = per_api_modules.clone() {
                let mut reg = crate::modules::ModuleRegistry::new();
                reg = apply_modules_cfg(reg, cfg);
                if let Some(paths_obj) = openapi_config
                    .openapi
                    .spec
                    .get("paths")
                    .and_then(|v| v.as_object())
                {
                    for (path_key, _value) in paths_obj.iter() {
                        // Assign same registry for all paths in this API
                        route_modules.insert(path_key.clone(), reg.clone());
                        // database allowed if per-api modules include it
                        let allow_db = per_api_modules
                            .as_ref()
                            .and_then(|c| c.access.as_ref())
                            .map(|v| v.iter().any(|m| m == "database"))
                            .unwrap_or(false);
                        route_allow_database.insert(path_key.clone(), allow_db);
                    }
                }
            }
        }

        // Build per-operation module registries from OpenAPI x-modules on operations
        let mut operation_modules: HashMap<String, crate::modules::ModuleRegistry> = HashMap::new();
        let mut operation_allow_database: HashMap<String, bool> = HashMap::new();
        if let Some(ch) = &crud_handler {
            let spec = ch.api_generator.get_spec();
            if let Some(paths_obj) = spec.get("paths").and_then(|v| v.as_object()) {
                for (path_key, path_item) in paths_obj.iter() {
                    if let Some(po) = path_item.as_object() {
                        for method in [
                            "get", "post", "put", "patch", "delete", "head", "options", "trace",
                        ]
                        .iter()
                        {
                            if let Some(op) = po.get(*method)
                                && let Some(xmods) = op.get("x-modules")
                                && let Some(cfg) = modules_from_value(xmods)
                            {
                                // compute allow_db before moving cfg
                                let allow_db = cfg
                                    .access
                                    .as_ref()
                                    .map(|v| v.iter().any(|m| m == "database"))
                                    .unwrap_or(false);
                                let reg =
                                    apply_modules_cfg(crate::modules::ModuleRegistry::new(), cfg);
                                let key = format!("{} {}", method.to_uppercase(), path_key);
                                operation_modules.insert(key.clone(), reg);
                                operation_allow_database.insert(key, allow_db);
                            }
                        }
                    }
                }
            }
        }

        // Build consumers maps
        let mut consumers_map = HashMap::new();
        let mut key_map = HashMap::new();
        for c in consumers {
            for k in &c.keys {
                key_map.insert(k.clone(), c.name.clone());
            }
            consumers_map.insert(c.name.clone(), c);
        }

        Ok(Self {
            routes,
            route_responses,
            crud_handler,
            modules: modules_registry,
            route_modules,
            operation_modules,
            consumers: consumers_map,
            key_to_consumer: key_map,
            allow_database_listener,
            route_allow_database,
            operation_allow_database,
        })
    }
}

/// Helper to apply ModulesConfig into a ModuleRegistry
fn apply_modules_cfg(
    mut reg: crate::modules::ModuleRegistry,
    cfg: ModulesConfig,
) -> crate::modules::ModuleRegistry {
    use std::sync::Arc;
    // Access modules
    if let Some(list) = cfg.access {
        for name in list {
            match name.as_str() {
                "key_auth" => {
                    reg = reg.with(Arc::new(crate::modules::key_auth::KeyAuthModule::new()));
                }
                // database is a gating flag only (no runtime module implementation required)
                "database" => {
                    // no-op: presence handled elsewhere for gating
                }
                _ => eprintln!("Unknown access module: {}", name),
            }
        }
    }
    // Rewrite modules placeholder
    if let Some(list) = cfg.rewrite {
        for name in list {
            eprintln!("Unknown rewrite module (not implemented yet): {}", name);
        }
    }
    reg
}

/// Parse a serde_json value into ModulesConfig if shape matches { access: [..], rewrite: [..] }
fn modules_from_value(v: &serde_json::Value) -> Option<ModulesConfig> {
    let mut cfg = ModulesConfig::default();
    if let Some(obj) = v.as_object() {
        if let Some(acc) = obj.get("access").and_then(|a| a.as_array()) {
            let list: Vec<String> = acc
                .iter()
                .filter_map(|x| x.as_str().map(|s| s.to_string()))
                .collect();
            if !list.is_empty() {
                cfg.access = Some(list);
            }
        }
        if let Some(rw) = obj.get("rewrite").and_then(|a| a.as_array()) {
            let list: Vec<String> = rw
                .iter()
                .filter_map(|x| x.as_str().map(|s| s.to_string()))
                .collect();
            if !list.is_empty() {
                cfg.rewrite = Some(list);
            }
        }
    }
    if cfg.access.is_some() || cfg.rewrite.is_some() {
        Some(cfg)
    } else {
        None
    }
}

impl AppState {
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

    pub fn lookup_consumer_by_key(&self, key: &str) -> Option<&ConsumerConfig> {
        self.key_to_consumer
            .get(key)
            .and_then(|name| self.consumers.get(name))
    }

    /// Resolve whether database is allowed for this request by precedence: operation > route > listener
    pub fn is_database_allowed(&self, method: &str, path_pattern: Option<&str>) -> bool {
        if let Some(p) = path_pattern {
            let op_key = format!("{} {}", method.to_uppercase(), p);
            if let Some(v) = self.operation_allow_database.get(&op_key) {
                return *v;
            }
            if let Some(v) = self.route_allow_database.get(p) {
                return *v;
            }
        }
        self.allow_database_listener
    }
}
