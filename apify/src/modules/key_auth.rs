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
        let header_val = ctx.headers.get("X-Api-Key").and_then(|v| v.to_str().ok());
        if header_val.is_none() {
            return ModuleOutcome::Respond(error_response(
                StatusCode::UNAUTHORIZED,
                "missing api key",
            ));
        }
        let key = header_val.unwrap();
        if let Some(consumer) = state.lookup_consumer_by_key(key) {
            ctx.extensions.insert(ConsumerIdentity {
                name: consumer.name.clone(),
            });
            ModuleOutcome::Continue
        } else {
            ModuleOutcome::Respond(error_response(StatusCode::UNAUTHORIZED, "invalid api key"))
        }
    }
}
