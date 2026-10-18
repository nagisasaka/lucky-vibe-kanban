use std::collections::HashMap;

use axum::{
    Form, Json,
    body::Body,
    routing::{get, post},
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use http_body_util::BodyExt;
use jsonwebtoken::{Algorithm, EncodingKey, Header, encode};
use rsa::{
    RsaPrivateKey,
    pkcs8::{EncodePrivateKey, LineEnding},
    traits::PublicKeyParts,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tower::ServiceExt;

use super::*;

struct Fixture {
    access: Access,
    _dir: tempfile::TempDir,
}
impl Fixture {
    async fn new(public: bool) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(4)
            .connect_with(
                sqlx::sqlite::SqliteConnectOptions::new()
                    .filename(dir.path().join("test.sqlite"))
                    .create_if_missing(true)
                    .foreign_keys(true)
                    .busy_timeout(Duration::from_secs(5)),
            )
            .await
            .unwrap();
        sqlx::raw_sql(include_str!(
            "../../db/migrations/20261015000000_server_access.sql"
        ))
        .execute(&pool)
        .await
        .unwrap();
        sqlx::raw_sql(include_str!(
            "../../db/migrations/20261015010000_preview_access.sql"
        ))
        .execute(&pool)
        .await
        .unwrap();
        sqlx::raw_sql(include_str!(
            "../../db/migrations/20261018010000_chatgpt_client_credentials.sql"
        ))
        .execute(&pool)
        .await
        .unwrap();
        store::ensure_key(&dir.path().join("access.key")).unwrap();
        let access = Access(Arc::new(Inner {
            pool,
            public,
            preview_domain: Some("preview.example".into()),
            public_url: public.then(|| "https://203.0.113.7:8443".into()),
            forced_mode: None,
            key_path: dir.path().join("access.key"),
            internal_token: store::random(),
            provider: Mutex::new(provider::Cache::default()),
            attempts: Mutex::new((now(), 0)),
            password_slots: tokio::sync::Semaphore::new(2),
            provisioning: Mutex::new(()),
        }));
        Self { access, _dir: dir }
    }
    fn app(&self) -> Router {
        self.access.layer(
            router(self.access.clone())
                .route(
                    "/api/projects",
                    get(|| async { "business" }).post(|| async { "changed" }),
                )
                .route("/mcp", post(|| async { "tool result" }))
                .route("/api/health", get(|| async { "ok" })),
        )
    }
    async fn call(
        &self,
        method: &str,
        path: &str,
        headers: &[(&str, &str)],
        body: Value,
    ) -> Response {
        let mut req = http::Request::builder().method(method).uri(path).header(
            "host",
            if self.access.0.public {
                "203.0.113.7:8443"
            } else {
                "localhost:3000"
            },
        );
        for (name, value) in headers {
            req = req.header(*name, *value);
        }
        self.app()
            .oneshot(
                req.header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap()
    }
    async fn setup(&self) -> (String, String) {
        let token = issue_key(self.access.pool(), &self.access.0.key_path, false)
            .await
            .unwrap();
        let r = self
            .call(
                "POST",
                "/api/access/setup",
                &[("x-lvk-csrf", "setup")],
                json!({"key":token}),
            )
            .await;
        assert_eq!(r.status(), StatusCode::OK);
        let cookie = response_cookie(&r, "lvk_session");
        let body = body(r).await;
        (cookie, body["csrf"].as_str().unwrap().into())
    }
    async fn basic(&self) -> (String, String) {
        let (cookie, csrf) = self.setup().await;
        assert_eq!(
            self.call(
                "POST",
                "/api/access/basic",
                &[("cookie", &cookie), ("x-lvk-csrf", &csrf)],
                json!({"username":"owner","password":"a long test password"})
            )
            .await
            .status(),
            StatusCode::OK
        );
        let authorization = format!(
            "Basic {}",
            base64::engine::general_purpose::STANDARD.encode("owner:a long test password")
        );
        let r = self
            .call(
                "POST",
                "/api/access/basic-login",
                &[("authorization", &authorization), ("x-lvk-csrf", "basic")],
                Value::Null,
            )
            .await;
        assert_eq!(r.status(), StatusCode::OK);
        let cookie = response_cookie(&r, "lvk_session");
        let body = body(r).await;
        (cookie, body["csrf"].as_str().unwrap().into())
    }
}
async fn body(r: Response) -> Value {
    serde_json::from_slice(&r.into_body().collect().await.unwrap().to_bytes()).unwrap()
}
fn response_cookie(r: &Response, name: &str) -> String {
    r.headers()
        .get_all(header::SET_COOKIE)
        .iter()
        .map(|v| v.to_str().unwrap())
        .find(|s| s.starts_with(&format!("{name}=")))
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .into()
}

#[tokio::test]
async fn public_lock_origin_setup_keys_and_recovery_boundaries() {
    let f = Fixture::new(true).await;
    assert_eq!(
        f.call("GET", "/api/projects", &[], Value::Null)
            .await
            .status(),
        StatusCode::LOCKED
    );
    assert_eq!(
        f.call("POST", "/mcp", &[], Value::Null).await.status(),
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        f.call("GET", "/api/health", &[], Value::Null)
            .await
            .status(),
        StatusCode::OK
    );
    assert_eq!(
        f.call(
            "GET",
            "/api/projects",
            &[("x-vk-relayed", "1"), ("origin", "https://evil.example")],
            Value::Null
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );
    let expired = issue_key(f.access.pool(), &f.access.0.key_path, false)
        .await
        .unwrap();
    sqlx::query("UPDATE access_keys SET expires=?")
        .bind(now() - 1)
        .execute(f.access.pool())
        .await
        .unwrap();
    assert_eq!(
        f.call(
            "POST",
            "/api/access/setup",
            &[("x-lvk-csrf", "setup")],
            json!({"key":expired})
        )
        .await
        .status(),
        StatusCode::UNAUTHORIZED
    );
    let token = issue_key(f.access.pool(), &f.access.0.key_path, false)
        .await
        .unwrap();
    let (a, b) = tokio::join!(
        f.call(
            "POST",
            "/api/access/setup",
            &[("x-lvk-csrf", "setup")],
            json!({"key":token})
        ),
        f.call(
            "POST",
            "/api/access/setup",
            &[("x-lvk-csrf", "setup")],
            json!({"key":token})
        )
    );
    assert_eq!(
        [a.status(), b.status()]
            .iter()
            .filter(|s| s.is_success())
            .count(),
        1
    );
    let winner = if a.status().is_success() { a } else { b };
    let setup = response_cookie(&winner, "lvk_session");
    assert_eq!(
        f.call("GET", "/api/projects", &[("cookie", &setup)], Value::Null)
            .await
            .status(),
        StatusCode::LOCKED
    );
    assert_eq!(
        f.call("GET", "/api/access/configuration", &[], Value::Null)
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );
    let reissued = issue_key(f.access.pool(), &f.access.0.key_path, false)
        .await
        .unwrap();
    assert_eq!(
        f.call(
            "GET",
            "/api/access/configuration",
            &[("cookie", &setup)],
            Value::Null
        )
        .await
        .status(),
        StatusCode::UNAUTHORIZED
    );
    assert_ne!(token, reissued);
    let (cookie, csrf) = f.basic().await;
    assert!(
        issue_key(f.access.pool(), &f.access.0.key_path, false)
            .await
            .is_err()
    );
    assert_eq!(
        f.call("GET", "/api/projects", &[("cookie", &cookie)], Value::Null)
            .await
            .status(),
        StatusCode::OK
    );
    assert_eq!(
        f.call("POST", "/api/projects", &[("cookie", &cookie)], Value::Null)
            .await
            .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        f.call(
            "POST",
            "/api/projects",
            &[("cookie", &cookie), ("x-lvk-csrf", &csrf)],
            Value::Null
        )
        .await
        .status(),
        StatusCode::OK
    );
    let recovery = issue_key(f.access.pool(), &f.access.0.key_path, true)
        .await
        .unwrap();
    let r = f
        .call(
            "POST",
            "/api/access/setup",
            &[("x-lvk-csrf", "setup")],
            json!({"key":recovery}),
        )
        .await;
    let recovery_cookie = response_cookie(&r, "lvk_session");
    assert_eq!(
        f.call(
            "GET",
            "/api/projects",
            &[("cookie", &recovery_cookie)],
            Value::Null
        )
        .await
        .status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        f.call(
            "GET",
            "/api/access/members",
            &[("cookie", &recovery_cookie)],
            Value::Null
        )
        .await
        .status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        f.call(
            "GET",
            "/api/access/configuration",
            &[("cookie", &recovery_cookie)],
            Value::Null
        )
        .await
        .status(),
        StatusCode::OK
    );
    assert_eq!(
        f.call(
            "POST",
            "/api/access/logout",
            &[("cookie", &cookie), ("x-lvk-csrf", &csrf)],
            Value::Null
        )
        .await
        .status(),
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        f.call("GET", "/api/projects", &[("cookie", &cookie)], Value::Null)
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );
}
#[tokio::test]
async fn local_dns_rebinding_mcp_and_corrupt_settings_fail_closed() {
    let f = Fixture::new(false).await;
    assert_eq!(
        f.call("GET", "/api/projects", &[], Value::Null)
            .await
            .status(),
        StatusCode::OK
    );
    assert_eq!(
        f.call(
            "POST",
            "/api/projects",
            &[("origin", "https://evil.example"), ("x-lvk-csrf", "local")],
            Value::Null
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );
    let r = f
        .app()
        .oneshot(
            http::Request::builder()
                .uri("/api/projects")
                .header("host", "evil.example")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::FORBIDDEN);
    assert_eq!(
        f.call("POST", "/mcp", &[], Value::Null).await.status(),
        StatusCode::NOT_FOUND
    );
    sqlx::query("INSERT INTO access_settings VALUES(1,1,X'00')")
        .execute(f.access.pool())
        .await
        .unwrap();
    assert_eq!(
        f.call("GET", "/api/projects", &[], Value::Null)
            .await
            .status(),
        StatusCode::SERVICE_UNAVAILABLE
    );
}
#[tokio::test]
async fn session_expiry_restart_secret_loss_and_basic_mcp_disabled() {
    let f = Fixture::new(true).await;
    let (cookie, _) = f.basic().await;
    assert_eq!(
        f.call(
            "POST",
            "/api/access/basic-login",
            &[("cookie", &cookie), ("x-lvk-csrf", "basic")],
            Value::Null
        )
        .await
        .status(),
        StatusCode::UNAUTHORIZED,
        "a session cookie cannot replace password reauthentication"
    );
    let restarted = Access(Arc::new(Inner {
        pool: f.access.pool().clone(),
        preview_domain: f.access.0.preview_domain.clone(),
        public: true,
        public_url: f.access.0.public_url.clone(),
        forced_mode: None,
        key_path: f.access.0.key_path.clone(),
        internal_token: f.access.0.internal_token.clone(),
        provider: Mutex::new(provider::Cache::default()),
        attempts: Mutex::new((now(), 0)),
        password_slots: tokio::sync::Semaphore::new(2),
        provisioning: Mutex::new(()),
    }));
    let mut headers = HeaderMap::new();
    headers.insert(header::COOKIE, cookie.parse().unwrap());
    assert!(restarted.web_principal(&headers).await.is_ok());
    assert_eq!(
        f.call("POST", "/mcp", &[("cookie", &cookie)], Value::Null)
            .await
            .status(),
        StatusCode::NOT_FOUND
    );
    sqlx::query("UPDATE access_sessions SET touched=?")
        .bind(now() - WEB_IDLE - 1)
        .execute(f.access.pool())
        .await
        .unwrap();
    assert_eq!(
        f.call("GET", "/api/projects", &[("cookie", &cookie)], Value::Null)
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );
    std::fs::remove_file(&f.access.0.key_path).unwrap();
    assert_eq!(
        f.call("GET", "/api/projects", &[("cookie", &cookie)], Value::Null)
            .await
            .status(),
        StatusCode::SERVICE_UNAVAILABLE
    );
    assert!(
        issue_key(f.access.pool(), &f.access.0.key_path, true)
            .await
            .is_ok()
    );
    assert!(f.access.load().await.is_err()); // Recovery doesn't silently decrypt or unlock old configuration.
}

#[derive(Clone)]
struct Mock {
    issuer: String,
    pem: Arc<String>,
    jwk: Value,
    codes: Arc<Mutex<HashMap<String, (Value, String)>>>,
    requests: Arc<std::sync::atomic::AtomicUsize>,
    rotated: Arc<std::sync::atomic::AtomicBool>,
}
struct Idp {
    state: Mock,
    task: tokio::task::JoinHandle<()>,
}
impl Drop for Idp {
    fn drop(&mut self) {
        self.task.abort();
    }
}
impl Idp {
    async fn new() -> Self {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let issuer = format!("http://{}/", listener.local_addr().unwrap());
        static KEYS: std::sync::OnceLock<(String, Value)> = std::sync::OnceLock::new();
        let (pem,jwk)=KEYS.get_or_init(||{
            let key=RsaPrivateKey::new(&mut rand::rngs::OsRng,2048).unwrap();
            (key.to_pkcs8_pem(LineEnding::LF).unwrap().to_string(),json!({"kty":"RSA","kid":"test-key","use":"sig","alg":"RS256","n":URL_SAFE_NO_PAD.encode(key.n().to_bytes_be()),"e":URL_SAFE_NO_PAD.encode(key.e().to_bytes_be())}))
        }).clone();
        let state = Mock {
            issuer,
            pem: Arc::new(pem),
            jwk,
            codes: Arc::new(Mutex::new(HashMap::new())),
            requests: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
            rotated: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        };
        let app=Router::new().route("/.well-known/openid-configuration",get(|State(s):State<Mock>|async move{Json(json!({"issuer":s.issuer,"authorization_endpoint":format!("{}authorize",s.issuer),"token_endpoint":format!("{}oauth/token",s.issuer),"jwks_uri":format!("{}jwks",s.issuer),"response_types_supported":["code"],"subject_types_supported":["public"],"id_token_signing_alg_values_supported":["RS256"],"code_challenge_methods_supported":["S256"],"authorization_response_iss_parameter_supported":true,"token_endpoint_auth_methods_supported":["client_secret_basic","none"]}))}))
            .route("/jwks",get(|State(s):State<Mock>|async move{s.requests.fetch_add(1,std::sync::atomic::Ordering::SeqCst);let mut jwk=s.jwk.clone(); if s.rotated.load(std::sync::atomic::Ordering::SeqCst) {jwk["kid"]=json!("rotated-key");} Json(json!({"keys":[jwk]}))}))
            .route("/oauth/token",post(|State(s):State<Mock>,headers:HeaderMap,Form(form):Form<HashMap<String,String>>|async move{
                let expected=format!("Basic {}",base64::engine::general_purpose::STANDARD.encode("web-client:test-web-secret"));
                if headers.get(header::AUTHORIZATION).and_then(|h|h.to_str().ok()) != Some(expected.as_str()) {return (StatusCode::UNAUTHORIZED,Json(json!({"error":"invalid_client"})));}
                let code=form.get("code").cloned().unwrap_or_default();
                let Some((claims,challenge))=s.codes.lock().await.remove(&code) else {return (StatusCode::BAD_REQUEST,Json(json!({"error":"invalid_grant"})));};
                let verifier=form.get("code_verifier").cloned().unwrap_or_default();
                if URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))!=challenge {return (StatusCode::BAD_REQUEST,Json(json!({"error":"invalid_grant"})));}
                (StatusCode::OK,Json(json!({"access_token":"discarded-web-access-token","token_type":"Bearer","expires_in":300,"id_token":s.sign(claims)})))
            })).with_state(state.clone());
        let task = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        Self { state, task }
    }
    async fn code(&self, url: &str, sub: &str, overrides: Value) -> (String, String) {
        let params: HashMap<String, String> = Url::parse(url)
            .unwrap()
            .query_pairs()
            .into_owned()
            .collect();
        assert_eq!(params["code_challenge_method"], "S256");
        assert!(!params.contains_key("client_secret"));
        assert_eq!(
            params["redirect_uri"],
            "https://203.0.113.7:8443/api/access/callback"
        );
        // Real providers may bind the access token into the ID token. Exercise
        // this post-exchange check on successful callbacks too.
        let digest = Sha256::digest(b"discarded-web-access-token");
        let at_hash = URL_SAFE_NO_PAD.encode(&digest[..16]);
        let mut claims = json!({"iss":self.state.issuer,"sub":sub,"aud":"web-client","iat":now(),"exp":now()+300,"nonce":params["nonce"],"name":"Test Person","at_hash":at_hash});
        if let Some(overrides) = overrides.as_object() {
            for (k, v) in overrides {
                claims[k] = v.clone();
            }
        }
        let code = store::random();
        self.state
            .codes
            .lock()
            .await
            .insert(code.clone(), (claims, params["code_challenge"].clone()));
        (code, params["state"].clone())
    }
}
impl Mock {
    fn sign(&self, claims: Value) -> String {
        let mut h = Header::new(Algorithm::RS256);
        h.kid = Some(
            if self.rotated.load(std::sync::atomic::Ordering::SeqCst) {
                "rotated-key"
            } else {
                "test-key"
            }
            .into(),
        );
        encode(
            &h,
            &claims,
            &EncodingKey::from_rsa_pem(self.pem.as_bytes()).unwrap(),
        )
        .unwrap()
    }
}
async fn start_test(f: &Fixture, idp: &Idp, cookie: &str, csrf: &str) -> (String, String) {
    let r=f.call("POST","/api/access/auth0/test",&[("cookie",cookie),("x-lvk-csrf",csrf)],json!({"issuer":idp.state.issuer,"client_id":"web-client","client_secret":"test-web-secret","mcp_enabled":true})).await;
    assert_eq!(r.status(), StatusCode::OK);
    let tx = response_cookie(&r, "lvk_transaction");
    let url = body(r).await["authorize_url"].as_str().unwrap().to_owned();
    (tx, url)
}
async fn callback(f: &Fixture, idp: &Idp, cookie: &str, code: &str, state: &str) -> Response {
    let query = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("state", state)
        .append_pair("code", code)
        .append_pair("iss", &idp.state.issuer)
        .finish();
    f.call(
        "GET",
        &format!("/api/access/callback?{query}"),
        &[("cookie", cookie)],
        Value::Null,
    )
    .await
}
async fn activate(f: &Fixture, idp: &Idp, cookie: &str, csrf: &str) -> (String, String) {
    let (tx, url) = start_test(f, idp, cookie, csrf).await;
    let (code, state) = idp.code(&url, "auth0|admin", Value::Null).await;
    assert_eq!(
        callback(f, idp, &format!("{cookie}; {tx}"), &code, &state)
            .await
            .status(),
        StatusCode::SEE_OTHER
    );
    let r = f
        .call(
            "POST",
            "/api/access/activate",
            &[("cookie", cookie), ("x-lvk-csrf", csrf)],
            Value::Null,
        )
        .await;
    assert_eq!(r.status(), StatusCode::OK);
    let cookie = response_cookie(&r, "lvk_session");
    let csrf = body(r).await["csrf"].as_str().unwrap().to_owned();
    (cookie, csrf)
}
#[tokio::test]
async fn oidc_auth0_rfc3339_updated_at_preserves_validation() {
    let f = Fixture::new(true).await;
    let idp = Idp::new().await;
    let (cookie, csrf) = f.setup().await;
    for updated_at in [
        json!("2026-10-17T07:53:42.672Z"),
        json!("2026-10-17T16:53:42+09:00"),
        json!(1729167600),
        Value::Null,
    ] {
        let (tx, url) = start_test(&f, &idp, &cookie, &csrf).await;
        let (code, state) = idp
            .code(&url, "github|test-user", json!({"updated_at":updated_at}))
            .await;
        assert_eq!(
            callback(&f, &idp, &format!("{cookie}; {tx}"), &code, &state)
                .await
                .status(),
            StatusCode::SEE_OTHER
        );
    }
    for overrides in [
        json!({"updated_at":"not-a-date"}),
        json!({"updated_at":true}),
        json!({"updated_at":"2026-10-17T07:53:42Z", "nonce":"wrong"}),
        json!({"updated_at":"2026-10-17T07:53:42Z", "aud":"wrong"}),
        json!({"updated_at":"2026-10-17T07:53:42Z", "iss":"https://wrong.auth0.com/"}),
        json!({"updated_at":"2026-10-17T07:53:42Z", "exp":now()-300}),
        // The compatibility option also supports RFC3339 security timestamps;
        // expiry validation must still reject an expired string timestamp.
        json!({"updated_at":"2026-10-17T07:53:42Z", "exp":"2000-01-01T00:00:00Z"}),
        json!({"updated_at":"2026-10-17T07:53:42Z", "at_hash":"wrong"}),
    ] {
        let (tx, url) = start_test(&f, &idp, &cookie, &csrf).await;
        let (code, state) = idp.code(&url, "github|test-user", overrides).await;
        assert_eq!(
            callback(&f, &idp, &format!("{cookie}; {tx}"), &code, &state)
                .await
                .status(),
            StatusCode::UNAUTHORIZED
        );
    }
}

#[tokio::test]
async fn oidc_browser_state_nonce_audience_replay_and_initial_admin_binding() {
    let f = Fixture::new(true).await;
    let idp = Idp::new().await;
    let (cookie, csrf) = f.setup().await;
    let (tx, url) = start_test(&f, &idp, &cookie, &csrf).await;
    let (code, state) = idp.code(&url, "auth0|admin", Value::Null).await;
    assert_eq!(
        callback(&f, &idp, &cookie, &code, &state).await.status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        callback(
            &f,
            &idp,
            &format!("{cookie}; {tx}"),
            &code,
            "different-state"
        )
        .await
        .status(),
        StatusCode::UNAUTHORIZED
    );
    // Even a stolen transaction cookie cannot replace the setup browser session.
    assert_eq!(
        callback(&f, &idp, &tx, &code, &state).await.status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        f.call(
            "POST",
            "/api/access/activate",
            &[("cookie", &cookie), ("x-lvk-csrf", &csrf)],
            Value::Null
        )
        .await
        .status(),
        StatusCode::UNAUTHORIZED
    );
    for overrides in [
        json!({"nonce":"wrong"}),
        json!({"aud":"another-client"}),
        json!({"iss":"https://wrong.auth0.com/"}),
        json!({"exp":now()-300}),
        json!({"at_hash":"wrong-access-token-hash"}),
    ] {
        let (tx, url) = start_test(&f, &idp, &cookie, &csrf).await;
        let (code, state) = idp.code(&url, "auth0|admin", overrides).await;
        assert_eq!(
            callback(&f, &idp, &format!("{cookie}; {tx}"), &code, &state)
                .await
                .status(),
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            callback(&f, &idp, &format!("{cookie}; {tx}"), &code, &state)
                .await
                .status(),
            StatusCode::UNAUTHORIZED
        );
    }
    let (web, web_csrf) = activate(&f, &idp, &cookie, &csrf).await;
    assert_ne!(cookie, web);
    assert_eq!(
        f.call("GET", "/api/projects", &[("cookie", &web)], Value::Null)
            .await
            .status(),
        StatusCode::OK
    );
    let current = body(
        f.call(
            "GET",
            "/api/access/configuration",
            &[("cookie", &web)],
            Value::Null,
        )
        .await,
    )
    .await;
    assert!(!current.to_string().contains("test-web-secret"));
    assert_eq!(
        f.call(
            "POST",
            "/api/access/activate",
            &[("cookie", &cookie), ("x-lvk-csrf", &csrf)],
            Value::Null
        )
        .await
        .status(),
        StatusCode::UNAUTHORIZED
    );
    let list = body(
        f.call(
            "GET",
            "/api/access/members",
            &[("cookie", &web)],
            Value::Null,
        )
        .await,
    )
    .await;
    let id = list["members"][0]["id"].as_str().unwrap();
    assert_eq!(
        f.call(
            "POST",
            "/api/access/members",
            &[("cookie", &web), ("x-lvk-csrf", &web_csrf)],
            json!({"id":id,"state":"disabled","role":"member"})
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );
}
#[tokio::test]
async fn jwt_resource_membership_scope_stream_revocation_and_no_basic_bypass() {
    let f = Fixture::new(true).await;
    let idp = Idp::new().await;
    let (basic_cookie, basic_csrf) = f.basic().await;
    // A failed candidate cannot change the currently active protection.
    let r=f.call("POST","/api/access/auth0/test",&[("cookie",&basic_cookie),("x-lvk-csrf",&basic_csrf)],json!({"issuer":"http://169.254.169.254/","client_id":"web-client","client_secret":"secret","mcp_enabled":true})).await;
    assert_eq!(r.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        f.call(
            "GET",
            "/api/projects",
            &[("cookie", &basic_cookie)],
            Value::Null
        )
        .await
        .status(),
        StatusCode::OK
    );
    let (web, csrf) = activate(&f, &idp, &basic_cookie, &basic_csrf).await;
    assert_eq!(
        f.call(
            "GET",
            "/api/projects",
            &[("cookie", &basic_cookie)],
            Value::Null
        )
        .await
        .status(),
        StatusCode::UNAUTHORIZED
    );
    let old_basic = format!(
        "Basic {}",
        base64::engine::general_purpose::STANDARD.encode("owner:a long test password")
    );
    assert_eq!(
        f.call(
            "GET",
            "/api/projects",
            &[("authorization", &old_basic)],
            Value::Null
        )
        .await
        .status(),
        StatusCode::UNAUTHORIZED
    );
    let metadata = body(
        f.call(
            "GET",
            "/.well-known/oauth-protected-resource/mcp",
            &[],
            Value::Null,
        )
        .await,
    )
    .await;
    assert_eq!(metadata["resource"], "https://203.0.113.7:8443/mcp");
    let missing = f
        .call(
            "POST",
            "/mcp",
            &[("cookie", &web), ("mcp-session-id", "stolen")],
            Value::Null,
        )
        .await;
    assert_eq!(missing.status(), StatusCode::UNAUTHORIZED);
    assert!(missing.headers().contains_key(header::WWW_AUTHENTICATE));
    let claims = json!({"iss":idp.state.issuer,"sub":"auth0|admin","aud":["https://203.0.113.7:8443/mcp","userinfo"],"exp":now()+300,"scope":"lvk:read lvk:write lvk:merge"});
    let token = idp.state.sign(claims.clone());
    let bearer = format!("Bearer {token}");
    assert_eq!(
        f.call("POST", "/mcp", &[("authorization", &bearer)], Value::Null)
            .await
            .status(),
        StatusCode::OK
    );
    for overrides in [
        json!({"iss":"https://wrong.auth0.com/"}),
        json!({"aud":"web-client"}),
        json!({"aud":"https://203.0.113.7:8443/mcp/"}),
        json!({"exp":now()-1}),
        json!({"nbf":now()+300}),
    ] {
        let mut bad = claims.clone();
        for (k, v) in overrides.as_object().unwrap() {
            bad[k] = v.clone();
        }
        let bearer = format!("Bearer {}", idp.state.sign(bad));
        assert_eq!(
            f.call("POST", "/mcp", &[("authorization", &bearer)], Value::Null)
                .await
                .status(),
            StatusCode::UNAUTHORIZED
        );
    }
    let mut no_scope = claims.clone();
    no_scope["scope"] = json!("openid profile");
    let r = f
        .call(
            "POST",
            "/mcp",
            &[(
                "authorization",
                &format!("Bearer {}", idp.state.sign(no_scope)),
            )],
            Value::Null,
        )
        .await;
    assert_eq!(r.status(), StatusCode::FORBIDDEN);
    assert!(
        r.headers()[header::WWW_AUTHENTICATE]
            .to_str()
            .unwrap()
            .contains("insufficient_scope")
    );
    let mut pending = claims.clone();
    pending["sub"] = json!("github|member");
    let pending_bearer = format!("Bearer {}", idp.state.sign(pending));
    assert_eq!(
        f.call(
            "POST",
            "/mcp",
            &[("authorization", &pending_bearer)],
            Value::Null
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );
    let member = f
        .access
        .identity(&idp.state.issuer, "github|member", None)
        .await
        .unwrap();
    assert_eq!(
        f.call(
            "POST",
            "/api/access/members",
            &[("cookie", &web), ("x-lvk-csrf", &csrf)],
            json!({"id":member.id,"state":"active","role":"member"})
        )
        .await
        .status(),
        StatusCode::OK
    );
    let mut headers = HeaderMap::new();
    headers.insert(header::AUTHORIZATION, pending_bearer.parse().unwrap());
    let p = f.access.authenticate_mcp(&headers).await.unwrap();
    let lease = Lease {
        access: f.access.clone(),
        principal: p,
    };
    let (member_session, _) = f
        .access
        .create_session(Some(&member.id), "web", 2)
        .await
        .unwrap();
    let member_cookie = format!("lvk_session={member_session}");
    assert_eq!(
        f.call(
            "GET",
            "/api/access/members",
            &[("cookie", &member_cookie)],
            Value::Null
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        f.call(
            "POST",
            "/api/access/members",
            &[("cookie", &web), ("x-lvk-csrf", &csrf)],
            json!({"id":member.id,"state":"disabled","role":"member"})
        )
        .await
        .status(),
        StatusCode::OK
    );
    assert_eq!(
        f.call(
            "POST",
            "/mcp",
            &[
                ("authorization", &pending_bearer),
                ("mcp-session-id", "another-user")
            ],
            Value::Null
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        f.call(
            "GET",
            "/api/projects",
            &[("cookie", &member_cookie)],
            Value::Null
        )
        .await
        .status(),
        StatusCode::UNAUTHORIZED
    );
    assert!(
        tokio::time::timeout(Duration::from_secs(3), lease.until_revoked())
            .await
            .is_ok()
    );
    let before = idp.state.requests.load(std::sync::atomic::Ordering::SeqCst);
    for n in 0..8 {
        let mut h = Header::new(Algorithm::RS256);
        h.kid = Some(format!("unknown-{n}"));
        let bad = encode(
            &h,
            &claims,
            &EncodingKey::from_rsa_pem(idp.state.pem.as_bytes()).unwrap(),
        )
        .unwrap();
        let _ = f
            .call(
                "POST",
                "/mcp",
                &[("authorization", &format!("Bearer {bad}"))],
                Value::Null,
            )
            .await;
    }
    assert_eq!(
        idp.state.requests.load(std::sync::atomic::Ordering::SeqCst),
        before
    );
}
#[test]
fn trusted_issuer_and_public_ip_configuration() {
    assert!(validate_public_url("https://203.0.113.7:8443").is_ok());
    for url in [
        "http://203.0.113.7",
        "https://203.0.113.7/",
        "https://u:p@203.0.113.7",
        "https://203.0.113.7/path",
    ] {
        assert!(validate_public_url(url).is_err());
    }
    assert!(provider::validate_issuer("https://tenant.jp.auth0.com/").is_ok());
    for issuer in [
        "https://evil.example/",
        "https://tenant.auth0.com",
        "https://tenant.auth0.com@evil.example/",
        "https://tenant.auth0.com/path",
    ] {
        assert!(provider::validate_issuer(issuer).is_err());
    }
}

#[tokio::test]
async fn identities_do_not_merge_and_setup_keys_are_not_reusable() {
    let f = Fixture::new(true).await;
    let google = f
        .access
        .identity(
            "https://tenant.auth0.com/",
            "google-oauth2|123",
            Some("same-name"),
        )
        .await
        .unwrap();
    let github = f
        .access
        .identity("https://tenant.auth0.com/", "github|123", Some("same-name"))
        .await
        .unwrap();
    let tenant = f
        .access
        .identity("https://other.auth0.com/", "github|123", None)
        .await
        .unwrap();
    assert_ne!(google.id, github.id);
    assert_ne!(github.id, tenant.id);
    assert_eq!(google.state, "pending");
    assert_eq!(google.role, "member");
    let key = issue_key(f.access.pool(), &f.access.0.key_path, false)
        .await
        .unwrap();
    let r = f
        .call(
            "POST",
            "/api/access/setup",
            &[("x-lvk-csrf", "setup")],
            json!({"key":key}),
        )
        .await;
    assert_eq!(r.status(), StatusCode::OK);
    assert_eq!(
        f.call(
            "POST",
            "/api/access/setup",
            &[("x-lvk-csrf", "setup")],
            json!({"key":key})
        )
        .await
        .status(),
        StatusCode::UNAUTHORIZED
    );
    let cookie = response_cookie(&r, "lvk_session");
    let csrf = body(r).await["csrf"].as_str().unwrap().to_owned();
    assert_eq!(
        f.call(
            "POST",
            "/api/access/local",
            &[("cookie", &cookie), ("x-lvk-csrf", &csrf)],
            Value::Null
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );
    sqlx::query("UPDATE access_sessions SET expires=?")
        .bind(now() - 1)
        .execute(f.access.pool())
        .await
        .unwrap();
    assert_eq!(
        f.call(
            "GET",
            "/api/access/configuration",
            &[("cookie", &cookie)],
            Value::Null
        )
        .await
        .status(),
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
async fn bearer_signatures_sessions_and_web_cookie_separation() {
    let f = Fixture::new(true).await;
    let idp = Idp::new().await;
    let (setup, csrf) = f.setup().await;
    let (web, _) = activate(&f, &idp, &setup, &csrf).await;
    let claims = json!({"iss":idp.state.issuer,"sub":"auth0|admin","aud":f.access.resource(),"exp":now()+300,"scope":"lvk:read"});
    let token = idp.state.sign(claims.clone());
    assert_eq!(
        f.call(
            "POST",
            "/mcp",
            &[
                ("authorization", &format!("Bearer {token}")),
                ("mcp-session-id", "someone-elses-session")
            ],
            Value::Null
        )
        .await
        .status(),
        StatusCode::BAD_REQUEST
    );
    let mut pieces: Vec<String> = token.split('.').map(str::to_owned).collect();
    let mut signature = URL_SAFE_NO_PAD.decode(&pieces[2]).unwrap();
    signature[0] ^= 1;
    pieces[2] = URL_SAFE_NO_PAD.encode(signature);
    assert_eq!(
        f.call(
            "POST",
            "/mcp",
            &[("authorization", &format!("Bearer {}", pieces.join(".")))],
            Value::Null
        )
        .await
        .status(),
        StatusCode::UNAUTHORIZED
    );
    let hs = encode(
        &Header::new(Algorithm::HS256),
        &claims,
        &EncodingKey::from_secret(b"untrusted"),
    )
    .unwrap();
    assert_eq!(
        f.call(
            "POST",
            "/mcp",
            &[("authorization", &format!("Bearer {hs}"))],
            Value::Null
        )
        .await
        .status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        f.call(
            "POST",
            "/mcp",
            &[(
                "authorization",
                &format!("Bearer {}", web.split_once('=').unwrap().1)
            )],
            Value::Null
        )
        .await
        .status(),
        StatusCode::UNAUTHORIZED
    );
    let status = body(
        f.call(
            "GET",
            "/api/access/status",
            &[("cookie", &web)],
            Value::Null,
        )
        .await,
    )
    .await;
    let csrf = status["csrf"].as_str().unwrap();
    let mut headers = HeaderMap::new();
    headers.insert(header::COOKIE, web.parse().unwrap());
    let p = f.access.web_principal(&headers).await.unwrap();
    let lease = Lease {
        access: f.access.clone(),
        principal: p,
    };
    assert_eq!(
        f.call(
            "POST",
            "/api/access/logout",
            &[("cookie", &web), ("x-lvk-csrf", csrf)],
            Value::Null
        )
        .await
        .status(),
        StatusCode::NO_CONTENT
    );
    assert!(
        tokio::time::timeout(Duration::from_secs(3), lease.until_revoked())
            .await
            .is_ok()
    );
    // Web logout does not revoke a separate OAuth grant/JWT.
    assert_eq!(
        f.call(
            "POST",
            "/mcp",
            &[("authorization", &format!("Bearer {token}"))],
            Value::Null
        )
        .await
        .status(),
        StatusCode::OK
    );
}

#[tokio::test]
async fn deleted_settings_do_not_restore_local_access_and_audit_redacts_secrets() {
    let f = Fixture::new(false).await;
    let (cookie, csrf) = f.setup().await;
    assert_eq!(
        f.call(
            "POST",
            "/api/access/local",
            &[("cookie", &cookie), ("x-lvk-csrf", &csrf)],
            Value::Null
        )
        .await
        .status(),
        StatusCode::OK
    );
    sqlx::query("DELETE FROM access_settings")
        .execute(f.access.pool())
        .await
        .unwrap();
    assert_eq!(
        f.call("GET", "/api/projects", &[], Value::Null)
            .await
            .status(),
        StatusCode::LOCKED
    );
    let audit: Vec<String> =
        sqlx::query_scalar("SELECT action||COALESCE(target,'')||outcome FROM access_audit")
            .fetch_all(f.access.pool())
            .await
            .unwrap();
    assert!(
        audit
            .iter()
            .all(|event| !event.contains(&cookie) && !event.contains(&csrf))
    );
}

#[tokio::test]
async fn sse_body_stops_after_server_session_revocation() {
    use axum::response::sse::{Event, Sse};
    let f = Fixture::new(true).await;
    let (cookie, _) = f.basic().await;
    let app = f.access.layer(Router::new().route(
        "/api/events",
        get(|| async {
            Sse::new(futures_util::stream::unfold(0, |n| async move {
                tokio::time::sleep(Duration::from_millis(25)).await;
                Some((
                    Ok::<_, std::convert::Infallible>(Event::default().data(n.to_string())),
                    n + 1,
                ))
            }))
        }),
    ));
    let response = app
        .oneshot(
            http::Request::builder()
                .uri("/api/events")
                .header("host", "203.0.113.7:8443")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    sqlx::query("DELETE FROM access_sessions")
        .execute(f.access.pool())
        .await
        .unwrap();
    let collected =
        tokio::time::timeout(Duration::from_secs(3), response.into_body().collect()).await;
    assert!(
        collected.is_ok(),
        "an established SSE response must end after logout/revocation"
    );
}

#[tokio::test]
async fn concurrent_administrator_disable_keeps_one_active_admin() {
    let f = Fixture::new(true).await;
    let idp = Idp::new().await;
    let (setup, csrf) = f.setup().await;
    let (first, first_csrf) = activate(&f, &idp, &setup, &csrf).await;
    let first_member = f
        .access
        .identity(&idp.state.issuer, "auth0|admin", None)
        .await
        .unwrap();
    let second_member = f
        .access
        .identity(&idp.state.issuer, "auth0|second", None)
        .await
        .unwrap();
    sqlx::query("UPDATE access_members SET state='active',role='admin' WHERE id=?")
        .bind(&second_member.id)
        .execute(f.access.pool())
        .await
        .unwrap();
    let (token, second_csrf) = f
        .access
        .create_session(Some(&second_member.id), "web", 1)
        .await
        .unwrap();
    let second = format!("lvk_session={token}");
    let first_headers = [
        ("cookie", first.as_str()),
        ("x-lvk-csrf", first_csrf.as_str()),
    ];
    let second_headers = [
        ("cookie", second.as_str()),
        ("x-lvk-csrf", second_csrf.as_str()),
    ];
    let (a, b) = tokio::join!(
        f.call(
            "POST",
            "/api/access/members",
            &first_headers,
            json!({"id":second_member.id,"state":"disabled","role":"member"})
        ),
        f.call(
            "POST",
            "/api/access/members",
            &second_headers,
            json!({"id":first_member.id,"state":"disabled","role":"member"})
        )
    );
    assert_eq!(
        [a.status(), b.status()]
            .iter()
            .filter(|s| s.is_success())
            .count(),
        1
    );
    let remaining: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM access_members WHERE state='active' AND role='admin'",
    )
    .fetch_one(f.access.pool())
    .await
    .unwrap();
    assert_eq!(remaining, 1);
}

#[tokio::test]
async fn web_and_mcp_signing_key_rotation_uses_the_trusted_cache() {
    use std::sync::atomic::Ordering;
    let f = Fixture::new(true).await;
    let idp = Idp::new().await;
    let (cookie, csrf) = f.setup().await;
    let (tx, url) = start_test(&f, &idp, &cookie, &csrf).await;
    idp.state.rotated.store(true, Ordering::SeqCst);
    f.access.0.provider.lock().await.advance_refresh_window();
    let (code, state) = idp.code(&url, "auth0|admin", Value::Null).await;
    assert_eq!(
        callback(&f, &idp, &format!("{cookie}; {tx}"), &code, &state)
            .await
            .status(),
        StatusCode::SEE_OTHER
    );
    assert_eq!(idp.state.requests.load(Ordering::SeqCst), 2);
    let response = f
        .call(
            "POST",
            "/api/access/activate",
            &[("cookie", &cookie), ("x-lvk-csrf", &csrf)],
            Value::Null,
        )
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    // Rotate again after the Web flow; MCP must refresh and verify too.
    idp.state.rotated.store(false, Ordering::SeqCst);
    f.access.0.provider.lock().await.advance_refresh_window();
    let token=idp.state.sign(json!({"iss":idp.state.issuer,"sub":"auth0|admin","aud":f.access.resource(),"exp":now()+300,"scope":"lvk:read"}));
    assert_eq!(
        f.call(
            "POST",
            "/mcp",
            &[("authorization", &format!("Bearer {token}"))],
            Value::Null
        )
        .await
        .status(),
        StatusCode::OK
    );
    assert_eq!(idp.state.requests.load(Ordering::SeqCst), 3);
}

#[tokio::test]
async fn invalid_setup_attempts_are_rate_limited_and_window_recovers() {
    let f = Fixture::new(true).await;
    for _ in 0..30 {
        assert_eq!(
            f.call(
                "POST",
                "/api/access/setup",
                &[("x-lvk-csrf", "setup")],
                json!({"key":"invalid"})
            )
            .await
            .status(),
            StatusCode::UNAUTHORIZED
        );
    }
    assert_eq!(
        f.call(
            "POST",
            "/api/access/setup",
            &[("x-lvk-csrf", "setup")],
            json!({"key":"invalid"})
        )
        .await
        .status(),
        StatusCode::TOO_MANY_REQUESTS
    );
    *f.access.0.attempts.lock().await = (now() - 61, 30);
    assert_eq!(
        f.call(
            "POST",
            "/api/access/setup",
            &[("x-lvk-csrf", "setup")],
            json!({"key":"invalid"})
        )
        .await
        .status(),
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
async fn websocket_stops_after_disable_and_expiry() {
    use axum::extract::ws::{Message, WebSocketUpgrade};
    use futures_util::StreamExt;
    use tokio_tungstenite::tungstenite::client::IntoClientRequest;
    for expire in [false, true] {
        let f = Fixture::new(true).await;
        let (cookie, _) = f.basic().await;
        let app = f.access.layer(Router::new().route(
            "/api/socket",
            get(
                |ws: WebSocketUpgrade, axum::Extension(lease): axum::Extension<Lease>| async move {
                    ws.on_upgrade(move |mut socket| async move {
                        lease
                            .run(async move {
                                socket.send(Message::Text("ready".into())).await.unwrap();
                                while socket.recv().await.is_some() {}
                            })
                            .await;
                    })
                },
            ),
        ));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let mut request = format!("ws://{address}/api/socket")
            .into_client_request()
            .unwrap();
        request
            .headers_mut()
            .insert(header::HOST, "203.0.113.7:8443".parse().unwrap());
        request
            .headers_mut()
            .insert(header::COOKIE, cookie.parse().unwrap());
        let (mut socket, _) = tokio_tungstenite::connect_async(request).await.unwrap();
        assert_eq!(
            socket.next().await.unwrap().unwrap().into_text().unwrap(),
            "ready"
        );
        if expire {
            sqlx::query("UPDATE access_sessions SET expires=?")
                .bind(now() - 1)
                .execute(f.access.pool())
                .await
                .unwrap();
        } else {
            sqlx::query("UPDATE access_members SET state='disabled'")
                .execute(f.access.pool())
                .await
                .unwrap();
        }
        let ended = tokio::time::timeout(Duration::from_secs(4), socket.next())
            .await
            .expect("WebSocket must stop after revocation");
        assert!(!matches!(
            ended,
            Some(Ok(tokio_tungstenite::tungstenite::Message::Text(_)))
        ));
        task.abort();
        let _ = task.await;
    }
}

// Invoked in a fresh OS process by the transition test. No production network
// or account is used; the child reopens the persisted database and key files.
#[tokio::test]
async fn persisted_authentication_child_process() {
    let Ok(directory) = std::env::var("LVK_TEST_ACCESS_REOPEN") else {
        return;
    };
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .connect_with(
            sqlx::sqlite::SqliteConnectOptions::new()
                .filename(std::path::Path::new(&directory).join("test.sqlite"))
                .foreign_keys(true),
        )
        .await
        .unwrap();
    let access = Access(Arc::new(Inner {
        pool,
        preview_domain: Some("preview.example".into()),
        public: true,
        public_url: Some("https://203.0.113.7:8443".into()),
        forced_mode: None,
        key_path: std::path::Path::new(&directory).join("access.key"),
        internal_token: store::random(),
        provider: Mutex::new(provider::Cache::default()),
        attempts: Mutex::new((now(), 0)),
        password_slots: tokio::sync::Semaphore::new(2),
        provisioning: Mutex::new(()),
    }));
    let expected = std::env::var("LVK_TEST_EXPECT_MODE").unwrap();
    assert_eq!(
        access.configuration().await.unwrap().0.unwrap().mode(),
        expected
    );
    let saved: Value = serde_json::from_slice(
        &std::fs::read(std::path::Path::new(&directory).join("assertions.json")).unwrap(),
    )
    .unwrap();
    for (name, allowed) in [("old", expected == "basic"), ("new", expected == "auth0")] {
        if let Some(cookie) = saved[name].as_str() {
            let mut headers = HeaderMap::new();
            headers.insert(header::COOKIE, cookie.parse().unwrap());
            assert_eq!(access.web_principal(&headers).await.is_ok(), allowed);
        }
    }
    access.pool().close().await;
}

#[tokio::test]
async fn authentication_transition_survives_process_restart() {
    let f = Fixture::new(true).await;
    let idp = Idp::new().await;
    let (old, csrf) = f.basic().await;
    let probe = |expected: &str, new: Option<&str>| {
        std::fs::write(
            f._dir.path().join("assertions.json"),
            serde_json::to_vec(&json!({"old":old,"new":new})).unwrap(),
        )
        .unwrap();
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "tests::persisted_authentication_child_process",
                "--nocapture",
            ])
            .env("LVK_TEST_ACCESS_REOPEN", f._dir.path())
            .env("LVK_TEST_EXPECT_MODE", expected)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "child failed: {} {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    };
    probe("basic", None);
    // An unfinished candidate must not alter the persisted active protection.
    let _ = start_test(&f, &idp, &old, &csrf).await;
    probe("basic", None);
    let (new, _) = activate(&f, &idp, &old, &csrf).await;
    f.access.pool().close().await;
    probe("auth0", Some(&new));
}

#[tokio::test]
async fn preview_handoff_is_browser_origin_bound_and_revocable() {
    let f = Fixture::new(true).await;
    let (web, csrf) = f.basic().await;
    let origin = "https://5173.preview.example:8443";
    let preview = f
        .access
        .preview_layer(Router::new().fallback(|headers: HeaderMap| async move {
            assert!(!headers.contains_key(header::AUTHORIZATION));
            assert!(
                !headers
                    .get(header::COOKIE)
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or("")
                    .contains("lvk_")
            );
            "project content"
        }));
    let request = |path: &str, cookie: &str, method: &str, body: String| {
        http::Request::builder()
            .uri(path)
            .method(method)
            .header("host", "5173.preview.example:8443")
            .header("cookie", cookie)
            .header("origin", "https://203.0.113.7:8443")
            .header("content-type", "application/x-www-form-urlencoded")
            .body(Body::from(body))
            .unwrap()
    };
    let landing = preview
        .clone()
        .oneshot(
            http::Request::builder()
                .uri("/")
                .header("host", "5173.preview.example:8443")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let browser = response_cookie(&landing, "__Host-lvk_preview_browser");
    let browser_hash = store::hash(browser.split_once('=').unwrap().1);
    let values = [
        ("origin", origin),
        ("browser", browser_hash.as_str()),
        ("csrf", csrf.as_str()),
    ];
    let encoded = url::form_urlencoded::Serializer::new(String::new())
        .extend_pairs(values)
        .finish();
    let response = f
        .app()
        .oneshot(
            http::Request::builder()
                .uri("/api/access/preview/authorize")
                .method("POST")
                .header("host", "203.0.113.7:8443")
                .header("origin", "https://203.0.113.7:8443")
                .header("cookie", &web)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(encoded))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let html = String::from_utf8(
        response
            .into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes()
            .to_vec(),
    )
    .unwrap();
    let grant = html
        .split("name=grant value=\"")
        .nth(1)
        .unwrap()
        .split('"')
        .next()
        .unwrap();
    let encoded = format!("grant={grant}");
    let denied = preview
        .clone()
        .oneshot(request(
            "/.lvk-preview/exchange",
            "__Host-lvk_preview_browser=other",
            "POST",
            encoded.clone(),
        ))
        .await
        .unwrap();
    assert_eq!(denied.status(), StatusCode::UNAUTHORIZED);
    let accepted = preview
        .clone()
        .oneshot(request(
            "/.lvk-preview/exchange",
            &browser,
            "POST",
            encoded.clone(),
        ))
        .await
        .unwrap();
    assert_eq!(accepted.status(), StatusCode::SEE_OTHER);
    let credential = response_cookie(&accepted, "__Host-lvk_preview");
    assert_eq!(
        preview
            .clone()
            .oneshot(request("/.lvk-preview/exchange", &browser, "POST", encoded))
            .await
            .unwrap()
            .status(),
        StatusCode::UNAUTHORIZED
    );
    // Preview capability cannot authenticate the business API.
    assert_eq!(
        f.call(
            "GET",
            "/api/projects",
            &[("cookie", &credential)],
            Value::Null
        )
        .await
        .status(),
        StatusCode::UNAUTHORIZED
    );
    let project_request = || {
        http::Request::builder()
            .uri("/")
            .header("host", "5173.preview.example:8443")
            .header("cookie", format!("{credential}; {web}"))
            .header("authorization", "Basic must-not-reach-project")
            .body(Body::empty())
            .unwrap()
    };
    let response = preview.clone().oneshot(project_request()).await.unwrap();
    assert_eq!(
        response.into_body().collect().await.unwrap().to_bytes(),
        "project content"
    );
    let foreign = preview
        .clone()
        .oneshot(
            http::Request::builder()
                .uri("/")
                .header("host", "5174.preview.example:8443")
                .header("cookie", &credential)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let content = String::from_utf8(
        foreign
            .into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes()
            .to_vec(),
    )
    .unwrap();
    assert!(content.contains("authorise this preview"));
    f.call(
        "POST",
        "/api/access/logout",
        &[("cookie", &web), ("x-lvk-csrf", &csrf)],
        Value::Null,
    )
    .await;
    let response = preview.oneshot(project_request()).await.unwrap();
    let content = String::from_utf8(
        response
            .into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes()
            .to_vec(),
    )
    .unwrap();
    assert!(content.contains("authorise this preview"));
}

mod provisioning;

mod chatgpt;
