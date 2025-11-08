//! Database operations and connection management (SQLite-only for now; Postgres support to follow)

use crate::schema_generator::{SchemaGenerator, TableSchema};
use serde_json::{json, Value};
use sqlx::{sqlite::{SqliteConnectOptions, SqlitePool, SqlitePoolOptions, SqliteRow}, Column, Row};
use sqlx::{QueryBuilder, Sqlite};
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
pub struct DatabaseConfig {
    pub url: String,
    pub max_size: u32,
}

impl DatabaseConfig {
    pub fn sqlite_default() -> Self {
        // SQLite connection - use file-based database
        // SqliteConnectOptions accepts standard sqlite: protocol URLs
        let db_path = std::env::current_dir()
            .map(|d| d.join("apify.sqlite").to_string_lossy().to_string())
            .unwrap_or_else(|_| "./apify.sqlite".to_string());
        let url = format!("sqlite:{}", db_path);
        eprintln!("Using SQLite database file: {}", url);
        Self { url, max_size: 10 }
    }

    pub async fn create_pool(&self) -> Result<SqlitePool, DatabaseError> {
        eprintln!("Attempting to connect to SQLite database: {}", self.url);
        
        // Use SqliteConnectOptions builder for better path handling
        let opts = if self.url == "sqlite::memory:" {
            SqliteConnectOptions::from_str(&self.url)
                .map_err(DatabaseError::PoolError)?
        } else {
            // Extract filename from URL (format: sqlite:/path/to/file)
            let filename = self.url.strip_prefix("sqlite:").unwrap_or(&self.url);
            SqliteConnectOptions::new()
                .filename(filename)
                .create_if_missing(true)
        };
        
        let pool = SqlitePoolOptions::new()
            .max_connections(self.max_size)
            .connect_with(opts)
            .await
            .map_err(|e| {
                eprintln!("SQLite connection error: {:?}, URL: {}", e, self.url);
                DatabaseError::PoolError(e)
            })?;
        eprintln!("SQLite database connected successfully");
        Ok(pool)
    }
}

#[derive(Debug, Clone)]
pub struct DatabaseManager { pool: SqlitePool }

impl DatabaseManager {
    pub async fn new(config: DatabaseConfig) -> Result<Self, DatabaseError> {
        let pool = config.create_pool().await?;
        let manager = Self { pool };
        Ok(manager)
    }

    /// Initialize database schema with dynamic table schemas
    pub async fn initialize_schema(&self, table_schemas: Vec<TableSchema>) -> Result<(), DatabaseError> {
        for schema in table_schemas {
            let sql = SchemaGenerator::generate_create_table_sql_sqlite(&schema);
            eprintln!("Creating table: {}\nSQL: {}", schema.table_name, sql);
            
            // Execute the schema initialization
            sqlx::raw_sql(&sql)
                .execute(&self.pool)
                .await
                .map_err(|e| {
                    eprintln!("Failed to create table {}: {}", schema.table_name, e);
                    DatabaseError::QueryError(e)
                })?;
                
            eprintln!("Table {} created successfully", schema.table_name);
        }
        
        Ok(())
    }

    pub async fn select(
        &self,
        table: &str,
        columns: Option<Vec<String>>,
        where_clause: Option<HashMap<String, Value>>,
        limit: Option<u32>,
        offset: Option<u32>,
    ) -> Result<Vec<Value>, DatabaseError> {
        let mut qb = QueryBuilder::<Sqlite>::new("SELECT ");
        if let Some(cols) = columns { if !cols.is_empty() { qb.push(cols.join(", ")); } else { qb.push("*"); } } else { qb.push("*"); }
        qb.push(" FROM ").push(table);
        if let Some(conds) = where_clause { push_where_sqlite(&mut qb, conds); }
        if let Some(l) = limit { qb.push(" LIMIT ").push_bind(l as i64); }
        if let Some(o) = offset { qb.push(" OFFSET ").push_bind(o as i64); }
        let rows = qb.build().fetch_all(&self.pool).await.map_err(DatabaseError::QueryError)?;
        Ok(rows.into_iter().map(|r| row_to_json_sqlite(&r)).collect())
    }

    pub async fn insert(
        &self,
        table: &str,
        data: HashMap<String, Value>,
    ) -> Result<Value, DatabaseError> {
        if data.is_empty() { return Err(DatabaseError::ValidationError("No data provided for insert".to_string())); }
        let cols: Vec<String> = data.keys().cloned().collect();
        let mut qb = QueryBuilder::<Sqlite>::new("INSERT INTO ");
        qb.push(table).push(" (");
        qb.push(cols.join(", ")).push(") VALUES (");
        let mut sep = qb.separated(", ");
        for (_k, v) in &data { push_bind_sqlite(&mut sep, v); }
        qb.push(")");
        let res = qb.build().execute(&self.pool).await.map_err(DatabaseError::QueryError)?;
        Ok(json!({"message": "Record inserted", "affected_rows": res.rows_affected()}))
    }

    pub async fn update(
        &self,
        table: &str,
        data: HashMap<String, Value>,
        where_clause: HashMap<String, Value>,
    ) -> Result<Value, DatabaseError> {
        if data.is_empty() { return Err(DatabaseError::ValidationError("No data provided for update".to_string())); }
        if where_clause.is_empty() { return Err(DatabaseError::ValidationError("WHERE clause is required for update".to_string())); }
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
                Value::Array(_) | Value::Object(_) => { qb.push_bind(serde_json::to_string(&v).unwrap_or_default()); }
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
                Value::Array(_) | Value::Object(_) => { qb.push_bind(serde_json::to_string(&v).unwrap_or_default()); }
            }
        }
        let res = qb.build().execute(&self.pool).await.map_err(DatabaseError::QueryError)?;
        Ok(json!({"message": "Record updated", "affected_rows": res.rows_affected()}))
    }

    pub async fn delete(
        &self,
        table: &str,
        where_clause: HashMap<String, Value>,
    ) -> Result<u64, DatabaseError> {
        if where_clause.is_empty() { return Err(DatabaseError::ValidationError("WHERE clause is required for delete".to_string())); }
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
                Value::Array(_) | Value::Object(_) => { qb.push_bind(serde_json::to_string(&v).unwrap_or_default()); }
            }
        }
        let res = qb.build().execute(&self.pool).await.map_err(DatabaseError::QueryError)?;
        Ok(res.rows_affected())
    }
}

fn row_to_json_sqlite(row: &SqliteRow) -> Value {
    let mut obj = serde_json::Map::new();
    for (i, col) in row.columns().iter().enumerate() {
        let name = col.name().to_string();
        if let Ok(v) = row.try_get::<String, _>(i) { obj.insert(name, Value::String(v)); continue; }
        if let Ok(v) = row.try_get::<i64, _>(i) { obj.insert(name, Value::Number(v.into())); continue; }
        if let Ok(v) = row.try_get::<f64, _>(i) { obj.insert(name, serde_json::Number::from_f64(v).map(Value::Number).unwrap_or(Value::Null)); continue; }
        if let Ok(v) = row.try_get::<bool, _>(i) { obj.insert(name, Value::Bool(v)); continue; }
        if let Ok(s) = row.try_get::<String, _>(i) {
            if (s.starts_with('{') && s.ends_with('}')) || (s.starts_with('[') && s.ends_with(']')) {
                if let Ok(j) = serde_json::from_str::<Value>(&s) { obj.insert(name, j); continue; }
            }
            obj.insert(name, Value::String(s));
            continue;
        }
        obj.insert(name, Value::Null);
    }
    Value::Object(obj)
}

fn push_where_sqlite(qb: &mut QueryBuilder<Sqlite>, conds: HashMap<String, Value>) {
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
            Value::Array(_) | Value::Object(_) => { qb.push_bind(serde_json::to_string(&v).unwrap_or_default()); }
        }
    }
}

fn push_bind_sqlite(sep: &mut sqlx::query_builder::Separated<'_, '_, Sqlite, &str>, v: &Value) {
    match v {
        Value::Null => { sep.push("NULL"); }
        Value::Bool(b) => { sep.push_bind(*b); }
        Value::Number(n) => {
            if let Some(i) = n.as_i64() { sep.push_bind(i); }
            else if let Some(f) = n.as_f64() { sep.push_bind(f); }
            else { sep.push_bind(n.to_string()); }
        }
        Value::String(s) => { sep.push_bind(s.clone()); }
        Value::Array(_) | Value::Object(_) => { sep.push_bind(serde_json::to_string(v).unwrap_or_default()); }
    }
}
