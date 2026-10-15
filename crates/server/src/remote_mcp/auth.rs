use std::collections::BTreeSet;
#[cfg(test)]
use std::{sync::Arc, time::Duration};

#[cfg(test)]
use axum::{
    extract::{Request, State},
    http::{HeaderValue, StatusCode, header},
    middleware::Next,
    response::{IntoResponse, Response},
};
#[cfg(test)]
use serde::Deserialize;
#[cfg(test)]
use serde_json::Value;

#[cfg(test)]
use super::config::Config;

#[derive(Clone)]
pub struct Principal {
    pub subject: String,
    pub scopes: BTreeSet<String>,
}

impl Principal {
    pub fn allows(&self, scope: &str) -> bool {
        self.scopes.contains(scope)
    }
}

/// The trusted authorization server verifies cryptography and revocation. LVK
/// validates the resource, owner and permissions in its RFC 7662 response.
#[cfg(test)]
#[derive(Deserialize)]
pub(super) struct Introspection {
    pub active: bool,
    pub token_type: Option<String>,
    pub sub: Option<String>,
    pub iss: Option<String>,
    pub aud: Option<Value>,
    pub exp: Option<i64>,
    pub nbf: Option<i64>,
    pub scope: Option<String>,
}

#[cfg(test)]
impl Introspection {
    pub(super) fn principal(self, config: &Config, now: i64) -> Option<Principal> {
        let audience = self.aud.as_ref().is_some_and(|aud| {
            aud.as_str() == Some(config.resource().as_str())
                || aud.as_array().is_some_and(|values| {
                    values
                        .iter()
                        .any(|v| v.as_str() == Some(config.resource().as_str()))
                })
        });
        if !self.active
            || !self
                .token_type
                .as_deref()
                .is_some_and(|t| t.eq_ignore_ascii_case("bearer"))
            || self.sub.as_deref() != Some(&config.owner_subject)
            || self.iss.as_deref() != Some(&config.issuer)
            || !audience
            || self.exp.is_none_or(|exp| exp <= now)
            || self.nbf.is_some_and(|nbf| nbf > now)
        {
            return None;
        }
        Some(Principal {
            subject: self.sub?,
            scopes: self
                .scope
                .unwrap_or_default()
                .split_whitespace()
                .map(str::to_owned)
                .collect(),
        })
    }
}

#[cfg(test)]
#[derive(Clone)]
pub struct Auth {
    pub config: Arc<Config>,
    client: reqwest::Client,
    secret: Arc<String>,
}

#[cfg(test)]
impl Auth {
    pub fn new(config: Arc<Config>, secret: String) -> anyhow::Result<Self> {
        anyhow::ensure!(
            !secret.trim().is_empty(),
            "Missing MCP introspection secret"
        );
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(Duration::from_secs(5))
            .build()?;
        Ok(Self {
            config,
            client,
            secret: Arc::new(secret),
        })
    }

    async fn verify(&self, token: &str) -> Result<Principal, StatusCode> {
        let mut response = self
            .client
            .post(&self.config.introspection_endpoint)
            .basic_auth(
                &self.config.introspection_client_id,
                Some(self.secret.as_str()),
            )
            .form(&[("token", token), ("token_type_hint", "access_token")])
            .send()
            .await
            .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
        if !response.status().is_success() {
            return Err(StatusCode::SERVICE_UNAVAILABLE);
        }
        // Bound even chunked responses; never log the token or the provider body.
        let mut body = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?
        {
            if body.len() + chunk.len() > 32_768 {
                return Err(StatusCode::SERVICE_UNAVAILABLE);
            }
            body.extend_from_slice(&chunk);
        }
        let introspection: Introspection =
            serde_json::from_slice(&body).map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
        introspection
            .principal(&self.config, chrono::Utc::now().timestamp())
            .ok_or(StatusCode::UNAUTHORIZED)
    }
}

#[cfg(test)]
pub async fn require(State(auth): State<Auth>, mut request: Request, next: Next) -> Response {
    let token = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .filter(|v| !v.is_empty() && v.len() <= 8192 && !v.chars().any(char::is_whitespace));
    let result = match token {
        Some(token) => auth.verify(token).await,
        None => Err(StatusCode::UNAUTHORIZED),
    };
    match result {
        Ok(principal) if principal.allows("lvk:read") => {
            request.headers_mut().remove(header::AUTHORIZATION);
            request.extensions_mut().insert(principal);
            next.run(request).await
        }
        result => {
            let status = result.err().unwrap_or(StatusCode::FORBIDDEN);
            let mut response = status.into_response();
            if status != StatusCode::SERVICE_UNAVAILABLE {
                let challenge = format!(
                    "Bearer resource_metadata=\"{}\", scope=\"lvk:read\"",
                    auth.config.metadata_url()
                );
                if let Ok(value) = HeaderValue::from_str(&challenge) {
                    response
                        .headers_mut()
                        .insert(header::WWW_AUTHENTICATE, value);
                }
            }
            response
                .headers_mut()
                .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
            response
        }
    }
}
