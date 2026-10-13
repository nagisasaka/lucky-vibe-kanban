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
            assert_eq!(
                body["result"]["tools"].as_array().unwrap().len(),
                tools().len()
            );
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

#[tokio::test]
async fn board_reads_use_real_schema_bound_pages_and_reject_mixed_targets() {
    use db::models::workspace::{CreateWorkspace, Workspace};
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::migrate!("../db/migrations").run(&pool).await.unwrap();
    let project = Uuid::from_u128(1);
    let foreign_project = Uuid::from_u128(2);
    for id in [project, foreign_project] {
        sqlx::query("INSERT INTO projects(id,name) VALUES(?,'Test project')")
            .bind(id)
            .execute(&pool)
            .await
            .unwrap();
    }
    let status = Uuid::new_v4();
    sqlx::query("INSERT INTO local_project_statuses(id,project_id,name,color,sort_order) VALUES(?,?,'Todo','blue',1)").bind(status).bind(project).execute(&pool).await.unwrap();
    for number in 1..=105u128 {
        sqlx::query("INSERT INTO local_issues(id,project_id,issue_number,simple_id,status_id,title,description,sort_order) VALUES(?,?,?,?,?,'Issue title',?,0)")
            .bind(Uuid::from_u128(1000+number)).bind(project).bind(number as i64).bind(format!("TEST-{number}")).bind(status).bind("仕様😀".repeat(6000)).execute(&pool).await.unwrap();
    }
    let issue = Uuid::from_u128(1001);
    let mut workspaces = Vec::new();
    for index in 0..2 {
        let id = Uuid::new_v4();
        Workspace::create(
            &pool,
            &CreateWorkspace {
                branch: format!("test/{index}"),
                name: Some(format!("Attempt {index}")),
            },
            id,
        )
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO local_workspace_links(workspace_id,project_id,issue_id) VALUES(?,?,?)",
        )
        .bind(id)
        .bind(project)
        .bind(issue)
        .execute(&pool)
        .await
        .unwrap();
        workspaces.push(id);
    }
    let remote = Remote {
        pool: pool.clone(),
        config: Arc::new(config()),
    };
    let totals = read::call(&remote, "get_project", json!({"project_id":project}))
        .await
        .unwrap();
    assert_eq!(totals["issue_count"], 105);
    let mut cursor = None;
    let mut count = 0;
    loop {
        let mut args = json!({"project_id":project,"limit":20});
        if let Some(cursor) = cursor {
            args["cursor"] = cursor;
        }
        let result = read::call(&remote, "list_issues", args).await.unwrap();
        let items = result["items"].as_array().unwrap();
        assert!(items.len() <= 20);
        assert!(items.iter().all(|item| item.get("description").is_none()));
        count += items.len();
        if result["has_more"] == false {
            break;
        }
        cursor = Some(result["next_cursor"].clone());
    }
    assert_eq!(count, 105);
    let first = read::call(
        &remote,
        "get_issue",
        json!({"project_id":project,"issue_id":issue,"limit":7}),
    )
    .await
    .unwrap();
    assert_eq!(
        first["description_page"].as_str().unwrap().chars().count(),
        7
    );
    assert_eq!(first["next_offset"], 7);
    assert_eq!(first["total_characters"], 18000);
    assert!(
        read::call(
            &remote,
            "get_issue",
            json!({"project_id":project,"issue_id":issue,"offset":7})
        )
        .await
        .is_err()
    );
    let next = json!({"project_id":project,"issue_id":issue,"offset":7,"expected_revision":first["revision"]});
    assert!(read::call(&remote, "get_issue", next.clone()).await.is_ok());
    sqlx::query("UPDATE local_issues SET description='direct UI change',updated_at='2099-01-01T00:00:00Z' WHERE id=?").bind(issue).execute(&pool).await.unwrap();
    assert!(read::call(&remote, "get_issue", next).await.is_err());
    let current = read::call(
        &remote,
        "get_issue",
        json!({"project_id":project,"issue_id":issue}),
    )
    .await
    .unwrap();
    assert_eq!(current["description_page"], "direct UI change");
    let attempts = read::call(
        &remote,
        "list_issue_workspaces",
        json!({"project_id":project,"issue_id":issue}),
    )
    .await
    .unwrap();
    assert_eq!(attempts["items"].as_array().unwrap().len(), 2);
    assert!(
        attempts["items"][0]["url"]
            .as_str()
            .unwrap()
            .starts_with("https://lvk.example/team/projects/")
    );
    assert!(
        read::call(
            &remote,
            "list_issue_workspaces",
            json!({"project_id":project,"issue_id":Uuid::from_u128(1002),"cursor":workspaces[0]})
        )
        .await
        .is_err()
    );
    assert!(
        read::call(
            &remote,
            "get_issue",
            json!({"project_id":foreign_project,"issue_id":issue})
        )
        .await
        .is_err()
    );
    assert!(
        read::call(
            &remote,
            "get_issue",
            json!({"project_id":project,"issue_id":issue,"limit":16001})
        )
        .await
        .is_err()
    );
    for table in [
        "agent_runs",
        "project_chat_messages",
        "remote_mcp_operations",
    ] {
        assert_eq!(
            sqlx::query_scalar::<_, i64>(&format!("SELECT count(*) FROM {table}"))
                .fetch_one(&pool)
                .await
                .unwrap(),
            0
        );
    }
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM workspaces")
            .fetch_one(&pool)
            .await
            .unwrap(),
        2
    );
    let session = Uuid::new_v4();
    db::models::session::Session::create(
        &pool,
        &db::models::session::CreateSession {
            executor: Some("CODEX".into()),
            name: None,
        },
        session,
        workspaces[0],
    )
    .await
    .unwrap();
    let run = Uuid::new_v4();
    let request = json!({"input":{"content":"Direct instruction from LVK"},"workspace":{"path":"/private/never-return"}});
    sqlx::query("INSERT INTO agent_runs(id,session_id,workspace_id,request_id,idempotency_key,correlation_id,schema_version,payload_version,runtime_profile_id,provider_id,workspace_mode,workspace_path,status,request_envelope) VALUES(?,?,?,?,?,?,1,1,'CODEX:default','codex','shared_workspace','/private/never-return','running',?)")
        .bind(run).bind(session).bind(workspaces[0]).bind(Uuid::new_v4()).bind(run.to_string()).bind(Uuid::new_v4()).bind(request.to_string()).execute(&pool).await.unwrap();
    let state = json!({"terminal_output":{"content":"reported result ".repeat(2000)},"goal":{"status":"blocked","objective":"Agreed goal"},"provider_session":{"private":"never-return"}});
    sqlx::query("INSERT INTO agent_run_state(agent_run_id,state_schema_version,reducer_version,state_json) VALUES(?,1,1,?)").bind(run).bind(state.to_string()).execute(&pool).await.unwrap();
    let runs = runtime_read::call(
        &remote,
        "list_workspace_runs",
        json!({"project_id":project,"workspace_id":workspaces[0]}),
    )
    .await
    .unwrap();
    assert_eq!(runs["items"][0]["id"], json!(run));
    assert_eq!(runs["items"][0]["goal_status"], "blocked");
    let input_args = json!({"project_id":project,"workspace_id":workspaces[0],"agent_run_id":run,"section":"input"});
    let input = runtime_read::call(&remote, "get_execution", input_args)
        .await
        .unwrap();
    assert_eq!(input["content"], "Direct instruction from LVK");
    assert!(!input.to_string().contains("never-return"));
    let output_args = json!({"project_id":project,"workspace_id":workspaces[0],"agent_run_id":run,"section":"final_response","limit":100});
    let output = runtime_read::call(&remote, "get_execution", output_args.clone())
        .await
        .unwrap();
    assert_eq!(output["content"].as_str().unwrap().chars().count(), 100);
    assert_eq!(output["has_more"], true);
    assert_eq!(output["verification"]["status"], "unknown");
    assert_eq!(output["status"], "running");
    let mut other = output_args.clone();
    other["workspace_id"] = json!(workspaces[1]);
    assert!(
        runtime_read::call(&remote, "get_execution", other)
            .await
            .is_err()
    );
    sqlx::query(
        "UPDATE agent_runs SET status='failed',updated_at='2099-01-01T00:00:00Z' WHERE id=?",
    )
    .bind(run)
    .execute(&pool)
    .await
    .unwrap();
    let mut stale = output_args.clone();
    stale["offset"] = json!(100);
    stale["expected_revision"] = output["revision"].clone();
    assert!(
        runtime_read::call(&remote, "get_execution", stale)
            .await
            .is_err()
    );
    let latest = runtime_read::call(&remote, "get_execution", output_args)
        .await
        .unwrap();
    assert_eq!(latest["status"], "failed");
    assert_eq!(latest["verification"]["status"], "unknown");
}

#[tokio::test]
async fn question_reads_preserve_options_distinguish_approvals_and_observe_direct_answers() {
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    sqlx::raw_sql("CREATE TABLE projects(id BLOB); CREATE TABLE local_issues(id BLOB,project_id BLOB); CREATE TABLE workspaces(id BLOB); CREATE TABLE local_workspace_links(workspace_id BLOB,project_id BLOB,issue_id BLOB); CREATE TABLE sessions(id BLOB,workspace_id BLOB);
      CREATE TABLE agent_runs(id BLOB,session_id BLOB,workspace_id BLOB,status TEXT,request_envelope TEXT,created_at TEXT);
      CREATE TABLE agent_run_attempts(id BLOB,agent_run_id BLOB,attempt_number INTEGER,status TEXT);
      CREATE TABLE agent_events(run_attempt_id BLOB,agent_run_id BLOB,session_id BLOB,event_envelope TEXT,created_at TEXT,run_attempt_number INTEGER,sequence INTEGER);
      CREATE TABLE agent_run_state(agent_run_id BLOB,last_run_attempt_number INTEGER);
      CREATE TABLE agent_run_commands(command_id BLOB,agent_run_id BLOB,delivery_status TEXT,created_at TEXT,command_envelope TEXT);
      CREATE TABLE agent_question_answers(question_id TEXT PRIMARY KEY,session_id BLOB,request_id BLOB,result TEXT);").execute(&pool).await.unwrap();
    let project = Uuid::from_u128(1);
    let issue = Uuid::new_v4();
    let workspace = Uuid::new_v4();
    let session = Uuid::new_v4();
    let run = Uuid::new_v4();
    let attempt = Uuid::new_v4();
    sqlx::query("INSERT INTO projects VALUES(?)")
        .bind(project)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO local_issues VALUES(?,?)")
        .bind(issue)
        .bind(project)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO workspaces VALUES(?)")
        .bind(workspace)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO local_workspace_links VALUES(?,?,?)")
        .bind(workspace)
        .bind(project)
        .bind(issue)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO sessions VALUES(?,?)")
        .bind(session)
        .bind(workspace)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO agent_runs VALUES(?,?,?,'running','{}','2026-10-13')")
        .bind(run)
        .bind(session)
        .bind(workspace)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO agent_run_attempts VALUES(?,?,1,'running')")
        .bind(attempt)
        .bind(run)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO agent_run_state VALUES(?,1)")
        .bind(run)
        .execute(&pool)
        .await
        .unwrap();
    let payloads = [
        json!({"type":"provider_extension","data":{"provider_namespace":"codex","provider_event":"async_questions","payload":{"schema_version":1,"message_id":"msg","questions":[{"title":"Which colour?","options":["Blue","自由回答"]}]}}}),
        json!({"type":"input_requested","data":{"input_id":"blocking-id","prompt":"A or B?"}}),
        json!({"type":"approval_requested","data":{"approval_id":"approval-id","action":"command","details":{"command":"private-command-do-not-export"}}}),
    ];
    for (index, payload) in payloads.into_iter().enumerate() {
        let envelope = json!({"session_id":session,"agent_run_id":run,"payload":payload});
        sqlx::query("INSERT INTO agent_events VALUES(?,?,?,?,'2026-10-13',1,?)")
            .bind(attempt)
            .bind(run)
            .bind(session)
            .bind(envelope.to_string())
            .bind(index as i64 + 1)
            .execute(&pool)
            .await
            .unwrap();
    }
    let remote = Remote {
        pool: pool.clone(),
        config: Arc::new(config()),
    };
    let args = json!({"project_id":project,"workspace_id":workspace,"session_id":session});
    let listed = questions::call(&remote, "list_questions", args.clone())
        .await
        .unwrap();
    assert_eq!(listed["items"].as_array().unwrap().len(), 3);
    let mut specification = None;
    for item in listed["items"].as_array().unwrap() {
        let mut detail = args.clone();
        for key in ["question_id", "kind", "agent_run_id"] {
            detail[key] = item[key].clone();
        }
        let response = questions::call(&remote, "get_question", detail.clone())
            .await
            .unwrap();
        let content: Value = serde_json::from_str(response["content"].as_str().unwrap()).unwrap();
        if item["kind"] == "async_specification" {
            assert_eq!(content["options"], json!(["Blue", "自由回答"]));
            assert_eq!(content["free_text_allowed"], true);
            specification = Some(detail);
        }
        assert_eq!(response["requires_lvk"], item["kind"] == "approval");
        assert!(
            !response
                .to_string()
                .contains("private-command-do-not-export")
        );
        assert_eq!(item["issue_id"], json!(issue));
    }
    let specification = specification.unwrap();
    sqlx::query("INSERT INTO agent_question_answers VALUES(?,?,?,'{\"accepted\":true}')")
        .bind(specification["question_id"].as_str().unwrap())
        .bind(session)
        .bind(Uuid::new_v4())
        .execute(&pool)
        .await
        .unwrap();
    assert!(
        questions::call(&remote, "get_question", specification)
            .await
            .is_err()
    );
    let fresh = questions::call(&remote, "list_questions", args.clone())
        .await
        .unwrap();
    assert_eq!(fresh["items"].as_array().unwrap().len(), 2);
    let mut foreign = args;
    foreign["session_id"] = json!(Uuid::new_v4());
    assert!(
        questions::call(&remote, "list_questions", foreign)
            .await
            .is_err()
    );
}
