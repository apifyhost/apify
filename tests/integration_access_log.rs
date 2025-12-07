use reqwest::Client;
use serial_test::serial;
use std::fs;
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::time::{Duration, Instant};
use tempfile::TempDir;
use tokio::process::Command;

async fn wait_for_ready_tcp(
    host: &str,
    port: u16,
    timeout: Duration,
) -> Result<(), Box<dyn std::error::Error>> {
    let start = Instant::now();
    tokio::time::sleep(Duration::from_millis(150)).await;
    while start.elapsed() < timeout {
        match TcpStream::connect((host, port)) {
            Ok(_) => return Ok(()),
            Err(_) => {
                tokio::time::sleep(Duration::from_millis(150)).await;
            }
        }
    }
    Err("Server did not become ready in time".into())
}

async fn wait_for_log_content(
    path: &Path,
    substring: &str,
    timeout: Duration,
) -> Result<(), Box<dyn std::error::Error>> {
    let start = Instant::now();
    while start.elapsed() < timeout {
        if path.exists() {
            let content = fs::read_to_string(path)?;
            if content.contains(substring) {
                return Ok(());
            }
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    Err(format!("Log file {:?} did not contain '{}' in time", path, substring).into())
}

#[tokio::test]
#[serial]
async fn test_access_log_full_features() -> Result<(), Box<dyn std::error::Error>> {
    let temp = TempDir::new()?;
    let root = temp.path();

    // 1. Create API Spec
    let api_spec = r#"
openapi:
  spec:
    openapi: "3.0.0"
    info:
      title: "Log Test API"
      version: "1.0.0"
    paths:
      /echo:
        post:
          responses:
            '200':
              description: "OK"
"#;
    fs::write(root.join("api.yaml"), api_spec)?;

    // 2. Create Config
    // We use a random port
    let listener = TcpListener::bind(("127.0.0.1", 0))?;
    let port = listener.local_addr()?.port();
    drop(listener);

    let global_log_path = root.join("global.log");
    let api_log_path = root.join("api.log");

    let config_yaml = format!(
        r#"
datasource:
  default:
    driver: "sqlite"
    database: ":memory:"

listeners:
  - port: {port}
    ip: "127.0.0.1"
    protocol: "http"
    apis:
      - path: "api.yaml"
        access_log:
          enabled: true
          path: "{api_log}"
          format: "json"
          headers: ["user-agent", "x-test-header"]
          query: true
          body: true
          cookies: true

modules:
  access_log:
    enabled: true
    path: "{global_log}"
    format: "json"
"#,
        port = port,
        api_log = api_log_path.to_string_lossy(),
        global_log = global_log_path.to_string_lossy()
    );

    fs::write(root.join("config.yaml"), config_yaml)?;

    // 3. Start Server
    let bin_path = env!("CARGO_BIN_EXE_apify");
    let mut child = Command::new(bin_path)
        .arg("--config")
        .arg(root.join("config.yaml"))
        .kill_on_drop(true)
        .spawn()?;

    wait_for_ready_tcp("127.0.0.1", port, Duration::from_secs(5)).await?;
    tokio::time::sleep(Duration::from_secs(1)).await;

    // 4. Make Request
    let client = Client::builder()
        .no_proxy()
        .build()?;
    let url = format!("http://127.0.0.1:{}/echo?foo=bar", port);
    
    let resp = client
        .post(&url)
        .header("User-Agent", "TestAgent/1.0")
        .header("X-Test-Header", "test-value")
        .header("Cookie", "session=123")
        .json(&serde_json::json!({"msg": "hello"}))
        .send()
        .await?;

    let status = resp.status();
    let body = resp.text().await?;
    println!("Response status: {}, body: {}", status, body);

    // We expect 500 because "echo" table doesn't exist in sqlite memory
    // But we accept 503 if it happens during startup race, though we tried to wait.
    assert!(status.as_u16() == 500 || status.as_u16() == 503, "Unexpected status code: {}", status);

    // 5. Verify Global Log
    wait_for_log_content(&global_log_path, "/echo", Duration::from_secs(5)).await?;
    let global_content = fs::read_to_string(&global_log_path)?;
    
    assert!(global_content.contains(r#""path":"/echo""#));
    assert!(global_content.contains(r#""method":"POST""#));
    // Status might be 500 or 503
    assert!(global_content.contains(&format!(r#""status":{}"#, status.as_u16())));

    // 6. Verify API Log (Detailed)
    wait_for_log_content(&api_log_path, "TestAgent", Duration::from_secs(5)).await?;
    let api_content = fs::read_to_string(&api_log_path)?;
    
    // Check Headers
    assert!(api_content.contains(r#""user-agent":"TestAgent/1.0""#));
    assert!(api_content.contains(r#""x-test-header":"test-value""#));
    
    // Check Query
    assert!(api_content.contains(r#""foo":"bar""#));
    
    // Check Body
    assert!(api_content.contains(r#""msg":"hello""#));
    
    // Check Cookies
    assert!(api_content.contains(r#""session":"123""#));

    // Check Structure
    assert!(api_content.contains(r#""request":{"#));
    assert!(api_content.contains(r#""response":{"#));

    // Cleanup
    child.kill().await?;

    Ok(())
}
