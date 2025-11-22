//! OAuth / OpenID Connect authentication module (Access phase)
//! Validates bearer tokens via OIDC discovery + JWKS and optional introspection.
//! NOTE: This intentionally avoids treating tokens purely as JWT if introspection is configured.

use super::{ConsumerIdentity, Module, ModuleOutcome, error_response};
use crate::app_state::AppState;
use crate::hyper::StatusCode;
use crate::phases::{Phase, RequestContext};
use once_cell::sync::OnceCell;
use serde::Deserialize;
use std::sync::Arc;

// Minimal cached provider metadata
#[derive(Debug, Clone, Deserialize)]
struct OIDCDiscovery {
    issuer: String,
    jwks_uri: Option<String>,
    introspection_endpoint: Option<String>,
}

#[derive(Clone)]
pub struct OAuthModule {
    provider_name: String,
}

impl OAuthModule {
    pub fn new(provider_name: String) -> Self { Self { provider_name } }
}

static DISCOVERY: OnceCell<OIDCDiscovery> = OnceCell::new();
static JWKS: OnceCell<serde_json::Value> = OnceCell::new();

fn fetch_discovery(issuer: &str) -> Option<OIDCDiscovery> {
    let url = format!("{}/.well-known/openid-configuration", issuer.trim_end_matches('/'));
    reqwest::blocking::get(&url).ok()?.json::<OIDCDiscovery>().ok()
}

fn fetch_jwks(jwks_uri: &str) -> Option<serde_json::Value> {
    reqwest::blocking::get(jwks_uri).ok()?.json::<serde_json::Value>().ok()
}

impl Module for OAuthModule {
    fn name(&self) -> &str { "oauth" }
    fn phases(&self) -> &'static [Phase] { &[Phase::Access] }

    fn run(&self, phase: Phase, ctx: &mut RequestContext, state: &Arc<AppState>) -> ModuleOutcome {
        debug_assert_eq!(phase, Phase::Access);

        // Extract bearer token
        let auth = ctx.headers.get("Authorization").and_then(|v| v.to_str().ok());
        let Some(auth_val) = auth else {
            return ModuleOutcome::Respond(error_response(StatusCode::UNAUTHORIZED, "missing Authorization header"));
        };
        if !auth_val.starts_with("Bearer ") {
            return ModuleOutcome::Respond(error_response(StatusCode::UNAUTHORIZED, "invalid auth scheme"));
        }
        let token = auth_val.trim_start_matches("Bearer ").trim();
        if token.is_empty() {
            return ModuleOutcome::Respond(error_response(StatusCode::UNAUTHORIZED, "empty bearer token"));
        }

        // Select provider config (first available for now)
        let provider_cfg = match state.oauth_providers.values().next() {
            Some(p) => p,
            None => return ModuleOutcome::Respond(error_response(StatusCode::INTERNAL_SERVER_ERROR, "oauth provider not configured")),
        };

        // Discovery caching
        let discovery = DISCOVERY.get_or_init(|| {
            fetch_discovery(&provider_cfg.issuer).unwrap_or(OIDCDiscovery { issuer: provider_cfg.issuer.clone(), jwks_uri: None, introspection_endpoint: None })
        });

        // Attempt introspection if configured
        if provider_cfg.introspection.unwrap_or(true) {
            if let Some(introspect_url) = &discovery.introspection_endpoint {
                if let (Some(cid), Some(csec)) = (&provider_cfg.client_id, &provider_cfg.client_secret) {
                    let form = [ ("token", token) ];
                    let client = reqwest::blocking::Client::new();
                    let resp = client.post(introspect_url)
                        .basic_auth(cid, Some(csec))
                        .form(&form)
                        .send();
                    if let Ok(r) = resp {
                        if let Ok(json) = r.json::<serde_json::Value>() {
                            if json.get("active").and_then(|v| v.as_bool()).unwrap_or(false) {
                                // Use subject or username
                                let subject = json.get("sub").and_then(|v| v.as_str()).or_else(|| json.get("username").and_then(|v| v.as_str())).unwrap_or("unknown");
                                ctx.extensions.insert(ConsumerIdentity { name: subject.to_string() });
                                return ModuleOutcome::Continue;
                            } else {
                                return ModuleOutcome::Respond(error_response(StatusCode::UNAUTHORIZED, "inactive token"));
                            }
                        }
                    }
                }
            }
        }

        // Fallback: verify JWT locally if JWKS available
        if let Some(jwks_uri) = &discovery.jwks_uri {
            let jwks_val = JWKS.get_or_init(|| fetch_jwks(jwks_uri).unwrap_or(serde_json::json!({"keys": []})));
            if let Some(keys) = jwks_val.get("keys").and_then(|v| v.as_array()) {
                if let Ok(header) = jsonwebtoken::decode_header(token) {
                    if let Some(kid) = header.kid {
                        if let Some(jwk) = keys.iter().find(|k| k.get("kid").and_then(|v| v.as_str()) == Some(kid.as_str())) {
                            if let (Some(n), Some(e)) = (jwk.get("n").and_then(|v| v.as_str()), jwk.get("e").and_then(|v| v.as_str())) {
                                use jsonwebtoken::{decode, DecodingKey, Validation, Algorithm};
                                let decoding_key_res = DecodingKey::from_rsa_components(n, e);
                                if let Ok(decoding_key) = decoding_key_res {
                                    let mut validation = Validation::new(Algorithm::RS256);
                                    validation.set_issuer(&[discovery.issuer.clone()]);
                                    if let Some(aud) = &provider_cfg.audience { validation.set_audience(&[aud.clone()]); }
                                    if let Ok(data) = decode::<serde_json::Value>(token, &decoding_key, &validation) {
                                        let subject = data.claims.get("sub").and_then(|v| v.as_str()).unwrap_or("unknown");
                                        ctx.extensions.insert(ConsumerIdentity { name: subject.to_string() });
                                        return ModuleOutcome::Continue;
                                    } else {
                                        return ModuleOutcome::Respond(error_response(StatusCode::UNAUTHORIZED, "jwt validation failed"));
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        ModuleOutcome::Respond(error_response(StatusCode::UNAUTHORIZED, "token verification failed"))
    }
}
