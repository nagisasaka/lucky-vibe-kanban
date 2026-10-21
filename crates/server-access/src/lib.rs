//! Authentication for one trusted-team LVK server. Auth0 issues OAuth tokens;
//! this crate owns membership, browser sessions and setup, never OAuth grants.
mod chatgpt;
mod handlers;
mod preview;
mod provider;
mod provisioning;
mod store;

use std::{
    collections::BTreeSet,
    path::PathBuf,
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use anyhow::ensure;
use axum::{
    Router,
    extract::{Request, State},
    http::{HeaderMap, HeaderValue, Method, StatusCode, header},
    middleware::Next,
    response::{IntoResponse, Response},
};
pub use handlers::router;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
pub use store::issue_key;
use tokio::sync::Mutex;
use url::Url;
use uuid::Uuid;

pub const SCOPES: &[&str] = &[
    "lvk:read",
    "lvk:write",
    "lvk:merge",
    "lvk:runtime:read",
    "lvk:runtime:execute",
];
const WEB_LIFETIME: i64 = 12 * 3600;
const WEB_IDLE: i64 = 30 * 60;
const SETUP_LIFETIME: i64 = 15 * 60;
const TRANSACTION_LIFETIME: i64 = 5 * 60;

pub fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case", deny_unknown_fields)]
pub enum Config {
    Local,
    Basic {
        username: String,
        password_hash: String,
    },
    Auth0 {
        issuer: String,
        client_id: String,
        client_secret: String,
        mcp_enabled: bool,
    },
}
impl Config {
    pub fn mode(&self) -> &'static str {
        match self {
            Self::Local => "local",
            Self::Basic { .. } => "basic",
            Self::Auth0 { .. } => "auth0",
        }
    }
    pub fn issuer(&self) -> Option<&str> {
        if let Self::Auth0 { issuer, .. } = self {
            Some(issuer)
        } else {
            None
        }
    }
}

#[derive(Clone)]
pub struct Access(Arc<Inner>);
struct Inner {
    pool: SqlitePool,
    public_url: Option<String>,
    public: bool,
    preview_domain: Option<String>,
    forced_mode: Option<String>,
    key_path: PathBuf,
    internal_token: String,
    provider: Mutex<provider::Cache>,
    attempts: Mutex<(i64, u32)>,
    password_slots: tokio::sync::Semaphore,
    provisioning: Mutex<()>,
}

#[derive(Clone, Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct Member {
    pub id: String,
    pub issuer: String,
    pub subject: String,
    pub display_name: String,
    pub state: String,
    pub role: String,
}
#[derive(Clone, Debug)]
pub struct Principal {
    pub user_id: Uuid,
    pub subject: String,
    pub issuer: String,
    pub admin: bool,
    pub scopes: BTreeSet<String>,
    pub channel: &'static str,
    pub expires: i64,
    pub revision: i64,
    session_hash: Option<String>,
}
impl Principal {
    fn local() -> Self {
        Self {
            user_id: Uuid::from_u128(1),
            subject: "local".into(),
            issuer: "urn:lvk:local".into(),
            admin: true,
            scopes: SCOPES.iter().map(|s| s.to_string()).collect(),
            channel: "local",
            expires: i64::MAX,
            revision: 0,
            session_hash: None,
        }
    }
    fn member(
        member: &Member,
        revision: i64,
        channel: &'static str,
        expires: i64,
    ) -> Result<Self, Error> {
        if member.state != "active" {
            return Err(Error::forbidden(
                "Membership approval required or account disabled",
            ));
        }
        Ok(Self {
            user_id: Uuid::parse_str(&member.id).map_err(|_| Error::unavailable())?,
            issuer: member.issuer.clone(),
            subject: member.subject.clone(),
            admin: member.role == "admin",
            scopes: SCOPES.iter().map(|s| s.to_string()).collect(),
            channel,
            expires,
            revision,
            session_hash: None,
        })
    }
}

#[derive(Clone)]
pub struct Lease {
    access: Access,
    principal: Principal,
}
impl Lease {
    /// Revalidation also stops upgraded sockets after logout, mode changes,
    /// expiry or member disable. No credential is kept in the socket task.
    pub async fn until_revoked(&self) {
        loop {
            tokio::time::sleep(Duration::from_secs(2)).await;
            if !self.access.valid(&self.principal).await {
                return;
            }
        }
    }
    pub async fn run<F: std::future::Future<Output = ()>>(&self, task: F) {
        ACTOR
            .scope(self.principal.clone(), async {
                tokio::select! { _ = self.until_revoked() => {}, _ = task => {} }
            })
            .await;
    }
}

tokio::task_local! { pub static ACTOR: Principal; }
/// Used only for new operation attribution. Historical local identities remain intact.
pub fn actor_id() -> Uuid {
    ACTOR.try_with(|p| p.user_id).unwrap_or(Uuid::from_u128(1))
}
pub fn actor_issuer() -> String {
    ACTOR
        .try_with(|p| p.issuer.clone())
        .unwrap_or_else(|_| "urn:lvk:local".into())
}
pub fn actor_is_admin() -> bool {
    ACTOR.try_with(|p| p.admin).unwrap_or(true)
}

#[derive(Debug)]
pub struct Error {
    status: StatusCode,
    message: std::borrow::Cow<'static, str>,
}
impl Error {
    fn bad(message: &'static str) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message: message.into(),
        }
    }
    fn unauthorized() -> Self {
        Self {
            status: StatusCode::UNAUTHORIZED,
            message: "Authentication required".into(),
        }
    }
    fn forbidden(message: &'static str) -> Self {
        Self {
            status: StatusCode::FORBIDDEN,
            message: message.into(),
        }
    }
    fn unavailable() -> Self {
        Self {
            status: StatusCode::SERVICE_UNAVAILABLE,
            message: "Authentication unavailable; operator recovery may be required".into(),
        }
    }
}
impl<E: Into<anyhow::Error>> From<E> for Error {
    fn from(_: E) -> Self {
        Self::unavailable()
    }
}
impl IntoResponse for Error {
    fn into_response(self) -> Response {
        let mut r = (
            self.status,
            axum::Json(serde_json::json!({"error":self.message})),
        )
            .into_response();
        r.headers_mut()
            .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
        r
    }
}

impl Access {
    pub fn from_env(pool: SqlitePool, data_dir: PathBuf) -> anyhow::Result<Self> {
        let placement = std::env::var("LVK_ACCESS_DEPLOYMENT").unwrap_or_else(|_| "local".into());
        ensure!(
            matches!(placement.as_str(), "local" | "public"),
            "LVK_ACCESS_DEPLOYMENT must be local or public"
        );
        if std::env::var_os("LVK_ACCESS_DEPLOYMENT").is_none() {
            ensure!(
                std::env::var("HOST").map_or(true, |h| matches!(
                    h.as_str(),
                    "127.0.0.1" | "localhost" | "::1"
                )),
                "Non-loopback binding requires explicit LVK_ACCESS_DEPLOYMENT"
            );
        }
        let public = placement == "public";
        let public_url = std::env::var("LVK_PUBLIC_URL")
            .ok()
            .filter(|s| !s.is_empty());
        if public {
            validate_public_url(
                public_url
                    .as_deref()
                    .ok_or_else(|| anyhow::anyhow!("LVK_PUBLIC_URL is required"))?,
            )?;
        }
        if let Some(url) = &public_url {
            validate_public_url(url)?;
        }
        let forced_mode = std::env::var("LVK_AUTH_MODE")
            .ok()
            .filter(|s| !s.is_empty());
        if let Some(mode) = &forced_mode {
            ensure!(
                matches!(mode.as_str(), "local" | "basic" | "auth0")
                    && !(public && mode == "local"),
                "Invalid forced authentication mode"
            );
        }
        // Legacy external introspection must never silently remain an auth bypass.
        ensure!(
            std::env::var_os("LVK_MCP_CONFIG").is_none(),
            "Migrate LVK_MCP_CONFIG through the access settings UI before enabling remote MCP"
        );
        let internal_token = store::internal_token(&data_dir.join("access-internal.token"))?;
        Ok(Self(Arc::new(Inner {
            pool,
            public_url,
            public,
            preview_domain: std::env::var("VK_PREVIEW_DOMAIN")
                .ok()
                .filter(|s| !s.is_empty()),
            forced_mode,
            internal_token,
            key_path: data_dir.join("access.key"),
            provider: Mutex::new(provider::Cache::default()),
            attempts: Mutex::new((now(), 0)),
            password_slots: tokio::sync::Semaphore::new(2),
            provisioning: Mutex::new(()),
        })))
    }
    pub fn pool(&self) -> &SqlitePool {
        &self.0.pool
    }
    pub fn public_url(&self) -> Option<&str> {
        self.0.public_url.as_deref()
    }
    pub fn resource(&self) -> Option<String> {
        self.public_url().map(|u| format!("{u}/mcp"))
    }
    pub async fn configuration(&self) -> Result<(Option<Config>, i64), Error> {
        self.load().await
    }
    pub fn layer(&self, router: Router) -> Router {
        router.layer(axum::middleware::from_fn_with_state(self.clone(), gate))
    }
    fn internal_request(&self, headers: &HeaderMap) -> bool {
        use subtle::ConstantTimeEq;
        let token = headers
            .get("x-lvk-internal-token")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        let host = headers
            .get(header::HOST)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| Url::parse(&format!("http://{v}")).ok());
        headers.get(header::ORIGIN).is_none()
            && host
                .is_some_and(|h| matches!(h.host_str(), Some("127.0.0.1" | "localhost" | "[::1]")))
            && bool::from(token.as_bytes().ct_eq(self.0.internal_token.as_bytes()))
    }
    async fn rate_limit(&self) -> Result<(), Error> {
        let mut rate = self.0.attempts.lock().await;
        if now() - rate.0 >= 60 {
            *rate = (now(), 0);
        }
        rate.1 += 1;
        if rate.1 > 30 {
            return Err(Error {
                status: StatusCode::TOO_MANY_REQUESTS,
                message: "Too many authentication attempts; retry in one minute".into(),
            });
        }
        Ok(())
    }
    fn allow_mode(&self, mode: &str) -> Result<(), Error> {
        if (self.0.public && mode == "local")
            || (!self.0.public && mode != "local")
            || self.0.forced_mode.as_deref().is_some_and(|m| m != mode)
        {
            return Err(Error::forbidden(
                "This mode is disabled by deployment configuration",
            ));
        }
        Ok(())
    }
    fn host_origin(&self, headers: &HeaderMap) -> Result<(), Error> {
        let host = headers
            .get(header::HOST)
            .and_then(|v| v.to_str().ok())
            .ok_or_else(Error::unauthorized)?;
        if let Some(base) = self.public_url() {
            let url = Url::parse(base)?;
            if host != &url[url::Position::BeforeHost..url::Position::AfterPort] {
                return Err(Error::forbidden("Invalid host"));
            }
            if let Some(origin) = headers.get(header::ORIGIN)
                && origin.to_str().ok() != Some(base)
            {
                return Err(Error::forbidden("Invalid origin"));
            }
        } else {
            let url = Url::parse(&format!("http://{host}"))?;
            if !matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "[::1]"))
                || !url.username().is_empty()
                || url.password().is_some()
                || url.path() != "/"
                || url.query().is_some()
                || url.fragment().is_some()
            {
                return Err(Error::forbidden(
                    "Local deployment only accepts loopback hosts",
                ));
            }
            if let Some(origin) = headers.get(header::ORIGIN)
                && origin.to_str().ok() != Some(format!("http://{host}").as_str())
            {
                return Err(Error::forbidden("Invalid origin"));
            }
        }
        Ok(())
    }
    /// Persist explicit device notification consent for an authenticated member. The
    /// encrypted envelope is purpose-bound and never contains a login token.
    pub fn seal_push_registration(
        &self,
        subscription: serde_json::Value,
    ) -> Result<Vec<u8>, Error> {
        let p = ACTOR
            .try_with(Clone::clone)
            .map_err(|_| Error::unauthorized())?;
        if !matches!(p.channel, "local" | "basic" | "web") {
            return Err(Error::unauthorized());
        }
        self.encrypt(
            &serde_json::json!({"purpose":"web_push_v1", "subscription":subscription,
            "user_id":p.user_id, "revision":p.revision, "channel":p.channel}),
        )
    }

    /// Device consent outlives browser idle/logout expiry so background jobs can
    /// notify overnight. OFF revokes it; membership/mode changes revoke delivery.
    /// This envelope grants no API access and never renews a browser session.
    pub async fn open_push_registration(
        &self,
        encrypted: &[u8],
        owner: Uuid,
    ) -> Option<serde_json::Value> {
        let value: serde_json::Value = self.decrypt(encrypted).ok()?;
        if value["purpose"] != "web_push_v1" || value["user_id"].as_str()? != owner.to_string() {
            return None;
        }
        let mut p = Principal::local();
        p.user_id = owner;
        p.revision = value["revision"].as_i64()?;
        p.expires = i64::MAX;
        p.session_hash = None;
        p.channel = match value["channel"].as_str()? {
            "local" => "local",
            "basic" => "basic",
            "web" => "web",
            _ => return None,
        };
        self.valid(&p).await.then(|| value["subscription"].clone())
    }

    async fn valid(&self, p: &Principal) -> bool {
        if p.expires <= now() {
            return false;
        }
        let Ok((config, revision)) = self.load().await else {
            return false;
        };
        if revision != p.revision {
            return false;
        }
        if p.channel == "local" {
            return matches!(config, Some(Config::Local));
        }
        if p.channel == "internal" {
            return config.is_some();
        }
        let member = sqlx::query_scalar::<_, String>("SELECT state FROM access_members WHERE id=?")
            .bind(p.user_id.to_string())
            .fetch_optional(self.pool())
            .await;
        if !matches!(member,Ok(Some(s)) if s=="active") {
            return false;
        }
        if let Some(hash) = &p.session_hash {
            return sqlx::query_scalar::<_,bool>("SELECT EXISTS(SELECT 1 FROM access_sessions WHERE hash=? AND expires>? AND touched>?)").bind(hash).bind(now()).bind(now()-WEB_IDLE).fetch_one(self.pool()).await.unwrap_or(false);
        }
        true
    }
    pub async fn authenticate_mcp(&self, headers: &HeaderMap) -> Result<Principal, Response> {
        let result = async {
            let (config, revision) = self.load().await?;
            let Some(
                config @ Config::Auth0 {
                    mcp_enabled: true, ..
                },
            ) = config
            else {
                return Err(Error {
                    status: StatusCode::NOT_FOUND,
                    message: "Remote MCP is disabled".into(),
                });
            };
            let token = headers
                .get(header::AUTHORIZATION)
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.split_once(' '))
                .filter(|(scheme, _)| scheme.eq_ignore_ascii_case("Bearer"))
                .map(|(_, credential)| credential)
                .filter(|v| {
                    !v.is_empty() && v.len() <= 16384 && !v.chars().any(char::is_whitespace)
                })
                .ok_or_else(Error::unauthorized)?;
            let claims = self.verify_access(&config, token).await?;
            let member = self.identity(&claims.iss, &claims.sub, None).await?;
            let mut p = Principal::member(&member, revision, "mcp", claims.exp)?;
            p.scopes = claims.scope.split_whitespace().map(str::to_owned).collect();
            if !p.scopes.contains("lvk:read") {
                return Err(Error::forbidden("insufficient_scope"));
            }
            if headers.contains_key("mcp-session-id") {
                return Err(Error::bad(
                    "This MCP transport is stateless; omit MCP Session ID",
                ));
            }
            Ok(p)
        }
        .await;
        result.map_err(|err:Error| {
            let scope=err.message=="insufficient_scope";
            let status=err.status;
            let mut response=err.into_response();
            if matches!(status,StatusCode::UNAUTHORIZED|StatusCode::FORBIDDEN)
                && let Some(base)=self.public_url() {
                    let extra=if scope {", error=\"insufficient_scope\", scope=\"lvk:read\""} else {""};
                    if let Ok(value)=HeaderValue::from_str(&format!("Bearer resource_metadata=\"{base}/.well-known/oauth-protected-resource/mcp\"{extra}")) {response.headers_mut().insert(header::WWW_AUTHENTICATE,value);}
            } response
        })
    }
}

pub fn validate_public_url(value: &str) -> anyhow::Result<()> {
    let url = Url::parse(value)?;
    ensure!(
        url.scheme() == "https"
            && url.host_str().is_some()
            && url.username().is_empty()
            && url.password().is_none()
            && url.query().is_none()
            && url.fragment().is_none()
            && url.path() == "/"
            && !value.ends_with('/'),
        "Public URL must be an exact HTTPS origin without trailing slash"
    );
    ensure!(
        url.origin().ascii_serialization() == value,
        "Public URL must use canonical URL spelling"
    );
    Ok(())
}

fn safe(method: &Method) -> bool {
    matches!(*method, Method::GET | Method::HEAD | Method::OPTIONS)
}
fn cookie(headers: &HeaderMap, name: &str) -> Option<String> {
    // Duplicate authentication cookies are ambiguous; reject them.
    let mut values = headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|h| h.to_str().ok())
        .flat_map(|v| v.split(';'))
        .filter_map(|v| cookie::Cookie::parse(v.trim()).ok())
        .filter(|c| c.name() == name)
        .map(|c| c.value().to_owned());
    let value = values.next()?;
    if values.next().is_some() {
        None
    } else {
        Some(value)
    }
}
fn session_cookie(access: &Access, name: &str, value: &str, max_age: i64) -> String {
    format!(
        "{name}={value}; Path=/; HttpOnly; SameSite=Lax; Max-Age={max_age}{}",
        if access.0.public { "; Secure" } else { "" }
    )
}
fn set_cookie(response: &mut Response, access: &Access, name: &str, value: &str, max_age: i64) {
    if let Ok(value) = HeaderValue::from_str(&session_cookie(access, name, value, max_age)) {
        response.headers_mut().append(header::SET_COOKIE, value);
    }
}

async fn gate(State(access): State<Access>, mut request: Request, next: Next) -> Response {
    let internal = access.internal_request(request.headers());
    if !internal && let Err(err) = access.host_origin(request.headers()) {
        return err.into_response();
    }
    let path = request.uri().path().to_owned();
    let public = path.starts_with("/api/access/")
        || path == "/api/health"
        || path == "/.well-known/oauth-protected-resource/mcp"
        || !path.starts_with("/api/") && path != "/mcp" && !path.starts_with("/mcp/");
    if public {
        let auth_change = path.starts_with("/api/access/") && !safe(request.method());
        let actor = if auth_change {
            access
                .session(request.headers())
                .await
                .ok()
                .and_then(|s| s.user_id)
        } else {
            None
        };
        let response = next.run(request).await;
        if auth_change {
            let _ = access
                .audit(
                    actor.as_deref(),
                    "access-api",
                    &path,
                    None,
                    if response.status().is_success() {
                        "success"
                    } else {
                        "rejected"
                    },
                )
                .await;
        }
        return secure_response(response);
    }
    // Runner callbacks retain their existing installation/token verification.
    // They never gain membership through a loopback or forwarded-header guess.
    let segments: Vec<_> = path.split('/').collect();
    let runner_callback = (segments.len() == 6
        && segments[1..4] == ["api", "execution-bridge", "runners"]
        && segments[5] == "poll")
        || (segments.len() == 8
            && segments[1..4] == ["api", "execution-bridge", "runners"]
            && segments[5] == "operations"
            && matches!(segments[7], "report" | "settled"));
    if runner_callback && matches!(access.load().await, Ok((Some(_), _))) {
        return secure_response(next.run(request).await);
    }
    let principal = if internal && path.starts_with("/api/") {
        let revision = match access.load().await {
            Ok((Some(_), revision)) => revision,
            _ => return Error::unavailable().into_response(),
        };
        let mut p = Principal::local();
        p.user_id = Uuid::nil();
        p.subject = "internal-agent".into();
        p.issuer = "urn:lvk:internal".into();
        p.channel = "internal";
        p.admin = false;
        p.revision = revision;
        p
    } else if path == "/mcp" || path.starts_with("/mcp/") {
        match access.authenticate_mcp(request.headers()).await {
            Ok(p) => p,
            Err(r) => return r,
        }
    } else {
        match access.web_principal(request.headers()).await {
            Ok(p) => p,
            Err(e) => {
                let mut response = e.into_response();
                if matches!(access.load().await, Ok((Some(Config::Basic { .. }), _)))
                    && response.status() == StatusCode::UNAUTHORIZED
                {
                    response.headers_mut().insert(
                        header::WWW_AUTHENTICATE,
                        HeaderValue::from_static("Basic realm=\"LVK\", charset=\"UTF-8\""),
                    );
                }
                return response;
            }
        }
    };
    if principal.channel != "mcp"
        && principal.channel != "internal"
        && !safe(request.method())
        && let Err(err) = access.csrf(request.headers(), &principal).await
    {
        return err.into_response();
    }
    let lease = Lease {
        access: access.clone(),
        principal: principal.clone(),
    };
    request.extensions_mut().insert(lease.clone());
    request.extensions_mut().insert(principal.clone());
    request.headers_mut().remove(header::AUTHORIZATION);
    request.headers_mut().remove("x-lvk-internal-token");
    // Downstream previews/relay clients never receive the LVK browser session.
    let remaining = request
        .headers()
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(';'))
        .filter(|part| {
            !part.trim().starts_with("lvk_session=") && !part.trim().starts_with("lvk_transaction=")
        })
        .map(str::trim)
        .collect::<Vec<_>>()
        .join("; ");
    request.headers_mut().remove(header::COOKIE);
    if !remaining.is_empty()
        && let Ok(value) = HeaderValue::from_str(&remaining)
    {
        request.headers_mut().insert(header::COOKIE, value);
    }
    let method = request.method().clone();
    let upgrade = request.headers().contains_key(header::UPGRADE);
    let mut response = ACTOR.scope(principal.clone(), next.run(request)).await;
    if !safe(&method) || upgrade {
        // Path contains identifiers, never query parameters, bodies or credentials.
        let _ = access
            .audit(
                Some(&principal.user_id.to_string()),
                principal.channel,
                method.as_str(),
                Some(&path),
                if response.status().is_success()
                    || response.status() == StatusCode::SWITCHING_PROTOCOLS
                {
                    "success"
                } else {
                    "rejected"
                },
            )
            .await;
    }
    if response
        .headers()
        .get(header::CONTENT_TYPE)
        .is_some_and(|v| v.as_bytes().starts_with(b"text/event-stream"))
    {
        use futures_util::StreamExt;
        let (parts, body) = response.into_parts();
        let stream = body
            .into_data_stream()
            .take_until(async move { lease.until_revoked().await });
        response = Response::from_parts(parts, axum::body::Body::from_stream(stream));
    }
    secure_response(response)
}
fn secure_response(mut response: Response) -> Response {
    response
        .headers_mut()
        .insert(header::X_FRAME_OPTIONS, HeaderValue::from_static("DENY"));
    response.headers_mut().insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("no-referrer"),
    );
    response.headers_mut().insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

#[cfg(test)]
mod tests;

#[cfg(feature = "test-support")]
#[doc(hidden)]
pub mod test_support;
