//! Integration tests for Apify server
//! Spawns the binary with a temporary config and issues HTTP requests.

// use assert_cmd::cargo::cargo_bin! macro via full path
use reqwest::Client;
use serial_test::serial;
use std::fs;
use std::net::{TcpListener, TcpStream};
use std::time::{Duration, Instant};
use tempfile::TempDir;
use tokio::process::Command as TokioCommand;

async fn wait_for_ready_tcp(
    host: &str,
    port: u16,
    timeout: Duration,
) -> Result<(), Box<dyn std::error::Error>> {
    let start = Instant::now();
    // Small initial delay to let the server bind the port
    tokio::time::sleep(Duration::from_millis(150)).await;
    while start.elapsed() < timeout {
        match TcpStream::connect((host, port)) {
            Ok(_) => return Ok(()),
            Err(e) => {
                eprintln!("TCP readiness connect error: {}", e);
            }
        }
        tokio::time::sleep(Duration::from_millis(150)).await;
    }
    Err("Server did not become ready in time".into())
}

#[tokio::test]
#[serial]
async fn key_auth_required_for_users() -> Result<(), Box<dyn std::error::Error>> {
    let temp = TempDir::new()?;
    let config_dir = temp.path();

    // Pick a free ephemeral port to avoid conflicts
    let listener = TcpListener::bind(("127.0.0.1", 0))?;
    let port = listener.local_addr()?.port();
    drop(listener);

    // Write minimal openapi users spec with x-modules key_auth
    let users_spec = r#"openapi:
  spec:
    openapi: "3.0.0"
    info:
      title: "Test Users"
      version: "1.0.0"
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
          responses:
            "200": { description: "ok" }
"#;
    let users_path = config_dir.join("users.yaml");
    fs::write(&users_path, users_spec)?;

    // Main config with consumer key
    let main_cfg = format!(
        r#"listeners:
  - port: {port}
    ip: 127.0.0.1
    protocol: HTTP
    apis:
      - path: ./users.yaml
    consumers:
      - name: test
        keys:
          - t-key-001
"#
    );
    let main_cfg_path = config_dir.join("config.yaml");
    fs::write(&main_cfg_path, main_cfg)?;

    // Spawn server
    let bin_path = assert_cmd::cargo::cargo_bin!("apify");
    let mut child = TokioCommand::new(bin_path)
        .arg("-c")
        .arg(main_cfg_path.to_string_lossy().to_string())
        .spawn()?;
    let ready_url = format!("http://127.0.0.1:{}/users", port);
    if let Err(e) = wait_for_ready_tcp("127.0.0.1", port, Duration::from_secs(15)).await {
        // Ensure child is terminated on readiness failure to avoid orphan processes
        let _ = child.kill().await;
        return Err(e);
    }

    let client = Client::builder().no_proxy().build()?;
    // No key -> 401
    let resp_no_key = client.get(&ready_url).send().await?;
    let status_no_key = resp_no_key.status().as_u16();
    let body_no_key = resp_no_key.text().await.unwrap_or_default();
    eprintln!(
        "Unauthorized probe status={} body={}",
        status_no_key, body_no_key
    );
    assert_eq!(
        status_no_key, 401,
        "users endpoint should require key_auth (got {} body {})",
        status_no_key, body_no_key
    );

    // With key -> 200 (empty list or whatever response)
    let resp_key = client
        .get(&ready_url)
        .header("X-Api-Key", "t-key-001")
        .send()
        .await?;
    let status_key = resp_key.status().as_u16();
    let body_key = resp_key.text().await.unwrap_or_default();
    eprintln!("Authorized probe status={} body={}", status_key, body_key);
    assert_eq!(
        status_key, 200,
        "users endpoint should succeed with valid key (got {} body {})",
        status_key, body_key
    );

    // Cleanup: kill child
    child.kill().await?;
    Ok(())
}
