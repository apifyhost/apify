//! Module system root: traits, registry, shared types

use crate::phases::{Phase, RequestContext};
use crate::app_state::AppState;
use crate::hyper::{Response, StatusCode};
use crate::http_body_util::Full;
use crate::hyper::body::Bytes;
use std::sync::Arc;
use std::error::Error;

pub mod key_auth;

/// Result of executing a module hook
pub enum ModuleOutcome {
    Continue,
    Respond(Response<Full<Bytes>>),
    Error(Box<dyn Error + Send + Sync>),
}

pub trait Module: Send + Sync {
    fn name(&self) -> &str;
    fn phases(&self) -> &'static [Phase];
    fn run(&self, phase: Phase, ctx: &mut RequestContext, state: &Arc<AppState>) -> ModuleOutcome;
}

#[derive(Clone, Debug)]
pub struct ConsumerIdentity { pub name: String }

#[derive(Clone)]
pub struct ModuleRegistry { modules: Vec<Arc<dyn Module>> }
impl ModuleRegistry {
    pub fn new() -> Self { Self { modules: Vec::new() } }
    pub fn with(mut self, module: Arc<dyn Module>) -> Self { self.modules.push(module); self }
    pub fn has_phase(&self, phase: Phase) -> bool { self.modules.iter().any(|m| m.phases().iter().any(|p| *p == phase)) }
    pub fn run_phase(&self, phase: Phase, ctx: &mut RequestContext, state: &Arc<AppState>) -> Option<ModuleOutcome> {
        for m in &self.modules { if m.phases().iter().any(|p| *p == phase) { match m.run(phase, ctx, state) { ModuleOutcome::Continue => {}, other => return Some(other) } } }
        None
    }
}
impl Default for ModuleRegistry { fn default() -> Self { Self::new() } }

/// Helper for building error response bodies
pub fn error_response(status: StatusCode, message: &str) -> Response<Full<Bytes>> {
    let body = serde_json::json!({"error": message, "status": status.as_u16()}).to_string();
    Response::builder().status(status).header("Content-Type", "application/json").body(Full::new(Bytes::from(body))).unwrap()
}
