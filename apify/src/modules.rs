//! Built-in module system for phase hooks
//!
//! Modules are internal (not user plugins) and can attach logic at specific phases.
//! They can read & mutate `RequestContext`, store typed data in `extensions`, and
//! return control decisions (continue, short-circuit with response, or error).

use crate::phases::{Phase, RequestContext};
use crate::app_state::AppState;
use crate::hyper::{Response, StatusCode};
use crate::http_body_util::Full;
use crate::hyper::body::Bytes;
use std::sync::Arc;
use std::error::Error;

/// Result of executing a module hook
pub enum ModuleOutcome {
    /// Continue to next module / phase
    Continue,
    /// Short-circuit the pipeline and immediately send this response
    Respond(Response<Full<Bytes>>),
    /// Abort with an error (converted to 500 or mapped higher up)
    Error(Box<dyn Error + Send + Sync>),
}

/// Trait implemented by all internal modules attaching to phases
pub trait Module: Send + Sync {
    /// Human-readable name for diagnostics
    fn name(&self) -> &str;
    /// Phases this module wants to run in (could be multiple)
    fn phases(&self) -> &'static [Phase];
    /// Execute logic for a specific phase
    fn run(&self, phase: Phase, ctx: &mut RequestContext, state: &Arc<AppState>) -> ModuleOutcome;
}

/// Simple access control module checking presence of `X-Auth` header
pub struct AuthHeaderModule;

impl AuthHeaderModule {
    pub fn new() -> Self { Self }
}

impl Module for AuthHeaderModule {
    fn name(&self) -> &str { "auth_header" }
    fn phases(&self) -> &'static [Phase] { &[Phase::Access] }

    fn run(&self, phase: Phase, ctx: &mut RequestContext, _state: &Arc<AppState>) -> ModuleOutcome {
        debug_assert_eq!(phase, Phase::Access);
        // Check header
        if ctx.headers.get("X-Auth").is_some() {
            // Store a flag in extensions
            ctx.extensions.insert(AuthStatus { authorized: true });
            ModuleOutcome::Continue
        } else {
            let body = serde_json::json!({
                "error": "missing auth header",
                "status": 401u16
            }).to_string();
            let resp = Response::builder()
                .status(StatusCode::UNAUTHORIZED)
                .header("Content-Type", "application/json")
                .body(Full::new(Bytes::from(body)))
                .unwrap();
            ModuleOutcome::Respond(resp)
        }
    }
}

/// Typed extension value stored after auth check
#[derive(Clone, Debug)]
pub struct AuthStatus { pub authorized: bool }

/// Registry holding active modules
#[derive(Clone)]
pub struct ModuleRegistry {
    modules: Vec<Arc<dyn Module>>,
}

impl ModuleRegistry {
    pub fn new() -> Self { Self { modules: Vec::new() } }
    pub fn with(mut self, module: Arc<dyn Module>) -> Self { self.modules.push(module); self }
    /// Check if any module is registered for the given phase
    pub fn has_phase(&self, phase: Phase) -> bool {
        self.modules.iter().any(|m| m.phases().iter().any(|p| *p == phase))
    }

    /// Run modules for a given phase sequentially; first non-Continue outcome stops.
    pub fn run_phase(&self, phase: Phase, ctx: &mut RequestContext, state: &Arc<AppState>) -> Option<ModuleOutcome> {
        for m in &self.modules {
            if m.phases().iter().any(|p| *p == phase) {
                match m.run(phase, ctx, state) {
                    ModuleOutcome::Continue => continue,
                    other => return Some(other),
                }
            }
        }
        None
    }
}

impl Default for ModuleRegistry { fn default() -> Self { Self::new() } }
