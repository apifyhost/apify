//! HTTP request handling logic

use super::app_state::AppState;
use super::crud_handler::CRUDError;
use super::hyper::{Request, Response, StatusCode};
use super::{Arc, http_body_util::Full, hyper::body::Bytes};
use serde_json::Value;
use std::collections::HashMap;
use std::error::Error;

/// Handle HTTP request and generate response
// Updated error type to cover all possible errors
pub async fn handle_request(
    req: Request<hyper::body::Incoming>,
    state: Arc<AppState>,
) -> Result<Response<Full<Bytes>>, Box<dyn Error + Send + Sync>> {
    let (parts, body_stream) = req.into_parts();
    let path = parts.uri.path();
    let method = parts.method.clone();
    let query_params = extract_query_params(parts.uri.query());

    // Try CRUD handler first if available
    if let Some(crud_handler) = &state.crud_handler {
        // Extract path parameters from the route
        let path_params = extract_path_params(&state, path, &method);

        // Parse request body for POST/PUT/PATCH requests
        let body = if matches!(method.as_str(), "POST" | "PUT" | "PATCH") {
            let body_bytes = http_body_util::BodyExt::collect(body_stream)
                .await?
                .to_bytes();
            if body_bytes.is_empty() {
                None
            } else {
                match serde_json::from_slice::<Value>(&body_bytes) {
                    Ok(value) => Some(value),
                    Err(_) => {
                        return Ok(create_error_response(
                            StatusCode::BAD_REQUEST,
                            "Invalid JSON body",
                        ));
                    }
                }
            }
        } else {
            None
        };

        match crud_handler
            .handle_request(method.as_str(), path, path_params, query_params, body)
            .await
        {
            Ok(result) => {
                let json_response = serde_json::to_string(&result)
                    .map_err(|e| format!("Failed to serialize response: {}", e))?;
                Ok(create_json_response(StatusCode::OK, json_response))
            }
            Err(CRUDError::NotFoundError(_)) => Ok(create_error_response(
                StatusCode::NOT_FOUND,
                "Resource not found",
            )),
            Err(CRUDError::ValidationError(msg)) => {
                Ok(create_error_response(StatusCode::BAD_REQUEST, &msg))
            }
            Err(CRUDError::InvalidParameterError(msg)) => {
                Ok(create_error_response(StatusCode::BAD_REQUEST, &msg))
            }
            Err(CRUDError::DatabaseError(e)) => {
                eprintln!("Database error: {:?}", e);
                Ok(create_error_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    &format!("Database error: {}", e),
                ))
            }
        }
    } else {
        // Fallback to original route matching
        let (status, body) = match state.match_route(path, &method) {
            Some(resp_body) => (StatusCode::OK, resp_body.clone()),
            None => (StatusCode::NOT_FOUND, "Not Found".to_string()),
        };

        // Build response - fixed error handling
        let response = Response::builder()
            .status(status)
            .body(Full::new(Bytes::from(body)))
            .map_err(|e| format!("Failed to build response: {}", e))?;

        Ok(response)
    }
}

/// Extract query parameters from URI query string
fn extract_query_params(query: Option<&str>) -> HashMap<String, String> {
    let mut params = HashMap::new();
    if let Some(query_str) = query {
        for pair in query_str.split('&') {
            if let Some((key, value)) = pair.split_once('=') {
                params.insert(key.to_string(), value.to_string());
            }
        }
    }
    params
}

/// Extract path parameters from route matching
fn extract_path_params(
    state: &AppState,
    path: &str,
    method: &hyper::Method,
) -> HashMap<String, String> {
    // Try to extract path parameters using the CRUD handler's route patterns
    if let Some(crud_handler) = &state.crud_handler {
        if let Some(pattern) = crud_handler.api_generator.match_operation(method.as_str(), path) {
            return crud_handler.api_generator.extract_path_params(pattern, path);
        }
    }
    HashMap::new()
}

/// Create a JSON response
fn create_json_response(status: StatusCode, body: String) -> Response<Full<Bytes>> {
    Response::builder()
        .status(status)
        .header("Content-Type", "application/json")
        .body(Full::new(Bytes::from(body)))
        .unwrap_or_else(|_| {
            Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(Full::new(Bytes::from("Internal Server Error")))
                .unwrap()
        })
}

/// Create an error response
fn create_error_response(status: StatusCode, message: &str) -> Response<Full<Bytes>> {
    let error_body = serde_json::json!({
        "error": message,
        "status": status.as_u16()
    });

    create_json_response(status, error_body.to_string())
}
