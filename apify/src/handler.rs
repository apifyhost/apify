//! HTTP request handling logic

use super::app_state::AppState;
use super::hyper::{Request, Response, StatusCode};
use super::{Arc, http_body_util::Full, hyper::body::Bytes};
use std::error::Error;

/// Handle HTTP request and generate response
// Updated error type to cover all possible errors
pub async fn handle_request(
    req: Request<hyper::body::Incoming>,
    state: Arc<AppState>,
) -> Result<Response<Full<Bytes>>, Box<dyn Error + Send + Sync>> {
    let path = req.uri().path();
    let method = req.method().clone();

    // Match route and generate response
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
