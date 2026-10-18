use axum::extract::OriginalUri;

use super::*;

#[derive(Default)]
struct Remote {
    clients: Vec<Value>,
    resources: Vec<Value>,
    tenant: Value,
    writes: Vec<(String, Value)>,
    enabled: HashMap<String, BTreeSet<String>>,
    fail_resource_once: bool,
    redirect: bool,
    reject: Option<StatusCode>,
    management_token: Option<String>,
}
struct Auth0 {
    issuer: String,
    state: Arc<Mutex<Remote>>,
    task: tokio::task::JoinHandle<()>,
}
impl Drop for Auth0 {
    fn drop(&mut self) {
        self.task.abort();
    }
}
impl Auth0 {
    async fn new() -> Self {
        let state = Arc::new(Mutex::new(Remote {
            tenant: json!({"unrelated_setting":true}),
            enabled: HashMap::from([
                ("con_github".into(), BTreeSet::from(["other_app".into()])),
                ("con_password".into(), BTreeSet::from(["other_app".into()])),
            ]),
            ..Default::default()
        }));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let issuer = format!("http://{}/", listener.local_addr().unwrap());
        let app = Router::new().fallback(mock).with_state(state.clone());
        let task = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        Self {
            issuer,
            state,
            task,
        }
    }
    fn input(&self, step: &str) -> Value {
        json!({"issuer":self.issuer,"management_token":"management-secret","step":step,"connection_id":"con_github","mcp_enabled":true,"refresh_tokens":true,"chatgpt_callback":"https://chatgpt.com/connector/callback/test"})
    }
}
async fn mock(
    State(state): State<Arc<Mutex<Remote>>>,
    method: Method,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    bytes: axum::body::Bytes,
) -> Response {
    let token = state
        .lock()
        .await
        .management_token
        .clone()
        .unwrap_or_else(|| "management-secret".into());
    assert_eq!(headers["authorization"], format!("Bearer {token}"));
    let path = uri.path().strip_prefix("/api/v2/").unwrap();
    let mut state = state.lock().await;
    if let Some(status) = state.reject {
        return (
            status,
            Json(json!({"error":"management-secret MUST NOT ESCAPE"})),
        )
            .into_response();
    }
    if state.redirect {
        return (
            StatusCode::FOUND,
            [(header::LOCATION, "http://127.0.0.1:1/secret")],
        )
            .into_response();
    }
    let body: Value = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap()
    };
    if method != Method::GET {
        state.writes.push((path.into(), body.clone()));
    }
    let value = match (method, path) {
        (Method::GET, "clients") => json!(
            state
                .clients
                .iter()
                .skip(
                    url::form_urlencoded::parse(uri.query().unwrap_or_default().as_bytes())
                        .find(|(key, _)| key == "page")
                        .and_then(|(_, value)| value.parse::<usize>().ok())
                        .unwrap_or(0)
                        * 100
                )
                .take(100)
                .map(|c| {
                    let mut c = c.clone();
                    c.as_object_mut().unwrap().remove("client_secret");
                    c
                })
                .collect::<Vec<_>>()
        ),
        (Method::GET, "connections") => {
            json!([{"id":"con_github","name":"GitHub","strategy":"github"},{"id":"con_password","name":"Database","strategy":"auth0"}])
        }
        (Method::GET, "resource-servers") => json!(state.resources),
        (Method::GET, "tenants/settings") => state.tenant.clone(),
        (Method::PATCH, "tenants/settings") => {
            state
                .tenant
                .as_object_mut()
                .unwrap()
                .extend(body.as_object().unwrap().clone());
            state.tenant.clone()
        }
        (Method::POST, "clients") => {
            let mut c = body;
            c["client_id"] = json!(format!("client_{}", state.clients.len()));
            c["client_secret"] = json!(format!("secret_{}", state.clients.len()));
            state.clients.push(c.clone());
            c
        }
        (Method::POST, "resource-servers") => {
            if state.fail_resource_once {
                state.fail_resource_once = false;
                return (
                    StatusCode::BAD_REQUEST,
                    Json(json!({"error":"management-secret MUST NOT ESCAPE"})),
                )
                    .into_response();
            }
            let mut r = body;
            r["id"] = json!("api_1");
            state.resources.push(r.clone());
            r
        }
        (Method::PATCH, p) if p.starts_with("clients/") => {
            let c = state
                .clients
                .iter_mut()
                .find(|c| c["client_id"] == p.trim_start_matches("clients/"))
                .unwrap();
            c.as_object_mut()
                .unwrap()
                .extend(body.as_object().unwrap().clone());
            c.clone()
        }
        (Method::PATCH, p) if p.starts_with("resource-servers/") => {
            let r = state
                .resources
                .iter_mut()
                .find(|r| r["id"] == p.trim_start_matches("resource-servers/"))
                .unwrap();
            r.as_object_mut()
                .unwrap()
                .extend(body.as_object().unwrap().clone());
            r.clone()
        }
        (Method::PATCH, p) if p.starts_with("connections/") && p.ends_with("/clients") => {
            let connection = p.split('/').nth(1).unwrap();
            for change in body.as_array().unwrap() {
                let enabled = state.enabled.entry(connection.into()).or_default();
                let id = change["client_id"].as_str().unwrap().to_owned();
                if change["status"] == true {
                    enabled.insert(id);
                } else {
                    enabled.remove(&id);
                }
            }
            return StatusCode::NO_CONTENT.into_response();
        }
        (Method::GET, p) if p.starts_with("clients/") => state
            .clients
            .iter()
            .find(|c| c["client_id"] == p.trim_start_matches("clients/"))
            .unwrap()
            .clone(),
        _ => panic!("unexpected mock path {path}"),
    };
    Json(value).into_response()
}
const PATH: &str = "/api/access/auth0/provision";
async fn preview(f: &Fixture, remote: &Auth0, cookie: &str, csrf: &str, mut input: Value) -> Value {
    input["step"] = json!("preview");
    let response = f
        .call(
            "POST",
            PATH,
            &[("cookie", cookie), ("x-lvk-csrf", csrf)],
            input,
        )
        .await;
    assert_eq!(
        response.status(),
        StatusCode::OK,
        "{}",
        body(response).await
    );
    let _ = remote;
    body(response).await
}
async fn apply(
    f: &Fixture,
    cookie: &str,
    csrf: &str,
    mut input: Value,
    receipt: &Value,
) -> Response {
    input["step"] = json!("apply");
    input["confirmation"] = receipt["confirmation"].clone();
    f.call(
        "POST",
        PATH,
        &[("cookie", cookie), ("x-lvk-csrf", csrf)],
        input,
    )
    .await
}

#[tokio::test]
async fn provisioning_preview_apply_and_retry_preserve_auth_and_other_apps() {
    let f = Fixture::new(true).await;
    let (cookie, csrf) = f.setup().await;
    let remote = Auth0::new().await;
    let input = remote.input("preview");
    let receipt = preview(&f, &remote, &cookie, &csrf, input.clone()).await;
    assert!(remote.state.lock().await.writes.is_empty());
    assert!(!receipt.to_string().contains("management-secret"));
    let response = apply(&f, &cookie, &csrf, input.clone(), &receipt).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    let result = body(response).await;
    assert_ne!(result["web"]["client_id"], result["chatgpt"]["client_id"]);
    assert_eq!(result["web"]["client_secret"], "secret_0");
    assert!(f.access.load().await.unwrap().0.is_none());
    let member_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM access_members")
        .fetch_one(f.access.pool())
        .await
        .unwrap();
    assert_eq!(member_count, 0);
    {
        let state = remote.state.lock().await;
        assert_eq!(state.clients.len(), 2);
        assert_eq!(
            state.clients[0]["callbacks"],
            json!(["https://203.0.113.7:8443/api/access/callback"])
        );
        assert_eq!(
            state.clients[0]["grant_types"],
            json!(["authorization_code"])
        );
        assert_eq!(
            state.clients[1]["refresh_token"]["rotation_type"],
            "rotating"
        );
        assert_eq!(state.resources[0]["signing_alg"], "RS256");
        assert_eq!(
            state.resources[0]["skip_consent_for_verifiable_first_party_clients"],
            false
        );
        assert_eq!(state.resources[0]["scopes"].as_array().unwrap().len(), 3);
        assert_eq!(state.tenant["unrelated_setting"], true);
        assert_eq!(state.enabled["con_github"].len(), 3);
        assert_eq!(
            state.enabled["con_password"],
            BTreeSet::from(["other_app".into()])
        );
    }
    let receipt = preview(&f, &remote, &cookie, &csrf, input.clone()).await;
    assert_eq!(receipt["plan"]["web"]["id"], "client_0");
    assert_eq!(
        apply(&f, &cookie, &csrf, input, &receipt).await.status(),
        StatusCode::OK
    );
    assert_eq!(remote.state.lock().await.clients.len(), 2);
    let audits: String =
        sqlx::query_scalar("SELECT group_concat(action || outcome) FROM access_audit")
            .fetch_one(f.access.pool())
            .await
            .unwrap();
    assert!(!audits.contains("secret"));
}

#[tokio::test]
async fn provisioning_partial_failure_is_redacted_and_resumable() {
    let f = Fixture::new(true).await;
    let (cookie, csrf) = f.setup().await;
    let remote = Auth0::new().await;
    remote.state.lock().await.fail_resource_once = true;
    let input = remote.input("preview");
    let receipt = preview(&f, &remote, &cookie, &csrf, input.clone()).await;
    let response = apply(&f, &cookie, &csrf, input.clone(), &receipt).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert!(
        !body(response)
            .await
            .to_string()
            .contains("management-secret")
    );
    assert_eq!(remote.state.lock().await.clients.len(), 1);
    let receipt = preview(&f, &remote, &cookie, &csrf, input.clone()).await;
    assert_eq!(
        apply(&f, &cookie, &csrf, input, &receipt).await.status(),
        StatusCode::OK
    );
    assert_eq!(remote.state.lock().await.clients.len(), 2);
}

#[tokio::test]
async fn provisioning_requires_session_csrf_valid_issuer_and_current_preview() {
    let f = Fixture::new(true).await;
    let remote = Auth0::new().await;
    let input = remote.input("preview");
    assert_eq!(
        f.call("POST", PATH, &[], input.clone()).await.status(),
        StatusCode::UNAUTHORIZED
    );
    let (cookie, csrf) = f.setup().await;
    assert_eq!(
        f.call("POST", PATH, &[("cookie", &cookie)], input.clone())
            .await
            .status(),
        StatusCode::FORBIDDEN
    );
    let mut malicious = input.clone();
    malicious["issuer"] = json!("https://tenant.auth0.com@evil.example/");
    assert_eq!(
        f.call(
            "POST",
            PATH,
            &[("cookie", &cookie), ("x-lvk-csrf", &csrf)],
            malicious
        )
        .await
        .status(),
        StatusCode::BAD_REQUEST
    );
    let receipt = preview(&f, &remote, &cookie, &csrf, input.clone()).await;
    let mut changed = input.clone();
    changed["chatgpt_callback"] = json!("https://chatgpt.com/different");
    assert_eq!(
        apply(&f, &cookie, &csrf, changed, &receipt).await.status(),
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        apply(
            &f,
            &cookie,
            &csrf,
            input.clone(),
            &json!({"confirmation":"tampered"})
        )
        .await
        .status(),
        StatusCode::BAD_REQUEST
    );
    let (other_cookie, other_csrf) = f.setup().await;
    assert_eq!(
        apply(&f, &other_cookie, &other_csrf, input.clone(), &receipt)
            .await
            .status(),
        StatusCode::BAD_REQUEST
    );
    let valid = preview(&f, &remote, &other_cookie, &other_csrf, input.clone()).await;
    let mut expired: Value = f
        .access
        .decrypt(
            &URL_SAFE_NO_PAD
                .decode(valid["confirmation"].as_str().unwrap())
                .unwrap(),
        )
        .unwrap();
    expired["expires"] = json!(now() - 1);
    let expired =
        json!({"confirmation":URL_SAFE_NO_PAD.encode(f.access.encrypt(&expired).unwrap())});
    assert_eq!(
        apply(&f, &other_cookie, &other_csrf, input.clone(), &expired)
            .await
            .status(),
        StatusCode::BAD_REQUEST
    );
    assert!(remote.state.lock().await.writes.is_empty());
    remote.state.lock().await.redirect = true;
    let response = f
        .call(
            "POST",
            PATH,
            &[("cookie", &other_cookie), ("x-lvk-csrf", &other_csrf)],
            input,
        )
        .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn provisioning_existing_clients_and_web_only() {
    let f = Fixture::new(true).await;
    let (cookie, csrf) = f.setup().await;
    let remote = Auth0::new().await;
    remote.state.lock().await.clients.push(json!({"client_id":"existing","client_secret":"retained","name":"My Web","app_type":"regular_web","callbacks":["https://existing.example/callback"],"allowed_logout_urls":["https://existing.example/"],"client_metadata":{"other":"kept"}}));
    let mut input = remote.input("preview");
    input["mcp_enabled"] = json!(false);
    input["web_client_id"] = json!("existing");
    let receipt = preview(&f, &remote, &cookie, &csrf, input.clone()).await;
    assert_eq!(
        apply(&f, &cookie, &csrf, input, &receipt).await.status(),
        StatusCode::OK
    );
    let state = remote.state.lock().await;
    assert_eq!(state.clients.len(), 1);
    assert!(state.resources.is_empty());
    assert_eq!(state.clients[0]["callbacks"].as_array().unwrap().len(), 2);
    assert_eq!(state.clients[0]["client_metadata"]["other"], "kept");
    assert_eq!(state.clients[0]["client_secret"], "retained");
}

#[tokio::test]
async fn provisioning_rejects_non_admin_forced_mode_and_remote_auth_errors() {
    let mut f = Fixture::new(true).await;
    let (cookie, csrf) = f.setup().await;
    let remote = Auth0::new().await;
    let member = f
        .access
        .identity("https://fixture.auth0.com/", "member", None)
        .await
        .unwrap();
    sqlx::query("UPDATE access_members SET state='active'")
        .execute(f.access.pool())
        .await
        .unwrap();
    let (token, member_csrf) = f
        .access
        .create_session(Some(&member.id), "web", 0)
        .await
        .unwrap();
    let member_cookie = format!("lvk_session={token}");
    assert_eq!(
        f.call(
            "POST",
            PATH,
            &[("cookie", &member_cookie), ("x-lvk-csrf", &member_csrf)],
            remote.input("inspect")
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );
    Arc::get_mut(&mut f.access.0).unwrap().forced_mode = Some("basic".into());
    assert_eq!(
        f.call(
            "POST",
            PATH,
            &[("cookie", &cookie), ("x-lvk-csrf", &csrf)],
            remote.input("inspect")
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );
    Arc::get_mut(&mut f.access.0).unwrap().forced_mode = None;
    for status in [
        StatusCode::UNAUTHORIZED,
        StatusCode::FORBIDDEN,
        StatusCode::TOO_MANY_REQUESTS,
    ] {
        remote.state.lock().await.reject = Some(status);
        let response = f
            .call(
                "POST",
                PATH,
                &[("cookie", &cookie), ("x-lvk-csrf", &csrf)],
                remote.input("inspect"),
            )
            .await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert!(
            !body(response)
                .await
                .to_string()
                .contains("management-secret")
        );
    }
    assert!(remote.state.lock().await.writes.is_empty());
}

#[tokio::test]
async fn provisioning_paginates_and_rejects_changes_after_preview() {
    let f = Fixture::new(true).await;
    let (cookie, csrf) = f.setup().await;
    let remote = Auth0::new().await;
    for n in 0..101 {
        remote.state.lock().await.clients.push(json!({"client_id":format!("existing_{n}"),"name":format!("Web {n}"),"app_type":"regular_web","client_secret":"secret-not-in-preview","grant_types":["authorization_code"]}));
    }
    let inspect = f
        .call(
            "POST",
            PATH,
            &[("cookie", &cookie), ("x-lvk-csrf", &csrf)],
            remote.input("inspect"),
        )
        .await;
    assert_eq!(inspect.status(), StatusCode::OK);
    let inspect = body(inspect).await;
    assert_eq!(inspect["clients"].as_array().unwrap().len(), 101);
    assert!(!inspect.to_string().contains("secret-not-in-preview"));
    let mut input = remote.input("preview");
    input["web_client_id"] = json!("existing_100");
    input["mcp_enabled"] = json!(false);
    let receipt = preview(&f, &remote, &cookie, &csrf, input.clone()).await;
    assert!(!receipt.to_string().contains("secret-not-in-preview"));
    remote.state.lock().await.clients[100]["grant_types"] =
        json!(["authorization_code", "refresh_token"]);
    assert_eq!(
        apply(&f, &cookie, &csrf, input, &receipt).await.status(),
        StatusCode::BAD_REQUEST
    );
    assert!(remote.state.lock().await.writes.is_empty());
}

#[tokio::test]
async fn provisioning_accepts_long_tokens_and_rejects_oversized_input_without_writes() {
    let f = Fixture::new(true).await;
    let (cookie, csrf) = f.setup().await;
    let remote = Auth0::new().await;
    for length in [8920, 65536] {
        let token = "t".repeat(length);
        remote.state.lock().await.management_token = Some(token.clone());
        let mut input = remote.input("inspect");
        input["management_token"] = json!(token);
        let response = f
            .call(
                "POST",
                PATH,
                &[("cookie", &cookie), ("x-lvk-csrf", &csrf)],
                input,
            )
            .await;
        assert_eq!(response.status(), StatusCode::OK);
    }
    for (token, message) in [
        (String::new(), "Enter an Auth0 Management API access token"),
        ("t".repeat(65537), "65536-byte limit"),
    ] {
        let mut input = remote.input("inspect");
        input["management_token"] = json!(token);
        let response = f
            .call(
                "POST",
                PATH,
                &[("cookie", &cookie), ("x-lvk-csrf", &csrf)],
                input,
            )
            .await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert!(body(response).await.to_string().contains(message));
    }
    assert!(remote.state.lock().await.writes.is_empty());
}
