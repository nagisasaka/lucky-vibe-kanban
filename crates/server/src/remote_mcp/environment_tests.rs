//! Exercise the real external HTTP MCP router with a local mock introspection provider.
use std::{
    path::{Path, PathBuf},
    time::Duration,
};

use super::{
    environment::{Environment, Grant},
    *,
};

pub(super) struct Http {
    pub(super) base: String,
    pub(super) task: tokio::task::JoinHandle<()>,
}
impl Drop for Http {
    fn drop(&mut self) {
        self.task.abort();
    }
}
pub(super) async fn serve(remote: Remote, auth: Auth) -> Http {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let task =
        tokio::spawn(async move { axum::serve(listener, router(remote, auth)).await.unwrap() });
    Http { base, task }
}
impl Http {
    pub(super) async fn rpc(&self, token: &str, method: &str, params: Value) -> (u16, Value) {
        let response = reqwest::Client::new()
            .post(format!("{}/mcp", self.base))
            .header("Host", "lvk.example")
            .header("Accept", "application/json, text/event-stream")
            .bearer_auth(token)
            .json(&json!({"jsonrpc":"2.0","id":1,"method":method,"params":params}))
            .send()
            .await
            .unwrap();
        let status = response.status().as_u16();
        let bytes = response.bytes().await.unwrap();
        (
            status,
            if bytes.is_empty() {
                Value::Null
            } else {
                serde_json::from_slice(&bytes).unwrap()
            },
        )
    }
    pub(super) async fn call(&self, token: &str, name: &str, input: Value) -> Value {
        let (status, body) = self
            .rpc(token, "tools/call", json!({"name":name,"arguments":input}))
            .await;
        assert_eq!(status, 200, "{body}");
        assert!(body.get("error").is_none(), "{body}");
        body["result"].clone()
    }
    pub(super) async fn value(&self, name: &str, input: Value) -> Value {
        let result = self.call("runtime", name, input).await;
        assert_ne!(result["isError"], true, "{result}");
        result["structuredContent"].clone()
    }
    pub(super) async fn settle(&self, request: Uuid) -> Value {
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                let value = self
                    .value(
                        "get_operation",
                        json!({"project_id":Uuid::from_u128(1),"request_id":request}),
                    )
                    .await;
                if !matches!(value["status"].as_str(), Some("accepted" | "running")) {
                    return value;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap()
    }
}
pub(super) fn grant(root: &Path, executable: PathBuf) -> Grant {
    Grant {
        id: "example-api".into(),
        project_id: Uuid::from_u128(1),
        root: root.into(),
        subjects: vec!["owner-subject".into()],
        executables: vec![executable],
        allow_uncoordinated_execution: true,
    }
}

#[tokio::test]
async fn environment_http_catalog_and_authorization_are_explicit() {
    let (mut remote, auth, provider) = tests::fixture().await;
    let dir = tempfile::tempdir().unwrap();
    let mut region = grant(dir.path(), PathBuf::from("/bin/echo"));
    region.subjects = vec!["someone-else".into()];
    remote.environment = Some(Arc::new(Environment::new(vec![region]).unwrap()));
    let http = serve(remote.clone(), auth.clone()).await;
    let (status, catalog) = http.rpc("runtime", "tools/list", json!({})).await;
    assert_eq!(status, 200);
    let tools = catalog["result"]["tools"].as_array().unwrap();
    for name in [
        "get_runtime_access",
        "list_runtime_files",
        "read_runtime_file",
        "list_processes",
        "inspect_process",
        "run_process",
        "read_process_output",
        "stop_process",
    ] {
        let tool = tools.iter().find(|t| t["name"] == name).unwrap();
        let mutation = matches!(name, "run_process" | "stop_process");
        assert_eq!(tool["annotations"]["readOnlyHint"], !mutation);
        assert_eq!(tool["annotations"]["destructiveHint"], mutation);
        assert_eq!(tool["inputSchema"]["additionalProperties"], false);
        assert!(
            tool["_meta"]["securitySchemes"][0]["scopes"]
                .as_array()
                .unwrap()
                .contains(&json!("lvk:runtime:read"))
        );
        if mutation {
            assert!(tool["inputSchema"]["properties"]["request_id"].is_object());
        }
    }
    for (token, name, status) in [
        ("revoked", "get_runtime_access", 401),
        ("read-only", "get_runtime_access", 403),
        ("runtime-read", "run_process", 403),
        ("runtime-read", "stop_process", 403),
    ] {
        assert_eq!(
            http.rpc(token, "tools/call", json!({"name":name,"arguments":{}}))
                .await
                .0,
            status
        );
    }
    assert_eq!(
        http.value("get_runtime_access", json!({})).await["regions"],
        json!([])
    );
    assert_eq!(
        http.call(
            "runtime",
            "read_runtime_file",
            json!({"region":"example-api","path":"log"})
        )
        .await["isError"],
        true
    );
    assert_eq!(
        http.call(
            "runtime",
            "read_runtime_file",
            json!({"region":"missing","path":"log"})
        )
        .await["isError"],
        true
    );
    remote.environment = None;
    let disabled = serve(remote, auth).await;
    assert_eq!(
        disabled
            .call("runtime", "get_runtime_access", json!({}))
            .await["isError"],
        true
    );
    provider.abort();
}

#[tokio::test]
async fn environment_http_api_lifecycle_receipts_and_restart() {
    let (mut remote, auth, provider) = tests::fixture().await;
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::migrate!("../db/migrations").run(&pool).await.unwrap();
    sqlx::query("INSERT INTO projects(id,name) VALUES(?,'Example API')")
        .bind(Uuid::from_u128(1))
        .execute(&pool)
        .await
        .unwrap();
    remote.pool = pool;
    let dir = tempfile::tempdir().unwrap();
    let python = Path::new("/usr/bin/python3")
        .canonicalize()
        .expect("Python 3 fixture prerequisite");
    std::fs::write(
        dir.path().join("application.log"),
        "untracked application log\n",
    )
    .unwrap();
    std::fs::write(
        dir.path().join("api.py"),
        r#"from http.server import HTTPServer, BaseHTTPRequestHandler
class Handler(BaseHTTPRequestHandler):
    def do_GET(self):
        self.send_response(200)
        self.end_headers()
        self.wfile.write(b'example-api')
server = HTTPServer(('127.0.0.1', 0), Handler)
print(server.server_port, flush=True)
server.serve_forever()
"#,
    )
    .unwrap();
    let region = grant(dir.path(), python.clone());
    let mut second_region = region.clone();
    second_region.id = "same-directory-other-grant".into();
    remote.environment = Some(Arc::new(
        Environment::new(vec![region.clone(), second_region.clone()]).unwrap(),
    ));
    let http = serve(remote.clone(), auth.clone()).await;
    let read = http
        .value(
            "read_runtime_file",
            json!({"region":"example-api","path":"application.log"}),
        )
        .await;
    assert!(
        read.to_string().contains("untracked application log"),
        "{read}"
    );
    for bad in [
        json!({"region":"example-api","path":"../escape"}),
        json!({"region":"example-api","path":"application.log","unexpected":true}),
    ] {
        assert_eq!(
            http.call("runtime", "read_runtime_file", bad).await["isError"],
            true
        );
    }
    let request = Uuid::new_v4();
    let input = json!({"region":"example-api","request_id":request,"purpose":"Test isolated example API","executable":python,"args":["api.py"],"cwd":".","service":true,"lifetime_seconds":30});
    http.value("run_process", input.clone()).await;
    let receipt = http.settle(request).await;
    assert_eq!(receipt["status"], "succeeded", "{receipt}");
    let run = receipt["result"].clone();
    let id = run["run_id"].as_str().unwrap();
    let identity = run["identity"].as_str().unwrap();
    let _cleanup = super::environment_process::TestProcessCleanup::new(identity).unwrap();
    assert_eq!(
        http.call(
            "runtime",
            "read_process_output",
            json!({"region":"same-directory-other-grant","process_id":id,"stream":"stdout"})
        )
        .await["isError"],
        true
    );
    let cross_stop = Uuid::new_v4();
    http.value("stop_process", json!({"region":"same-directory-other-grant","request_id":cross_stop,"purpose":"Verify grant isolation","identity":id})).await;
    assert_eq!(http.settle(cross_stop).await["status"], "failed");
    let repeat = http.value("run_process", input.clone()).await;
    assert_eq!(repeat["operation_id"], receipt["operation_id"]);
    assert_eq!(repeat["result"]["run_id"], id);
    let mut different = input;
    different["args"] = json!(["different.py"]);
    assert_eq!(
        http.call("runtime", "run_process", different).await["isError"],
        true
    );
    let port = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let output = http
                .value(
                    "read_process_output",
                    json!({"region":"example-api","process_id":id,"stream":"stdout"}),
                )
                .await;
            if let Some(port) = output["output"]["text"]
                .as_str()
                .and_then(|t| t.trim().parse::<u16>().ok())
            {
                break port;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    assert_eq!(
        reqwest::get(format!("http://127.0.0.1:{port}/"))
            .await
            .unwrap()
            .text()
            .await
            .unwrap(),
        "example-api"
    );
    assert_eq!(
        http.value(
            "inspect_process",
            json!({"region":"example-api","identity":identity})
        )
        .await["identity"],
        identity
    );
    let processes = http
        .value("list_processes", json!({"region":"example-api","limit":1}))
        .await;
    assert!(
        processes["processes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|p| p["identity"] == identity)
    );
    let stop = Uuid::new_v4();
    http.value("stop_process",json!({"region":"example-api","request_id":stop,"purpose":"End isolated test","identity":id})).await;
    assert_eq!(http.settle(stop).await["status"], "succeeded");
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let output = http
                .value(
                    "read_process_output",
                    json!({"region":"example-api","process_id":id,"stream":"stderr"}),
                )
                .await;
            if output["state"] == "exited" {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM access_audit WHERE action='runtime_process_exit' AND target=?")
                .bind(receipt["operation_id"].as_str().unwrap()).fetch_one(&remote.pool).await.unwrap();
            if count == 1 { break; }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }).await.unwrap();
    // New MCP server instance retains durable receipt, but does not invent volatile output.
    drop(http);
    remote.process_instance = Uuid::new_v4();
    remote.environment = Some(Arc::new(
        Environment::new(vec![region, second_region]).unwrap(),
    ));
    let restored = serve(remote, auth).await;
    let persisted = restored.settle(request).await;
    assert_eq!(persisted["operation_id"], receipt["operation_id"]);
    assert_eq!(persisted["result"]["run_id"], id);
    let restored_cross_stop = Uuid::new_v4();
    restored.value("stop_process", json!({"region":"same-directory-other-grant","request_id":restored_cross_stop,"purpose":"Verify durable grant isolation","identity":identity})).await;
    let denied = restored.settle(restored_cross_stop).await;
    assert_eq!(denied["status"], "failed");
    assert_eq!(
        denied["result"]["message"],
        "Managed process belongs to another runtime grant or owner"
    );
    assert_eq!(
        restored
            .call(
                "runtime",
                "read_process_output",
                json!({"region":"example-api","process_id":id,"stream":"stdout"})
            )
            .await["isError"],
        true
    );
    provider.abort();
}

#[test]
fn environment_configuration_refuses_root_and_duplicate_grants() {
    let directory = tempfile::tempdir().unwrap();
    let region = grant(directory.path(), PathBuf::from("/bin/echo"));
    assert!(Environment::new(vec![region.clone(), region.clone()]).is_err());
    let mut broad = region;
    broad.root = PathBuf::from("/");
    assert!(Environment::new(vec![broad]).is_err());
}

#[tokio::test]
async fn environment_http_launch_boundaries_and_failures() {
    let (mut remote, auth, provider) = tests::fixture().await;
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::migrate!("../db/migrations").run(&pool).await.unwrap();
    sqlx::query("INSERT INTO projects(id,name) VALUES(?,'Launch boundaries')")
        .bind(Uuid::from_u128(1))
        .execute(&pool)
        .await
        .unwrap();
    remote.pool = pool.clone();
    let directory = tempfile::tempdir().unwrap();
    let echo = Path::new("/bin/echo").canonicalize().unwrap();
    let invalid_program = directory.path().join("not-executable");
    std::fs::write(&invalid_program, "ordinary file, not executable").unwrap();
    let mut region = grant(directory.path(), echo.clone());
    region.executables.push(invalid_program.clone());
    remote.environment = Some(Arc::new(Environment::new(vec![region]).unwrap()));
    let http = serve(remote, auth).await;
    let input = |request: Uuid, executable: &Path, cwd: &str| json!({"region":"example-api","request_id":request,"purpose":"Verify launch boundary","executable":executable,"args":[],"cwd":cwd,"lifetime_seconds":10});
    assert_eq!(
        http.call(
            "runtime",
            "run_process",
            input(Uuid::new_v4(), Path::new("/bin/sleep"), ".")
        )
        .await["isError"],
        true
    );
    for cwd in ["../", "/", "missing-directory"] {
        let request = Uuid::new_v4();
        http.value("run_process", input(request, &echo, cwd)).await;
        let result = http.settle(request).await;
        assert_eq!(result["status"], "failed", "{result}");
        assert!(result["result"]["run_id"].is_null());
    }
    let request = Uuid::new_v4();
    http.value("run_process", input(request, &invalid_program, "."))
        .await;
    let failure = http.settle(request).await;
    assert_eq!(failure["status"], "failed", "{failure}");
    let repeated = http
        .value("run_process", input(request, &invalid_program, "."))
        .await;
    assert_eq!(repeated["operation_id"], failure["operation_id"]);
    assert_eq!(repeated["status"], "failed");
    // A workspace at this root prevents even an otherwise allowed executable.
    sqlx::query("INSERT INTO workspaces(id,branch,container_ref) VALUES(?,'test',?)")
        .bind(Uuid::new_v4())
        .bind(directory.path().to_str().unwrap())
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        http.call("runtime", "run_process", input(Uuid::new_v4(), &echo, "."))
            .await["isError"],
        true
    );
    // No AgentRun exists, and archival does not change this current restriction.
    sqlx::query("UPDATE workspaces SET archived=1")
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        http.call("runtime", "run_process", input(Uuid::new_v4(), &echo, "."))
            .await["isError"],
        true
    );
    assert_eq!(http.call("runtime", "stop_process", json!({"region":"example-api","request_id":Uuid::new_v4(),"identity":"unselected","purpose":"Verify overlap admission"})).await["isError"], true);
    let read = http
        .value(
            "read_runtime_file",
            json!({"region":"example-api","path":"not-executable"}),
        )
        .await;
    assert!(read.to_string().contains("ordinary file, not executable"));
    provider.abort();
}
