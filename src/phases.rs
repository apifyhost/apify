//! Execution phases and request context for the HTTP pipeline

use crate::api_generator::RoutePattern;
use crate::hyper::http::Extensions;
use crate::hyper::{HeaderMap, Method, Uri};
use serde_json::Value;
use std::collections::HashMap;

/// Logical execution phases (subset; SSL phases reserved for future TLS support)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Init,
    // SslHello,
    // Ssl,
    HeaderParse,
    BodyParse,
    Route,
    Access,
    Data,
    Response,
    Log,
}

use std::time::Instant;

/// Mutable request-scoped context passed between phases
#[derive(Debug)]
pub struct RequestContext {
    pub start_time: Instant,
    pub client_ip: Option<std::net::IpAddr>,
    pub response_status: Option<u16>,
    pub method: Method,
    pub uri: Uri,
    pub path: String,
    pub headers: HeaderMap,
    pub response_headers: HeaderMap, // Response headers
    pub query_params: HashMap<String, String>,
    pub path_params: HashMap<String, String>,
    pub raw_body: Option<Vec<u8>>, // avoid extra dependency for now
    pub json_body: Option<Value>,
    pub matched_route: Option<RoutePattern>,
    pub result_json: Option<Value>,
    pub extensions: Extensions, // typed storage for modules (auth claims, tracing, etc.)
}

impl RequestContext {
    pub fn new(
        method: Method,
        uri: Uri,
        headers: HeaderMap,
        client_ip: Option<std::net::IpAddr>,
    ) -> Self {
        let path = uri.path().to_string();
        Self {
            start_time: Instant::now(),
            client_ip,
            response_status: None,
            method,
            uri,
            path,
            headers,
            response_headers: HeaderMap::new(),
            query_params: HashMap::new(),
            path_params: HashMap::new(),
            raw_body: None,
            json_body: None,
            matched_route: None,
            result_json: None,
            extensions: Extensions::default(),
        }
    }
}
