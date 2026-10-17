use std::collections::VecDeque;

use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode, decode_header, jwk::JwkSet};
use openidconnect::{
    AccessTokenHash, AdditionalProviderMetadata, AuthorizationCode, ClientId, ClientSecret,
    CsrfToken, IssuerUrl, Nonce, OAuth2TokenResponse, PkceCodeChallenge, PkceCodeVerifier,
    ProviderMetadata, RedirectUrl, Scope, TokenResponse, core::*,
};

use super::*;

pub(crate) fn validate_issuer(value: &str) -> Result<(), Error> {
    let url = Url::parse(value).map_err(|_| Error::bad("Invalid Auth0 issuer"))?;
    #[cfg(test)]
    if url.scheme() == "http" && url.host_str() == Some("127.0.0.1") && url.path() == "/" {
        return Ok(());
    }
    let host = url.host_str().unwrap_or("");
    if url.scheme() != "https"
        || !host.ends_with(".auth0.com")
        || host == "auth0.com"
        || url.port().is_some()
        || url.path() != "/"
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || value != format!("https://{host}/")
    {
        return Err(Error::bad(
            "Use the exact Auth0 tenant issuer: https://TENANT.REGION.auth0.com/ (including the final slash)",
        ));
    }
    Ok(())
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct Extra {
    #[serde(default)]
    code_challenge_methods_supported: Vec<String>,
    #[serde(default)]
    authorization_response_iss_parameter_supported: bool,
}
impl AdditionalProviderMetadata for Extra {}
type Metadata = ProviderMetadata<
    Extra,
    CoreAuthDisplay,
    CoreClientAuthMethod,
    CoreClaimName,
    CoreClaimType,
    CoreGrantType,
    CoreJweContentEncryptionAlgorithm,
    CoreJweKeyManagementAlgorithm,
    CoreJsonWebKey,
    CoreResponseMode,
    CoreResponseType,
    CoreSubjectIdentifierType,
>;

#[derive(Default)]
pub(crate) struct Cache {
    entries: VecDeque<Entry>,
    attempts: VecDeque<(String, i64)>,
}
#[cfg(test)]
impl Cache {
    pub(crate) fn advance_refresh_window(&mut self) {
        for entry in &mut self.entries {
            entry.last_refresh -= 31;
        }
        for (_, at) in &mut self.attempts {
            *at -= 31;
        }
    }
}
struct Entry {
    issuer: String,
    metadata: Metadata,
    at: i64,
    last_refresh: i64,
}

fn http_client(
    issuer: String,
) -> impl Fn(
    openidconnect::HttpRequest,
) -> std::pin::Pin<
    Box<
        dyn std::future::Future<Output = Result<openidconnect::HttpResponse, std::io::Error>>
            + Send,
    >,
> + Clone {
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(10))
        .connect_timeout(Duration::from_secs(5))
        .build()
        .expect("TLS HTTP client");
    move |request| {
        let issuer = issuer.clone();
        let client = client.clone();
        Box::pin(async move {
            let failure = || std::io::Error::other("Auth0 HTTPS request failed");
            let target = Url::parse(&request.uri().to_string()).map_err(|_| failure())?;
            let trusted = Url::parse(&issuer).map_err(|_| failure())?;
            if target.origin() != trusted.origin()
                || !target.username().is_empty()
                || target.password().is_some()
                || target.fragment().is_some()
            {
                return Err(failure());
            }
            let (parts, body) = request.into_parts();
            let mut response = client
                .request(parts.method, target)
                .headers(parts.headers)
                .body(body)
                .send()
                .await
                .map_err(|_| failure())?;
            let mut builder = http::Response::builder().status(response.status());
            *builder.headers_mut().ok_or_else(failure)? = response.headers().clone();
            let mut body = Vec::new();
            while let Some(chunk) = response.chunk().await.map_err(|_| failure())? {
                if body.len() + chunk.len() > 262144 {
                    return Err(failure());
                }
                body.extend_from_slice(&chunk);
            }
            builder.body(body).map_err(|_| failure())
        })
    }
}
impl Access {
    async fn metadata(&self, issuer: &str, unknown_kid: bool) -> Result<Metadata, Error> {
        validate_issuer(issuer)?;
        // This lock coalesces fetches, bounds cached issuers, and throttles unknown
        // kid refreshes. JWT headers cannot select a network destination.
        let mut cache = self.0.provider.lock().await;
        if let Some(entry) = cache.entries.iter_mut().find(|e| e.issuer == issuer) {
            if now() - entry.at < 600 && (!unknown_kid || now() - entry.last_refresh < 30) {
                return Ok(entry.metadata.clone());
            }
            entry.last_refresh = now();
        }
        if cache
            .attempts
            .iter()
            .any(|(name, at)| name == issuer && now() - at < 30)
        {
            return Err(Error::unavailable());
        }
        cache.attempts.retain(|(name, _)| name != issuer);
        if cache.attempts.len() >= 2 {
            cache.attempts.pop_front();
        }
        cache.attempts.push_back((issuer.to_owned(), now()));
        let metadata = Metadata::discover_async(
            IssuerUrl::new(issuer.to_owned())?,
            &http_client(issuer.to_owned()),
        )
        .await
        .map_err(|_| Error::unavailable())?;
        let trusted = Url::parse(issuer)?;
        if metadata.issuer().as_str() != issuer {
            return Err(Error::unavailable());
        }
        for url in [
            metadata.authorization_endpoint().url(),
            metadata.jwks_uri().url(),
            metadata
                .token_endpoint()
                .ok_or_else(Error::unavailable)?
                .url(),
        ] {
            if url.origin() != trusted.origin()
                || !url.username().is_empty()
                || url.password().is_some()
                || url.fragment().is_some()
            {
                return Err(Error::unavailable());
            }
        }
        cache.entries.retain(|e| e.issuer != issuer);
        if cache.entries.len() >= 2 {
            cache.entries.pop_front();
        }
        cache.entries.push_back(Entry {
            issuer: issuer.to_owned(),
            metadata: metadata.clone(),
            at: now(),
            last_refresh: now(),
        });
        Ok(metadata)
    }
    pub(crate) async fn check_provider(&self, config: &Config) -> Result<serde_json::Value, Error> {
        let Config::Auth0 { issuer, .. } = config else {
            return Err(Error::bad("Auth0 required"));
        };
        let metadata = self.metadata(issuer, false).await?;
        let encoded = serde_json::to_value(metadata)?;
        // Keep metadata truthful. Auth0 settings must publish actual support.
        let pkce = encoded["code_challenge_methods_supported"]
            .as_array()
            .is_some_and(|a| a.iter().any(|v| v == "S256"));
        Ok(
            serde_json::json!({"discovery":true,"jwks":true,"pkce_s256":pkce,"client_secret_basic":encoded["token_endpoint_auth_methods_supported"].as_array().is_some_and(|a|a.iter().any(|v|v=="client_secret_basic")),"issuer_response":encoded["authorization_response_iss_parameter_supported"],"real_web_login":"unverified","chatgpt":"unverified"}),
        )
    }
    pub(crate) async fn start_oidc(
        &self,
        config: &Config,
    ) -> Result<(String, String, String, String), Error> {
        let Config::Auth0 {
            issuer,
            client_id,
            client_secret,
            ..
        } = config
        else {
            return Err(Error::bad("Auth0 required"));
        };
        let metadata = self.metadata(issuer, false).await?;
        let client = CoreClient::from_provider_metadata(
            metadata,
            ClientId::new(client_id.clone()),
            Some(ClientSecret::new(client_secret.clone())),
        )
        .set_redirect_uri(RedirectUrl::new(format!(
            "{}/api/access/callback",
            self.public_url().ok_or_else(Error::unavailable)?
        ))?);
        let (challenge, verifier) = PkceCodeChallenge::new_random_sha256();
        let (url, state, nonce) = client
            .authorize_url(
                CoreAuthenticationFlow::AuthorizationCode,
                CsrfToken::new_random,
                Nonce::new_random,
            )
            .add_scope(Scope::new("profile".into()))
            .set_pkce_challenge(challenge)
            .url();
        Ok((
            url.to_string(),
            state.secret().clone(),
            nonce.secret().clone(),
            verifier.secret().clone(),
        ))
    }
    pub(crate) async fn finish_oidc(
        &self,
        config: &Config,
        code: String,
        nonce: String,
        verifier: String,
    ) -> Result<(String, String), Error> {
        let Config::Auth0 {
            issuer,
            client_id,
            client_secret,
            ..
        } = config
        else {
            return Err(Error::bad("Auth0 required"));
        };
        let metadata = self.metadata(issuer, false).await?;
        let client = CoreClient::from_provider_metadata(
            metadata,
            ClientId::new(client_id.clone()),
            Some(ClientSecret::new(client_secret.clone())),
        )
        .set_redirect_uri(RedirectUrl::new(format!(
            "{}/api/access/callback",
            self.public_url().ok_or_else(Error::unavailable)?
        ))?);
        let token = client
            .exchange_code(AuthorizationCode::new(code))
            .map_err(|_| oidc_rejected("token_request_configuration"))?
            .set_pkce_verifier(PkceCodeVerifier::new(verifier))
            .request_async(&http_client(issuer.clone()))
            .await
            .map_err(|error| {
                use openidconnect::RequestTokenError;
                let reason = match error {
                    RequestTokenError::ServerResponse(_) => "token_endpoint_rejected",
                    RequestTokenError::Request(_) => "token_transport",
                    RequestTokenError::Parse(error, body) => {
                        log_token_parse_failure(&error, &body);
                        "token_response_parse"
                    }
                    _ => "token_response_other",
                };
                oidc_rejected(reason)
            })?;
        let id_token = token
            .id_token()
            .ok_or_else(|| oidc_rejected("id_token_missing"))?;
        let mut verifier = client
            .id_token_verifier()
            .set_allowed_algs(vec![CoreJwsSigningAlgorithm::RsaSsaPkcs1V15Sha256]);
        if matches!(
            id_token.signing_key(&verifier),
            Err(openidconnect::SignatureVerificationError::NoMatchingKey)
        ) {
            let refreshed = self.metadata(issuer, true).await?;
            verifier = CoreIdTokenVerifier::new_confidential_client(
                ClientId::new(client_id.clone()),
                ClientSecret::new(client_secret.clone()),
                IssuerUrl::new(issuer.clone())?,
                refreshed.jwks().clone(),
            )
            .set_allowed_algs(vec![CoreJwsSigningAlgorithm::RsaSsaPkcs1V15Sha256]);
        }
        let claims = id_token
            .claims(&verifier, &Nonce::new(nonce))
            .map_err(|error| oidc_rejected(claims_failure(&error)))?;
        if let Some(expected) = claims.access_token_hash() {
            let actual = AccessTokenHash::from_token(
                token.access_token(),
                id_token
                    .signing_alg()
                    .map_err(|_| oidc_rejected("access_token_hash_algorithm"))?,
                id_token
                    .signing_key(&verifier)
                    .map_err(|_| oidc_rejected("access_token_hash_key"))?,
            )
            .map_err(|_| oidc_rejected("access_token_hash_calculation"))?;
            if actual != *expected {
                return Err(oidc_rejected("access_token_hash_mismatch"));
            }
        }
        let sub = claims.subject().as_str().to_owned();
        let name = claims
            .name()
            .and_then(|name| name.get(None))
            .map(|name| name.as_str())
            .unwrap_or(&sub)
            .chars()
            .take(200)
            .collect();
        // Access/refresh/upstream tokens are deliberately dropped here.
        Ok((sub, name))
    }
    pub(crate) async fn verify_access(
        &self,
        config: &Config,
        token: &str,
    ) -> Result<AccessClaims, Error> {
        let issuer = config.issuer().ok_or_else(Error::unauthorized)?;
        let header = decode_header(token).map_err(|_| Error::unauthorized())?;
        if header.alg != Algorithm::RS256 {
            return Err(Error::unauthorized());
        }
        let kid = header
            .kid
            .filter(|s| !s.is_empty() && s.len() <= 256)
            .ok_or_else(Error::unauthorized)?;
        let metadata = self.metadata(issuer, false).await?;
        let mut jwks: JwkSet = serde_json::from_value(serde_json::to_value(metadata.jwks())?)?;
        if jwks.find(&kid).is_none() {
            let refreshed = self.metadata(issuer, true).await?;
            jwks = serde_json::from_value(serde_json::to_value(refreshed.jwks())?)?;
        }
        let jwk = jwks.find(&kid).ok_or_else(Error::unauthorized)?;
        let description = serde_json::to_value(jwk)?;
        if description.get("use").is_some_and(|v| v != "sig")
            || description.get("alg").is_some_and(|v| v != "RS256")
            || description.get("key_ops").is_some_and(|v| {
                !v.as_array()
                    .is_some_and(|a| a.iter().any(|v| v == "verify"))
            })
        {
            return Err(Error::unauthorized());
        }
        let key = DecodingKey::from_jwk(jwk).map_err(|_| Error::unauthorized())?;
        let mut validation = Validation::new(Algorithm::RS256);
        validation.set_issuer(&[issuer]);
        validation.set_audience(&[self.resource().ok_or_else(Error::unavailable)?]);
        validation.validate_nbf = true;
        validation.leeway = 0;
        validation.set_required_spec_claims(&["exp", "iss", "aud", "sub"]);
        let claims = decode::<AccessClaims>(token, &key, &validation)
            .map_err(|_| Error::unauthorized())?
            .claims;
        if claims.sub.is_empty() || claims.exp <= now() {
            return Err(Error::unauthorized());
        }
        Ok(claims)
    }
}
#[derive(Clone, Deserialize)]
pub(crate) struct AccessClaims {
    pub iss: String,
    pub sub: String,
    pub exp: i64,
    #[serde(default)]
    pub scope: String,
}

#[cfg(feature = "test-support")]
pub(crate) fn fixture_cache(issuer: &str, jwks: serde_json::Value) -> Cache {
    let metadata: Metadata = serde_json::from_value(serde_json::json!({
        "issuer":issuer,"authorization_endpoint":format!("{issuer}authorize"),
        "token_endpoint":format!("{issuer}oauth/token"),"jwks_uri":format!("{issuer}jwks"),
        "response_types_supported":["code"],"subject_types_supported":["public"],
        "id_token_signing_alg_values_supported":["RS256"],"jwks":jwks
    }))
    .unwrap();
    // ProviderMetadata deserialization does not populate the separately fetched JWKS.
    let metadata = metadata.set_jwks(serde_json::from_value(jwks).unwrap());
    Cache {
        entries: VecDeque::from([Entry {
            issuer: issuer.into(),
            metadata,
            at: now(),
            last_refresh: now(),
        }]),
        attempts: VecDeque::new(),
    }
}

// Never format provider/library errors here: their payloads can contain tokens,
// response bodies, nonce values or identity claims. Only fixed codes are logged.
fn oidc_rejected(reason: &'static str) -> Error {
    tracing::warn!(reason, "OIDC callback rejected");
    Error::unauthorized()
}

// The response and JWT are untrusted and may contain credentials. Diagnostic
// decoding never authenticates a user and only emits allowlisted static labels.
fn parse_location(
    error: &serde_path_to_error::Error<serde_json::Error>,
) -> (&'static str, &'static str) {
    let path = error.path().to_string();
    let field = [
        "access_token",
        "token_type",
        "expires_in",
        "refresh_token",
        "scope",
        "id_token",
        "error",
        "error_description",
        "iss",
        "sub",
        "aud",
        "exp",
        "iat",
        "auth_time",
        "nonce",
        "at_hash",
        "c_hash",
        "azp",
        "name",
        "nickname",
        "preferred_username",
        "profile",
        "picture",
        "website",
        "email",
        "email_verified",
        "updated_at",
        "phone_number",
        "phone_number_verified",
        "birthdate",
        "gender",
        "locale",
        "zoneinfo",
        "address",
        "given_name",
        "family_name",
        "middle_name",
    ]
    .into_iter()
    .find(|field| path == *field)
    .unwrap_or("unknown");
    let category = match error.inner().classify() {
        serde_json::error::Category::Io => "io",
        serde_json::error::Category::Syntax => "syntax",
        serde_json::error::Category::Data => "data",
        serde_json::error::Category::Eof => "eof",
    };
    (field, category)
}

fn id_token_parse_location(body: &[u8]) -> (&'static str, &'static str, &'static str) {
    use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
    let Ok(response) = serde_json::from_slice::<serde_json::Value>(body) else {
        return ("response_json", "unknown", "invalid");
    };
    let Some(token) = response.get("id_token").and_then(|v| v.as_str()) else {
        return ("id_token", "id_token", "missing_or_not_string");
    };
    let parts: Vec<_> = token.split('.').collect();
    if parts.len() != 3 {
        return ("jwt_compact", "id_token", "invalid");
    }
    for (index, stage) in [(0, "jwt_header"), (1, "jwt_claims")] {
        let Ok(bytes) = URL_SAFE_NO_PAD.decode(parts[index]) else {
            return (stage, "unknown", "base64");
        };
        if index == 0 {
            if serde_json::from_slice::<serde_json::Value>(&bytes).is_err() {
                return (stage, "unknown", "json");
            }
        } else {
            // IdTokenClaims flattens StandardClaims, which erases nested error
            // paths. Parse the standard claims separately for diagnostic location.
            let mut de = serde_json::Deserializer::from_slice(&bytes);
            if let Err(error) = serde_path_to_error::deserialize::<
                _,
                openidconnect::StandardClaims<CoreGenderClaim>,
            >(&mut de)
            {
                let (field, category) = parse_location(&error);
                return (stage, field, category);
            }
            let mut de = serde_json::Deserializer::from_slice(&bytes);
            if let Err(error) = serde_path_to_error::deserialize::<_, CoreIdTokenClaims>(&mut de) {
                let (field, category) = parse_location(&error);
                return (stage, field, category);
            }
        }
    }
    ("jwt_other", "unknown", "unknown")
}

fn log_token_parse_failure(error: &serde_path_to_error::Error<serde_json::Error>, body: &[u8]) {
    let (field, category) = parse_location(error);
    tracing::warn!(
        stage = "token_response",
        field,
        category,
        "OIDC parse diagnostic"
    );
    // Flattened token response fields can also lose their original path.
    let (stage, field, category) = id_token_parse_location(body);
    tracing::warn!(stage, field, category, "OIDC parse diagnostic");
}

fn claims_failure(error: &openidconnect::ClaimsVerificationError) -> &'static str {
    use openidconnect::{ClaimsVerificationError as C, SignatureVerificationError as S};
    match error {
        C::Expired(_) => "id_token_expired",
        C::InvalidAudience(_) => "id_token_audience",
        C::InvalidAuthContext(_) => "id_token_auth_context",
        C::InvalidAuthTime(_) => "id_token_auth_time",
        C::InvalidIssuer(_) => "id_token_issuer",
        C::InvalidNonce(_) => "id_token_nonce",
        C::InvalidSubject(_) => "id_token_subject",
        C::Unsupported(_) => "id_token_unsupported",
        C::SignatureVerification(error) => match error {
            S::NoMatchingKey => "id_token_key_missing",
            S::AmbiguousKeyId(_) => "id_token_key_ambiguous",
            S::DisallowedAlg(_) | S::UnsupportedAlg(_) => "id_token_algorithm",
            S::NoSignature => "id_token_unsigned",
            S::InvalidKey(_) => "id_token_key_invalid",
            _ => "id_token_signature",
        },
        _ => "id_token_claims_other",
    }
}

#[cfg(test)]
mod diagnostic_tests {
    use super::*;

    #[test]
    fn token_parse_diagnostics_identify_field_without_values() {
        let mut de = serde_json::Deserializer::from_str(
            r#"{"access_token":"secret","token_type":"Bearer","expires_in":"private-value"}"#,
        );
        let error = serde_path_to_error::deserialize::<_, CoreTokenResponse>(&mut de).unwrap_err();
        assert_eq!(parse_location(&error), ("expires_in", "data"));
        let mut de = serde_json::Deserializer::from_str(r#"{"private-field":"private-value"}"#);
        let error =
            serde_path_to_error::deserialize::<_, std::collections::BTreeMap<String, u64>>(&mut de)
                .unwrap_err();
        assert_eq!(parse_location(&error), ("unknown", "data"));
    }

    #[test]
    fn embedded_claim_diagnostic_never_returns_claim_values() {
        use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
        let claims = serde_json::json!({
            "iss":"https://example.auth0.com/", "sub":"private-user", "aud":"client",
            "iat":1, "exp":2, "updated_at":true
        });
        let token = format!(
            "{}.{}.AA",
            URL_SAFE_NO_PAD.encode(br#"{"alg":"RS256"}"#),
            URL_SAFE_NO_PAD.encode(serde_json::to_vec(&claims).unwrap())
        );
        let body = serde_json::to_vec(&serde_json::json!({"id_token":token})).unwrap();
        assert_eq!(
            id_token_parse_location(&body),
            ("jwt_claims", "updated_at", "data")
        );
        assert_eq!(
            id_token_parse_location(br#"{"id_token":"secret"}"#),
            ("jwt_compact", "id_token", "invalid")
        );
        assert_eq!(
            id_token_parse_location(b"not-json"),
            ("response_json", "unknown", "invalid")
        );
    }

    #[test]
    fn claim_diagnostics_discard_sensitive_error_payloads() {
        use openidconnect::{ClaimsVerificationError as C, SignatureVerificationError as S};
        let secret = "private-nonce-token-and-identity".to_owned();
        for (error, expected) in [
            (C::InvalidNonce(secret.clone()), "id_token_nonce"),
            (C::InvalidIssuer(secret.clone()), "id_token_issuer"),
            (C::InvalidAudience(secret.clone()), "id_token_audience"),
            (C::Expired(secret.clone()), "id_token_expired"),
            (C::Unsupported(secret.clone()), "id_token_unsupported"),
            (C::Other(secret.clone()), "id_token_claims_other"),
            (
                C::SignatureVerification(S::CryptoError(secret.clone())),
                "id_token_signature",
            ),
            (
                C::SignatureVerification(S::NoMatchingKey),
                "id_token_key_missing",
            ),
        ] {
            let reason = claims_failure(&error);
            assert_eq!(reason, expected);
            assert!(!reason.contains(&secret));
            let response = oidc_rejected(reason);
            assert_eq!(response.status, axum::http::StatusCode::UNAUTHORIZED);
            assert_eq!(response.message, "Authentication required");
        }
    }
}
