//! Postgres backend implementation for DatabaseBackend

use serde_json::{Value, json};
use sqlx::postgres::{PgPool, PgPoolOptions, PgRow};
use sqlx::types::chrono;
use sqlx::{Column, Postgres, QueryBuilder, Row};
use std::collections::HashMap;

use crate::database::{DatabaseBackend, DatabaseError, DatabaseRuntimeConfig};
use crate::schema_generator::{ColumnDefinition, SchemaGenerator, TableSchema};

#[derive(Debug, Clone)]
pub struct PostgresBackend {
    pub pool: PgPool,
}

impl PostgresBackend {
    pub async fn connect(config: DatabaseRuntimeConfig) -> Result<Self, DatabaseError> {
        tracing::info!(
            ">>> CONNECTING TO POSTGRES. CONFIG URL: {} MAX SIZE: {} <<<",
            config.url,
            config.max_size
        );
        let pool = PgPoolOptions::new()
            .max_connections(config.max_size)
            .acquire_timeout(std::time::Duration::from_secs(5))
            .connect(&config.url)
            .await
            .map_err(DatabaseError::PoolError)?;

        tracing::info!(
            "Connected to Postgres. Max pool size: {}, Current Size: {}, Idle: {}",
            config.max_size,
            pool.size(),
            pool.num_idle()
        );

        Ok(Self { pool })
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

        let pool_size = self.pool.size();
        let pool_idle = self.pool.num_idle();
        if pool_idle == 0 {
            tracing::warn!(
                "Postgres pool exhaustion risk? Size: {}, Idle: {} [Presuming select on table: {}]",
                pool_size,
                pool_idle,
                table
            );
        }
        let start = std::time::Instant::now();

        let rows: Vec<PgRow> = qb.build().fetch_all(&self.pool).await.map_err(|e| {
            tracing::error!(
                "Postgres select error on table {}: {:?}. Pool Size: {}, Idle: {}",
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
                "Slow Postgres select on table {} took {:?}. Pool Size: {}, Idle: {}",
                table,
                elapsed,
                self.pool.size(),
                self.pool.num_idle()
            );
        }

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
        // Ensure keys and values are aligned by collecting them together
        let mut keys = Vec::new();
        let mut values = Vec::new();
        for (k, v) in data {
            keys.push(k);
            values.push(v);
        }

        let mut qb = QueryBuilder::<Postgres>::new("INSERT INTO ");
        qb.push(table).push(" (");
        qb.push(keys.join(", ")).push(") VALUES (");

        let mut first = true;
        for (i, v) in values.iter().enumerate() {
            if !first {
                qb.push(", ");
            }
            first = false;

            match v {
                Value::Null => {
                    qb.push("NULL");
                }
                Value::Bool(b) => {
                    qb.push_bind(b);
                }
                Value::Number(n) => {
                    if let Some(f) = n.as_f64() {
                        qb.push_bind(f);
                    } else {
                        qb.push_bind(n.to_string());
                    }
                }
                Value::String(s) => {
                    qb.push_bind(s);
                    // Explicitly cast audit fields to TIMESTAMPTZ to avoid type mismatch errors
                    // when Postgres expects a timestamp but receives a text parameter.
                    let key = &keys[i];
                    if key.eq_ignore_ascii_case("createdAt")
                        || key.eq_ignore_ascii_case("updatedAt")
                    {
                        qb.push("::TIMESTAMPTZ");
                    }
                }
                Value::Array(_) | Value::Object(_) => {
                    qb.push_bind(serde_json::to_string(v).unwrap_or_default());
                }
            }
        }
        qb.push(") RETURNING *");

        let pool_size = self.pool.size();
        let pool_idle = self.pool.num_idle();
        if pool_idle == 0 {
            tracing::warn!(
                "Postgres pool exhaustion risk? Size: {}, Idle: {} [Presuming insert on table: {}]",
                pool_size,
                pool_idle,
                table
            );
        }
        let start = std::time::Instant::now();

        let row = qb.build().fetch_one(&self.pool).await.map_err(|e| {
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
                "Slow Postgres insert on table {} took {:?}. Pool Size: {}, Idle: {}",
                table,
                elapsed,
                self.pool.size(),
                self.pool.num_idle()
            );
        }

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
                    if k.eq_ignore_ascii_case("createdAt") || k.eq_ignore_ascii_case("updatedAt") {
                        qb.push("::TIMESTAMPTZ");
                    }
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
                    if k.eq_ignore_ascii_case("createdAt") || k.eq_ignore_ascii_case("updatedAt") {
                        qb.push("::TIMESTAMPTZ");
                    }
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
                "Postgres pool exhaustion risk? Size: {}, Idle: {} [Presuming update on table: {}]",
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
                "Slow Postgres update on table {} took {:?}. Pool Size: {}, Idle: {}",
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
        let pool = self.pool.clone();
        Box::pin(async move {
            // Acquire a transaction-scoped advisory lock to ensure only one instance runs migrations.
            // We hold this transaction open until the end.
            let mut lock_tx = pool.begin().await.map_err(DatabaseError::PoolError)?;
            sqlx::query("SELECT pg_advisory_xact_lock(123456789)")
                .execute(&mut *lock_tx)
                .await
                .map_err(DatabaseError::QueryError)?;

            for schema in table_schemas {
                // Check if table exists
                // We use the pool (fetch a new connection) for queries inside the locked section.
                // ample pool size is required to avoid deadlock (lock_tx holds 1, we need 1 more).
                let query = r#"
                    SELECT 
                        c.column_name, 
                        c.data_type, 
                        c.is_nullable, 
                        c.column_default,
                        CASE WHEN tc.constraint_type = 'PRIMARY KEY' THEN true ELSE false END as is_primary_key
                    FROM information_schema.columns c
                    LEFT JOIN information_schema.key_column_usage kcu 
                        ON c.table_name = kcu.table_name 
                        AND c.column_name = kcu.column_name
                    LEFT JOIN information_schema.table_constraints tc 
                        ON kcu.constraint_name = tc.constraint_name 
                        AND kcu.table_name = tc.table_name 
                        AND tc.constraint_type = 'PRIMARY KEY'
                    WHERE c.table_name = $1 AND c.table_schema = current_schema()
                "#;

                let rows = sqlx::query(query)
                    .bind(&schema.table_name)
                    .fetch_all(&pool)
                    .await
                    .map_err(DatabaseError::QueryError)?;

                let current_schema = if rows.is_empty() {
                    None
                } else {
                    let mut columns = Vec::new();
                    for row in rows {
                        let name: String = row.get("column_name");
                        let data_type: String = row.get("data_type");
                        let is_nullable: String = row.get("is_nullable");
                        let column_default: Option<String> = row.get("column_default");
                        let is_primary_key: Option<bool> = row.get("is_primary_key");

                        columns.push(ColumnDefinition {
                            name,
                            column_type: data_type,
                            nullable: is_nullable == "YES",
                            primary_key: is_primary_key.unwrap_or(false),
                            unique: false,
                            auto_increment: column_default
                                .as_ref()
                                .map(|d| d.contains("nextval"))
                                .unwrap_or(false),
                            default_value: column_default,
                            auto_field: false,
                        });
                    }
                    Some(TableSchema {
                        table_name: schema.table_name.clone(),
                        columns,
                        indexes: vec![],
                        relations: vec![],
                    })
                };

                if let Some(current) = current_schema {
                    // Table exists, migrate
                    let migration_sqls =
                        SchemaGenerator::generate_migration_sql(&current, &schema, "postgres")
                            .map_err(DatabaseError::ValidationError)?;

                    for sql in migration_sqls {
                        tracing::info!(sql = %sql, "Executing migration SQL");
                        sqlx::raw_sql(&sql)
                            .execute(&pool)
                            .await
                            .map_err(DatabaseError::QueryError)?;
                    }
                } else {
                    // Table does not exist, create
                    let full_sql = SchemaGenerator::generate_create_table_sql_postgres(&schema);
                    tracing::info!(sql = %full_sql, "Executing schema creation SQL");
                    for stmt in full_sql.split(';') {
                        let s = stmt.trim();
                        if s.is_empty() {
                            continue;
                        }
                        tracing::info!(statement = %s, "Executing SQL statement");
                        sqlx::raw_sql(s)
                            .execute(&pool)
                            .await
                            .map_err(DatabaseError::QueryError)?;
                    }
                }
            }

            // Release lock
            lock_tx.commit().await.map_err(DatabaseError::QueryError)?;
            Ok(())
        })
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
        Box::pin(async move {
            let query = r#"
                SELECT 
                    c.column_name, 
                    c.data_type, 
                    c.is_nullable, 
                    c.column_default,
                    CASE WHEN tc.constraint_type = 'PRIMARY KEY' THEN true ELSE false END as is_primary_key
                FROM information_schema.columns c
                LEFT JOIN information_schema.key_column_usage kcu 
                    ON c.table_name = kcu.table_name 
                    AND c.column_name = kcu.column_name
                LEFT JOIN information_schema.table_constraints tc 
                    ON kcu.constraint_name = tc.constraint_name 
                    AND kcu.table_name = tc.table_name 
                    AND tc.constraint_type = 'PRIMARY KEY'
                WHERE c.table_name = $1 AND c.table_schema = current_schema()
            "#;

            let rows = sqlx::query(query)
                .bind(table)
                .fetch_all(&self.pool)
                .await
                .map_err(DatabaseError::QueryError)?;

            if rows.is_empty() {
                return Ok(None);
            }

            let mut columns = Vec::new();
            for row in rows {
                let name: String = row.get("column_name");
                let data_type: String = row.get("data_type");
                let is_nullable: String = row.get("is_nullable");
                let column_default: Option<String> = row.get("column_default");
                let is_primary_key: Option<bool> = row.get("is_primary_key");

                columns.push(ColumnDefinition {
                    name,
                    column_type: data_type,
                    nullable: is_nullable == "YES",
                    primary_key: is_primary_key.unwrap_or(false),
                    unique: false, // TODO: Check unique constraints
                    auto_increment: column_default
                        .as_ref()
                        .map(|d| d.contains("nextval"))
                        .unwrap_or(false),
                    default_value: column_default,
                    auto_field: false,
                });
            }

            Ok(Some(TableSchema {
                table_name: table.to_string(),
                columns,
                indexes: vec![],
                relations: vec![],
            }))
        })
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
            obj.insert(name, Value::String(v));
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
                if k.eq_ignore_ascii_case("createdAt") || k.eq_ignore_ascii_case("updatedAt") {
                    qb.push("::TIMESTAMPTZ");
                }
            }
            Value::Array(_) | Value::Object(_) => {
                qb.push_bind(serde_json::to_string(&v).unwrap_or_default());
            }
        }
    }
}

#[allow(dead_code)]
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
