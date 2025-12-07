//! Key-based authentication module (Access phase)
//! Uses X-Api-Key header to identify a configured consumer.

use super::{ConsumerIdentity, Module, ModuleOutcome, error_response};
use crate::app_state::AppState;
use crate::hyper::StatusCode;
use crate::phases::{Phase, RequestContext};
use std::sync::Arc;

pub struct KeyAuthModule;
impl Default for KeyAuthModule {
    fn default() -> Self {
        Self::new()
    }
}

impl KeyAuthModule {
    pub fn new() -> Self {
        Self
    }
}

impl Module for KeyAuthModule {
    fn name(&self) -> &str {
        "key_auth"
    }
    fn phases(&self) -> &'static [Phase] {
        &[Phase::Access]
    }

    fn run(&self, phase: Phase, ctx: &mut RequestContext, state: &Arc<AppState>) -> ModuleOutcome {
        debug_assert_eq!(phase, Phase::Access);

        if let Some(authenticators) = &state.auth_config {
            for authenticator in authenticators {
                if let crate::config::Authenticator::ApiKey(cfg) = authenticator {
                    if !cfg.enabled.unwrap_or(true) {
                        continue;
                    }

                    let key_name = cfg.config.key_name.as_deref().unwrap_or("X-Api-Key");
                    // TODO: Support Query source
                    if let Some(key) = ctx.headers.get(key_name).and_then(|v| v.to_str().ok()) {
                        if let Some(consumer) = state.lookup_consumer_by_key(key) {
                            ctx.extensions.insert(ConsumerIdentity {
                                name: consumer.name.clone(),
                            });
                            return ModuleOutcome::Continue;
                        }
                    }
                }
            }
        }

        ModuleOutcome::Respond(error_response(
            StatusCode::UNAUTHORIZED,
            "missing or invalid api key",
        ))
    }
}
