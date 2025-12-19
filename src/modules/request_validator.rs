//! Request validation module (BodyParse phase)
//! Validates request body size, structure, and parameters (query, header, path)

use crate::app_state::AppState;
use crate::hyper::StatusCode;
use crate::modules::{Module, ModuleOutcome, error_response};
use crate::phases::{Phase, RequestContext};
use jsonschema::JSONSchema;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;

/// Request validator configuration
pub struct RequestValidatorConfig {
    /// Maximum body size in bytes
    pub max_body_size: usize,
    /// Require JSON content-type header for JSON bodies
    pub enforce_content_type: bool,
    /// OpenAPI spec for schema validation
    pub openapi_spec: Option<Value>,
}

impl Default for RequestValidatorConfig {
    fn default() -> Self {
        Self {
            max_body_size: 1024 * 1024, // 1MB default
            enforce_content_type: true,
            openapi_spec: None,
        }
    }
}

struct ParameterValidator {
    name: String,
    location: String, // "query", "header", "path", "cookie"
    required: bool,
    schema: Option<JSONSchema>,
    type_hint: Option<String>,
}

struct RouteValidators {
    body_schema: Option<JSONSchema>,
    parameters: Vec<ParameterValidator>,
}

/// Request validation module
pub struct RequestValidator {
    config: RequestValidatorConfig,
    validators: HashMap<(String, String), RouteValidators>, // (method, path) -> validators
}

impl RequestValidator {
    pub fn new(config: RequestValidatorConfig) -> Self {
        let mut validators = HashMap::new();
        if let Some(ref spec) = config.openapi_spec {
            validators = Self::compile_validators(spec);
        }
        // Debug: log registered validators
        for key in validators.keys() {
            tracing::debug!("RequestValidator registered: {} {}", key.0, key.1);
        }
        Self { config, validators }
    }

    pub fn with_defaults() -> Self {
        Self::new(RequestValidatorConfig::default())
    }

    fn resolve_ref<'a>(spec: &'a Value, node: &'a Value) -> Option<&'a Value> {
        if let Some(ref_str) = node.get("$ref").and_then(|s| s.as_str())
            && ref_str.starts_with("#/")
        {
            let parts: Vec<&str> = ref_str[2..].split('/').collect();
            let mut current = spec;
            for part in parts {
                current = current.get(part)?;
            }
            return Some(current);
        }
        Some(node)
    }

    fn compile_validators(spec: &Value) -> HashMap<(String, String), RouteValidators> {
        let mut validators = HashMap::new();
        if let Some(paths) = spec.get("paths").and_then(|p| p.as_object()) {
            for (path, path_item) in paths {
                if let Some(path_obj) = path_item.as_object() {
                    // Path-level parameters
                    let mut path_params = Vec::new();
                    if let Some(params) = path_obj.get("parameters").and_then(|p| p.as_array()) {
                        for param in params {
                            if let Some(resolved) = Self::resolve_ref(spec, param) {
                                path_params.push(resolved);
                            }
                        }
                    }

                    for (method, operation) in path_obj {
                        if method == "parameters" {
                            continue;
                        }
                        let method_upper = method.to_uppercase();

                        if let Some(op_obj) = operation.as_object() {
                            let mut route_val = RouteValidators {
                                body_schema: None,
                                parameters: Vec::new(),
                            };

                            // Compile Body Schema
                            if let Some(body) = op_obj.get("requestBody")
                                && let Some(content) = body.get("content")
                                && let Some(json_media) = content.get("application/json")
                                && let Some(schema) = json_media.get("schema")
                            {
                                // Create a self-contained schema by adding components from the root spec
                                let mut schema_with_components = schema.clone();
                                if let Some(components) = spec.get("components")
                                    && let Some(obj) = schema_with_components.as_object_mut()
                                {
                                    obj.insert("components".to_string(), components.clone());
                                }

                                match JSONSchema::options().compile(&schema_with_components) {
                                    Ok(compiled) => {
                                        route_val.body_schema = Some(compiled);
                                    }
                                    Err(e) => {
                                        tracing::warn!(
                                            "Failed to compile body schema for {} {}: {}",
                                            method,
                                            path,
                                            e
                                        );
                                    }
                                }
                            }

                            // Compile Parameters
                            let mut op_params = path_params.clone();
                            if let Some(params) =
                                op_obj.get("parameters").and_then(|p| p.as_array())
                            {
                                for param in params {
                                    if let Some(resolved) = Self::resolve_ref(spec, param) {
                                        op_params.push(resolved);
                                    }
                                }
                            }

                            for param in op_params {
                                if let (Some(name), Some(location)) = (
                                    param.get("name").and_then(|s| s.as_str()),
                                    param.get("in").and_then(|s| s.as_str()),
                                ) {
                                    let required = param
                                        .get("required")
                                        .and_then(|b| b.as_bool())
                                        .unwrap_or(false);
                                    let mut validator = ParameterValidator {
                                        name: name.to_string(),
                                        location: location.to_string(),
                                        required,
                                        schema: None,
                                        type_hint: None,
                                    };

                                    if let Some(schema) = param.get("schema") {
                                        validator.type_hint = schema
                                            .get("type")
                                            .and_then(|s| s.as_str())
                                            .map(|s| s.to_string());
                                        match JSONSchema::options()
                                            .with_document("root.json".to_string(), spec.clone())
                                            .compile(schema)
                                        {
                                            Ok(compiled) => {
                                                validator.schema = Some(compiled);
                                            }
                                            Err(e) => {
                                                tracing::warn!(
                                                    "Failed to compile param schema for {} {} param {}: {}",
                                                    method,
                                                    path,
                                                    name,
                                                    e
                                                );
                                            }
                                        }
                                    }
                                    route_val.parameters.push(validator);
                                }
                            }

                            validators.insert((method_upper, path.clone()), route_val);
                        }
                    }
                }
            }
        }
        validators
    }
}

impl Module for RequestValidator {
    fn name(&self) -> &str {
        "request_validator"
    }

    fn phases(&self) -> &'static [Phase] {
        &[Phase::BodyParse]
    }

    fn run(&self, phase: Phase, ctx: &mut RequestContext, _state: &Arc<AppState>) -> ModuleOutcome {
        debug_assert_eq!(phase, Phase::BodyParse);

        // Check body size if body exists
        if let Some(ref body) = ctx.raw_body
            && body.len() > self.config.max_body_size
        {
            return ModuleOutcome::Respond(error_response(
                StatusCode::PAYLOAD_TOO_LARGE,
                &format!(
                    "Request body too large: {} bytes (max: {})",
                    body.len(),
                    self.config.max_body_size
                ),
            ));
        }

        // Enforce Content-Type header for JSON bodies
        if self.config.enforce_content_type && ctx.json_body.is_some() {
            if let Some(content_type) = ctx.headers.get("content-type") {
                let ct_str = content_type.to_str().unwrap_or("");
                if !ct_str.contains("application/json") {
                    return ModuleOutcome::Respond(error_response(
                        StatusCode::UNSUPPORTED_MEDIA_TYPE,
                        "Content-Type must be application/json for JSON bodies",
                    ));
                }
            } else {
                return ModuleOutcome::Respond(error_response(
                    StatusCode::BAD_REQUEST,
                    "Missing Content-Type header for JSON body",
                ));
            }
        }

        // Validate against OpenAPI schema if available
        if let Some(ref route) = ctx.matched_route {
            let key = (
                ctx.method.as_str().to_uppercase(),
                route.path_pattern.clone(),
            );
            if let Some(validators) = self.validators.get(&key) {
                // 1. Validate Body
                if let Some(ref schema) = validators.body_schema
                    && let Some(ref json_body) = ctx.json_body
                    && let Err(errors) = schema.validate(json_body)
                {
                    let error_msg = errors
                        .map(|e| format!("Body validation error: {}", e))
                        .collect::<Vec<_>>()
                        .join("; ");

                    tracing::warn!("Validation Error: {}", error_msg);
                    // Debug: print the schema being used (if possible, JSONSchema doesn't implement Debug nicely usually, but let's try printing the key)
                    tracing::debug!(
                        "Validation failed for route: {} {}",
                        ctx.method,
                        route.path_pattern
                    );

                    return ModuleOutcome::Respond(error_response(
                        StatusCode::BAD_REQUEST,
                        &error_msg,
                    ));
                }

                // 2. Validate Parameters
                for param in &validators.parameters {
                    let value_str = match param.location.as_str() {
                        "query" => ctx.query_params.get(&param.name).map(|s| s.as_str()),
                        "header" => ctx.headers.get(&param.name).and_then(|v| v.to_str().ok()),
                        "path" => ctx.path_params.get(&param.name).map(|s| s.as_str()),
                        _ => None,
                    };

                    if param.required && value_str.is_none() {
                        return ModuleOutcome::Respond(error_response(
                            StatusCode::BAD_REQUEST,
                            &format!(
                                "Missing required {} parameter: {}",
                                param.location, param.name
                            ),
                        ));
                    }

                    if let Some(val_str) = value_str
                        && let Some(ref schema) = param.schema
                    {
                        // Attempt type coercion for validation
                        let json_val = match param.type_hint.as_deref() {
                            Some("integer") => val_str
                                .parse::<i64>()
                                .map(Value::from)
                                .unwrap_or(Value::String(val_str.to_string())),
                            Some("number") => val_str
                                .parse::<f64>()
                                .map(Value::from)
                                .unwrap_or(Value::String(val_str.to_string())),
                            Some("boolean") => val_str
                                .parse::<bool>()
                                .map(Value::from)
                                .unwrap_or(Value::String(val_str.to_string())),
                            _ => Value::String(val_str.to_string()),
                        };

                        if let Err(errors) = schema.validate(&json_val) {
                            let error_msg = errors
                                .map(|e| {
                                    format!("Parameter '{}' validation error: {}", param.name, e)
                                })
                                .collect::<Vec<_>>()
                                .join("; ");

                            tracing::warn!("Validation Error: {}", error_msg);

                            return ModuleOutcome::Respond(error_response(
                                StatusCode::BAD_REQUEST,
                                &error_msg,
                            ));
                        }
                    }
                }
            }
        }

        ModuleOutcome::Continue
    }
}
