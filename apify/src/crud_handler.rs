//! CRUD operation handlers

use crate::api_generator::{APIGenerator, OperationType, RoutePattern};
use crate::database::DatabaseManager;
use serde_json::Value;
use std::collections::HashMap;

#[derive(Debug)]
pub enum CRUDError {
    DatabaseError(crate::database::DatabaseError),
    ValidationError(String),
    NotFoundError(String),
    InvalidParameterError(String),
}

impl std::fmt::Display for CRUDError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CRUDError::DatabaseError(err) => write!(f, "Database error: {err}"),
            CRUDError::ValidationError(err) => write!(f, "Validation error: {err}"),
            CRUDError::NotFoundError(err) => write!(f, "Not found: {err}"),
            CRUDError::InvalidParameterError(err) => write!(f, "Invalid parameter: {err}"),
        }
    }
}

impl std::error::Error for CRUDError {}

impl From<crate::database::DatabaseError> for CRUDError {
    fn from(err: crate::database::DatabaseError) -> Self {
        CRUDError::DatabaseError(err)
    }
}

#[derive(Debug)]
pub struct CRUDHandler {
    db_manager: DatabaseManager,
    api_generator: APIGenerator,
}

impl CRUDHandler {
    pub fn new(db_manager: DatabaseManager, api_generator: APIGenerator) -> Self {
        Self {
            db_manager,
            api_generator,
        }
    }

    /// Handle CRUD operations based on route pattern
    pub async fn handle_request(
        &self,
        method: &str,
        path: &str,
        path_params: HashMap<String, String>,
        query_params: HashMap<String, String>,
        body: Option<Value>,
    ) -> Result<Value, CRUDError> {
        // Find matching route pattern
        let pattern = self
            .api_generator
            .match_operation(method, path)
            .ok_or_else(|| {
                CRUDError::NotFoundError(format!("No matching route for {} {}", method, path))
            })?;

        match pattern.operation_type {
            OperationType::List => self.handle_list(pattern, query_params).await,
            OperationType::Get => self.handle_get(pattern, path_params).await,
            OperationType::Create => self.handle_create(pattern, body).await,
            OperationType::Update => self.handle_update(pattern, path_params, body).await,
            OperationType::Delete => self.handle_delete(pattern, path_params).await,
        }
    }

    /// Handle GET /table (list all records)
    async fn handle_list(
        &self,
        pattern: &RoutePattern,
        query_params: HashMap<String, String>,
    ) -> Result<Value, CRUDError> {
        let table = &pattern.table_name;

        // Extract pagination parameters
        let limit = query_params
            .get("limit")
            .and_then(|s| s.parse::<u32>().ok());
        let offset = query_params
            .get("offset")
            .and_then(|s| s.parse::<u32>().ok());

        // Extract filter parameters (exclude pagination params)
        let mut where_clause = HashMap::new();
        for (key, value) in query_params {
            if key != "limit" && key != "offset" {
                where_clause.insert(key, Value::String(value));
            }
        }

        let results = self
            .db_manager
            .select(
                table,
                None, // Select all columns
                if where_clause.is_empty() {
                    None
                } else {
                    Some(where_clause)
                },
                limit,
                offset,
            )
            .await?;

        Ok(Value::Array(results))
    }

    /// Handle GET /table/{id} (get single record)
    async fn handle_get(
        &self,
        pattern: &RoutePattern,
        path_params: HashMap<String, String>,
    ) -> Result<Value, CRUDError> {
        let table = &pattern.table_name;

        // Use the first path parameter as the primary key
        let id_param = path_params
            .keys()
            .next()
            .ok_or_else(|| CRUDError::InvalidParameterError("No ID parameter found".to_string()))?;
        let id_value = path_params.get(id_param).ok_or_else(|| {
            CRUDError::InvalidParameterError("ID parameter value not found".to_string())
        })?;

        let mut where_clause = HashMap::new();
        where_clause.insert(id_param.clone(), Value::String(id_value.clone()));

        let results = self
            .db_manager
            .select(
                table,
                None, // Select all columns
                Some(where_clause),
                Some(1), // Limit to 1 record
                None,
            )
            .await?;

        results.into_iter().next().ok_or_else(|| {
            CRUDError::NotFoundError(format!("Record with {} = {} not found", id_param, id_value))
        })
    }

    /// Handle POST /table (create new record)
    async fn handle_create(
        &self,
        pattern: &RoutePattern,
        body: Option<Value>,
    ) -> Result<Value, CRUDError> {
        let table = &pattern.table_name;

        let data =
            body.ok_or_else(|| CRUDError::ValidationError("Request body is required".to_string()))?;

        let data_map = match data {
            Value::Object(map) => map,
            _ => {
                return Err(CRUDError::ValidationError(
                    "Request body must be a JSON object".to_string(),
                ));
            }
        };

        // Convert serde_json::Map to HashMap<String, Value>
        let mut data_hashmap = HashMap::new();
        for (key, value) in data_map {
            data_hashmap.insert(key, value);
        }

        let result = self.db_manager.insert(table, data_hashmap).await?;
        Ok(result)
    }

    /// Handle PUT/PATCH /table/{id} (update record)
    async fn handle_update(
        &self,
        pattern: &RoutePattern,
        path_params: HashMap<String, String>,
        body: Option<Value>,
    ) -> Result<Value, CRUDError> {
        let table = &pattern.table_name;

        let data =
            body.ok_or_else(|| CRUDError::ValidationError("Request body is required".to_string()))?;

        let data_map = match data {
            Value::Object(map) => map,
            _ => {
                return Err(CRUDError::ValidationError(
                    "Request body must be a JSON object".to_string(),
                ));
            }
        };

        // Convert serde_json::Map to HashMap<String, Value>
        let mut data_hashmap = HashMap::new();
        for (key, value) in data_map {
            data_hashmap.insert(key, value);
        }

        // Use the first path parameter as the primary key for WHERE clause
        let id_param = path_params
            .keys()
            .next()
            .ok_or_else(|| CRUDError::InvalidParameterError("No ID parameter found".to_string()))?;
        let id_value = path_params.get(id_param).ok_or_else(|| {
            CRUDError::InvalidParameterError("ID parameter value not found".to_string())
        })?;

        let mut where_clause = HashMap::new();
        where_clause.insert(id_param.clone(), Value::String(id_value.clone()));

        let result = self
            .db_manager
            .update(table, data_hashmap, where_clause)
            .await?;
        Ok(result)
    }

    /// Handle DELETE /table/{id} (delete record)
    async fn handle_delete(
        &self,
        pattern: &RoutePattern,
        path_params: HashMap<String, String>,
    ) -> Result<Value, CRUDError> {
        let table = &pattern.table_name;

        // Use the first path parameter as the primary key for WHERE clause
        let id_param = path_params
            .keys()
            .next()
            .ok_or_else(|| CRUDError::InvalidParameterError("No ID parameter found".to_string()))?;
        let id_value = path_params.get(id_param).ok_or_else(|| {
            CRUDError::InvalidParameterError("ID parameter value not found".to_string())
        })?;

        let mut where_clause = HashMap::new();
        where_clause.insert(id_param.clone(), Value::String(id_value.clone()));

        let affected_rows = self.db_manager.delete(table, where_clause).await?;

        if affected_rows == 0 {
            return Err(CRUDError::NotFoundError(format!(
                "Record with {} = {} not found",
                id_param, id_value
            )));
        }

        // Return success response
        let mut response = HashMap::new();
        response.insert(
            "message".to_string(),
            Value::String("Record deleted successfully".to_string()),
        );
        response.insert(
            "affected_rows".to_string(),
            Value::Number(serde_json::Number::from(affected_rows)),
        );

        Ok(Value::Object(response.into_iter().collect()))
    }
}
