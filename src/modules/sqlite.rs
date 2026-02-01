//! SQLite backend implementation for DatabaseBackend

use once_cell::sync::Lazy;
use serde_json::{Value, json};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePool, SqlitePoolOptions, SqliteRow};
use sqlx::{Column, Row};
use sqlx::{QueryBuilder, Sqlite};
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use tokio::sync::Mutex;

use crate::database::{DatabaseBackend, DatabaseError, DatabaseRuntimeConfig};
use crate::schema_generator::{ColumnDefinition, SchemaGenerator, TableSchema};

static MIGRATION_LOCKS: Lazy<StdMutex<HashMap<String, Arc<Mutex<()>>>>> =
    Lazy::new(|| StdMutex::new(HashMap::new()));

#[derive(Debug, Clone)]
pub struct SqliteBackend {
    pub pool: SqlitePool,
    pub migration_lock: Arc<Mutex<()>>,
}

impl SqliteBackend {
    pub async fn connect(config: DatabaseRuntimeConfig) -> Result<Self, DatabaseError> {
        let (opts, filename_key) = if config.url == "sqlite::memory:" {
            (
                SqliteConnectOptions::from_str(&config.url).map_err(DatabaseError::PoolError)?,
                "sqlite::memory:".to_string(),
            )
        } else {
            let filename = config.url.strip_prefix("sqlite:").unwrap_or(&config.url);
            (
                SqliteConnectOptions::new()
                    .filename(filename)
                    .create_if_missing(true)
                    .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
                    .busy_timeout(std::time::Duration::from_secs(5)),
                filename.to_string(),
            )
        };
        let pool = SqlitePoolOptions::new()
            .max_connections(config.max_size)
            .connect_with(opts)
            .await
            .map_err(DatabaseError::PoolError)?;

        tracing::info!(
            "Connected to SQLite. Max pool size: {}, Current Size: {}, Idle: {}",
            config.max_size,
            pool.size(),
            pool.num_idle()
        );

        let migration_lock = {
            let mut locks = MIGRATION_LOCKS.lock().unwrap();
            locks
                .entry(filename_key)
                .or_insert_with(|| Arc::new(Mutex::new(())))
                .clone()
        };

        Ok(Self {
            pool,
            migration_lock,
        })
    }

    async fn do_get_table_schema(&self, table: &str) -> Result<Option<TableSchema>, DatabaseError> {
        // Check if table exists first
        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name=$1)",
        )
        .bind(table)
        .fetch_one(&self.pool)
        .await
        .map_err(DatabaseError::QueryError)?;

        if !exists {
            return Ok(None);
        }

        let query = format!("PRAGMA table_info({})", table);
        let rows = sqlx::query(&query)
            .fetch_all(&self.pool)
            .await
            .map_err(DatabaseError::QueryError)?;

        let mut columns = Vec::new();
        for row in rows {
            let name: String = row.get("name");
            let type_: String = row.get("type");
            let notnull: i32 = row.get("notnull");
            let dflt_value: Option<String> = row.get("dflt_value");
            let pk: i32 = row.get("pk");

            columns.push(ColumnDefinition {
                name,
                column_type: type_,
                nullable: notnull == 0 && pk == 0,
                primary_key: pk > 0,
                unique: false,         // TODO: Check unique constraints
                auto_increment: false, // SQLite handles this implicitly for INTEGER PRIMARY KEY
                default_value: dflt_value,
                auto_field: false,
            });
        }

        Ok(Some(TableSchema {
            table_name: table.to_string(),
            columns,
            indexes: vec![],
            relations: vec![],
        }))
    }

    async fn do_initialize_schema(
        &self,
        table_schemas: Vec<TableSchema>,
    ) -> Result<(), DatabaseError> {
        let _guard = self.migration_lock.lock().await;
        tracing::info!(
            "do_initialize_schema called with {} schemas",
            table_schemas.len()
        );
        for schema in table_schemas {
            tracing::info!("Processing schema for table: {}", schema.table_name);
            let current_schema = self.do_get_table_schema(&schema.table_name).await?;

            if let Some(current) = current_schema {
                // Table exists, migrate
                let migration_sqls =
                    SchemaGenerator::generate_migration_sql(&current, &schema, "sqlite")
                        .map_err(DatabaseError::ValidationError)?;
                for sql in migration_sqls {
                    tracing::info!(sql = %sql, "Executing migration SQL");
                    sqlx::raw_sql(&sql)
                        .execute(&self.pool)
                        .await
                        .map_err(DatabaseError::QueryError)?;
                }
            } else {
                tracing::info!("Table {} does not exist, creating...", schema.table_name);
                let full_sql = SchemaGenerator::generate_create_table_sql_sqlite(&schema);
                for stmt in full_sql.split(';') {
                    let s = stmt.trim();
                    if s.is_empty() {
                        continue;
                    }
                    sqlx::raw_sql(s)
                        .execute(&self.pool)
                        .await
                        .map_err(DatabaseError::QueryError)?;
                }
            }
        }
        Ok(())
    }

    async fn do_select(
        &self,
        table: &str,
        columns: Option<Vec<String>>,
        where_clause: Option<HashMap<String, Value>>,
        limit: Option<u32>,
        offset: Option<u32>,
    ) -> Result<Vec<Value>, DatabaseError> {
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

        let pool_size = self.pool.size();
        let pool_idle = self.pool.num_idle();
        if pool_idle == 0 {
            tracing::warn!(
                "SQLite pool exhaustion risk? Size: {}, Idle: {} [Presuming select on table: {}]",
                pool_size,
                pool_idle,
                table
            );
        }

        let start = std::time::Instant::now();
        let rows = qb.build().fetch_all(&self.pool).await.map_err(|e| {
            tracing::error!(
                "SQLite select error on table {}: {:?}. Pool Size: {}, Idle: {}",
                table,
                e,
                self.pool.size(),
                self.pool.num_idle()
            );
            DatabaseError::QueryError(e)
        })?;

        let elapsed = start.elapsed();
        if elapsed > std::time::Duration::from_millis(500) {
            tracing::warn!(
                "Slow SQLite select on table {} took {:?}. Pool Size: {}, Idle: {}",
                table,
                elapsed,
                self.pool.size(),
                self.pool.num_idle()
            );
        }

        Ok(rows.into_iter().map(|r| row_to_json_sqlite(&r)).collect())
    }

    async fn do_insert(
        &self,
        table: &str,
        data: HashMap<String, Value>,
    ) -> Result<Value, DatabaseError> {
        if data.is_empty() {
            return Err(DatabaseError::ValidationError(
                "No data provided for insert".to_string(),
            ));
        }
        let (cols, vals): (Vec<String>, Vec<Value>) = data.into_iter().unzip();
        let mut qb = QueryBuilder::<Sqlite>::new("INSERT INTO ");
        qb.push(table).push(" (");
        qb.push(cols.join(", ")).push(") VALUES (");
        let mut sep = qb.separated(", ");
        for v in &vals {
            push_bind_sqlite(&mut sep, v);
        }
        qb.push(")");

        let pool_size = self.pool.size();
        let pool_idle = self.pool.num_idle();
        if pool_idle == 0 {
            tracing::warn!(
                "SQLite pool exhaustion risk? Size: {}, Idle: {} [Presuming insert on table: {}]",
                pool_size,
                pool_idle,
                table
            );
        }
        let start = std::time::Instant::now();

        let res = qb.build().execute(&self.pool).await.map_err(|e| {
            tracing::error!(
                "Insert query failed on table {}: {:?}. Pool Size: {}, Idle: {}",
                table,
                e,
                self.pool.size(),
                self.pool.num_idle()
            );
            DatabaseError::QueryError(e)
        })?;

        let elapsed = start.elapsed();
        if elapsed > std::time::Duration::from_millis(500) {
            tracing::warn!(
                "Slow SQLite insert on table {} took {:?}. Pool Size: {}, Idle: {}",
                table,
                elapsed,
                self.pool.size(),
                self.pool.num_idle()
            );
        }

        let last_id = res.last_insert_rowid();
        Ok(json!({
            "message": "Record inserted",
            "affected_rows": res.rows_affected(),
            "id": last_id
        }))
    }

    async fn do_update(
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
        let mut qb = QueryBuilder::<Sqlite>::new("UPDATE ");
        qb.push(table).push(" SET ");
        let mut first = true;
        for (k, v) in data {
            if !first {
                qb.push(", ");
            }
            first = false;
            qb.push(&k).push(" = ");
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
        qb.push(" WHERE ");
        let mut first = true;
        for (k, v) in where_clause {
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

        let pool_size = self.pool.size();
        let pool_idle = self.pool.num_idle();
        if pool_idle == 0 {
            tracing::warn!(
                "SQLite pool exhaustion risk? Size: {}, Idle: {} [Presuming update on table: {}]",
                pool_size,
                pool_idle,
                table
            );
        }
        let start = std::time::Instant::now();

        let res = qb.build().execute(&self.pool).await.map_err(|e| {
            tracing::error!(
                "Update query failed on table {}: {:?}. Pool Size: {}, Idle: {}",
                table,
                e,
                self.pool.size(),
                self.pool.num_idle()
            );
            DatabaseError::QueryError(e)
        })?;

        let elapsed = start.elapsed();
        if elapsed > std::time::Duration::from_millis(500) {
            tracing::warn!(
                "Slow SQLite update on table {} took {:?}. Pool Size: {}, Idle: {}",
                table,
                elapsed,
                self.pool.size(),
                self.pool.num_idle()
            );
        }

        Ok(json!({"message": "Record updated", "affected_rows": res.rows_affected()}))
    }

    async fn do_delete(
        &self,
        table: &str,
        where_clause: HashMap<String, Value>,
    ) -> Result<u64, DatabaseError> {
        if where_clause.is_empty() {
            return Err(DatabaseError::ValidationError(
                "WHERE clause is required for delete".to_string(),
            ));
        }
        let mut qb = QueryBuilder::<Sqlite>::new("DELETE FROM ");
        qb.push(table).push(" WHERE ");
        let mut first = true;
        for (k, v) in where_clause {
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

        let pool_size = self.pool.size();
        let pool_idle = self.pool.num_idle();
        if pool_idle == 0 {
            tracing::warn!(
                "SQLite pool exhaustion risk? Size: {}, Idle: {} [Presuming delete on table: {}]",
                pool_size,
                pool_idle,
                table
            );
        }
        let start = std::time::Instant::now();

        let res = qb.build().execute(&self.pool).await.map_err(|e| {
            tracing::error!(
                "Delete query failed on table {}: {:?}. Pool Size: {}, Idle: {}",
                table,
                e,
                self.pool.size(),
                self.pool.num_idle()
            );
            DatabaseError::QueryError(e)
        })?;

        let elapsed = start.elapsed();
        if elapsed > std::time::Duration::from_millis(500) {
            tracing::warn!(
                "Slow SQLite delete on table {} took {:?}. Pool Size: {}, Idle: {}",
                table,
                elapsed,
                self.pool.size(),
                self.pool.num_idle()
            );
        }

        Ok(res.rows_affected())
    }
}

impl DatabaseBackend for SqliteBackend {
    fn initialize_schema<'a>(
        &'a self,
        table_schemas: Vec<TableSchema>,
    ) -> core::pin::Pin<Box<dyn core::future::Future<Output = Result<(), DatabaseError>> + Send + 'a>>
    {
        Box::pin(async move { self.do_initialize_schema(table_schemas).await })
    }
    fn select<'a>(
        &'a self,
        table: &'a str,
        columns: Option<Vec<String>>,
        where_clause: Option<HashMap<String, Value>>,
        limit: Option<u32>,
        offset: Option<u32>,
    ) -> core::pin::Pin<
        Box<dyn core::future::Future<Output = Result<Vec<Value>, DatabaseError>> + Send + 'a>,
    > {
        Box::pin(async move {
            self.do_select(table, columns, where_clause, limit, offset)
                .await
        })
    }
    fn insert<'a>(
        &'a self,
        table: &'a str,
        data: HashMap<String, Value>,
    ) -> core::pin::Pin<
        Box<dyn core::future::Future<Output = Result<Value, DatabaseError>> + Send + 'a>,
    > {
        Box::pin(async move { self.do_insert(table, data).await })
    }
    fn update<'a>(
        &'a self,
        table: &'a str,
        data: HashMap<String, Value>,
        where_clause: HashMap<String, Value>,
    ) -> core::pin::Pin<
        Box<dyn core::future::Future<Output = Result<Value, DatabaseError>> + Send + 'a>,
    > {
        Box::pin(async move { self.do_update(table, data, where_clause).await })
    }
    fn delete<'a>(
        &'a self,
        table: &'a str,
        where_clause: HashMap<String, Value>,
    ) -> core::pin::Pin<
        Box<dyn core::future::Future<Output = Result<u64, DatabaseError>> + Send + 'a>,
    > {
        Box::pin(async move { self.do_delete(table, where_clause).await })
    }

    fn get_table_schema<'a>(
        &'a self,
        table: &'a str,
    ) -> core::pin::Pin<
        Box<
            dyn core::future::Future<Output = Result<Option<TableSchema>, DatabaseError>>
                + Send
                + 'a,
        >,
    > {
        Box::pin(async move { self.do_get_table_schema(table).await })
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
