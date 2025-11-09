//! HTTP request handling logic

use super::app_state::AppState;
use super::crud_handler::CRUDError;
use super::hyper::{Request, Response, StatusCode};
use super::{Arc, http_body_util::Full, hyper::body::Bytes};
use crate::modules::ModuleOutcome;
use crate::phases::{Phase, RequestContext};
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
    let method = parts.method.clone();

    // Phase: HeaderParse (and build initial context)
    let mut ctx = RequestContext::new(method.clone(), parts.uri.clone(), parts.headers.clone());
    ctx.extensions = parts.extensions; // carry over existing request extensions
    ctx.query_params = extract_query_params(parts.uri.query());

    // Health endpoint shortcut
    if method == hyper::Method::GET && ctx.path == "/healthz" {
        return Ok(create_json_response(
            StatusCode::OK,
            serde_json::json!({"status":"ok"}).to_string(),
        ));
    }

    // Try CRUD handler first if available
    if let Some(crud_handler) = &state.crud_handler {
        // Phase: BodyParse (only for methods that expect body)
        if matches!(method.as_str(), "POST" | "PUT" | "PATCH") {
            let body_bytes = http_body_util::BodyExt::collect(body_stream)
                .await?
                .to_bytes();
            if !body_bytes.is_empty() {
                ctx.raw_body = Some(body_bytes.to_vec());
                match serde_json::from_slice::<Value>(&body_bytes) {
                    Ok(value) => {
                        ctx.json_body = Some(value);
                    }
                    Err(_) => {
                        return Ok(create_error_response(
                            StatusCode::BAD_REQUEST,
                            "Invalid JSON body",
                        ));
                    }
                }
            }
        };

        // Phase: Route - determine matched route and extract path params
        if let Some(pattern) = crud_handler
            .api_generator
            .match_operation(method.as_str(), &ctx.path)
        {
            ctx.matched_route = Some(pattern.clone());
            ctx.path_params = crud_handler
                .api_generator
                .extract_path_params(pattern, &ctx.path);
        }

        // Phase: Access
        // Prefer operation-level modules > route-level > listener-level
        let op_registry = if let Some(ref pattern) = ctx.matched_route {
            let key = format!(
                "{} {}",
                method.as_str().to_uppercase(),
                pattern.path_pattern
            );
            state.operation_modules.get(&key).cloned()
        } else {
            None
        };
        let route_registry = if let Some(ref pattern) = ctx.matched_route {
            state.route_modules.get(&pattern.path_pattern).cloned()
        } else {
            None
        };

        if let Some(reg) = op_registry
            .as_ref()
            .or(route_registry.as_ref())
            .or_else(|| {
                if state.modules.has_phase(Phase::Access) {
                    Some(&state.modules)
                } else {
                    None
                }
            })
            && let Some(outcome) = reg.run_phase(Phase::Access, &mut ctx, &state)
        {
            match outcome {
                ModuleOutcome::Continue => {}
                ModuleOutcome::Respond(resp) => {
                    return Ok(resp);
                }
                ModuleOutcome::Error(e) => {
                    eprintln!("Module error: {e}");
                    return Ok(create_error_response(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "Module error",
                    ));
                }
            }
        }

        // Phase: Data (CRUD execution)
        match crud_handler
            .handle_request(
                method.as_str(),
                &ctx.path,
                ctx.path_params.clone(),
                ctx.query_params.clone(),
                ctx.json_body.clone(),
            )
            .await
        {
            Ok(result) => {
                ctx.result_json = Some(result);
            }
            Err(CRUDError::NotFoundError(_)) => {
                return Ok(create_error_response(
                    StatusCode::NOT_FOUND,
                    "Resource not found",
                ));
            }
            Err(CRUDError::ValidationError(msg)) => {
                return Ok(create_error_response(StatusCode::BAD_REQUEST, &msg));
            }
            Err(CRUDError::InvalidParameterError(msg)) => {
                return Ok(create_error_response(StatusCode::BAD_REQUEST, &msg));
            }
            Err(CRUDError::DatabaseError(e)) => {
                eprintln!("Database error: {:?}", e);
                return Ok(create_error_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    &format!("Database error: {}", e),
                ));
            }
        };

        // Phase: Response (serialize JSON)
        if let Some(ref val) = ctx.result_json {
            let json_response = serde_json::to_string(val)
                .map_err(|e| format!("Failed to serialize response: {}", e))?;
            
            // Phase: Log (after response is ready)
            let _ = state.modules.run_phase(Phase::Log, &mut ctx, &state);
            
            return Ok(create_json_response(StatusCode::OK, json_response));
        }

        // Should not reach here
        Ok(create_error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Empty response",
        ))
    } else {
        // Fallback to original route matching
        let (status, body) = match state.match_route(&ctx.path, &method) {
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
