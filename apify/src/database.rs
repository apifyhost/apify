//! Database operations and connection management (now supports SQLite and Postgres)

use crate::schema_generator::{SchemaGenerator, TableSchema};
use serde_json::{Value, json};
use sqlx::{Column, Row};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePool, SqlitePoolOptions, SqliteRow};
use sqlx::postgres::{PgPool, PgPoolOptions, PgRow};
use sqlx::{QueryBuilder, Sqlite, Postgres};
use std::collections::HashMap;
use std::str::FromStr;

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
    Sqlite { pool: SqlitePool },
    Postgres { pool: PgPool },
}

impl DatabaseManager {
    pub async fn new(config: DatabaseRuntimeConfig) -> Result<Self, DatabaseError> {
        match config.driver.as_str() {
            "postgres" => {
                eprintln!("Connecting to Postgres: {}", config.url);
                let pool = PgPoolOptions::new()
                    .max_connections(config.max_size)
                    .connect(&config.url)
                    .await
                    .map_err(DatabaseError::PoolError)?;
                Ok(Self::Postgres { pool })
            }
            _ => {
                eprintln!("Attempting to connect to SQLite database: {}", config.url);
                let opts = if config.url == "sqlite::memory:" {
                    SqliteConnectOptions::from_str(&config.url).map_err(DatabaseError::PoolError)?
                } else {
                    let filename = config.url.strip_prefix("sqlite:").unwrap_or(&config.url);
                    SqliteConnectOptions::new().filename(filename).create_if_missing(true)
                };
                let pool = SqlitePoolOptions::new()
                    .max_connections(config.max_size)
                    .connect_with(opts)
                    .await
                    .map_err(|e| { eprintln!("SQLite connection error: {:?}, URL: {}", e, config.url); DatabaseError::PoolError(e) })?;
                eprintln!("SQLite database connected successfully");
                Ok(Self::Sqlite { pool })
            }
        }
    }

    /// Initialize database schema with dynamic table schemas (idempotent, per-table + per-index checks)
    pub async fn initialize_schema(
        &self,
        table_schemas: Vec<TableSchema>,
    ) -> Result<(), DatabaseError> {
        match self {
            DatabaseManager::Sqlite { pool } => {
                for schema in table_schemas {
                    let full_sql = SchemaGenerator::generate_create_table_sql_sqlite(&schema);
                    for stmt in full_sql.split(';') {
                        let s = stmt.trim();
                        if s.is_empty() { continue; }
                        sqlx::raw_sql(s)
                            .execute(pool)
                            .await
                            .map_err(DatabaseError::QueryError)?;
                    }
                }
                Ok(())
            }
            DatabaseManager::Postgres { pool } => {
                for schema in table_schemas {
                    let full_sql = SchemaGenerator::generate_create_table_sql_postgres(&schema);
                    for stmt in full_sql.split(';') {
                        let s = stmt.trim();
                        if s.is_empty() { continue; }
                        sqlx::raw_sql(s)
                            .execute(pool)
                            .await
                            .map_err(DatabaseError::QueryError)?;
                    }
                }
                Ok(())
            }
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
            DatabaseManager::Sqlite { pool } => {
                let mut qb = QueryBuilder::<Sqlite>::new("SELECT ");
                if let Some(cols) = columns {
                    if !cols.is_empty() {
                        qb.push(cols.join(", "));
                    } else {
                        qb.push("*");
                    }
                } else {
                    qb.push("*");
                }
                qb.push(" FROM ").push(table);
                if let Some(conds) = where_clause {
                    push_where_sqlite(&mut qb, conds);
                }
                if let Some(l) = limit {
                    qb.push(" LIMIT ").push_bind(l as i64);
                }
                if let Some(o) = offset {
                    qb.push(" OFFSET ").push_bind(o as i64);
                }
                let rows = qb
                    .build()
                    .fetch_all(pool)
                    .await
                    .map_err(DatabaseError::QueryError)?;
                Ok(rows.into_iter().map(|r| row_to_json_sqlite(&r)).collect())
            }
            DatabaseManager::Postgres { pool } => {
                let mut qb = QueryBuilder::<Postgres>::new("SELECT ");
                if let Some(cols) = columns {
                    if !cols.is_empty() {
                        qb.push(cols.join(", "));
                    } else {
                        qb.push("*");
                    }
                } else {
                    qb.push("*");
                }
                qb.push(" FROM ").push(table);
                if let Some(conds) = where_clause {
                    push_where_postgres(&mut qb, conds);
                }
                if let Some(l) = limit {
                    qb.push(" LIMIT ").push_bind(l as i64);
                }
                if let Some(o) = offset {
                    qb.push(" OFFSET ").push_bind(o as i64);
                }
                let rows: Vec<PgRow> = qb
                    .build()
                    .fetch_all(pool)
                    .await
                    .map_err(DatabaseError::QueryError)?;
                Ok(rows.into_iter().map(|r| row_to_json_postgres(&r)).collect())
            }
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
        let cols: Vec<String> = data.keys().cloned().collect();
        match self {
            DatabaseManager::Sqlite { pool } => {
                let mut qb = QueryBuilder::<Sqlite>::new("INSERT INTO ");
                qb.push(table).push(" (");
                qb.push(cols.join(", ")).push(") VALUES (");
                let mut sep = qb.separated(", ");
                for v in data.values() {
                    push_bind_sqlite(&mut sep, v);
                }
                qb.push(")");
                let res = qb
                    .build()
                    .execute(pool)
                    .await
                    .map_err(DatabaseError::QueryError)?;
                Ok(json!({"message": "Record inserted", "affected_rows": res.rows_affected()}))
            }
            DatabaseManager::Postgres { pool } => {
                let mut qb = QueryBuilder::<Postgres>::new("INSERT INTO ");
                qb.push(table).push(" (");
                qb.push(cols.join(", ")).push(") VALUES (");
                let mut sep = qb.separated(", ");
                for v in data.values() {
                    push_bind_postgres(&mut sep, v);
                }
                qb.push(")");
                let res = qb
                    .build()
                    .execute(pool)
                    .await
                    .map_err(DatabaseError::QueryError)?;
                Ok(json!({"message": "Record inserted", "affected_rows": res.rows_affected()}))
            }
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
            DatabaseManager::Sqlite { pool } => {
                let mut qb = QueryBuilder::<Sqlite>::new("UPDATE ");
                qb.push(table).push(" SET ");
                let mut first = true;
                for (k, v) in data {
                    if !first { qb.push(", "); }
                    first = false;
                    qb.push(&k).push(" = ");
                    match v {
                        Value::Null => { qb.push("NULL"); }
                        Value::Bool(b) => { qb.push_bind(b); }
                        Value::Number(n) => {
                            if let Some(i) = n.as_i64() { qb.push_bind(i); }
                            else if let Some(f) = n.as_f64() { qb.push_bind(f); }
                            else { qb.push_bind(n.to_string()); }
                        }
                        Value::String(s) => { qb.push_bind(s); }
                        Value::Array(_) | Value::Object(_) => {
                            qb.push_bind(serde_json::to_string(&v).unwrap_or_default());
                        }
                    }
                }
                qb.push(" WHERE ");
                let mut first = true;
                for (k, v) in where_clause {
                    if !first { qb.push(" AND "); }
                    first = false;
                    qb.push(format!("{} = ", k));
                    match v {
                        Value::Null => { qb.push("NULL"); }
                        Value::Bool(b) => { qb.push_bind(b); }
                        Value::Number(n) => {
                            if let Some(i) = n.as_i64() { qb.push_bind(i); }
                            else if let Some(f) = n.as_f64() { qb.push_bind(f); }
                            else { qb.push_bind(n.to_string()); }
                        }
                        Value::String(s) => { qb.push_bind(s); }
                        Value::Array(_) | Value::Object(_) => {
                            qb.push_bind(serde_json::to_string(&v).unwrap_or_default());
                        }
                    }
                }
                let res = qb.build().execute(pool).await.map_err(DatabaseError::QueryError)?;
                Ok(json!({"message": "Record updated", "affected_rows": res.rows_affected()}))
            }
            DatabaseManager::Postgres { pool } => {
                let mut qb = QueryBuilder::<Postgres>::new("UPDATE ");
                qb.push(table).push(" SET ");
                let mut first = true;
                for (k, v) in data {
                    if !first { qb.push(", "); }
                    first = false;
                    qb.push(&k).push(" = ");
                    match v {
                        Value::Null => { qb.push("NULL"); }
                        Value::Bool(b) => { qb.push_bind(b); }
                        Value::Number(n) => {
                            if let Some(i) = n.as_i64() { qb.push_bind(i); }
                            else if let Some(f) = n.as_f64() { qb.push_bind(f); }
                            else { qb.push_bind(n.to_string()); }
                        }
                        Value::String(s) => { qb.push_bind(s); }
                        Value::Array(_) | Value::Object(_) => {
                            qb.push_bind(serde_json::to_string(&v).unwrap_or_default());
                        }
                    }
                }
                qb.push(" WHERE ");
                let mut first = true;
                for (k, v) in where_clause {
                    if !first { qb.push(" AND "); }
                    first = false;
                    qb.push(format!("{} = ", k));
                    match v {
                        Value::Null => { qb.push("NULL"); }
                        Value::Bool(b) => { qb.push_bind(b); }
                        Value::Number(n) => {
                            if let Some(i) = n.as_i64() { qb.push_bind(i); }
                            else if let Some(f) = n.as_f64() { qb.push_bind(f); }
                            else { qb.push_bind(n.to_string()); }
                        }
                        Value::String(s) => { qb.push_bind(s); }
                        Value::Array(_) | Value::Object(_) => {
                            qb.push_bind(serde_json::to_string(&v).unwrap_or_default());
                        }
                    }
                }
                let res = qb.build().execute(pool).await.map_err(DatabaseError::QueryError)?;
                Ok(json!({"message": "Record updated", "affected_rows": res.rows_affected()}))
            }
        }
    }

    pub async fn delete(
        &self,
        table: &str,
        where_clause: HashMap<String, Value>,
    ) -> Result<u64, DatabaseError> {
        if where_clause.is_empty() {
            return Err(DatabaseError::ValidationError(
                "WHERE clause is required for delete".to_string(),
            ));
        }
        match self {
            DatabaseManager::Sqlite { pool } => {
                let mut qb = QueryBuilder::<Sqlite>::new("DELETE FROM ");
                qb.push(table).push(" WHERE ");
                let mut first = true;
                for (k, v) in where_clause {
                    if !first { qb.push(" AND "); }
                    first = false;
                    qb.push(format!("{} = ", k));
                    match v {
                        Value::Null => { qb.push("NULL"); }
                        Value::Bool(b) => { qb.push_bind(b); }
                        Value::Number(n) => {
                            if let Some(i) = n.as_i64() { qb.push_bind(i); }
                            else if let Some(f) = n.as_f64() { qb.push_bind(f); }
                            else { qb.push_bind(n.to_string()); }
                        }
                        Value::String(s) => { qb.push_bind(s); }
                        Value::Array(_) | Value::Object(_) => {
                            qb.push_bind(serde_json::to_string(&v).unwrap_or_default());
                        }
                    }
                }
                let res = qb.build().execute(pool).await.map_err(DatabaseError::QueryError)?;
                Ok(res.rows_affected())
            }
            DatabaseManager::Postgres { pool } => {
                let mut qb = QueryBuilder::<Postgres>::new("DELETE FROM ");
                qb.push(table).push(" WHERE ");
                let mut first = true;
                for (k, v) in where_clause {
                    if !first { qb.push(" AND "); }
                    first = false;
                    qb.push(format!("{} = ", k));
                    match v {
                        Value::Null => { qb.push("NULL"); }
                        Value::Bool(b) => { qb.push_bind(b); }
                        Value::Number(n) => {
                            if let Some(i) = n.as_i64() { qb.push_bind(i); }
                            else if let Some(f) = n.as_f64() { qb.push_bind(f); }
                            else { qb.push_bind(n.to_string()); }
                        }
                        Value::String(s) => { qb.push_bind(s); }
                        Value::Array(_) | Value::Object(_) => {
                            qb.push_bind(serde_json::to_string(&v).unwrap_or_default());
                        }
                    }
                }
                let res = qb.build().execute(pool).await.map_err(DatabaseError::QueryError)?;
                Ok(res.rows_affected())
            }
        }
    }
}

fn row_to_json_sqlite(row: &SqliteRow) -> Value {
    let mut obj = serde_json::Map::new();
    for (i, col) in row.columns().iter().enumerate() {
        let name = col.name().to_string();
        if let Ok(v) = row.try_get::<String, _>(i) {
            obj.insert(name, Value::String(v));
            continue;
        }
        if let Ok(v) = row.try_get::<i64, _>(i) {
            obj.insert(name, Value::Number(v.into()));
            continue;
        }
        if let Ok(v) = row.try_get::<f64, _>(i) {
            obj.insert(
                name,
                serde_json::Number::from_f64(v)
                    .map(Value::Number)
                    .unwrap_or(Value::Null),
            );
            continue;
        }
        if let Ok(v) = row.try_get::<bool, _>(i) {
            obj.insert(name, Value::Bool(v));
            continue;
        }
        if let Ok(s) = row.try_get::<String, _>(i) {
            if ((s.starts_with('{') && s.ends_with('}'))
                || (s.starts_with('[') && s.ends_with(']')))
                && let Ok(j) = serde_json::from_str::<Value>(&s)
            {
                obj.insert(name, j);
                continue;
            }
            obj.insert(name, Value::String(s));
            continue;
        }
        obj.insert(name, Value::Null);
    }
    Value::Object(obj)
}

fn push_where_sqlite(qb: &mut QueryBuilder<Sqlite>, conds: HashMap<String, Value>) {
    if conds.is_empty() {
        return;
    }
    qb.push(" WHERE ");
    let mut first = true;
    for (k, v) in conds {
        if !first {
            qb.push(" AND ");
        }
        first = false;
        qb.push(format!("{} = ", k));
        match v {
            Value::Null => {
                qb.push("NULL");
            }
            Value::Bool(b) => {
                qb.push_bind(b);
            }
            Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    qb.push_bind(i);
                } else if let Some(f) = n.as_f64() {
                    qb.push_bind(f);
                } else {
                    qb.push_bind(n.to_string());
                }
            }
            Value::String(s) => {
                qb.push_bind(s);
            }
            Value::Array(_) | Value::Object(_) => {
                qb.push_bind(serde_json::to_string(&v).unwrap_or_default());
            }
        }
    }
}

fn push_bind_sqlite(sep: &mut sqlx::query_builder::Separated<'_, '_, Sqlite, &str>, v: &Value) {
    match v {
        Value::Null => {
            sep.push("NULL");
        }
        Value::Bool(b) => {
            sep.push_bind(*b);
        }
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                sep.push_bind(i);
            } else if let Some(f) = n.as_f64() {
                sep.push_bind(f);
            } else {
                sep.push_bind(n.to_string());
            }
        }
        Value::String(s) => {
            sep.push_bind(s.clone());
        }
        Value::Array(_) | Value::Object(_) => {
            sep.push_bind(serde_json::to_string(v).unwrap_or_default());
        }
    }
}

fn row_to_json_postgres(row: &PgRow) -> Value {
    let mut obj = serde_json::Map::new();
    for (i, col) in row.columns().iter().enumerate() {
        let name = col.name().to_string();
        // Try JSON first if available
        if let Ok(v) = row.try_get::<serde_json::Value, _>(i) {
            obj.insert(name, v);
            continue;
        }
        if let Ok(v) = row.try_get::<String, _>(i) {
            // If looks like json, try to parse
            if ((v.starts_with('{') && v.ends_with('}')) || (v.starts_with('[') && v.ends_with(']')))
                && let Ok(j) = serde_json::from_str::<Value>(&v)
            {
                obj.insert(name, j);
            } else {
                obj.insert(name, Value::String(v));
            }
            continue;
        }
        if let Ok(v) = row.try_get::<i64, _>(i) {
            obj.insert(name, Value::Number(v.into()));
            continue;
        }
        if let Ok(v) = row.try_get::<f64, _>(i) {
            obj.insert(
                name,
                serde_json::Number::from_f64(v).map(Value::Number).unwrap_or(Value::Null),
            );
            continue;
        }
        if let Ok(v) = row.try_get::<bool, _>(i) {
            obj.insert(name, Value::Bool(v));
            continue;
        }
        obj.insert(name, Value::Null);
    }
    Value::Object(obj)
}

fn push_where_postgres(qb: &mut QueryBuilder<Postgres>, conds: HashMap<String, Value>) {
    if conds.is_empty() { return; }
    qb.push(" WHERE ");
    let mut first = true;
    for (k, v) in conds {
        if !first { qb.push(" AND "); }
        first = false;
        qb.push(format!("{} = ", k));
        match v {
            Value::Null => { qb.push("NULL"); }
            Value::Bool(b) => { qb.push_bind(b); }
            Value::Number(n) => {
                if let Some(i) = n.as_i64() { qb.push_bind(i); }
                else if let Some(f) = n.as_f64() { qb.push_bind(f); }
                else { qb.push_bind(n.to_string()); }
            }
            Value::String(s) => { qb.push_bind(s); }
            Value::Array(_) | Value::Object(_) => {
                qb.push_bind(serde_json::to_string(&v).unwrap_or_default());
            }
        }
    }
}

fn push_bind_postgres(
    sep: &mut sqlx::query_builder::Separated<'_, '_, Postgres, &str>,
    v: &Value,
) {
    match v {
        Value::Null => { sep.push("NULL"); }
        Value::Bool(b) => { sep.push_bind(*b); }
        Value::Number(n) => {
            if let Some(i) = n.as_i64() { sep.push_bind(i); }
            else if let Some(f) = n.as_f64() { sep.push_bind(f); }
            else { sep.push_bind(n.to_string()); }
        }
        Value::String(s) => { sep.push_bind(s.clone()); }
        Value::Array(_) | Value::Object(_) => {
            sep.push_bind(serde_json::to_string(v).unwrap_or_default());
        }
    }
}
