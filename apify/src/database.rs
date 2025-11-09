//! Database facade delegating to backend implementations in modules/.

use crate::schema_generator::TableSchema;
use serde_json::Value;
use std::collections::HashMap;

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

#[derive(Debug, Clone)]
pub enum DatabaseManager {
    Sqlite(crate::modules::sqlite::SqliteBackend),
    Postgres(crate::modules::postgres::PostgresBackend),
}

impl DatabaseManager {
    pub async fn new(config: DatabaseRuntimeConfig) -> Result<Self, DatabaseError> {
        match config.driver.as_str() {
            "postgres" => Ok(Self::Postgres(crate::modules::postgres::PostgresBackend::connect(config).await?)),
            _ => Ok(Self::Sqlite(crate::modules::sqlite::SqliteBackend::connect(config).await?)),
        }
    }

    /// Initialize database schema with dynamic table schemas (idempotent, per-table + per-index checks)
    pub async fn initialize_schema(
        &self,
        table_schemas: Vec<TableSchema>,
    ) -> Result<(), DatabaseError> {
        match self {
            DatabaseManager::Sqlite(b) => b.initialize_schema(table_schemas).await,
            DatabaseManager::Postgres(b) => b.initialize_schema(table_schemas).await,
        }
    }

    pub async fn select(
        &self,
        table: &str,
        columns: Option<Vec<String>>,
        where_clause: Option<HashMap<String, Value>>,
        limit: Option<u32>,
        offset: Option<u32>,
    ) -> Result<Vec<Value>, DatabaseError> {
        match self {
            DatabaseManager::Sqlite(b) => b.select(table, columns, where_clause, limit, offset).await,
            DatabaseManager::Postgres(b) => b.select(table, columns, where_clause, limit, offset).await,
        }
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
        match self {
            DatabaseManager::Sqlite(b) => b.insert(table, data).await,
            DatabaseManager::Postgres(b) => b.insert(table, data).await,
        }
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
        match self {
            DatabaseManager::Sqlite(b) => b.update(table, data, where_clause).await,
            DatabaseManager::Postgres(b) => b.update(table, data, where_clause).await,
        }
    }

    pub async fn delete(
        &self,
        table: &str,
        where_clause: HashMap<String, Value>,
    ) -> Result<u64, DatabaseError> {
        match self {
            DatabaseManager::Sqlite(b) => b.delete(table, where_clause).await,
            DatabaseManager::Postgres(b) => b.delete(table, where_clause).await,
        }
    }
}
