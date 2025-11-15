//! Request logger module (Log phase)
//! Logs request and response details

use crate::app_state::AppState;
use crate::modules::{Module, ModuleOutcome};
use crate::phases::{Phase, RequestContext};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

/// Request logger configuration
pub struct RequestLoggerConfig {
    /// Log request headers
    pub log_headers: bool,
    /// Log request body
    pub log_body: bool,
    /// Log response data
    pub log_response: bool,
}

impl Default for RequestLoggerConfig {
    fn default() -> Self {
        Self {
            log_headers: true,
            log_body: false, // Don't log body by default for security
            log_response: true,
        }
    }
}

/// Request logger module
pub struct RequestLogger {
    config: RequestLoggerConfig,
}

impl RequestLogger {
    pub fn new(config: RequestLoggerConfig) -> Self {
        Self { config }
    }

    pub fn with_defaults() -> Self {
        Self::new(RequestLoggerConfig::default())
    }

    pub fn verbose() -> Self {
        Self::new(RequestLoggerConfig {
            log_headers: true,
            log_body: true,
            log_response: true,
        })
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

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis();

        // Log basic request info
        println!(
            "[{}] {} {} - matched_route: {:?}",
            timestamp,
            ctx.method,
            ctx.path,
            ctx.matched_route.as_ref().map(|r| &r.path_pattern)
        );

        // Log headers if configured
        if self.config.log_headers && !ctx.headers.is_empty() {
            println!("  Headers:");
            for (name, value) in ctx.headers.iter() {
                if let Ok(val_str) = value.to_str() {
                    println!("    {}: {}", name, val_str);
                }
            }
        }

        // Log query params if present
        if !ctx.query_params.is_empty() {
            println!("  Query params: {:?}", ctx.query_params);
        }

        // Log path params if present
        if !ctx.path_params.is_empty() {
            println!("  Path params: {:?}", ctx.path_params);
        }

        // Log body if configured
        if self.config.log_body {
            if let Some(ref json_body) = ctx.json_body {
                println!("  Body: {}", json_body);
            } else if let Some(ref raw_body) = ctx.raw_body {
                println!("  Body size: {} bytes", raw_body.len());
            }
        }

        // Log response if configured
        if self.config.log_response
            && let Some(ref result) = ctx.result_json
        {
            println!("  Response: {}", result);
        }

        ModuleOutcome::Continue
    }
}
