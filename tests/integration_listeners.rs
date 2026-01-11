//! Integration tests for API-Listener binding logic
//! Verifies that APIs correctly attach to specified listeners (ports).

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
async fn test_api_listener_bindings() -> Result<(), Box<dyn std::error::Error>> {
    let temp = TempDir::new()?;
    let dir = temp.path();

    // Allocate 3 ports
    let l1 = TcpListener::bind(("127.0.0.1", 0))?;
    let port1 = l1.local_addr()?.port();
    drop(l1);

    let l2 = TcpListener::bind(("127.0.0.1", 0))?;
    let port2 = l2.local_addr()?.port();
    drop(l2);

    let l3 = TcpListener::bind(("127.0.0.1", 0))?;
    let port3 = l3.local_addr()?.port();
    drop(l3);

    let db_file = dir.join("test.sqlite");

    // API 1: Public API (attached to listener 1)
    let api1_spec = r#"openapi:
  spec:
    openapi: "3.0.0"
    info: { title: "Public API", version: "1.0.0" }
    x-table-schemas:
      - tableName: "public_items"
        columns:
          - { name: "id", columnType: "INTEGER", nullable: false, primaryKey: true, unique: false, autoIncrement: true, defaultValue: null }
        indexes: []
    paths:
      /public:
        get:
          x-table-name: "public_items"
          responses: { "200": { description: "ok" } }
"#;
    fs::write(dir.join("api1.yaml"), api1_spec)?;

    // API 2: Admin API (attached to listener 2)
    let api2_spec = r#"openapi:
  spec:
    openapi: "3.0.0"
    info: { title: "Admin API", version: "1.0.0" }
    x-table-schemas:
      - tableName: "admin_items"
        columns:
          - { name: "id", columnType: "INTEGER", nullable: false, primaryKey: true, unique: false, autoIncrement: true, defaultValue: null }
        indexes: []
    paths:
      /admin:
        get:
          x-table-name: "admin_items"
          responses: { "200": { description: "ok" } }
"#;
    fs::write(dir.join("api2.yaml"), api2_spec)?;

    // API 3: Shared API (attached to listener 1 and 2)
    let api3_spec = r#"openapi:
  spec:
    openapi: "3.0.0"
    info: { title: "Shared API", version: "1.0.0" }
    x-table-schemas:
      - tableName: "shared_items"
        columns:
          - { name: "id", columnType: "INTEGER", nullable: false, primaryKey: true, unique: false, autoIncrement: true, defaultValue: null }
        indexes: []
    paths:
      /shared:
        get:
          x-table-name: "shared_items"
          responses: { "200": { description: "ok" } }
"#;
    fs::write(dir.join("api3.yaml"), api3_spec)?;

    // API 4: Orphan API (attached to no listeners, or non-existent listener)
    let api4_spec = r#"openapi:
  spec:
    openapi: "3.0.0"
    info: { title: "Orphan API", version: "1.0.0" }
    x-table-schemas:
      - table_name: "orphan_items"
        columns:
          - { name: "id", column_type: "INTEGER", nullable: false, primary_key: true, unique: false, auto_increment: true, default_value: null }
        indexes: []
    paths:
      /orphan:
        get:
          x-table-name: "orphan_items"
          responses: { "200": { description: "ok" } }
"#;
    fs::write(dir.join("api4.yaml"), api4_spec)?;

    let config = format!(
        r#"datasource:
  default:
    driver: sqlite
    database: {}

listeners:
  - name: public
    port: {port1}
    ip: 127.0.0.1
    protocol: HTTP
  - name: admin
    port: {port2}
    ip: 127.0.0.1
    protocol: HTTP
  - name: unused
    port: {port3}
    ip: 127.0.0.1
    protocol: HTTP

apis:
  - path: ./api1.yaml
    listeners: [public]
  - path: ./api2.yaml
    listeners: [admin]
  - path: ./api3.yaml
    listeners: [public, admin]
  - path: ./api4.yaml
    listeners: [non_existent]
"#,
        db_file.display()
    );

    let config_path = dir.join("config.yaml");
    fs::write(&config_path, config)?;

    // Spawn server
    let bin = assert_cmd::cargo::cargo_bin!("apify");
    let mut child = TokioCommand::new(bin)
        .env("APIFY_THREADS", "1")
        .arg("-c")
        .arg(config_path.to_string_lossy().to_string())
        .spawn()?;

    // Wait for both listeners
    wait_for_ready("127.0.0.1", port1, Duration::from_secs(5)).await?;
    wait_for_ready("127.0.0.1", port2, Duration::from_secs(5)).await?;

    let client = Client::builder().no_proxy().build()?;
    let base1 = format!("http://127.0.0.1:{}", port1);
    let base2 = format!("http://127.0.0.1:{}", port2);
    let base3 = format!("http://127.0.0.1:{}", port3);

    // Test Listener 1 (Public)
    // Should have /public and /shared
    // Should NOT have /admin or /orphan
    let resp = client.get(format!("{}/public", base1)).send().await?;
    assert_eq!(resp.status(), 200, "Listener 1 should serve /public");

    let resp = client.get(format!("{}/shared", base1)).send().await?;
    assert_eq!(resp.status(), 200, "Listener 1 should serve /shared");

    let resp = client.get(format!("{}/admin", base1)).send().await?;
    assert_eq!(resp.status(), 404, "Listener 1 should NOT serve /admin");

    // Test Listener 2 (Admin)
    // Should have /admin and /shared
    // Should NOT have /public
    let resp = client.get(format!("{}/admin", base2)).send().await?;
    assert_eq!(resp.status(), 200, "Listener 2 should serve /admin");

    let resp = client.get(format!("{}/shared", base2)).send().await?;
    assert_eq!(resp.status(), 200, "Listener 2 should serve /shared");

    let resp = client.get(format!("{}/public", base2)).send().await?;
    assert_eq!(resp.status(), 404, "Listener 2 should NOT serve /public");

    // Test Orphan API
    // Should not be reachable on any listener
    let resp = client.get(format!("{}/orphan", base1)).send().await?;
    assert_eq!(resp.status(), 404);
    let resp = client.get(format!("{}/orphan", base2)).send().await?;
    assert_eq!(resp.status(), 404);

    // Test Unused Listener
    // Should be up but have no APIs (except healthz)
    wait_for_ready("127.0.0.1", port3, Duration::from_secs(5)).await?;
    let resp = client.get(format!("{}/public", base3)).send().await?;
    assert_eq!(resp.status(), 404);
    let resp = client.get(format!("{}/admin", base3)).send().await?;
    assert_eq!(resp.status(), 404);

    let _ = child.kill().await;
    std::fs::write("test_success_marker", "ok")?;
    Ok(())
}
