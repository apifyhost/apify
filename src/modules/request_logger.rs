//! Request logger module (Log phase)
//! Logs request and response details to file, stdout, or other destinations.

use crate::app_state::AppState;
use crate::config::AccessLogConfig;
use crate::modules::{Module, ModuleOutcome};
use crate::phases::{Phase, RequestContext};
use serde::Serialize;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::sync::Arc;
use std::thread;
use tokio::sync::mpsc;

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
}

/// Request logger module
pub struct RequestLogger {
    sender: mpsc::UnboundedSender<String>,
}

impl RequestLogger {
    pub fn new(config: Option<AccessLogConfig>) -> Self {
        let (sender, mut receiver) = mpsc::unbounded_channel();

        let config = config.unwrap_or(AccessLogConfig {
            enabled: Some(true),
            path: Some("logs/access.log".to_string()),
            format: Some("json".to_string()),
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

        Self { sender }
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
        
        let entry = AccessLogEntry {
            timestamp: chrono::Local::now().to_rfc3339(),
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
        };

        if let Ok(json) = serde_json::to_string(&entry) {
            let _ = self.sender.send(json);
        }

        ModuleOutcome::Continue
    }
}
