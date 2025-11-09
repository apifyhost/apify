//! Verify non-auth endpoint (books) is accessible without key while users is protected

use reqwest::Client;
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
async fn books_is_open_users_is_protected() -> Result<(), Box<dyn std::error::Error>> {
    let temp = TempDir::new()?;
    let cfg_dir = temp.path();

    // Free port and temp DB
    let listener = TcpListener::bind(("127.0.0.1", 0))?;
    let port = listener.local_addr()?.port();
    drop(listener);
    let db_file = cfg_dir.join("test.sqlite");

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
          x-modules:
            access: ["key_auth"]
          responses: { "200": { description: "ok" } }
"#;
    fs::write(cfg_dir.join("users.yaml"), users_spec)?;

    // books API without auth
    let books_spec = r#"openapi:
  spec:
    openapi: "3.0.0"
    info: { title: "Books", version: "1.0.0" }
    x-table-schemas:
      - table_name: "books"
        columns:
          - { name: "id", column_type: "INTEGER", nullable: false, primary_key: true, unique: false, auto_increment: true, default_value: null }
          - { name: "title", column_type: "TEXT", nullable: false, primary_key: false, unique: false, auto_increment: false, default_value: null }
        indexes: []
    paths:
      /books:
        get:
          operationId: listBooks
          responses: { "200": { description: "ok" } }
"#;
    fs::write(cfg_dir.join("books.yaml"), books_spec)?;

    let main_cfg = format!(
        r#"datasource:
  test_db:
    driver: sqlite
    database: {}
    max_pool_size: 5

consumers:
  - name: test
    keys: [ t-key-001 ]

listeners:
  - port: {port}
    ip: 127.0.0.1
    protocol: HTTP
    apis:
      - path: ./users.yaml
        datasource: test_db
      - path: ./books.yaml
        datasource: test_db
"#,
        db_file.display()
    );
    let main_cfg_path = cfg_dir.join("config.yaml");
    fs::write(&main_cfg_path, main_cfg)?;

    // Spawn with test env
    let bin = assert_cmd::cargo::cargo_bin!("apify");
    let db_url = format!("sqlite:{}", db_file.display());
    let mut child = TokioCommand::new(bin)
        .env("APIFY_DB_URL", &db_url)
        .env("APIFY_THREADS", "1")
        .arg("-c")
        .arg(main_cfg_path.to_string_lossy().to_string())
        .spawn()?;

    wait_for_ready("127.0.0.1", port, Duration::from_secs(8)).await?;
    let client = Client::builder().no_proxy().build()?;
    let base = format!("http://127.0.0.1:{}", port);

    // books without key -> 200
    let r = client.get(format!("{}/books", base)).send().await?;
    assert_eq!(r.status(), 200);
    // users without key -> 401
    let r = client.get(format!("{}/users", base)).send().await?;
    assert_eq!(r.status(), 401);
    // users with key -> 200
    let r = client
        .get(format!("{}/users", base))
        .header("X-Api-Key", "t-key-001")
        .send()
        .await?;
    assert_eq!(r.status(), 200);

    let _ = child.kill().await;
    Ok(())
}
