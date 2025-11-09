//! Body validation module (BodyParse phase)
//! Validates request body size and structure

use crate::app_state::AppState;
use crate::hyper::StatusCode;
use crate::modules::{error_response, Module, ModuleOutcome};
use crate::phases::{Phase, RequestContext};
use std::sync::Arc;

/// Body validator configuration
pub struct BodyValidatorConfig {
    /// Maximum body size in bytes
    pub max_body_size: usize,
    /// Require JSON content-type header for JSON bodies
    pub enforce_content_type: bool,
}

impl Default for BodyValidatorConfig {
    fn default() -> Self {
        Self {
            max_body_size: 1024 * 1024, // 1MB default
            enforce_content_type: true,
        }
    }
}

/// Body validation module
pub struct BodyValidator {
    config: BodyValidatorConfig,
}

impl BodyValidator {
    pub fn new(config: BodyValidatorConfig) -> Self {
        Self { config }
    }

    pub fn with_defaults() -> Self {
        Self::new(BodyValidatorConfig::default())
    }
}

impl Module for BodyValidator {
    fn name(&self) -> &str {
        "body_validator"
    }

    fn phases(&self) -> &'static [Phase] {
        &[Phase::BodyParse]
    }

    fn run(&self, phase: Phase, ctx: &mut RequestContext, _state: &Arc<AppState>) -> ModuleOutcome {
        debug_assert_eq!(phase, Phase::BodyParse);

        // Check body size if body exists
        if let Some(ref body) = ctx.raw_body
            && body.len() > self.config.max_body_size
        {
            return ModuleOutcome::Respond(error_response(
                StatusCode::PAYLOAD_TOO_LARGE,
                &format!(
                    "Request body too large: {} bytes (max: {})",
                    body.len(),
                    self.config.max_body_size
                ),
            ));
        }

        // Enforce Content-Type header for JSON bodies
        if self.config.enforce_content_type && ctx.json_body.is_some() {
            if let Some(content_type) = ctx.headers.get("content-type") {
                let ct_str = content_type.to_str().unwrap_or("");
                if !ct_str.contains("application/json") {
                    return ModuleOutcome::Respond(error_response(
                        StatusCode::UNSUPPORTED_MEDIA_TYPE,
                        "Content-Type must be application/json for JSON bodies",
                    ));
                }
            } else {
                return ModuleOutcome::Respond(error_response(
                    StatusCode::BAD_REQUEST,
                    "Missing Content-Type header for JSON body",
                ));
            }
        }

        ModuleOutcome::Continue
    }
}
