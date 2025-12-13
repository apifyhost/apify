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
    #[allow(dead_code)]
    provider_name: String,
}

impl OAuthModule {
    pub fn new(provider_name: String) -> Self {
        Self { provider_name }
    }
}

static DISCOVERY: OnceCell<OIDCDiscovery> = OnceCell::new();
static JWKS: OnceCell<serde_json::Value> = OnceCell::new();

fn resolve_url(url: &str) -> String {
    if std::env::var("APIFY_OAUTH_REPLACE_LOCALHOST").is_ok() {
        url.replace("localhost", "keycloak")
    } else {
        url.to_string()
    }
}

fn fetch_discovery(issuer: &str) -> Option<OIDCDiscovery> {
    let issuer = issuer.to_string();
    std::thread::spawn(move || {
        // In Docker environments, replace localhost with keycloak service name for actual HTTP requests
        // This allows tokens to have issuer=localhost while containers access keycloak service
        let actual_url = resolve_url(&issuer);

        let url = format!(
            "{}/.well-known/openid-configuration",
            actual_url.trim_end_matches('/')
        );
        reqwest::blocking::get(&url)
            .ok()?
            .json::<OIDCDiscovery>()
            .ok()
    })
    .join()
    .unwrap_or(None)
}

fn fetch_jwks(jwks_uri: &str) -> Option<serde_json::Value> {
    let jwks_uri = jwks_uri.to_string();
    std::thread::spawn(move || {
        reqwest::blocking::get(jwks_uri)
            .ok()?
            .json::<serde_json::Value>()
            .ok()
    })
    .join()
    .unwrap_or(None)
}

impl Module for OAuthModule {
    fn name(&self) -> &str {
        "oauth"
    }
    fn phases(&self) -> &'static [Phase] {
        &[Phase::Access]
    }

    fn run(&self, phase: Phase, ctx: &mut RequestContext, state: &Arc<AppState>) -> ModuleOutcome {
        debug_assert_eq!(phase, Phase::Access);

        // Extract bearer token
        let auth = ctx
            .headers
            .get("Authorization")
            .and_then(|v| v.to_str().ok());
        let Some(auth_val) = auth else {
            return ModuleOutcome::Respond(error_response(
                StatusCode::UNAUTHORIZED,
                "missing Authorization header",
            ));
        };
        if !auth_val.starts_with("Bearer ") {
            return ModuleOutcome::Respond(error_response(
                StatusCode::UNAUTHORIZED,
                "invalid auth scheme",
            ));
        }
        let token = auth_val.trim_start_matches("Bearer ").trim();
        if token.is_empty() {
            return ModuleOutcome::Respond(error_response(
                StatusCode::UNAUTHORIZED,
                "empty bearer token",
            ));
        }

        // Select provider config (first available for now)
        let provider_cfg = match state.oidc_providers.values().next() {
            Some(p) => {
                tracing::debug!(
                    issuer = %p.issuer,
                    "Using OIDC provider"
                );
                p
            }
            None => {
                tracing::error!("OAuth module called but no providers configured in state");
                return ModuleOutcome::Respond(error_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "oauth provider not configured",
                ));
            }
        };

        // Discovery caching
        let discovery = DISCOVERY.get_or_init(|| {
            tracing::info!(
                issuer = %provider_cfg.issuer,
                "Fetching OIDC discovery document"
            );
            match fetch_discovery(&provider_cfg.issuer) {
                Some(d) => {
                    tracing::info!(
                        issuer = %d.issuer,
                        jwks_uri = ?d.jwks_uri,
                        introspection_endpoint = ?d.introspection_endpoint,
                        "OIDC discovery successful"
                    );
                    d
                }
                None => {
                    tracing::error!(
                        issuer = %provider_cfg.issuer,
                        "Failed to fetch OIDC discovery document"
                    );
                    OIDCDiscovery {
                        issuer: provider_cfg.issuer.clone(),
                        jwks_uri: None,
                        introspection_endpoint: None,
                    }
                }
            }
        });

        // Attempt introspection if configured
        if provider_cfg.introspection.unwrap_or(true)
            && let Some(introspect_url) = &discovery.introspection_endpoint
            && let (Some(cid), Some(csec)) = (&provider_cfg.client_id, &provider_cfg.client_secret)
        {
            // Replace localhost with keycloak for Docker network access
            let actual_introspect_url = resolve_url(introspect_url);

            // Log token details for debugging (first 50 chars to avoid exposing full token)
            tracing::debug!(
                token_preview = &token[..token.len().min(50)],
                token_length = token.len(),
                "Introspecting token"
            );

            tracing::debug!(
                introspect_url = %actual_introspect_url,
                "Attempting token introspection"
            );

            let form = [("token", token.to_string())];
            let url = actual_introspect_url.clone();
            let client_id = cid.clone();
            let client_secret = csec.clone();

            let introspection_result = std::thread::spawn(move || {
                let client = reqwest::blocking::Client::new();
                let resp = client
                    .post(&url)
                    .basic_auth(client_id, Some(client_secret))
                    .form(&form)
                    .send();

                match resp {
                    Ok(r) => r.json::<serde_json::Value>().ok(),
                    Err(_) => None,
                }
            })
            .join()
            .unwrap_or(None);

            if let Some(json) = introspection_result {
                tracing::debug!(
                    response = %json,
                    "Token introspection full response"
                );
                if json
                    .get("active")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false)
                {
                    // Use subject or username
                    let subject = json
                        .get("sub")
                        .and_then(|v| v.as_str())
                        .or_else(|| json.get("username").and_then(|v| v.as_str()))
                        .unwrap_or("unknown");
                    ctx.extensions.insert(ConsumerIdentity {
                        name: subject.to_string(),
                    });
                    return ModuleOutcome::Continue;
                } else {
                    tracing::warn!("Token introspection returned inactive=false");
                    return ModuleOutcome::Respond(error_response(
                        StatusCode::UNAUTHORIZED,
                        "inactive token",
                    ));
                }
            }
        }

        // Fallback: verify JWT locally if JWKS available
        if let Some(jwks_uri) = &discovery.jwks_uri {
            // Replace localhost with keycloak for Docker network access
            let actual_jwks_uri = resolve_url(jwks_uri);

            let jwks_val = JWKS.get_or_init(|| {
                fetch_jwks(&actual_jwks_uri).unwrap_or(serde_json::json!({"keys": []}))
            });
            if let Some(keys) = jwks_val.get("keys").and_then(|v| v.as_array())
                && let Ok(header) = jsonwebtoken::decode_header(token)
                && let Some(kid) = header.kid
                && let Some(jwk) = keys
                    .iter()
                    .find(|k| k.get("kid").and_then(|v| v.as_str()) == Some(kid.as_str()))
                && let (Some(n), Some(e)) = (
                    jwk.get("n").and_then(|v| v.as_str()),
                    jwk.get("e").and_then(|v| v.as_str()),
                )
            {
                use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode};
                let decoding_key_res = DecodingKey::from_rsa_components(n, e);
                if let Ok(decoding_key) = decoding_key_res {
                    let mut validation = Validation::new(Algorithm::RS256);
                    validation.set_issuer(std::slice::from_ref(&discovery.issuer));
                    if let Some(aud) = &provider_cfg.audience {
                        validation.set_audience(std::slice::from_ref(aud));
                    }
                    if let Ok(data) = decode::<serde_json::Value>(token, &decoding_key, &validation)
                    {
                        let subject = data
                            .claims
                            .get("sub")
                            .and_then(|v| v.as_str())
                            .unwrap_or("unknown");
                        ctx.extensions.insert(ConsumerIdentity {
                            name: subject.to_string(),
                        });
                        return ModuleOutcome::Continue;
                    } else {
                        return ModuleOutcome::Respond(error_response(
                            StatusCode::UNAUTHORIZED,
                            "jwt validation failed",
                        ));
                    }
                }
            }
        }

        ModuleOutcome::Respond(error_response(
            StatusCode::UNAUTHORIZED,
            "token verification failed",
        ))
    }
}
