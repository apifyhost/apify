//! API generation based on OpenAPI specifications

use crate::schema_generator::TableSchema;
use regex::Regex;
use serde_json::Value;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct APIGenerator {
    spec: Value,
    route_patterns: Vec<RoutePattern>,
    table_schemas: HashMap<String, TableSchema>,
}

#[derive(Debug, Clone)]
pub struct RoutePattern {
    pub path_pattern: String,
    pub regex: Regex,
    pub param_names: Vec<String>,
    pub methods: Vec<String>,
    pub operation_type: OperationType,
    pub table_name: String,
}

#[derive(Debug, Clone)]
pub enum OperationType {
    List,   // GET /table
    Get,    // GET /table/{id}
    Create, // POST /table
    Update, // PUT /table/{id}
    Delete, // DELETE /table/{id}
}

impl APIGenerator {
    pub fn new(
        spec: Value,
        schemas: Vec<TableSchema>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let route_patterns = Self::build_route_patterns(&spec)?;

        for pattern in &route_patterns {
            eprintln!(
                "Debug: APIGenerator registered pattern: {} {:?}",
                pattern.path_pattern, pattern.methods
            );
        }

        let mut table_schemas = HashMap::new();
        for schema in schemas {
            table_schemas.insert(schema.table_name.clone(), schema);
        }

        Ok(Self {
            spec,
            route_patterns,
            table_schemas,
        })
    }

    pub fn get_table_schema(&self, table_name: &str) -> Option<&TableSchema> {
        self.table_schemas.get(table_name)
    }

    fn build_route_patterns(
        spec: &Value,
    ) -> Result<Vec<RoutePattern>, Box<dyn std::error::Error + Send + Sync>> {
        let mut patterns = Vec::new();

        if let Some(paths) = spec.get("paths").and_then(|p| p.as_object()) {
            for (path, path_item) in paths.iter() {
                if let Some(path_obj) = path_item.as_object() {
                    // Default table name from path (e.g., "/users" -> "users")
                    let default_table_name = Self::extract_table_name(path);

                    for (method, operation) in path_obj.iter() {
                        if let Some(op_obj) = operation.as_object() {
                            // Check for x-table-name in operation, otherwise use default
                            let table_name = op_obj
                                .get("x-table-name")
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string())
                                .unwrap_or_else(|| default_table_name.clone());

                            let operation_type = Self::determine_operation_type(method, path);
                            let regex = Self::build_regex_from_openapi_path(path)?;
                            let param_names = Self::extract_param_names_from_openapi(path);

                            patterns.push(RoutePattern {
                                path_pattern: path.to_string(),
                                regex,
                                param_names,
                                methods: vec![method.to_uppercase()],
                                operation_type,
                                table_name,
                            });
                        }
                    }
                }
            }
        }

        Ok(patterns)
    }

    fn extract_table_name(path: &str) -> String {
        // Extract table name from path like "/users" or "/users/{id}"
        let segments: Vec<&str> = path.split('/').collect();
        if segments.len() >= 2 {
            segments[1].to_string()
        } else {
            "unknown".to_string()
        }
    }

    fn determine_operation_type(method: &str, path: &str) -> OperationType {
        match method.to_lowercase().as_str() {
            "get" => {
                if path.contains('{') {
                    OperationType::Get
                } else {
                    OperationType::List
                }
            }
            "post" => OperationType::Create,
            "put" | "patch" => OperationType::Update,
            "delete" => OperationType::Delete,
            _ => OperationType::List,
        }
    }

    fn build_regex_from_openapi_path(openapi_path: &str) -> Result<Regex, regex::Error> {
        let mut regex_pattern = "^".to_string();

        for segment in openapi_path.split('/') {
            if segment.is_empty() {
                continue;
            }

            regex_pattern.push('/');

            // Check if segment contains OpenAPI parameter {param}
            if segment.starts_with('{') && segment.ends_with('}') {
                // Parameter segment - match any non-slash characters
                regex_pattern.push_str("([^/]+)");
            } else {
                // Static segment - escape special regex characters
                regex_pattern.push_str(&regex::escape(segment));
            }
        }

        regex_pattern.push('$');
        Regex::new(&regex_pattern)
    }

    fn extract_param_names_from_openapi(openapi_path: &str) -> Vec<String> {
        openapi_path
            .split('/')
            .filter_map(|segment| {
                if segment.starts_with('{') && segment.ends_with('}') {
                    // Remove the braces and return parameter name
                    Some(segment[1..segment.len() - 1].to_string())
                } else {
                    None
                }
            })
            .collect()
    }

    /// Match a request path and method to determine the operation
    pub fn match_operation(&self, method: &str, path: &str) -> Option<RoutePattern> {
        let method_upper = method.to_uppercase();
        eprintln!(
            "Debug: match_operation called for {} {}",
            method_upper, path
        );
        let operation = self.route_patterns.iter().find(|pattern| {
            let matched = pattern.regex.is_match(path) && pattern.methods.contains(&method_upper);
            if matched {
                eprintln!(
                    "Debug: Matched route: {} for path: {}",
                    pattern.path_pattern, path
                );
            }
            matched
        });
        operation.cloned()
    }

    /// Extract path parameters from a matched route
    pub fn extract_path_params(
        &self,
        pattern: &RoutePattern,
        path: &str,
    ) -> HashMap<String, String> {
        let mut params = HashMap::new();

        if let Some(captures) = pattern.regex.captures(path) {
            for (i, param_name) in pattern.param_names.iter().enumerate() {
                if let Some(capture) = captures.get(i + 1) {
                    params.insert(param_name.clone(), capture.as_str().to_string());
                }
            }
        }

        params
    }

    /// Get the OpenAPI specification
    pub fn get_spec(&self) -> &Value {
        &self.spec
    }

    /// Get all route patterns
    pub fn get_route_patterns(&self) -> &Vec<RoutePattern> {
        &self.route_patterns
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_table_name() {
        assert_eq!(APIGenerator::extract_table_name("/users"), "users");
        assert_eq!(APIGenerator::extract_table_name("/users/{id}"), "users");
        assert_eq!(APIGenerator::extract_table_name("/api/v1/users"), "api");
    }

    #[test]
    fn test_determine_operation_type() {
        assert!(matches!(
            APIGenerator::determine_operation_type("get", "/users"),
            OperationType::List
        ));
        assert!(matches!(
            APIGenerator::determine_operation_type("get", "/users/{id}"),
            OperationType::Get
        ));
        assert!(matches!(
            APIGenerator::determine_operation_type("post", "/users"),
            OperationType::Create
        ));
        assert!(matches!(
            APIGenerator::determine_operation_type("put", "/users/{id}"),
            OperationType::Update
        ));
        assert!(matches!(
            APIGenerator::determine_operation_type("delete", "/users/{id}"),
            OperationType::Delete
        ));
    }

    #[test]
    fn test_build_regex_from_openapi_path() {
        let regex = APIGenerator::build_regex_from_openapi_path("/users/{id}").unwrap();
        assert!(regex.is_match("/users/123"));
        assert!(regex.is_match("/users/abc"));
        assert!(!regex.is_match("/users/"));
        assert!(!regex.is_match("/users/123/posts"));

        let regex2 =
            APIGenerator::build_regex_from_openapi_path("/users/{userId}/posts/{postId}").unwrap();
        assert!(regex2.is_match("/users/john/posts/42"));
        assert!(!regex2.is_match("/users/john/posts"));
    }

    #[test]
    fn test_extract_param_names_from_openapi() {
        let params =
            APIGenerator::extract_param_names_from_openapi("/users/{userId}/posts/{postId}");
        assert_eq!(params, vec!["userId", "postId"]);

        let params2 = APIGenerator::extract_param_names_from_openapi("/users/{id}");
        assert_eq!(params2, vec!["id"]);

        let params3 = APIGenerator::extract_param_names_from_openapi("/users/static/path");
        assert_eq!(params3, Vec::<String>::new());
    }
}
