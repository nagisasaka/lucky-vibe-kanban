use std::collections::{BTreeSet, HashMap};

use axum::{
    Form,
    http::{HeaderMap, StatusCode},
};

use super::*;

fn config() -> Config {
    Config {
        public_base_url: "https://lvk.example/team".into(),
        issuer: "https://identity.example/realms/lvk".into(),
        introspection_endpoint:
            "https://identity.example/realms/lvk/protocol/openid-connect/token/introspect".into(),
        introspection_client_id: "lvk-resource".into(),
        introspection_secret_env: "LVK_INTROSPECTION_SECRET".into(),
        owner_subject: "owner-subject".into(),
        project_ids: BTreeSet::from([Uuid::from_u128(1)]),
    }
}

fn claims() -> Value {
    json!({"active":true,"token_type":"Bearer","sub":"owner-subject","iss":"https://identity.example/realms/lvk","aud":"https://lvk.example/team/mcp","exp":4_000_000_000i64,"scope":"lvk:read lvk:write"})
}

#[test]
fn rejects_unsafe_or_incomplete_operator_configuration() {
    let valid = config();
    valid.validate().unwrap();
    assert_eq!(
        valid.project_url(Uuid::from_u128(1)),
        "https://lvk.example/team/projects/00000000-0000-0000-0000-000000000001"
    );
    for url in [
        "http://lvk.example",
        "https://user:secret@lvk.example",
        "https://lvk.example/?token=secret",
        "https://lvk.example/#secret",
        "https://lvk.example/",
    ] {
        let mut invalid = valid.clone();
        invalid.public_base_url = url.into();
        assert!(invalid.validate().is_err(), "{url}");
    }
    let mut invalid = valid.clone();
    invalid.project_ids.clear();
    assert!(invalid.validate().is_err());
    let mut invalid = valid;
    invalid.owner_subject.clear();
    assert!(invalid.validate().is_err());
}

#[test]
fn verifies_active_owner_issuer_audience_expiry_and_not_before() {
    let valid = claims();
    let principal = serde_json::from_value::<auth::Introspection>(valid.clone())
        .unwrap()
        .principal(&config(), 100)
        .unwrap();
    assert_eq!(principal.subject, "owner-subject");
    assert!(principal.allows("lvk:write"));
    assert!(!principal.allows("lvk:merge"));
    for (key, value) in [
        ("active", json!(false)),
        ("token_type", json!("Refresh")),
        ("sub", json!("someone-else")),
        ("iss", json!("https://other.example")),
        ("aud", json!("other-service")),
        ("exp", json!(100)),
        ("exp", Value::Null),
        ("nbf", json!(101)),
    ] {
        let mut bad = valid.clone();
        bad[key] = value;
        assert!(
            serde_json::from_value::<auth::Introspection>(bad)
                .unwrap()
                .principal(&config(), 100)
                .is_none(),
            "{key}"
        );
    }
    let mut multiple = valid;
    multiple["aud"] = json!(["another", config().resource()]);
    assert!(
        serde_json::from_value::<auth::Introspection>(multiple)
            .unwrap()
            .principal(&config(), 100)
            .is_some()
    );
}

async fn fake_introspection(
    headers: HeaderMap,
    Form(form): Form<HashMap<String, String>>,
) -> (StatusCode, Json<Value>) {
    assert_eq!(
        headers["authorization"],
        "Basic bHZrLXJlc291cmNlOnRlc3Qtc2VjcmV0"
    );
    assert_eq!(form["token_type_hint"], "access_token");
    let mut result = claims();
    match form["token"].as_str() {
        "valid" => {}
        "expired" => result["exp"] = json!(1),
        "foreign" => result["sub"] = json!("other"),
        "no-scope" => result["scope"] = json!("unrelated"),
        "offline" => return (StatusCode::SERVICE_UNAVAILABLE, Json(Value::Null)),
        _ => result["active"] = json!(false),
    }
    (StatusCode::OK, Json(result))
}

async fn fixture() -> (Remote, Auth, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let provider = tokio::spawn(async move {
        axum::serve(
            listener,
            Router::new().route("/introspect", axum::routing::post(fake_introspection)),
        )
        .await
        .unwrap()
    });
    let mut config = config();
    config.validate().unwrap();
    // Only this test fixture bypasses HTTPS, for an isolated fake provider.
    config.introspection_endpoint = format!("http://{address}/introspect");
    config.project_ids = (1..=105).map(Uuid::from_u128).collect();
    let config = Arc::new(config);
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::query(
        "CREATE TABLE projects(id BLOB PRIMARY KEY, name TEXT NOT NULL, updated_at TEXT NOT NULL)",
    )
    .execute(&pool)
    .await
    .unwrap();
    for id in 1..=106 {
        sqlx::query("INSERT INTO projects VALUES(?,?,?)")
            .bind(Uuid::from_u128(id))
            .bind(format!("Project {id}"))
            .bind("2026-10-13T00:00:00Z")
            .execute(&pool)
            .await
            .unwrap();
    }
    (
        Remote {
            pool,
            config: config.clone(),
        },
        Auth::new(config, "test-secret".into()).unwrap(),
        provider,
    )
}

#[tokio::test]
async fn paginates_only_authorized_projects_without_modifying_data() {
    let (remote, _, provider) = fixture().await;
    let mut cursor = None;
    let mut total = 0;
    loop {
        let page = remote.projects(Page { limit: 20, cursor }).await.unwrap();
        assert!(page["items"].as_array().unwrap().len() <= 20);
        total += page["items"].as_array().unwrap().len();
        if !page["has_more"].as_bool().unwrap() {
            break;
        }
        cursor = Some(serde_json::from_value(page["next_cursor"].clone()).unwrap());
    }
    assert_eq!(total, 105);
    for limit in [0, 51, u32::MAX] {
        assert!(
            remote
                .projects(Page {
                    limit,
                    cursor: None
                })
                .await
                .is_err()
        );
    }
    assert!(
        remote
            .projects(Page {
                limit: 20,
                cursor: Some(Uuid::from_u128(106))
            })
            .await
            .is_err()
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM projects")
            .fetch_one(&remote.pool)
            .await
            .unwrap(),
        106
    );
    sqlx::query("DELETE FROM projects WHERE id=?")
        .bind(Uuid::from_u128(1))
        .execute(&remote.pool)
        .await
        .unwrap();
    assert!(
        remote
            .projects(Page {
                limit: 20,
                cursor: Some(Uuid::from_u128(1))
            })
            .await
            .is_err()
    );
    provider.abort();
    provider.await.unwrap_err();
}

#[tokio::test]
async fn http_discovery_authentication_and_tool_call_roundtrip() {
    let (remote, auth, provider) = fixture().await;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let server =
        tokio::spawn(async move { axum::serve(listener, router(remote, auth)).await.unwrap() });
    let client = reqwest::Client::new();
    let metadata: Value = client
        .get(format!("{base}/.well-known/oauth-protected-resource/mcp"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(metadata["resource"], "https://lvk.example/team/mcp");
    for (token, expected) in [
        (None, 401),
        (Some("expired"), 401),
        (Some("revoked"), 401),
        (Some("foreign"), 401),
        (Some("no-scope"), 403),
        (Some("offline"), 503),
    ] {
        let mut request = client.post(format!("{base}/mcp"));
        if let Some(token) = token {
            request = request.bearer_auth(token);
        }
        let response = request.send().await.unwrap();
        assert_eq!(response.status().as_u16(), expected);
        if expected == 401 {
            assert!(response.headers().contains_key("www-authenticate"));
        }
    }
    let requests = [
        json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-03-26","capabilities":{},"clientInfo":{"name":"test","version":"1"}}}),
        json!({"jsonrpc":"2.0","id":2,"method":"tools/list"}),
        json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"list_projects","arguments":{"limit":2}}}),
    ];
    for request in requests {
        let response = client
            .post(format!("{base}/mcp"))
            .header("Host", "lvk.example")
            .header("Accept", "application/json, text/event-stream")
            .header("MCP-Protocol-Version", "2025-03-26")
            .bearer_auth("valid")
            .json(&request)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body: Value = response.json().await.unwrap();
        assert!(body.get("error").is_none(), "{body}");
        if request["id"] == 2 {
            assert_eq!(body["result"]["tools"].as_array().unwrap().len(), 2);
        }
        if request["id"] == 3 {
            assert_eq!(
                body["result"]["structuredContent"]["items"]
                    .as_array()
                    .unwrap()
                    .len(),
                2
            );
        }
    }
    server.abort();
    server.await.unwrap_err();
    provider.abort();
    provider.await.unwrap_err();
}
