//! Database facade delegating to backend implementations in modules/.

use crate::schema_generator::TableSchema;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Debug)]
pub enum DatabaseError {
    PoolError(sqlx::Error),
    QueryError(sqlx::Error),
    ValidationError(String),
}

impl std::fmt::Display for DatabaseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DatabaseError::PoolError(err) => write!(f, "Pool error: {err}"),
            DatabaseError::QueryError(err) => write!(f, "Query error: {err}"),
            DatabaseError::ValidationError(err) => write!(f, "Validation error: {err}"),
        }
    }
}

impl std::error::Error for DatabaseError {}

#[derive(Clone, Debug)]
pub struct DatabaseRuntimeConfig {
    pub driver: String, // "sqlite" or "postgres"
    pub url: String,
    pub max_size: u32,
}

impl DatabaseRuntimeConfig {
    pub fn sqlite_default() -> Self {
        // Allow overriding DB url via APIFY_DB_URL (useful in tests)
        if let Ok(mut url) = std::env::var("APIFY_DB_URL") {
            let driver = if url.starts_with("postgres://") { "postgres".to_string() } else { "sqlite".to_string() };
            if driver == "sqlite" && !url.starts_with("sqlite:") { url = format!("sqlite:{}", url); }
            eprintln!("Using database URL from env: {}", url);
            return Self { driver, url, max_size: 10 };
        }
        // Default: file-based sqlite under current directory
        let db_path = std::env::current_dir()
            .map(|d| d.join("apify.sqlite").to_string_lossy().to_string())
            .unwrap_or_else(|_| "./apify.sqlite".to_string());
        let url = format!("sqlite:{}", db_path);
        eprintln!("Using SQLite database file: {}", url);
        Self { driver: "sqlite".into(), url, max_size: 10 }
    }
}

pub trait DatabaseBackend: Send + Sync {
    fn initialize_schema<'a>(&'a self, table_schemas: Vec<TableSchema>) -> 
        core::pin::Pin<Box<dyn core::future::Future<Output = Result<(), DatabaseError>> + Send + 'a>>;
    fn select<'a>(
        &'a self,
        table: &'a str,
        columns: Option<Vec<String>>,
        where_clause: Option<HashMap<String, Value>>,
        limit: Option<u32>,
        offset: Option<u32>,
    ) -> core::pin::Pin<Box<dyn core::future::Future<Output = Result<Vec<Value>, DatabaseError>> + Send + 'a>>;
    fn insert<'a>(
        &'a self,
        table: &'a str,
        data: HashMap<String, Value>,
    ) -> core::pin::Pin<Box<dyn core::future::Future<Output = Result<Value, DatabaseError>> + Send + 'a>>;
    fn update<'a>(
        &'a self,
        table: &'a str,
        data: HashMap<String, Value>,
        where_clause: HashMap<String, Value>,
    ) -> core::pin::Pin<Box<dyn core::future::Future<Output = Result<Value, DatabaseError>> + Send + 'a>>;
    fn delete<'a>(
        &'a self,
        table: &'a str,
        where_clause: HashMap<String, Value>,
    ) -> core::pin::Pin<Box<dyn core::future::Future<Output = Result<u64, DatabaseError>> + Send + 'a>>;
}

#[derive(Clone)]
pub struct DatabaseManager {
    backend: Arc<dyn DatabaseBackend>,
}

impl std::fmt::Debug for DatabaseManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DatabaseManager")
            .field("backend", &"dyn DatabaseBackend")
            .finish()
    }
}

impl DatabaseManager {
    pub async fn new(config: DatabaseRuntimeConfig) -> Result<Self, DatabaseError> {
        let backend: Arc<dyn DatabaseBackend> = match config.driver.as_str() {
            "postgres" => {
                let b = crate::modules::postgres::PostgresBackend::connect(config).await?;
                Arc::new(b)
            }
            _ => {
                let b = crate::modules::sqlite::SqliteBackend::connect(config).await?;
                Arc::new(b)
            }
        };
        Ok(Self { backend })
    }

    /// Initialize database schema with dynamic table schemas (idempotent, per-table + per-index checks)
    pub async fn initialize_schema(
        &self,
        table_schemas: Vec<TableSchema>,
    ) -> Result<(), DatabaseError> {
        self.backend.initialize_schema(table_schemas).await
    }

    pub async fn select(
        &self,
        table: &str,
        columns: Option<Vec<String>>,
        where_clause: Option<HashMap<String, Value>>,
        limit: Option<u32>,
        offset: Option<u32>,
    ) -> Result<Vec<Value>, DatabaseError> {
        self.backend
            .select(table, columns, where_clause, limit, offset)
            .await
    }

    pub async fn insert(
        &self,
        table: &str,
        data: HashMap<String, Value>,
    ) -> Result<Value, DatabaseError> {
        if data.is_empty() {
            return Err(DatabaseError::ValidationError(
                "No data provided for insert".to_string(),
            ));
        }
        self.backend.insert(table, data).await
    }

    pub async fn update(
        &self,
        table: &str,
        data: HashMap<String, Value>,
        where_clause: HashMap<String, Value>,
    ) -> Result<Value, DatabaseError> {
        if data.is_empty() {
            return Err(DatabaseError::ValidationError(
                "No data provided for update".to_string(),
            ));
        }
        if where_clause.is_empty() {
            return Err(DatabaseError::ValidationError(
                "WHERE clause is required for update".to_string(),
            ));
        }
        self.backend.update(table, data, where_clause).await
    }

    pub async fn delete(
        &self,
        table: &str,
        where_clause: HashMap<String, Value>,
    ) -> Result<u64, DatabaseError> {
        self.backend.delete(table, where_clause).await
    }
}
