//! Database operations and connection management

use deadpool_postgres::{Pool, Runtime};
use serde_json::Value;
use std::collections::HashMap;
use tokio_postgres::NoTls;

#[derive(Debug)]
pub enum DatabaseError {
    PoolError(deadpool_postgres::CreatePoolError),
    SslMode(String),
    QueryError(tokio_postgres::Error),
    SerializationError(serde_json::Error),
    ConnectionError(deadpool::managed::PoolError<tokio_postgres::Error>),
    ValidationError(String),
}

impl std::fmt::Display for DatabaseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DatabaseError::PoolError(err) => write!(f, "Pool error: {err}"),
            DatabaseError::SslMode(mode) => write!(f, "Invalid SSL mode: {mode}"),
            DatabaseError::QueryError(err) => write!(f, "Query error: {err}"),
            DatabaseError::SerializationError(err) => write!(f, "Serialization error: {err}"),
            DatabaseError::ConnectionError(err) => write!(f, "Connection error: {err}"),
            DatabaseError::ValidationError(err) => write!(f, "Validation error: {err}"),
        }
    }
}

impl std::error::Error for DatabaseError {}

#[derive(Clone, Debug)]
pub struct DatabaseConfig {
    pub host: String,
    pub port: u16,
    pub user: String,
    pub password: String,
    pub database: String,
    pub ssl_mode: String,
    pub max_size: usize,
}

impl DatabaseConfig {
    pub fn create_pool(&self) -> Result<Pool, DatabaseError> {
        let mut cfg = deadpool_postgres::Config::new();

        cfg.host = Some(self.host.clone());
        cfg.port = Some(self.port);
        cfg.user = Some(self.user.clone());
        cfg.password = Some(self.password.clone());
        cfg.dbname = Some(self.database.clone());
        cfg.pool = Some(deadpool_postgres::PoolConfig {
            max_size: self.max_size,
            ..Default::default()
        });

        let ssl_mode = match self.ssl_mode.as_str() {
            "prefer" => deadpool_postgres::SslMode::Prefer,
            "require" => deadpool_postgres::SslMode::Require,
            "disable" => deadpool_postgres::SslMode::Disable,
            _ => return Err(DatabaseError::SslMode(self.ssl_mode.clone())),
        };

        cfg.ssl_mode = Some(ssl_mode);

        cfg.create_pool(Some(Runtime::Tokio1), NoTls)
            .map_err(DatabaseError::PoolError)
    }
}

#[derive(Debug)]
pub struct DatabaseManager {
    pool: Pool,
}

impl DatabaseManager {
    pub fn new(config: DatabaseConfig) -> Result<Self, DatabaseError> {
        let pool = config.create_pool()?;
        Ok(Self { pool })
    }

    /// Convert serde_json::Value to a PostgreSQL-compatible parameter
    fn value_to_sql_param(value: &Value) -> Box<dyn tokio_postgres::types::ToSql + Sync + Send> {
        match value {
            Value::String(s) => Box::new(s.clone()),
            Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    Box::new(i)
                } else if let Some(f) = n.as_f64() {
                    Box::new(f)
                } else {
                    Box::new(n.to_string())
                }
            }
            Value::Bool(b) => Box::new(*b),
            Value::Null => Box::new(Option::<String>::None),
            Value::Array(arr) => {
                // Convert array to JSON string
                Box::new(serde_json::to_string(arr).unwrap_or_else(|_| "[]".to_string()))
            }
            Value::Object(obj) => {
                // Convert object to JSON string
                Box::new(serde_json::to_string(obj).unwrap_or_else(|_| "{}".to_string()))
            }
        }
    }

    /// Execute a SELECT query and return results as JSON
    pub async fn select(
        &self,
        table: &str,
        columns: Option<Vec<String>>,
        where_clause: Option<HashMap<String, Value>>,
        limit: Option<u32>,
        offset: Option<u32>,
    ) -> Result<Vec<Value>, DatabaseError> {
        let mut query = String::new();
        let mut params: Vec<Box<dyn tokio_postgres::types::ToSql + Sync + Send>> = Vec::new();
        let mut param_count = 1;

        // Build SELECT clause
        let column_list = match columns {
            Some(cols) => cols.join(", "),
            None => "*".to_string(),
        };
        query.push_str(&format!("SELECT {} FROM {}", column_list, table));

        // Build WHERE clause
        if let Some(conditions) = where_clause {
            if !conditions.is_empty() {
                query.push_str(" WHERE ");
                let mut conditions_vec: Vec<String> = Vec::new();
                
                for (key, value) in conditions {
                    conditions_vec.push(format!("{} = ${}", key, param_count));
                    params.push(Self::value_to_sql_param(&value));
                    param_count += 1;
                }
                
                query.push_str(&conditions_vec.join(" AND "));
            }
        }

        // Add LIMIT and OFFSET
        if let Some(limit_val) = limit {
            query.push_str(&format!(" LIMIT ${}", param_count));
            params.push(Box::new(limit_val as i32));
            param_count += 1;
        }

        if let Some(offset_val) = offset {
            query.push_str(&format!(" OFFSET ${}", param_count));
            params.push(Box::new(offset_val as i32));
        }

        let client = self.pool.get().await.map_err(DatabaseError::ConnectionError)?;
        let rows = client.query(&query, &[]).await.map_err(DatabaseError::QueryError)?;

        let mut results = Vec::new();
        for row in rows {
            let mut row_obj = serde_json::Map::new();
            for (i, column) in row.columns().iter().enumerate() {
                let column_name = column.name();
                let value: Value = match column.type_().name() {
                    "int4" | "int8" => {
                        if let Ok(val) = row.try_get::<_, i32>(i) {
                            Value::Number(serde_json::Number::from(val))
                        } else if let Ok(val) = row.try_get::<_, i64>(i) {
                            Value::Number(serde_json::Number::from(val))
                        } else {
                            Value::Null
                        }
                    }
                    "text" | "varchar" => {
                        if let Ok(val) = row.try_get::<_, String>(i) {
                            Value::String(val)
                        } else {
                            Value::Null
                        }
                    }
                    "bool" => {
                        if let Ok(val) = row.try_get::<_, bool>(i) {
                            Value::Bool(val)
                        } else {
                            Value::Null
                        }
                    }
                    "json" | "jsonb" => {
                        if let Ok(val) = row.try_get::<_, String>(i) {
                            match serde_json::from_str::<Value>(&val) {
                                Ok(json_val) => json_val,
                                Err(_) => Value::String(val),
                            }
                        } else {
                            Value::Null
                        }
                    }
                    _ => Value::Null,
                };
                row_obj.insert(column_name.to_string(), value);
            }
            results.push(Value::Object(row_obj));
        }

        Ok(results)
    }

    /// Insert a new record and return the created record
    pub async fn insert(
        &self,
        table: &str,
        data: HashMap<String, Value>,
    ) -> Result<Value, DatabaseError> {
        if data.is_empty() {
            return Err(DatabaseError::ValidationError("No data provided for insert".to_string()));
        }

        let columns: Vec<String> = data.keys().cloned().collect();
        let placeholders: Vec<String> = (1..=data.len())
            .map(|i| format!("${}", i))
            .collect();

        let query = format!(
            "INSERT INTO {} ({}) VALUES ({}) RETURNING *",
            table,
            columns.join(", "),
            placeholders.join(", ")
        );

        let _values: Vec<Box<dyn tokio_postgres::types::ToSql + Sync + Send>> = data
            .values()
            .map(|v| Self::value_to_sql_param(v))
            .collect();

        let client = self.pool.get().await.map_err(DatabaseError::ConnectionError)?;
        let rows = client.query(&query, &[]).await.map_err(DatabaseError::QueryError)?;

        if let Some(row) = rows.first() {
            let mut row_obj = serde_json::Map::new();
            for (i, column) in row.columns().iter().enumerate() {
                let column_name = column.name();
                let value: Value = match column.type_().name() {
                    "int4" | "int8" => {
                        if let Ok(val) = row.try_get::<_, i32>(i) {
                            Value::Number(serde_json::Number::from(val))
                        } else if let Ok(val) = row.try_get::<_, i64>(i) {
                            Value::Number(serde_json::Number::from(val))
                        } else {
                            Value::Null
                        }
                    }
                    "text" | "varchar" => {
                        if let Ok(val) = row.try_get::<_, String>(i) {
                            Value::String(val)
                        } else {
                            Value::Null
                        }
                    }
                    "bool" => {
                        if let Ok(val) = row.try_get::<_, bool>(i) {
                            Value::Bool(val)
                        } else {
                            Value::Null
                        }
                    }
                    "json" | "jsonb" => {
                        if let Ok(val) = row.try_get::<_, String>(i) {
                            match serde_json::from_str::<Value>(&val) {
                                Ok(json_val) => json_val,
                                Err(_) => Value::String(val),
                            }
                        } else {
                            Value::Null
                        }
                    }
                    _ => Value::Null,
                };
                row_obj.insert(column_name.to_string(), value);
            }
            Ok(Value::Object(row_obj))
        } else {
            Err(DatabaseError::ValidationError("No data returned from insert".to_string()))
        }
    }

    /// Update records and return the updated record
    pub async fn update(
        &self,
        table: &str,
        data: HashMap<String, Value>,
        where_clause: HashMap<String, Value>,
    ) -> Result<Value, DatabaseError> {
        if data.is_empty() {
            return Err(DatabaseError::ValidationError("No data provided for update".to_string()));
        }

        if where_clause.is_empty() {
            return Err(DatabaseError::ValidationError("WHERE clause is required for update".to_string()));
        }

        let mut param_count = 1;
        let mut set_clauses: Vec<String> = Vec::new();
        let mut params: Vec<Box<dyn tokio_postgres::types::ToSql + Sync + Send>> = Vec::new();

        // Build SET clause
        for (key, value) in data {
            set_clauses.push(format!("{} = ${}", key, param_count));
            params.push(Self::value_to_sql_param(&value));
            param_count += 1;
        }

        // Build WHERE clause
        let mut where_clauses: Vec<String> = Vec::new();
        for (key, value) in where_clause {
            where_clauses.push(format!("{} = ${}", key, param_count));
            params.push(Self::value_to_sql_param(&value));
            param_count += 1;
        }

        let query = format!(
            "UPDATE {} SET {} WHERE {} RETURNING *",
            table,
            set_clauses.join(", "),
            where_clauses.join(" AND ")
        );

        let client = self.pool.get().await.map_err(DatabaseError::ConnectionError)?;
        let rows = client.query(&query, &[]).await.map_err(DatabaseError::QueryError)?;

        if let Some(row) = rows.first() {
            let mut row_obj = serde_json::Map::new();
            for (i, column) in row.columns().iter().enumerate() {
                let column_name = column.name();
                let value: Value = match column.type_().name() {
                    "int4" | "int8" => {
                        if let Ok(val) = row.try_get::<_, i32>(i) {
                            Value::Number(serde_json::Number::from(val))
                        } else if let Ok(val) = row.try_get::<_, i64>(i) {
                            Value::Number(serde_json::Number::from(val))
                        } else {
                            Value::Null
                        }
                    }
                    "text" | "varchar" => {
                        if let Ok(val) = row.try_get::<_, String>(i) {
                            Value::String(val)
                        } else {
                            Value::Null
                        }
                    }
                    "bool" => {
                        if let Ok(val) = row.try_get::<_, bool>(i) {
                            Value::Bool(val)
                        } else {
                            Value::Null
                        }
                    }
                    "json" | "jsonb" => {
                        if let Ok(val) = row.try_get::<_, String>(i) {
                            match serde_json::from_str::<Value>(&val) {
                                Ok(json_val) => json_val,
                                Err(_) => Value::String(val),
                            }
                        } else {
                            Value::Null
                        }
                    }
                    _ => Value::Null,
                };
                row_obj.insert(column_name.to_string(), value);
            }
            Ok(Value::Object(row_obj))
        } else {
            Err(DatabaseError::ValidationError("No data returned from update".to_string()))
        }
    }

    /// Delete records and return the number of affected rows
    pub async fn delete(
        &self,
        table: &str,
        where_clause: HashMap<String, Value>,
    ) -> Result<u64, DatabaseError> {
        if where_clause.is_empty() {
            return Err(DatabaseError::ValidationError("WHERE clause is required for delete".to_string()));
        }

        let mut param_count = 1;
        let mut where_clauses: Vec<String> = Vec::new();
        let mut params: Vec<Box<dyn tokio_postgres::types::ToSql + Sync + Send>> = Vec::new();

        // Build WHERE clause
        for (key, value) in where_clause {
            where_clauses.push(format!("{} = ${}", key, param_count));
            params.push(Self::value_to_sql_param(&value));
            param_count += 1;
        }

        let query = format!(
            "DELETE FROM {} WHERE {}",
            table,
            where_clauses.join(" AND ")
        );

        let client = self.pool.get().await.map_err(DatabaseError::ConnectionError)?;
        let result = client.execute(&query, &[]).await.map_err(DatabaseError::QueryError)?;

        Ok(result)
    }
}
