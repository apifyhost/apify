//! Postgres backend implementation for DatabaseBackend

use serde_json::{Value, json};
use sqlx::postgres::{PgPool, PgPoolOptions, PgRow};
use sqlx::types::chrono;
use sqlx::{Column, Postgres, QueryBuilder, Row};
use std::collections::HashMap;

use crate::database::{DatabaseBackend, DatabaseError, DatabaseRuntimeConfig};
use crate::schema_generator::{SchemaGenerator, TableSchema};

#[derive(Debug, Clone)]
pub struct PostgresBackend {
    pub pool: PgPool,
}

impl PostgresBackend {
    pub async fn connect(config: DatabaseRuntimeConfig) -> Result<Self, DatabaseError> {
        let pool = PgPoolOptions::new()
            .max_connections(config.max_size)
            .connect(&config.url)
            .await
            .map_err(DatabaseError::PoolError)?;
        Ok(Self { pool })
    }

    async fn do_initialize_schema(
        &self,
        table_schemas: Vec<TableSchema>,
    ) -> Result<(), DatabaseError> {
        // Use advisory lock to prevent concurrent schema creation across threads
        // Lock ID: 123456789 (arbitrary number for schema creation)
        let lock_acquired = sqlx::query_scalar::<_, bool>("SELECT pg_try_advisory_lock(123456789)")
            .fetch_one(&self.pool)
            .await
            .map_err(DatabaseError::QueryError)?;

        if !lock_acquired {
            // Another thread is creating the schema, wait for the lock to be released
            sqlx::query_scalar::<_, bool>("SELECT pg_advisory_lock(123456789)")
                .fetch_one(&self.pool)
                .await
                .map_err(DatabaseError::QueryError)?;
            // Lock acquired after waiting, but schema should be ready now, so just unlock and return
            sqlx::query("SELECT pg_advisory_unlock(123456789)")
                .execute(&self.pool)
                .await
                .map_err(DatabaseError::QueryError)?;
            return Ok(());
        }

        // We have the lock, proceed with schema creation
        let result = async {
            for schema in table_schemas {
                let full_sql = SchemaGenerator::generate_create_table_sql_postgres(&schema);
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
            Ok::<(), DatabaseError>(())
        }
        .await;

        // Always unlock, even if there was an error
        sqlx::query("SELECT pg_advisory_unlock(123456789)")
            .execute(&self.pool)
            .await
            .map_err(DatabaseError::QueryError)?;

        result
    }

    async fn do_select(
        &self,
        table: &str,
        columns: Option<Vec<String>>,
        where_clause: Option<HashMap<String, Value>>,
        limit: Option<u32>,
        offset: Option<u32>,
    ) -> Result<Vec<Value>, DatabaseError> {
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
            .fetch_all(&self.pool)
            .await
            .map_err(DatabaseError::QueryError)?;
        Ok(rows.into_iter().map(|r| row_to_json_postgres(&r)).collect())
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
        let cols: Vec<String> = data.keys().cloned().collect();
        let mut qb = QueryBuilder::<Postgres>::new("INSERT INTO ");
        qb.push(table).push(" (");
        qb.push(cols.join(", ")).push(") VALUES (");
        let mut sep = qb.separated(", ");
        for v in data.values() {
            push_bind_postgres(&mut sep, v);
        }
        qb.push(") RETURNING *");
        let row = qb
            .build()
            .fetch_one(&self.pool)
            .await
            .map_err(DatabaseError::QueryError)?;
        let inserted = row_to_json_postgres(&row);

        // Extract the id field from the inserted record
        let id = inserted.get("id").cloned();

        Ok(json!({
            "message": "Record inserted",
            "affected_rows": 1,
            "record": inserted,
            "id": id
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
        let mut qb = QueryBuilder::<Postgres>::new("UPDATE ");
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
        let res = qb
            .build()
            .execute(&self.pool)
            .await
            .map_err(DatabaseError::QueryError)?;
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
        let mut qb = QueryBuilder::<Postgres>::new("DELETE FROM ");
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
        let res = qb
            .build()
            .execute(&self.pool)
            .await
            .map_err(DatabaseError::QueryError)?;
        Ok(res.rows_affected())
    }
}

impl DatabaseBackend for PostgresBackend {
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
}

fn row_to_json_postgres(row: &PgRow) -> Value {
    let mut obj = serde_json::Map::new();
    for (i, col) in row.columns().iter().enumerate() {
        let name = col.name().to_string();
        if let Ok(v) = row.try_get::<serde_json::Value, _>(i) {
            obj.insert(name, v);
            continue;
        }
        // Temporal types -> string
        if let Ok(v) = row.try_get::<chrono::NaiveDateTime, _>(i) {
            obj.insert(name, Value::String(v.to_string()));
            continue;
        }
        if let Ok(v) = row.try_get::<chrono::DateTime<chrono::Utc>, _>(i) {
            obj.insert(name, Value::String(v.to_rfc3339()));
            continue;
        }
        if let Ok(v) = row.try_get::<chrono::DateTime<chrono::Local>, _>(i) {
            obj.insert(name, Value::String(v.to_rfc3339()));
            continue;
        }
        if let Ok(v) = row.try_get::<chrono::NaiveDate, _>(i) {
            obj.insert(name, Value::String(v.to_string()));
            continue;
        }
        if let Ok(v) = row.try_get::<chrono::NaiveTime, _>(i) {
            obj.insert(name, Value::String(v.to_string()));
            continue;
        }
        if let Ok(v) = row.try_get::<String, _>(i) {
            if ((v.starts_with('{') && v.ends_with('}'))
                || (v.starts_with('[') && v.ends_with(']')))
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
        if let Ok(v) = row.try_get::<i32, _>(i) {
            obj.insert(name, Value::Number(v.into()));
            continue;
        }
        if let Ok(v) = row.try_get::<i16, _>(i) {
            obj.insert(name, Value::Number((v as i64).into()));
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
        // Handle REAL (f32) separately in case it's not covered by f64
        if let Ok(v) = row.try_get::<f32, _>(i) {
            obj.insert(
                name,
                serde_json::Number::from_f64(v as f64)
                    .map(Value::Number)
                    .unwrap_or(Value::Null),
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
                // Always bind numbers as f64 for consistency with REAL columns
                if let Some(f) = n.as_f64() {
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

fn push_bind_postgres(sep: &mut sqlx::query_builder::Separated<'_, '_, Postgres, &str>, v: &Value) {
    match v {
        Value::Null => {
            sep.push("NULL");
        }
        Value::Bool(b) => {
            sep.push_bind(*b);
        }
        Value::Number(n) => {
            // Always bind numbers as f64 to avoid type mismatch with REAL columns
            // Convert i64 to f64 first to ensure compatibility
            if let Some(f) = n.as_f64() {
                sep.push_bind(f);
            } else {
                // Fallback to string representation for very large numbers
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
