//! Application state management and route matching logic

use super::api_generator::APIGenerator;
use super::config::{
    Authenticator, ConsumerConfig, DatabaseSettings, MatchRule, ModulesConfig, OidcConfig,
    OpenAPIConfig, RouteConfig,
};
use super::crud_handler::CRUDHandler;
use super::database::DatabaseManager;
use super::hyper::Method;
use super::schema_generator::SchemaGenerator;
use std::collections::HashMap;
use std::sync::Arc;

/// Configuration for a single OpenAPI spec within AppState
#[derive(Clone)]
pub struct OpenApiStateConfig {
    pub config: OpenAPIConfig,
    pub modules: Option<ModulesConfig>,
    pub datasource: Option<String>,
    pub access_log: Option<crate::config::AccessLogConfig>,
    pub listeners: Option<Vec<String>>,
}

/// Configuration for creating AppState
pub struct AppStateConfig {
    pub routes: Option<Vec<RouteConfig>>,
    pub datasources: Option<HashMap<String, DatabaseSettings>>,
    pub openapi_configs: Vec<OpenApiStateConfig>,
    pub listener_modules: Option<ModulesConfig>,
    pub auth_config: Option<Vec<Authenticator>>,
    pub public_url: Option<String>,
    pub access_log_config: Option<crate::config::AccessLogConfig>,
    pub control_plane_db: Option<DatabaseManager>,
    pub control_plane_config: Option<crate::config::ControlPlaneConfig>,
}

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
    pub oidc_providers: HashMap<String, OidcConfig>, // name -> provider config
    pub auth_config: Option<Vec<Authenticator>>, // Full auth configuration
    pub control_plane_db: Option<DatabaseManager>, // Database for control plane
    pub control_plane_config: Option<crate::config::ControlPlaneConfig>, // Config for control plane
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
            oidc_providers: HashMap::new(),
            auth_config: None,
            control_plane_db: None,
            control_plane_config: None,
        }
    }

    /// Create new application state with CRUD support
    pub async fn new_with_crud(
        config: AppStateConfig,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let routes = config.routes.unwrap_or_default();
        let mut route_responses = HashMap::new();
        // Generate fixed responses for each route (hello + route name)
        for route in &routes {
            route_responses.insert(route.name.clone(), format!("hello {}", route.name));
        }

        // Build CRUD handler if OpenAPI configs exist and datasources are configured
        let crud_handler = if let Some(ds_map) = config.datasources.as_ref() {
            if !config.openapi_configs.is_empty() {
                tracing::debug!(
                    available_datasources = ?ds_map.keys().collect::<Vec<_>>(),
                    openapi_count = config.openapi_configs.len(),
                    "Building CRUD handler"
                );

                // Determine which datasource to use (from first API config or first available)
                let datasource_name = config
                    .openapi_configs
                    .first()
                    .and_then(|c| c.datasource.clone())
                    .or_else(|| ds_map.keys().next().cloned())
                    .ok_or("No datasource specified and none available")?;

                let ds = ds_map.get(&datasource_name).ok_or_else(|| {
                    format!("Datasource '{}' not found in config", datasource_name)
                })?;

                tracing::debug!(
                    datasource_name = %datasource_name,
                    driver = %ds.driver,
                    "Selected datasource for AppState"
                );

                // Build database URL
                let url = match ds.driver.as_str() {
                    "postgres" => {
                        format!(
                            "postgres://{}:{}@{}:{}/{}",
                            ds.user.as_deref().unwrap_or("postgres"),
                            ds.password.as_deref().unwrap_or(""),
                            ds.host.as_deref().unwrap_or("localhost"),
                            ds.port.unwrap_or(5432),
                            ds.database
                        )
                    }
                    "sqlite" => {
                        let path = &ds.database;
                        if path == ":memory:" {
                            "sqlite::memory:".to_string()
                        } else {
                            format!("sqlite:{}", path)
                        }
                    }
                    _ => return Err(format!("Unsupported database driver: {}", ds.driver).into()),
                };

                tracing::debug!(
                    url = %url,
                    max_size = ds.max_pool_size.unwrap_or(10),
                    "Constructed database URL"
                );

                let max_size = ds.max_pool_size.unwrap_or(10) as u32;
                let db_cfg = crate::database::DatabaseRuntimeConfig {
                    driver: ds.driver.clone(),
                    url,
                    max_size,
                };

                let db_manager = DatabaseManager::new(db_cfg).await?;

                // Extract table schemas from all OpenAPI specs
                let mut all_schemas = Vec::new();
                tracing::info!(
                    config_count = config.openapi_configs.len(),
                    "Extracting schemas from OpenAPI configs"
                );
                for (i, api_config) in config.openapi_configs.iter().enumerate() {
                    tracing::info!(index = i, "Extracting from OpenAPI config");

                    match SchemaGenerator::extract_schemas_from_openapi(
                        &api_config.config.openapi.spec,
                    ) {
                        Ok(schemas) => {
                            tracing::info!(
                                index = i,
                                schema_count = schemas.len(),
                                "Found schemas"
                            );
                            for schema in &schemas {
                                tracing::info!(
                                    table = %schema.table_name,
                                    columns = schema.columns.len(),
                                    relations = schema.relations.len(),
                                    "Schema details"
                                );
                            }
                            all_schemas.extend(schemas);
                        }
                        Err(e) => {
                            tracing::error!(index = i, error = %e, "ERROR extracting schemas");
                            return Err(e);
                        }
                    }
                }

                // Always initialize schema if schemas are defined (removed opt-in gating)
                if !all_schemas.is_empty() {
                    tracing::info!(
                        datasource = %datasource_name,
                        schema_count = all_schemas.len(),
                        "Initializing database with table schemas"
                    );
                    tracing::info!("Calling db_manager.initialize_schema...");
                    db_manager.initialize_schema(all_schemas.clone()).await?;
                    tracing::info!("Database initialization complete");
                } else {
                    tracing::warn!("No table schemas found in OpenAPI configurations");
                }

                // Merge all OpenAPI specs into one - deep merge for paths
                let mut merged_spec = serde_json::Map::new();
                let mut merged_paths = serde_json::Map::new();

                for api_config in &config.openapi_configs {
                    if let Some(spec_obj) = api_config.config.openapi.spec.as_object() {
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

                // Inject servers if public_url is provided
                if let Some(url) = config.public_url {
                    merged_spec.insert("servers".to_string(), serde_json::json!([{ "url": url }]));
                }

                // Add merged paths to the spec
                merged_spec.insert("paths".to_string(), serde_json::Value::Object(merged_paths));

                let merged_value = serde_json::Value::Object(merged_spec);
                let api_generator = APIGenerator::new(merged_value.clone(), all_schemas)?;
                Some(Arc::new(CRUDHandler::new(db_manager, api_generator)))
            } else {
                None
            }
        } else {
            None
        };

        // Build listener-level fallback module registry
        let mut modules_registry = crate::modules::ModuleRegistry::new();

        // Add Access Log module (globally enabled by default)
        let request_logger =
            crate::modules::request_logger::RequestLogger::new(config.access_log_config);
        modules_registry = modules_registry.with(Arc::new(request_logger));

        if let Some(cfg) = config.listener_modules {
            modules_registry = apply_modules_cfg(modules_registry, cfg);
        }

        // Build per-route module registries from per-API modules
        let mut route_modules: HashMap<String, crate::modules::ModuleRegistry> = HashMap::new();
        for api_config in &config.openapi_configs {
            let mut reg = crate::modules::ModuleRegistry::new();

            // Apply per-API access log if configured
            if let Some(log_cfg) = &api_config.access_log {
                let logger =
                    crate::modules::request_logger::RequestLogger::new(Some(log_cfg.clone()));
                reg = reg.with(Arc::new(logger));
            }

            // Apply configured modules
            if let Some(cfg) = &api_config.modules {
                reg = apply_modules_cfg(reg, cfg.clone());
            }

            // Always enable request validation
            {
                tracing::info!("Enabling request validation for API");
                let validator_config = crate::modules::request_validator::RequestValidatorConfig {
                    openapi_spec: Some(api_config.config.openapi.spec.clone()),
                    ..Default::default()
                };
                let validator =
                    crate::modules::request_validator::RequestValidator::new(validator_config);
                reg = reg.with(Arc::new(validator));
            }

            if let Some(paths_obj) = api_config
                .config
                .openapi
                .spec
                .get("paths")
                .and_then(|v| v.as_object())
            {
                for (path_key, _value) in paths_obj.iter() {
                    // Assign same registry for all paths in this API
                    route_modules.insert(path_key.clone(), reg.clone());
                }
            }
        }

        // Build per-operation module registries from OpenAPI (legacy x-modules + security schemes)
        let mut operation_modules: HashMap<String, crate::modules::ModuleRegistry> = HashMap::new();
        if let Some(ch) = &crud_handler {
            let spec = ch.api_generator.get_spec();

            // Parse global security (applies if operation has no local security)
            let mut global_access: Vec<String> = Vec::new();
            if let Some(sec_arr) = spec.get("security").and_then(|v| v.as_array()) {
                for req in sec_arr.iter().filter_map(|v| v.as_object()) {
                    if req.contains_key("ApiKeyAuth") {
                        global_access.push("key_auth".to_string());
                    }
                    if req.contains_key("BearerAuth") || req.contains_key("OpenID") {
                        global_access.push("oauth".to_string());
                    }
                }
            }
            // Deduplicate
            global_access.sort();
            global_access.dedup();

            if let Some(paths_obj) = spec.get("paths").and_then(|v| v.as_object()) {
                for (path_key, path_item) in paths_obj.iter() {
                    if let Some(po) = path_item.as_object() {
                        for method in [
                            "get", "post", "put", "patch", "delete", "head", "options", "trace",
                        ]
                        .iter()
                        {
                            if let Some(op) = po.get(*method) {
                                let mut cfg: ModulesConfig = ModulesConfig::default();

                                // 1. Legacy x-modules extension
                                if let Some(xmods) = op.get("x-modules")
                                    && let Some(parsed) = modules_from_value(xmods)
                                {
                                    cfg = parsed;
                                }

                                // 2. Security requirement objects (operation-level overrides global)
                                let mut access_from_security: Vec<String> = Vec::new();
                                if let Some(sec_arr) = op.get("security").and_then(|v| v.as_array())
                                {
                                    for req in sec_arr.iter().filter_map(|v| v.as_object()) {
                                        if req.contains_key("ApiKeyAuth") {
                                            access_from_security.push("key_auth".to_string());
                                        }
                                        if req.contains_key("BearerAuth")
                                            || req.contains_key("OpenID")
                                        {
                                            access_from_security.push("oauth".to_string());
                                        }
                                    }
                                } else {
                                    // Use global security if local absent
                                    access_from_security.extend(global_access.clone());
                                }
                                access_from_security.sort();
                                access_from_security.dedup();
                                if !access_from_security.is_empty() {
                                    // Merge with any existing access modules from legacy extension
                                    let mut merged: Vec<String> = cfg.access.unwrap_or_default();
                                    merged.extend(access_from_security);
                                    merged.sort();
                                    merged.dedup();
                                    cfg.access = Some(merged);
                                }

                                // Only create registry if we have at least one module configured
                                if cfg.access.is_some() || cfg.rewrite.is_some() {
                                    let reg = apply_modules_cfg(
                                        crate::modules::ModuleRegistry::new(),
                                        cfg,
                                    );
                                    let key = format!("{} {}", method.to_uppercase(), path_key);
                                    operation_modules.insert(key, reg);
                                }
                            }
                        }
                    }
                }
            }
        }

        // Build consumers maps and OIDC providers from AuthConfig
        let mut consumers_map = HashMap::new();
        let mut key_map = HashMap::new();
        let mut oidc_map = HashMap::new();

        // Move auth_config out to avoid partial move issues
        let auth_config = config.auth_config;

        if let Some(ref authenticators) = auth_config {
            for authenticator in authenticators {
                match authenticator {
                    Authenticator::ApiKey(api_key_auth) => {
                        if api_key_auth.enabled.unwrap_or(true) {
                            tracing::info!(name = %api_key_auth.name, "Loading API Key authenticator");
                            for c in &api_key_auth.config.consumers {
                                for k in &c.keys {
                                    key_map.insert(k.clone(), c.name.clone());
                                }
                                consumers_map.insert(c.name.clone(), c.clone());
                            }
                        }
                    }
                    Authenticator::Oidc(oidc_auth) => {
                        if oidc_auth.enabled.unwrap_or(true) {
                            tracing::info!(
                                name = %oidc_auth.name,
                                issuer = %oidc_auth.config.issuer,
                                "Loading OIDC authenticator"
                            );
                            oidc_map.insert(oidc_auth.name.clone(), oidc_auth.config.clone());
                        }
                    }
                }
            }
        }

        if oidc_map.is_empty() {
            tracing::debug!("No OIDC providers configured");
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
            oidc_providers: oidc_map,
            auth_config,
            control_plane_db: config.control_plane_db,
            control_plane_config: config.control_plane_config,
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
                "oauth" => {
                    reg = reg.with(Arc::new(crate::modules::oauth::OAuthModule::new(
                        "default".to_string(),
                    )));
                }
                _ => tracing::warn!("Unknown access module: {}", name),
            }
        }
    }
    // Rewrite modules placeholder
    if let Some(list) = cfg.rewrite {
        for name in list {
            tracing::warn!("Unknown rewrite module (not implemented yet): {}", name);
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
}
