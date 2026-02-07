use crate::database::{DatabaseError, DatabaseManager, DatabaseRuntimeConfig};
use http_body_util::Full;
use hyper::body::Bytes;
use hyper::{Request, Response, StatusCode};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

// -----------------------------------------------------------------------------
// Connection Pooling Management
// -----------------------------------------------------------------------------

/// Thread-safe cache for database connections
pub type DbCache = Arc<RwLock<HashMap<String, DatabaseManager>>>;

/// Factory function to create the cache
pub fn create_db_cache() -> DbCache {
    Arc::new(RwLock::new(HashMap::new()))
}

/// Helper: Construct database connection URL from settings
fn make_db_url(settings: &crate::config::DatabaseSettings) -> String {
    match settings.driver.as_str() {
        "postgres" | "postgresql" => format!(
            "postgres://{}:{}@{}:{}/{}",
            settings.user.as_deref().unwrap_or(""),
            settings.password.as_deref().unwrap_or(""),
            settings.host.as_deref().unwrap_or("localhost"),
            settings.port.unwrap_or(5432),
            settings.database
        ),
        _ => {
            if settings.database.starts_with("sqlite:") {
                settings.database.clone()
            } else {
                // Ensure proper URI format for sqlx
                if settings.database.starts_with('/') {
                    // Absolute path: sqlite:///path
                    format!("sqlite://{}", settings.database)
                } else {
                    // Relative path: sqlite:path or sqlite://path
                    format!("sqlite:{}", settings.database)
                }
            }
        }
    }
}

/// Helper: Get or create a DatabaseManager for a given user datasource
async fn get_user_datasource(
    cp_db: &DatabaseManager,
    cache: &DbCache,
    datasource_name: &str,
) -> Result<DatabaseManager, DatabaseError> {
    // 1. Try cache first
    {
        let read = cache.read().await;
        if let Some(db) = read.get(datasource_name) {
            return Ok(db.clone());
        }
    }

    // 2. Fetch config from Control Plane DB
    let mut where_clause = HashMap::new();
    where_clause.insert(
        "name".to_string(),
        Value::String(datasource_name.to_string()),
    );

    let records = cp_db
        .select("_meta_datasources", None, Some(where_clause), None, None)
        .await?;

    if records.is_empty() {
        return Err(DatabaseError::ValidationError(format!(
            "Datasource '{}' not found",
            datasource_name
        )));
    }

    let record = &records[0];
    let config_str = record
        .get("config")
        .and_then(|v| v.as_str())
        .ok_or_else(|| DatabaseError::ValidationError("Invalid config format".to_string()))?;

    let db_settings: crate::config::DatabaseSettings = serde_json::from_str(config_str)
        .map_err(|e| DatabaseError::ValidationError(format!("Failed to parse config: {}", e)))?;

    // 3. Create new connection
    let runtime_config = DatabaseRuntimeConfig {
        driver: db_settings.driver.clone(),
        url: make_db_url(&db_settings),
        max_size: 5, // Keep pool small for admin ops
    };

    let db_manager = DatabaseManager::new(runtime_config).await?;

    // 4. Update cache
    {
        let mut write = cache.write().await;
        write.insert(datasource_name.to_string(), db_manager.clone());
    }

    Ok(db_manager)
}

// -----------------------------------------------------------------------------
// Request Handling
// -----------------------------------------------------------------------------

pub async fn handle_data_manager_request(
    req: Request<hyper::body::Incoming>,
    cp_db: &DatabaseManager,
    cache: &DbCache,
) -> Result<Response<Full<Bytes>>, Box<dyn std::error::Error + Send + Sync>> {
    let (parts, body) = req.into_parts();
    let method = parts.method;
    let path = parts.uri.path().to_string();

    // Route format: /apify/admin/data/{datasource_name}/{table_nameOrAction}/...
    // Example: /apify/admin/data/mydb/tables
    // Example: /apify/admin/data/mydb/users/query

    let segments: Vec<&str> = path.trim_start_matches('/').split('/').collect();
    // segments: ["apify", "admin", "data", "datasource_name", ...]
    // Index:       0        1       2          3

    if segments.len() < 4 {
        return Ok(Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .body(Full::new(Bytes::from("Missing datasource name")))?);
    }

    let datasource_name = segments[3];
    let user_db = match get_user_datasource(cp_db, cache, datasource_name).await {
        Ok(db) => db,
        Err(e) => {
            let error_json = json!({ "error": e.to_string() });
            return Ok(Response::builder()
                .status(StatusCode::BAD_REQUEST) // Or NOT_FOUND
                .header("Content-Type", "application/json")
                .body(Full::new(Bytes::from(error_json.to_string())))?);
        }
    };

    if segments.len() == 4 {
        return Ok(Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .body(Full::new(Bytes::from("Missing resource/table name")))?);
    }

    // /apify/admin/data/{ds}/tables -> List tables (Not implemented widely in modules yet, hard to do generically without specific backend support logic, but let's assume 'schema' module calls)
    // Actually, getting all tables usually requires querying `information_schema` or `sqlite_master`.
    // The current `DatabaseBackend` trait doesn't have `list_tables`.
    // We might need to extend it or implement a special query here based on driver.
    // For now, let's implement the specific table operations.

    let resource = segments[4];

    if resource == "tables" && method == hyper::Method::GET {
        let tables = user_db.list_tables().await?;
        return Ok(Response::builder()
            .status(StatusCode::OK)
            .header("Content-Type", "application/json")
            .body(Full::new(Bytes::from(json!(tables).to_string())))?);
    }

    // /apify/admin/data/{ds}/schema/{table}
    if resource == "schema" {
        if segments.len() < 6 {
            return Ok(Response::builder()
                .status(StatusCode::BAD_REQUEST)
                .body(Full::new(Bytes::from("Missing table name")))?);
        }
        let table_name = segments[5];
        let schema = user_db.get_table_schema(table_name).await?;
        return Ok(Response::builder()
            .status(StatusCode::OK)
            .header("Content-Type", "application/json")
            .body(Full::new(Bytes::from(json!(schema).to_string())))?);
    }

    // /apify/admin/data/{ds}/{table}/query
    let table_name = resource; // assume segment 4 is table name if not reserved keyword

    if segments.len() >= 6 && segments[5] == "query" && method == hyper::Method::POST {
        let body_bytes = http_body_util::BodyExt::collect(body).await?.to_bytes();
        let payload: Value = serde_json::from_slice(&body_bytes)?;

        let limit = payload
            .get("limit")
            .and_then(|v| v.as_u64())
            .map(|v| v as u32);
        let offset = payload
            .get("offset")
            .and_then(|v| v.as_u64())
            .map(|v| v as u32);

        let mut where_clause = None;
        if let Some(w) = payload.get("where").and_then(|v| v.as_object()) {
            let mut map = HashMap::new();
            for (k, v) in w {
                map.insert(k.clone(), v.clone());
            }
            where_clause = Some(map);
        }

        // TODO: Columns selection
        let columns = None;

        let rows = user_db
            .select(table_name, columns, where_clause, limit, offset)
            .await
            .map_err(|e| format!("Query error: {}", e))?;

        return Ok(Response::builder()
            .status(StatusCode::OK)
            .header("Content-Type", "application/json")
            .body(Full::new(Bytes::from(json!(rows).to_string())))?);
    }

    // CRUD on table
    // POST /apify/admin/data/{ds}/{table} -> Insert
    if method == hyper::Method::POST {
        let body_bytes = http_body_util::BodyExt::collect(body).await?.to_bytes();
        let data: HashMap<String, Value> = serde_json::from_slice(&body_bytes)?;

        let result = user_db
            .insert(table_name, data)
            .await
            .map_err(|e| format!("Insert error: {}", e))?;
        return Ok(Response::builder()
            .status(StatusCode::CREATED)
            .header("Content-Type", "application/json")
            .body(Full::new(Bytes::from(result.to_string())))?);
    }
    // PUT /apify/admin/data/{ds}/{table}/{id} -> Update
    // Need to handle ID. Schema might not be 'id'.
    // NOTE: This simple implementation assumes 'id' in URL maps to a where clause.
    // Ideally we should look up PK, but for now allow passing filters in body or Assume PK is id?
    // Let's try to support parsing ID from URL.
    if method == hyper::Method::PUT {
        if segments.len() < 6 {
            return Ok(Response::builder()
                .status(StatusCode::BAD_REQUEST)
                .body(Full::new(Bytes::from("Missing ID")))?);
        }
        let id = segments[5];

        let body_bytes = http_body_util::BodyExt::collect(body).await?.to_bytes();
        let data: HashMap<String, Value> = serde_json::from_slice(&body_bytes)?;

        // Construct where clause assuming PK is "id"
        // TODO: Make this flexible
        let mut where_clause = HashMap::new();
        // Try parsing ID as number if possible, else string
        if let Ok(num) = id.parse::<i64>() {
            where_clause.insert("id".to_string(), json!(num));
        } else {
            where_clause.insert("id".to_string(), json!(id));
        }

        let result = user_db
            .update(table_name, data, where_clause)
            .await
            .map_err(|e| format!("Update error: {}", e))?;
        return Ok(Response::builder()
            .status(StatusCode::OK)
            .header("Content-Type", "application/json")
            .body(Full::new(Bytes::from(result.to_string())))?);
    }

    // DELETE /apify/admin/data/{ds}/{table}/{id}
    if method == hyper::Method::DELETE {
        if segments.len() < 6 {
            return Ok(Response::builder()
                .status(StatusCode::BAD_REQUEST)
                .body(Full::new(Bytes::from("Missing ID")))?);
        }
        let id = segments[5];

        let mut where_clause = HashMap::new();
        if let Ok(num) = id.parse::<i64>() {
            where_clause.insert("id".to_string(), json!(num));
        } else {
            where_clause.insert("id".to_string(), json!(id));
        }

        user_db
            .delete(table_name, where_clause)
            .await
            .map_err(|e| format!("Delete error: {}", e))?;
        return Ok(Response::builder()
            .status(StatusCode::NO_CONTENT)
            .body(Full::new(Bytes::from("")))?);
    }

    Ok(Response::builder()
        .status(StatusCode::NOT_FOUND)
        .body(Full::new(Bytes::from("Not Found")))?)
}
