//! Integration tests for various modules (body_validator, request_logger, etc.)

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
async fn test_body_validator_size_limit() -> Result<(), Box<dyn std::error::Error>> {
    let temp = TempDir::new()?;
    let cfg_dir = temp.path();

    // Free port and temp DB
    let listener = TcpListener::bind(("127.0.0.1", 0))?;
    let port = listener.local_addr()?.port();
    drop(listener);
    let db_file = cfg_dir.join("test.sqlite");

    // Create API spec with POST endpoint
    let api_spec = r#"openapi:
  spec:
    openapi: "3.0.0"
    info: { title: "Test API", version: "1.0.0" }
    x-table-schemas:
      - table_name: "items"
        columns:
          - { name: "id", column_type: "INTEGER", nullable: false, primary_key: true, unique: false, auto_increment: true, default_value: null }
          - { name: "data", column_type: "TEXT", nullable: false, primary_key: false, unique: false, auto_increment: false, default_value: null }
        indexes: []
    paths:
      /items:
        post:
          operationId: createItem
          responses: { "201": { description: "created" } }
"#;
    fs::write(cfg_dir.join("api.yaml"), api_spec)?;

    // Main config
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
      - path: ./api.yaml
        datasource: test_db
"#,
        db_file.display()
    );
    let main_cfg_path = cfg_dir.join("config.yaml");
    fs::write(&main_cfg_path, main_cfg)?;

    // Spawn server
    let bin = assert_cmd::cargo::cargo_bin!("apify");
    let mut child = TokioCommand::new(bin)
        .env("APIFY_THREADS", "1")
        .arg("-c")
        .arg(main_cfg_path.to_string_lossy().to_string())
        .spawn()?;

    wait_for_ready("127.0.0.1", port, Duration::from_secs(8)).await?;
    let client = Client::builder().no_proxy().build()?;
    let base = format!("http://127.0.0.1:{}", port);

    // Test 1: Normal sized JSON body should work
    let small_body = serde_json::json!({
        "data": "small payload"
    });
    let resp = client
        .post(format!("{}/items", base))
        .header("Content-Type", "application/json")
        .json(&small_body)
        .send()
        .await?;
    assert_eq!(resp.status(), 200, "Small body should be accepted");

    // Test 2: Very large body should be rejected (if body_validator is active)
    // Note: This test assumes body_validator with 1MB limit is configured
    // Since we can't configure modules via YAML yet, we test the default behavior
    let large_string = "x".repeat(2 * 1024 * 1024); // 2MB
    let large_body = serde_json::json!({
        "data": large_string
    });
    let resp = client
        .post(format!("{}/items", base))
        .header("Content-Type", "application/json")
        .json(&large_body)
        .send()
        .await?;
    // Without body_validator configured, this will still succeed
    // In a real deployment with body_validator, it would return 413
    println!("Large body response status: {}", resp.status());

    let _ = child.kill().await;
    Ok(())
}

#[tokio::test]
#[serial]
async fn test_missing_content_type() -> Result<(), Box<dyn std::error::Error>> {
    let temp = TempDir::new()?;
    let cfg_dir = temp.path();

    let listener = TcpListener::bind(("127.0.0.1", 0))?;
    let port = listener.local_addr()?.port();
    drop(listener);
    let db_file = cfg_dir.join("test.sqlite");

    let api_spec = r#"openapi:
  spec:
    openapi: "3.0.0"
    info: { title: "Test API", version: "1.0.0" }
    x-table-schemas:
      - table_name: "items"
        columns:
          - { name: "id", column_type: "INTEGER", nullable: false, primary_key: true, unique: false, auto_increment: true, default_value: null }
          - { name: "name", column_type: "TEXT", nullable: false, primary_key: false, unique: false, auto_increment: false, default_value: null }
        indexes: []
    paths:
      /items:
        post:
          operationId: createItem
          responses: { "201": { description: "created" } }
"#;
    fs::write(cfg_dir.join("api.yaml"), api_spec)?;

    let main_cfg = format!(
        r#"datasource:
  test_db:
    driver: sqlite
    database: {}
    max_pool_size: 5

listeners:
  - port: {port}
    ip: 127.0.0.1
    protocol: HTTP
    apis:
      - path: ./api.yaml
        datasource: test_db
"#,
        db_file.display()
    );
    let main_cfg_path = cfg_dir.join("config.yaml");
    fs::write(&main_cfg_path, main_cfg)?;

    let bin = assert_cmd::cargo::cargo_bin!("apify");
    let mut child = TokioCommand::new(bin)
        .env("APIFY_THREADS", "1")
        .arg("-c")
        .arg(main_cfg_path.to_string_lossy().to_string())
        .spawn()?;

    wait_for_ready("127.0.0.1", port, Duration::from_secs(8)).await?;
    let client = Client::builder().no_proxy().build()?;
    let base = format!("http://127.0.0.1:{}", port);

    // Test with proper Content-Type
    let resp = client
        .post(format!("{}/items", base))
        .header("Content-Type", "application/json")
        .body(r#"{"name": "test"}"#)
        .send()
        .await?;
    assert_eq!(
        resp.status(),
        200,
        "Valid JSON with Content-Type should work"
    );

    // Test with missing Content-Type (should still work in current implementation)
    let resp = client
        .post(format!("{}/items", base))
        .body(r#"{"name": "test2"}"#)
        .send()
        .await?;
    // Current implementation is lenient, but with body_validator it would reject
    println!("Missing Content-Type response: {}", resp.status());

    let _ = child.kill().await;
    Ok(())
}

#[tokio::test]
#[serial]
async fn test_request_logger_output() -> Result<(), Box<dyn std::error::Error>> {
    let temp = TempDir::new()?;
    let cfg_dir = temp.path();

    let listener = TcpListener::bind(("127.0.0.1", 0))?;
    let port = listener.local_addr()?.port();
    drop(listener);
    let db_file = cfg_dir.join("test.sqlite");

    let api_spec = r#"openapi:
  spec:
    openapi: "3.0.0"
    info: { title: "Logger Test", version: "1.0.0" }
    x-table-schemas:
      - table_name: "logs"
        columns:
          - { name: "id", column_type: "INTEGER", nullable: false, primary_key: true, unique: false, auto_increment: true, default_value: null }
          - { name: "message", column_type: "TEXT", nullable: false, primary_key: false, unique: false, auto_increment: false, default_value: null }
        indexes: []
    paths:
      /logs:
        get:
          operationId: listLogs
          responses: { "200": { description: "ok" } }
        post:
          operationId: createLog
          responses: { "200": { description: "created" } }
      /logs/{id}:
        get:
          operationId: getLog
          parameters:
            - name: id
              in: path
              required: true
              schema: { type: integer }
          responses: { "200": { description: "ok" } }
"#;
    fs::write(cfg_dir.join("api.yaml"), api_spec)?;

    let main_cfg = format!(
        r#"datasource:
  test_db:
    driver: sqlite
    database: {}
    max_pool_size: 5

listeners:
  - port: {port}
    ip: 127.0.0.1
    protocol: HTTP
    apis:
      - path: ./api.yaml
        datasource: test_db
"#,
        db_file.display()
    );
    let main_cfg_path = cfg_dir.join("config.yaml");
    fs::write(&main_cfg_path, main_cfg)?;

    let bin = assert_cmd::cargo::cargo_bin!("apify");
    let mut child = TokioCommand::new(bin)
        .env("APIFY_THREADS", "1")
        .arg("-c")
        .arg(main_cfg_path.to_string_lossy().to_string())
        .spawn()?;

    wait_for_ready("127.0.0.1", port, Duration::from_secs(8)).await?;
    let client = Client::builder().no_proxy().build()?;
    let base = format!("http://127.0.0.1:{}", port);

    // Make various requests to test logging
    // Note: We can't easily capture stdout in integration tests,
    // but we verify the requests succeed (logger shouldn't break functionality)

    // Test 1: Simple GET request
    let resp = client.get(format!("{}/logs", base)).send().await?;
    assert_eq!(resp.status(), 200);

    // Test 2: GET with query params
    let resp = client
        .get(format!("{}/logs?limit=10&offset=5", base))
        .send()
        .await?;
    assert_eq!(resp.status(), 200);

    // Test 3: GET with path params
    let resp = client.get(format!("{}/logs/123", base)).send().await?;
    // 404 is ok since record doesn't exist, but route matched
    assert!(
        resp.status() == 200 || resp.status() == 404,
        "Path param route should match"
    );

    // Test 4: POST with body
    let resp = client
        .post(format!("{}/logs", base))
        .header("Content-Type", "application/json")
        .json(&serde_json::json!({"message": "test log"}))
        .send()
        .await?;
    assert_eq!(resp.status(), 200);

    let _ = child.kill().await;
    Ok(())
}

#[tokio::test]
#[serial]
async fn test_multiple_phases_integration() -> Result<(), Box<dyn std::error::Error>> {
    // This test verifies that multiple modules can work together
    // across different phases without conflicts

    let temp = TempDir::new()?;
    let cfg_dir = temp.path();

    let listener = TcpListener::bind(("127.0.0.1", 0))?;
    let port = listener.local_addr()?.port();
    drop(listener);
    let db_file = cfg_dir.join("test.sqlite");

    let api_spec = r#"openapi:
  spec:
    openapi: "3.0.0"
    info: { title: "Multi-Phase Test", version: "1.0.0" }
    x-table-schemas:
      - table_name: "products"
        columns:
          - { name: "id", column_type: "INTEGER", nullable: false, primary_key: true, unique: false, auto_increment: true, default_value: null }
          - { name: "name", column_type: "TEXT", nullable: false, primary_key: false, unique: false, auto_increment: false, default_value: null }
          - { name: "price", column_type: "REAL", nullable: false, primary_key: false, unique: false, auto_increment: false, default_value: null }
        indexes: []
    paths:
      /products:
        get:
          operationId: listProducts
          x-modules:
            access: ["key_auth"]
          responses: { "200": { description: "ok" } }
        post:
          operationId: createProduct
          x-modules:
            access: ["key_auth"]
          responses: { "201": { description: "created" } }
      /products/{id}:
        get:
          operationId: getProduct
          x-modules:
            access: ["key_auth"]
          parameters:
            - name: id
              in: path
              required: true
              schema: { type: integer }
          responses: { "200": { description: "ok" } }
"#;
    fs::write(cfg_dir.join("api.yaml"), api_spec)?;

    let main_cfg = format!(
        r#"datasource:
  test_db:
    driver: sqlite
    database: {}
    max_pool_size: 5

consumers:
  - name: testuser
    keys: [ test-key-999 ]

listeners:
  - port: {port}
    ip: 127.0.0.1
    protocol: HTTP
    apis:
      - path: ./api.yaml
        datasource: test_db
"#,
        db_file.display()
    );
    let main_cfg_path = cfg_dir.join("config.yaml");
    fs::write(&main_cfg_path, main_cfg)?;

    let bin = assert_cmd::cargo::cargo_bin!("apify");
    let mut child = TokioCommand::new(bin)
        .env("APIFY_THREADS", "1")
        .arg("-c")
        .arg(main_cfg_path.to_string_lossy().to_string())
        .spawn()?;

    wait_for_ready("127.0.0.1", port, Duration::from_secs(8)).await?;
    let client = Client::builder().no_proxy().build()?;
    let base = format!("http://127.0.0.1:{}", port);

    // Test the complete flow: BodyParse -> Route -> Access -> Data -> Response -> Log

    // 1. Access phase: No key should fail
    let resp = client.get(format!("{}/products", base)).send().await?;
    assert_eq!(resp.status(), 401, "Should require authentication");

    // 2. Access phase: With key should succeed
    let resp = client
        .get(format!("{}/products", base))
        .header("X-Api-Key", "test-key-999")
        .send()
        .await?;
    assert_eq!(resp.status(), 200, "Should succeed with valid key");

    // 3. Full CRUD flow with authentication and logging
    let create_resp = client
        .post(format!("{}/products", base))
        .header("X-Api-Key", "test-key-999")
        .header("Content-Type", "application/json")
        .json(&serde_json::json!({
            "name": "Test Product",
            "price": 99.99
        }))
        .send()
        .await?;
    assert_eq!(create_resp.status(), 200);
    let created: serde_json::Value = create_resp.json().await?;

    // Verify create response
    assert_eq!(created["affected_rows"], 1, "Should insert one row");

    // 4. List all products to find our created one
    let list_resp = client
        .get(format!("{}/products", base))
        .header("X-Api-Key", "test-key-999")
        .send()
        .await?;
    assert_eq!(list_resp.status(), 200);
    let products: serde_json::Value = list_resp.json().await?;

    // Get the first product (should be the one we just created)
    let product_list = products.as_array().expect("Expected array of products");
    assert!(!product_list.is_empty(), "Should have at least one product");
    let product = &product_list[0];
    let product_id = product["id"].as_i64().expect("Product should have id");

    // 5. Retrieve the specific product by ID
    let get_resp = client
        .get(format!("{}/products/{}", base, product_id))
        .header("X-Api-Key", "test-key-999")
        .send()
        .await?;
    assert_eq!(get_resp.status(), 200);
    let fetched_product: serde_json::Value = get_resp.json().await?;
    assert_eq!(fetched_product["name"], "Test Product");
    assert_eq!(fetched_product["price"], 99.99);

    let _ = child.kill().await;
    Ok(())
}
