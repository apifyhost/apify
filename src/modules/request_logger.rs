//! Request logger module (Log phase)
//! Logs request and response details to file, stdout, or other destinations.

use crate::app_state::AppState;
use crate::config::AccessLogConfig;
use crate::modules::{Module, ModuleOutcome};
use crate::phases::{Phase, RequestContext};
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::sync::Arc;
use std::thread;
use tokio::sync::mpsc;
use sqlx::types::chrono::Local;

/// Access log entry structure
#[derive(Serialize)]
struct AccessLogEntry {
    timestamp: String,
    method: String,
    path: String,
    status: u16,
    duration_ms: u64,
    ip: String,
    user_agent: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
    request: RequestLogInfo,
    response: ResponseLogInfo,
}

#[derive(Serialize)]
struct RequestLogInfo {
    #[serde(skip_serializing_if = "Option::is_none")]
    headers: Option<HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    query: Option<HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    body: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cookies: Option<HashMap<String, String>>,
}

#[derive(Serialize)]
struct ResponseLogInfo {
    #[serde(skip_serializing_if = "Option::is_none")]
    headers: Option<HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    body: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cookies: Option<HashMap<String, String>>,
}

/// Request logger module
pub struct RequestLogger {
    sender: mpsc::UnboundedSender<String>,
    config: AccessLogConfig,
}

impl RequestLogger {
    pub fn new(config: Option<AccessLogConfig>) -> Self {
        let (sender, mut receiver) = mpsc::unbounded_channel();

        let config = config.unwrap_or(AccessLogConfig {
            enabled: Some(true),
            path: Some("logs/access.log".to_string()),
            format: Some("json".to_string()),
            headers: None,
            query: None,
            body: None,
            cookies: None,
        });

        if config.enabled.unwrap_or(true) {
            let path_str = config
                .path
                .clone()
                .unwrap_or_else(|| "logs/access.log".to_string());
            
            // Ensure directory exists
            if let Some(parent) = Path::new(&path_str).parent() {
                let _ = fs::create_dir_all(parent);
            }

            // Spawn a dedicated thread for logging to avoid blocking async runtime with file I/O
            thread::spawn(move || {
                let file = OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&path_str);

                let mut output: Box<dyn Write + Send> = match file {
                    Ok(f) => Box::new(f),
                    Err(e) => {
                        eprintln!("Failed to open access log file {}: {}", path_str, e);
                        // Fallback to stdout if file fails
                        Box::new(std::io::stdout())
                    }
                };

                while let Some(log_line) = receiver.blocking_recv() {
                    if let Err(e) = writeln!(output, "{}", log_line) {
                        eprintln!("Failed to write access log: {}", e);
                    }
                }
            });
        }

        Self { sender, config }
    }
}

impl Module for RequestLogger {
    fn name(&self) -> &str {
        "request_logger"
    }

    fn phases(&self) -> &'static [Phase] {
        &[Phase::Log]
    }

    fn run(&self, phase: Phase, ctx: &mut RequestContext, _state: &Arc<AppState>) -> ModuleOutcome {
        debug_assert_eq!(phase, Phase::Log);

        let duration = ctx.start_time.elapsed().as_millis() as u64;
        let status = ctx.response_status.unwrap_or(500);
        
        // Request Headers
        let req_headers = if let Some(header_names) = &self.config.headers {
            let mut h = HashMap::new();
            for name in header_names {
                if let Some(val) = ctx.headers.get(name) {
                    if let Ok(s) = val.to_str() {
                        h.insert(name.clone(), s.to_string());
                    }
                }
            }
            if h.is_empty() { None } else { Some(h) }
        } else {
            None
        };

        // Response Headers
        let res_headers = if let Some(header_names) = &self.config.headers {
            let mut h = HashMap::new();
            for name in header_names {
                if let Some(val) = ctx.response_headers.get(name) {
                    if let Ok(s) = val.to_str() {
                        h.insert(name.clone(), s.to_string());
                    }
                }
            }
            if h.is_empty() { None } else { Some(h) }
        } else {
            None
        };

        let query = if self.config.query.unwrap_or(false) {
            if ctx.query_params.is_empty() { None } else { Some(ctx.query_params.clone()) }
        } else {
            None
        };

        let req_body = if self.config.body.unwrap_or(false) {
            ctx.json_body.clone()
        } else {
            None
        };

        let res_body = if self.config.body.unwrap_or(false) {
            ctx.result_json.clone()
        } else {
            None
        };

        let req_cookies = if self.config.cookies.unwrap_or(false) {
            if let Some(val) = ctx.headers.get("cookie") {
                if let Ok(s) = val.to_str() {
                    let mut c = HashMap::new();
                    for part in s.split(';') {
                        let parts: Vec<&str> = part.splitn(2, '=').collect();
                        if parts.len() == 2 {
                            c.insert(parts[0].trim().to_string(), parts[1].trim().to_string());
                        }
                    }
                    if c.is_empty() { None } else { Some(c) }
                } else { None }
            } else { None }
        } else {
            None
        };

        let res_cookies = if self.config.cookies.unwrap_or(false) {
            // Parse Set-Cookie headers
            // Note: HeaderMap::get only returns the first value. We need get_all for Set-Cookie.
            let mut c = HashMap::new();
            for val in ctx.response_headers.get_all("set-cookie") {
                if let Ok(s) = val.to_str() {
                    // Set-Cookie format: name=value; Path=/; ...
                    let parts: Vec<&str> = s.splitn(2, ';').collect();
                    if let Some(first_part) = parts.first() {
                        let kv: Vec<&str> = first_part.splitn(2, '=').collect();
                        if kv.len() == 2 {
                            c.insert(kv[0].trim().to_string(), kv[1].trim().to_string());
                        }
                    }
                }
            }
            if c.is_empty() { None } else { Some(c) }
        } else {
            None
        };

        let entry = AccessLogEntry {
            timestamp: Local::now().to_rfc3339(),
            method: ctx.method.to_string(),
            path: ctx.path.to_string(),
            status,
            duration_ms: duration,
            ip: ctx.client_ip.map(|ip| ip.to_string()).unwrap_or_else(|| "0.0.0.0".to_string()),
            user_agent: ctx
                .headers
                .get("user-agent")
                .and_then(|v| v.to_str().ok())
                .map(|s| s.to_string()),
            error: None, // TODO: Capture error if any
            request: RequestLogInfo {
                headers: req_headers,
                query,
                body: req_body,
                cookies: req_cookies,
            },
            response: ResponseLogInfo {
                headers: res_headers,
                body: res_body,
                cookies: res_cookies,
            },
        };

        if let Ok(json) = serde_json::to_string(&entry) {
            let _ = self.sender.send(json);
        }

        ModuleOutcome::Continue
    }
}
