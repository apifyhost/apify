//! OpenAPI Documentation module
//! Serves Swagger UI and OpenAPI JSON specification

use crate::app_state::AppState;
use crate::http_body_util::Full;
use crate::hyper::body::Bytes;
use crate::hyper::{Request, Response, StatusCode};
use std::error::Error;
use std::sync::Arc;

/// Handle documentation requests (Swagger UI + OpenAPI JSON)
pub async fn handle_docs_request(
    req: Request<hyper::body::Incoming>,
    state: Arc<AppState>,
) -> Result<Response<Full<Bytes>>, Box<dyn Error + Send + Sync>> {
    let path = req.uri().path();

    // Serve OpenAPI JSON
    if req.method() == crate::hyper::Method::GET && path == "/openapi.json" {
        if let Some(crud) = &state.crud_handler {
            let spec = crud.api_generator.get_spec();
            let body = match serde_json::to_string(spec) {
                Ok(s) => s,
                Err(e) => {
                    return Ok(Response::builder()
                        .status(StatusCode::INTERNAL_SERVER_ERROR)
                        .header("Content-Type", "application/json")
                        .body(Full::new(Bytes::from(format!(
                            r#"{{"error": "Failed to serialize OpenAPI spec: {}"}}"#,
                            e
                        ))))
                        .map_err(|e| format!("Failed to build response: {}", e))?);
                }
            };
            return Ok(Response::builder()
                .status(StatusCode::OK)
                .header("Content-Type", "application/json")
                .body(Full::new(Bytes::from(body)))
                .map_err(|e| format!("Failed to build response: {}", e))?);
        } else {
            // Return empty/default spec if no APIs are configured
            let empty_spec = serde_json::json!({
                "openapi": "3.0.0",
                "info": {
                    "title": "Apify API",
                    "version": "1.0.0",
                    "description": "No APIs configured yet."
                },
                "paths": {},
                "components": {
                    "schemas": {}
                }
            });
            
            return Ok(Response::builder()
                .status(StatusCode::OK)
                .header("Content-Type", "application/json")
                .body(Full::new(Bytes::from(empty_spec.to_string())))
                .map_err(|e| format!("Failed to build response: {}", e))?);
        }
    }

    // Serve Swagger UI assets (embedded)
    if req.method() == crate::hyper::Method::GET {
        match path {
            "/docs/swagger-ui.css" => {
                return Ok(Response::builder()
                    .status(StatusCode::OK)
                    .header("Content-Type", "text/css")
                    .body(Full::new(Bytes::from(include_str!(
                        "../static/swagger-ui.css"
                    ))))
                    .map_err(|e| format!("Failed to build response: {}", e))?);
            }
            "/docs/swagger-ui-bundle.js" => {
                return Ok(Response::builder()
                    .status(StatusCode::OK)
                    .header("Content-Type", "application/javascript")
                    .body(Full::new(Bytes::from(include_str!(
                        "../static/swagger-ui-bundle.js"
                    ))))
                    .map_err(|e| format!("Failed to build response: {}", e))?);
            }
            "/docs/swagger-ui-standalone-preset.js" => {
                return Ok(Response::builder()
                    .status(StatusCode::OK)
                    .header("Content-Type", "application/javascript")
                    .body(Full::new(Bytes::from(include_str!(
                        "../static/swagger-ui-standalone-preset.js"
                    ))))
                    .map_err(|e| format!("Failed to build response: {}", e))?);
            }
            "/docs" | "/docs/" => {
                let html = r#"<!DOCTYPE html>
<html lang="en">
  <head>
    <meta charset="UTF-8" />
    <title>API Docs</title>
    <link rel="stylesheet" href="/docs/swagger-ui.css" />
    <style>body { margin: 0; } #swagger-ui { min-height: 100vh; }</style>
  </head>
  <body>
    <div id="swagger-ui"></div>
    <script src="/docs/swagger-ui-bundle.js"></script>
    <script src="/docs/swagger-ui-standalone-preset.js"></script>
    <script>
      window.onload = () => {
        SwaggerUIBundle({
          url: '/openapi.json',
          dom_id: '#swagger-ui',
          presets: [
            SwaggerUIBundle.presets.apis,
            SwaggerUIStandalonePreset
          ],
          layout: 'StandaloneLayout'
        });
      };
    </script>
  </body>
</html>"#;

                return Ok(Response::builder()
                    .status(StatusCode::OK)
                    .header("Content-Type", "text/html; charset=utf-8")
                    .body(Full::new(Bytes::from(html)))
                    .map_err(|e| format!("Failed to build response: {}", e))?);
            }
            _ => {}
        }
    }

    // Fallback for unknown docs paths
    Ok(Response::builder()
        .status(StatusCode::NOT_FOUND)
        .header("Content-Type", "application/json")
        .body(Full::new(Bytes::from(r#"{"error": "Not Found"}"#)))
        .map_err(|e| format!("Failed to build response: {}", e))?)
}
