//! CRUD flow tests for users endpoint

use reqwest::Client;
use serde_json::Value;
use serial_test::serial;
use std::fs;
use std::net::TcpListener;
use std::time::Duration;
use tempfile::TempDir;
use tokio::process::Command as TokioCommand;

async fn wait_for_ready(
    host: &str,
    port: u16,
    timeout: Duration,
) -> Result<(), Box<dyn std::error::Error>> {
    let client = reqwest::Client::builder().no_proxy().build()?;
    let start = std::time::Instant::now();
    while start.elapsed() < timeout {
        if let Ok(resp) = client
            .get(format!("http://{}:{}/healthz", host, port))
            .send()
            .await
            && resp.status().as_u16() == 200
        {
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(120)).await;
    }
    Err("Server did not become ready in time".into())
}

#[tokio::test]
#[serial]
async fn users_crud_flow() -> Result<(), Box<dyn std::error::Error>> {
    let temp = TempDir::new()?;
    let dir = temp.path();
    let listener = TcpListener::bind(("127.0.0.1", 0))?;
    let port = listener.local_addr()?.port();
    drop(listener);
    let db_file = dir.join("crud.sqlite");

    // users API with auth
    let users_spec = r#"openapi:
  spec:
    openapi: "3.0.0"
    info: { title: "Users", version: "1.0.0" }
    x-table-schemas:
      - table_name: "users"
        columns:
          - { name: "id", column_type: "INTEGER", nullable: false, primary_key: true, unique: false, auto_increment: true, default_value: null }
          - { name: "name", column_type: "TEXT", nullable: false, primary_key: false, unique: false, auto_increment: false, default_value: null }
        indexes: []
    paths:
      /users:
        get:
          operationId: listUsers
          x-modules: { access: ["key_auth", "database"] }
          responses: { "200": { description: "ok" } }
        post:
          operationId: createUser
          x-modules: { access: ["key_auth", "database"] }
          responses: { "200": { description: "ok" } }
      /users/{id}:
        get:
          operationId: getUser
          x-modules: { access: ["key_auth", "database"] }
          responses: { "200": { description: "ok" } }
        put:
          operationId: updateUser
          x-modules: { access: ["key_auth", "database"] }
          responses: { "200": { description: "ok" } }
        delete:
          operationId: deleteUser
          x-modules: { access: ["key_auth", "database"] }
          responses: { "200": { description: "ok" } }
"#;
    fs::write(dir.join("users.yaml"), users_spec)?;

    let cfg = format!(
        r#"listeners:
  - port: {port}
    ip: 127.0.0.1
    protocol: HTTP
    apis: [ {{ path: ./users.yaml }} ]
    consumers: [ {{ name: test, keys: [ t-key-001 ] }} ]
"#
    );
    let cfg_path = dir.join("config.yaml");
    fs::write(&cfg_path, cfg)?;

    // Provide database.yaml with init_schemas so tables are created before CRUD
    let db_cfg = format!(
        r#"database:
  driver: sqlite
  host: localhost
  port: 0
  user: user
  password: pass
  database: {}
  operations: ["init_schemas"]
"#,
        db_file.display()
    );
    fs::write(dir.join("database.yaml"), db_cfg)?;

    // Spawn
    let bin = assert_cmd::cargo::cargo_bin!("apify");
    let db_url = format!("sqlite:{}", db_file.display());
    let mut child = TokioCommand::new(bin)
        .env("APIFY_DB_URL", &db_url)
        .env("APIFY_THREADS", "1")
        .arg("-c")
        .arg(cfg_path.to_string_lossy().to_string())
        .spawn()?;

    wait_for_ready("127.0.0.1", port, Duration::from_secs(8)).await?;
    let client = Client::builder().no_proxy().build()?;
    let base = format!("http://127.0.0.1:{}", port);
    let key = ("X-Api-Key", "t-key-001");

    // Create
    let r = client
        .post(format!("{}/users", base))
        .header(key.0, key.1)
        .json(&serde_json::json!({"name":"Alice"}))
        .send()
        .await?;
    assert_eq!(r.status(), 200);

    // List and pick the inserted id
    let r = client
        .get(format!("{}/users", base))
        .header(key.0, key.1)
        .send()
        .await?;
    assert_eq!(r.status(), 200);
    let arr: Value = r.json().await?;
    let arr = arr.as_array().cloned().unwrap_or_default();
    let user = arr
        .iter()
        .find(|v| v.get("name").and_then(|n| n.as_str()) == Some("Alice"))
        .cloned()
        .expect("inserted user not found");
    let id = user.get("id").and_then(|v| v.as_i64()).unwrap_or_else(|| {
        user.get("id")
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse::<i64>().ok())
            .expect("id not parsable")
    });

    // Get by id
    let r = client
        .get(format!("{}/users/{}", base, id))
        .header(key.0, key.1)
        .send()
        .await?;
    assert_eq!(r.status(), 200);

    // Update
    let r = client
        .put(format!("{}/users/{}", base, id))
        .header(key.0, key.1)
        .json(&serde_json::json!({"name":"Alice2"}))
        .send()
        .await?;
    assert_eq!(r.status(), 200);

    // Verify
    let r = client
        .get(format!("{}/users/{}", base, id))
        .header(key.0, key.1)
        .send()
        .await?;
    assert_eq!(r.status(), 200);
    let obj: Value = r.json().await?;
    assert_eq!(obj.get("name").and_then(|v| v.as_str()), Some("Alice2"));

    // Delete
    let r = client
        .delete(format!("{}/users/{}", base, id))
        .header(key.0, key.1)
        .send()
        .await?;
    assert_eq!(r.status(), 200);

    // Ensure gone
    let r = client
        .get(format!("{}/users/{}", base, id))
        .header(key.0, key.1)
        .send()
        .await?;
    assert_eq!(r.status(), 404);

    let _ = child.kill().await;
    Ok(())
}
