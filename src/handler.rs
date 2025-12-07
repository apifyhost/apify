//! HTTP request handling logic

use super::app_state::AppState;
use super::crud_handler::CRUDError;
use super::hyper::{Request, Response, StatusCode};
use super::{Arc, http_body_util::Full, hyper::body::Bytes};
use crate::modules::ModuleOutcome;
use crate::modules::metrics::RequestMetrics;
use crate::phases::{Phase, RequestContext};
use serde_json::Value;
use std::collections::HashMap;
use std::error::Error;

/// Handle HTTP request and generate response
// Updated error type to cover all possible errors
#[tracing::instrument(skip(req, state), fields(http.method = %req.method(), http.uri = %req.uri()))]
pub async fn handle_request(
    req: Request<hyper::body::Incoming>,
    state: Arc<AppState>,
) -> Result<Response<Full<Bytes>>, Box<dyn Error + Send + Sync>> {
    let (parts, body_stream) = req.into_parts();
    let method = parts.method.clone();
    let path = parts.uri.path().to_string();
    let client_ip = parts
        .extensions
        .get::<std::net::SocketAddr>()
        .map(|addr| addr.ip());

    // Start metrics tracking
    let metrics = RequestMetrics::new(method.as_str(), &path);

    // Phase: HeaderParse (and build initial context)
    let mut ctx = RequestContext::new(
        method.clone(),
        parts.uri.clone(),
        parts.headers.clone(),
        client_ip,
    );
    ctx.extensions = parts.extensions; // carry over existing request extensions
    ctx.query_params = extract_query_params(parts.uri.query());

    // Inner handler that returns response
    let response = handle_request_inner(&mut ctx, body_stream, state.clone()).await?;

    // Record metrics before returning
    metrics.record(response.status().as_u16());

    // Capture response details for logging
    ctx.response_status = Some(response.status().as_u16());
    ctx.response_headers = response.headers().clone();

    // Phase: Log (after response is ready)
    // 1. Global modules
    let _ = state.modules.run_phase(Phase::Log, &mut ctx, &state);

    // 2. Route/Operation modules
    let matched_route = ctx.matched_route.clone();
    if let Some(pattern) = matched_route {
        // Route modules
        if let Some(reg) = state.route_modules.get(&pattern.path_pattern) {
            let _ = reg.run_phase(Phase::Log, &mut ctx, &state);
        }
        // Operation modules
        let key = format!(
            "{} {}",
            method.as_str().to_uppercase(),
            pattern.path_pattern
        );
        if let Some(reg) = state.operation_modules.get(&key) {
            let _ = reg.run_phase(Phase::Log, &mut ctx, &state);
        }
    }

    Ok(response)
}

/// Internal request handler (separated to ensure metrics are recorded)
async fn handle_request_inner(
    ctx: &mut RequestContext,
    body_stream: hyper::body::Incoming,
    state: Arc<AppState>,
) -> Result<Response<Full<Bytes>>, Box<dyn Error + Send + Sync>> {
    let method = ctx.method.clone();

    // Health endpoint shortcut
    if method == hyper::Method::GET && ctx.path == "/healthz" {
        return Ok(create_json_response(
            StatusCode::OK,
            serde_json::json!({"status":"ok"}).to_string(),
        ));
    }

    // Control Plane handling
    if let Some(db) = &state.control_plane_db {
        if ctx.path.starts_with("/_meta/") {
            let mut req_builder = Request::builder()
                .method(ctx.method.clone())
                .uri(ctx.uri.clone());
            if let Some(headers) = req_builder.headers_mut() {
                *headers = ctx.headers.clone();
            }
            
            return crate::control_plane::handle_control_plane_request(
                req_builder.body(body_stream)?,
                db,
            ).await;
        }
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
                .extract_path_params(&pattern, &ctx.path);
        }

        // Determine active registry for Access and BodyParse phases
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

        let active_registry = op_registry
            .as_ref()
            .or(route_registry.as_ref())
            .or_else(|| {
                // Fallback to listener modules if they have relevant phases
                if state.modules.has_phase(Phase::Access)
                    || state.modules.has_phase(Phase::BodyParse)
                {
                    Some(&state.modules)
                } else {
                    None
                }
            });

        // Phase: BodyParse (Validation)
        if let Some(reg) = active_registry
            && let Some(outcome) = reg.run_phase(Phase::BodyParse, ctx, &state)
        {
            match outcome {
                ModuleOutcome::Continue => {}
                ModuleOutcome::Respond(resp) => {
                    return Ok(resp);
                }
                ModuleOutcome::Error(e) => {
                    eprintln!("BodyParse Module error: {e}");
                    return Ok(create_error_response(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "Module error",
                    ));
                }
            }
        }

        // Phase: Access
        if let Some(reg) = active_registry
            && let Some(outcome) = reg.run_phase(Phase::Access, ctx, &state)
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
        eprintln!("Debug: Executing CRUD for {} {}", method, ctx.path);
        match crud_handler
            .handle_request(
                method.as_str(),
                &ctx.path,
                ctx.path_params.clone(),
                ctx.query_params.clone(),
                ctx.json_body.clone(),
                ctx,
            )
            .await
        {
            Ok(result) => {
                eprintln!("Debug: CRUD success for {} {}", method, ctx.path);
                ctx.result_json = Some(result);
            }
            Err(CRUDError::NotFoundError(_)) => {
                return Ok(create_error_response(
                    StatusCode::NOT_FOUND,
                    "Resource not found",
                ));
            }
            Err(CRUDError::ValidationError(msg)) => {
                eprintln!("Validation Error: {}", msg);
                return Ok(create_error_response(StatusCode::BAD_REQUEST, &msg));
            }
            Err(CRUDError::InvalidParameterError(msg)) => {
                eprintln!("Invalid Parameter Error: {}", msg);
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
