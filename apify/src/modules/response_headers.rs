//! Response headers module (Response phase)
//! Adds custom headers to responses

use crate::app_state::AppState;
use crate::modules::{Module, ModuleOutcome};
use crate::phases::{Phase, RequestContext};
use std::sync::Arc;

/// Response headers configuration
pub struct ResponseHeadersConfig {
    /// Headers to add to all responses
    pub headers: Vec<(String, String)>,
}

impl Default for ResponseHeadersConfig {
    fn default() -> Self {
        Self {
            headers: vec![
                ("X-Powered-By".to_string(), "Apify".to_string()),
                ("X-Content-Type-Options".to_string(), "nosniff".to_string()),
            ],
        }
    }
}

/// Response headers module - adds custom headers to responses
pub struct ResponseHeaders {
    #[allow(dead_code)]
    config: ResponseHeadersConfig,
}

impl ResponseHeaders {
    pub fn new(config: ResponseHeadersConfig) -> Self {
        Self { config }
    }

    pub fn with_defaults() -> Self {
        Self::new(ResponseHeadersConfig::default())
    }

    pub fn with_headers(headers: Vec<(String, String)>) -> Self {
        Self::new(ResponseHeadersConfig { headers })
    }
}

impl Module for ResponseHeaders {
    fn name(&self) -> &str {
        "response_headers"
    }

    fn phases(&self) -> &'static [Phase] {
        &[Phase::Response]
    }

    fn run(
        &self,
        phase: Phase,
        _ctx: &mut RequestContext,
        _state: &Arc<AppState>,
    ) -> ModuleOutcome {
        debug_assert_eq!(phase, Phase::Response);

        // Note: In a real implementation, you would modify the response headers
        // directly in the handler after this phase. For now, this is a placeholder
        // showing where response header modification would occur.

        // Future enhancement: Store headers in context for handler to apply
        // For example: ctx.response_headers = Some(self.config.headers.clone());

        ModuleOutcome::Continue
    }
}
