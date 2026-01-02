use apify::database::{DatabaseManager, DatabaseRuntimeConfig};
use apify::schema_generator::{ColumnDefinition, TableSchema};
use serde_json::json;
use std::collections::HashMap;

#[tokio::test]
async fn test_schema_migration_sqlite() {
    // 1. Setup in-memory SQLite
    let config = DatabaseRuntimeConfig {
        driver: "sqlite".to_string(),
        url: "sqlite::memory:".to_string(),
        max_size: 1,
    };
    let db = DatabaseManager::new(config).await.unwrap();

    // 2. Define initial schema (id, name)
    let schema_v1 = TableSchema {
        table_name: "users".to_string(),
        columns: vec![
            ColumnDefinition {
                name: "id".to_string(),
                column_type: "INTEGER".to_string(),
                nullable: false,
                primary_key: true,
                unique: false,
                auto_increment: true,
                default_value: None,
                auto_field: false,
            },
            ColumnDefinition {
                name: "name".to_string(),
                column_type: "TEXT".to_string(),
                nullable: false,
                primary_key: false,
                unique: false,
                auto_increment: false,
                default_value: None,
                auto_field: false,
            },
        ],
        indexes: vec![],
        relations: vec![],
    };

    // 3. Initialize schema v1
    db.initialize_schema(vec![schema_v1.clone()]).await.unwrap();

    // 4. Insert data
    let mut data = HashMap::new();
    data.insert("name".to_string(), json!("Alice"));
    db.insert("users", data).await.unwrap();

    // 5. Define schema v2 (id, name, email)
    let mut schema_v2 = schema_v1.clone();
    schema_v2.columns.push(ColumnDefinition {
        name: "email".to_string(),
        column_type: "TEXT".to_string(),
        nullable: true,
        primary_key: false,
        unique: false,
        auto_increment: false,
        default_value: None,
        auto_field: false,
    });

    // 6. Initialize schema v2 (should migrate)
    db.initialize_schema(vec![schema_v2]).await.unwrap();

    // 7. Verify schema and data
    let rows = db.select("users", None, None, None, None).await.unwrap();
    assert_eq!(rows.len(), 1);
    let row = &rows[0];
    assert_eq!(row["name"], "Alice");
    // email column should exist (value might be null)
    // Note: select returns JSON values. If column is null, it might be Value::Null or omitted depending on implementation.
    // In our implementation, we select *, so it should be there.

    // 8. Insert data with new column
    let mut data2 = HashMap::new();
    data2.insert("name".to_string(), json!("Bob"));
    data2.insert("email".to_string(), json!("bob@example.com"));
    db.insert("users", data2).await.unwrap();

    let rows = db.select("users", None, None, None, None).await.unwrap();
    assert_eq!(rows.len(), 2);

    let bob = rows.iter().find(|r| r["name"] == "Bob").unwrap();
    assert_eq!(bob["email"], "bob@example.com");
}
