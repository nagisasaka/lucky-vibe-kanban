//! Exercise the real external HTTP MCP router with a local mock introspection provider.
use std::{
    path::{Path, PathBuf},
    time::Duration,
};

use super::{
    environment::{Environment, RuntimeAccessSettings},
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
        self.settle_project(Uuid::from_u128(1), request).await
    }
    pub(super) async fn settle_project(&self, project: Uuid, request: Uuid) -> Value {
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                let value = self
                    .value(
                        "get_operation",
                        json!({"project_id":project,"request_id":request}),
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
pub(super) fn settings(root: &Path) -> RuntimeAccessSettings {
    RuntimeAccessSettings {
        file_roots: vec![root.into()],
        ..Default::default()
    }
}
pub(super) async fn runtime_fixture() -> (Remote, Auth, tokio::task::JoinHandle<()>) {
    let (mut remote, auth, provider) = tests::fixture().await;
    remote.pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::migrate!("../db/migrations")
        .run(&remote.pool)
        .await
        .unwrap();
    for id in [1, 2] {
        sqlx::query("INSERT INTO projects(id,name,key) VALUES(?,'Runtime fixture',?)")
            .bind(Uuid::from_u128(id))
            .bind(format!("TEST{id}"))
            .execute(&remote.pool)
            .await
            .unwrap();
    }
    (remote, auth, provider)
}
fn project(mut input: Value) -> Value {
    input["project_id"] = json!(Uuid::from_u128(1));
    input
}
async fn change(http: &Http, action: &str, path: &Path, extra: Value) -> Value {
    let id = Uuid::new_v4();
    let mut a = project(
        json!({"action":action,"path":path,"request_id":id,"purpose":"isolated file operation"}),
    );
    a.as_object_mut()
        .unwrap()
        .extend(extra.as_object().unwrap().clone());
    http.value("change_runtime_path", a.clone()).await;
    let result = http.settle(id).await;
    let repeated = http.value("change_runtime_path", a).await;
    assert_eq!(repeated["operation_id"], result["operation_id"]);
    result
}

#[tokio::test]
async fn environment_http_catalog_scopes_projects_and_toggle_matrix() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("log"), "hello").unwrap();
    for files_enabled in [false, true] {
        for execution_enabled in [false, true] {
            let (mut remote, auth, provider) = runtime_fixture().await;
            let s = RuntimeAccessSettings {
                files_enabled,
                execution_enabled,
                ..settings(dir.path())
            };
            remote.environment = Some(Arc::new(Environment::new(s).unwrap()));
            let http = serve(remote, auth).await;
            let (_, catalog) = http.rpc("runtime", "tools/list", json!({})).await;
            let tools = catalog["result"]["tools"].as_array().unwrap();
            for obsolete in [
                "read_runtime_file",
                "list_runtime_files",
                "list_processes",
                "inspect_process",
                "read_process_output",
            ] {
                assert!(!tools.iter().any(|t| t["name"] == obsolete));
            }
            let read = tools
                .iter()
                .find(|t| t["name"] == "read_runtime_path")
                .unwrap();
            assert_eq!(
                read["inputSchema"]["properties"]["max_bytes"]["maximum"],
                524288
            );
            assert_eq!(
                read["inputSchema"]["properties"]["max_bytes"]["default"],
                16384
            );
            assert_eq!(
                http.value("get_runtime_access", json!({})).await["settings"]["files_enabled"],
                files_enabled
            );
            for (token, name, expected) in [
                ("revoked", "get_runtime_access", 401),
                ("read-only", "get_runtime_access", 403),
                ("runtime-read", "run_process", 403),
                ("runtime-read", "change_runtime_path", 403),
                ("valid", "change_runtime_path", 403),
                ("runtime-files", "run_process", 403),
            ] {
                assert_eq!(
                    http.rpc(token, "tools/call", json!({"name":name,"arguments":{}}))
                        .await
                        .0,
                    expected
                );
            }
            let read = project(json!({"action":"read","path":dir.path().join("log")}));
            assert_eq!(
                http.call("runtime", "read_runtime_path", read.clone())
                    .await["isError"]
                    == true,
                !files_enabled
            );
            let mut denied = read;
            denied["project_id"] = json!(Uuid::from_u128(106));
            assert_eq!(
                http.call("runtime", "read_runtime_path", denied).await["isError"],
                true
            );
            if files_enabled {
                http.value(
                    "read_runtime_path",
                    project(json!({"action":"list","path":dir.path()})),
                )
                .await;
                let path = dir.path().join("written");
                let file_scope = project(
                    json!({"request_id":Uuid::new_v4(),"purpose":"write scope without execute scope","action":"write","path":path,"content":"scope"}),
                );
                assert_ne!(
                    http.call("runtime-files", "change_runtime_path", file_scope)
                        .await["isError"],
                    true
                );
                assert_eq!(
                    change(&http, "write", &path, json!({"content":"new"})).await["result"]["status"],
                    "written"
                );
                assert_eq!(
                    change(&http, "delete", &path, json!({})).await["result"]["status"],
                    "deleted"
                );
            } else {
                assert_eq!(http.call("runtime","change_runtime_path",project(json!({"request_id":Uuid::new_v4(),"purpose":"denied","action":"write","path":dir.path().join("denied"),"content":"x"}))).await["isError"],true);
            }
            let req = Uuid::new_v4();
            let launch = project(
                json!({"request_id":req,"purpose":"matrix","executable":"/bin/echo","args":["hello"],"cwd":dir.path(),"lifetime_seconds":5}),
            );
            let result = http.call("runtime", "run_process", launch).await;
            if execution_enabled {
                assert_ne!(result["isError"], true, "{result}");
                assert_eq!(http.settle(req).await["status"], "succeeded");
            } else {
                assert_eq!(result["isError"], true);
            }
            assert_eq!(
                http.call(
                    "runtime",
                    "observe_process",
                    project(json!({"action":"list"}))
                )
                .await["isError"]
                    == true,
                !execution_enabled
            );
            provider.abort();
        }
    }
}

#[tokio::test]
async fn environment_http_workspace_paths_file_changes_and_launch() {
    let (mut remote, auth, provider) = runtime_fixture().await;
    let dir = tempfile::tempdir().unwrap();
    let workspace = dir.path().join("workspace");
    let sibling = dir.path().join("sibling");
    std::fs::create_dir(&workspace).unwrap();
    std::fs::create_dir(&sibling).unwrap();
    std::fs::write(workspace.join("source"), "source").unwrap();
    sqlx::query("INSERT INTO workspaces(id,branch,container_ref) VALUES(?,'fixture',?)")
        .bind(Uuid::new_v4())
        .bind(workspace.to_str().unwrap())
        .execute(&remote.pool)
        .await
        .unwrap();
    remote.environment = Some(Arc::new(Environment::new(settings(dir.path())).unwrap()));
    let http = serve(remote, auth).await;
    let content = http
        .value(
            "read_runtime_path",
            project(json!({"action":"read","path":workspace.join("source")})),
        )
        .await;
    assert_eq!(content["content"], "source");
    for (action, path, extra) in [
        ("write", workspace.join("source"), json!({"content":"bad"})),
        ("write", workspace.join("new"), json!({"content":"bad"})),
        ("delete", workspace.clone(), json!({"recursive":true})),
        ("delete", dir.path().into(), json!({"recursive":true})),
    ] {
        assert_eq!(
            change(&http, action, &path, extra).await["status"],
            "failed"
        );
    }
    let file = sibling.join("data");
    assert_eq!(
        change(&http, "write", &file, json!({"content":"original"})).await["status"],
        "succeeded"
    );
    let snapshot = http
        .value(
            "read_runtime_path",
            project(json!({"action":"read","path":file})),
        )
        .await["snapshot"]
        .clone();
    std::fs::write(&file, "changed").unwrap();
    assert_eq!(
        change(
            &http,
            "write",
            &file,
            json!({"content":"stale","expected":snapshot})
        )
        .await["status"],
        "failed"
    );
    assert_eq!(
        change(&http, "delete", &sibling.join("missing"), json!({})).await["result"]["status"],
        "not_found"
    );
    std::os::unix::fs::symlink(&workspace, sibling.join("alias")).unwrap();
    assert_eq!(
        http.call(
            "runtime",
            "read_runtime_path",
            project(json!({"action":"read","path":sibling.join("alias/source")}))
        )
        .await["isError"],
        true
    );
    assert_eq!(
        change(
            &http,
            "write",
            &sibling.join("alias/source"),
            json!({"content":"bad"})
        )
        .await["status"],
        "failed"
    );
    for path in [
        PathBuf::from("relative"),
        dir.path().join("../outside"),
        PathBuf::from("/outside-file-roots"),
    ] {
        assert_eq!(
            http.call(
                "runtime",
                "read_runtime_path",
                project(json!({"action":"read","path":path}))
            )
            .await["isError"],
            true
        );
    }
    for (cwd, success) in [
        (sibling.as_path(), true),
        (dir.path(), true),
        (workspace.as_path(), false),
        (sibling.join("alias").as_path(), false),
    ] {
        let id = Uuid::new_v4();
        http.value("run_process",project(json!({"request_id":id,"purpose":"Workspace boundary","executable":"/bin/echo","cwd":cwd,"lifetime_seconds":5}))).await;
        assert_eq!(http.settle(id).await["status"] == "succeeded", success);
    }
    let request = Uuid::new_v4();
    // Execution file writes are allowed, including inline source. File-root controls do not sandbox them.
    http.value("run_process",project(json!({"request_id":request,"purpose":"permitted command side effect","executable":"/bin/sh","args":["-c","printf command > command-file"],"cwd":&sibling,"lifetime_seconds":5}))).await;
    assert_eq!(http.settle(request).await["status"], "succeeded");
    tokio::time::timeout(Duration::from_secs(3), async {
        while !sibling.join("command-file").exists() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    let moving = Uuid::new_v4();
    http.value("run_process",project(json!({"request_id":moving,"purpose":"post-launch cwd is not monitored","executable":"/bin/sh","args":["-c","cd \"$1\"; exec /bin/sleep 30","fixture",workspace.to_str().unwrap()],"cwd":sibling,"lifetime_seconds":35}))).await;
    let moving = http.settle(moving).await["result"].clone();
    let identity = moving["identity"].as_str().unwrap();
    let _cleanup = environment_process::TestProcessCleanup::new(identity).unwrap();
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let observed = http
                .value(
                    "observe_process",
                    project(json!({"action":"inspect","identity":identity})),
                )
                .await;
            if observed["cwd"] == json!(workspace) {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    let stopping = Uuid::new_v4();
    http.value(
        "stop_process",
        project(json!({"request_id":stopping,"purpose":"stop after chdir","identity":identity})),
    )
    .await;
    assert_eq!(http.settle(stopping).await["status"], "succeeded");
    assert_eq!(
        std::fs::read_to_string(workspace.join("source")).unwrap(),
        "source"
    );
    assert_eq!(
        change(&http, "delete", &sibling, json!({"recursive":true})).await["result"]["status"],
        "deleted"
    );
    assert!(workspace.exists());
    provider.abort();
}

#[tokio::test]
async fn environment_http_large_file_and_process_output_limits() {
    let (mut remote, auth, provider) = runtime_fixture().await;
    let dir = tempfile::tempdir().unwrap();
    remote.environment = Some(Arc::new(Environment::new(settings(dir.path())).unwrap()));
    let http = serve(remote, auth).await;
    for (bytes, encoding) in [
        (vec![b'x'; 524288], "utf-8"),
        (vec![255; 524288], "base64"),
        (vec![1; 524288], "utf-8"),
    ] {
        std::fs::write(dir.path().join("large"), bytes).unwrap();
        let args = project(json!({"action":"read","path":dir.path().join("large")}));
        assert_eq!(
            http.value("read_runtime_path", args.clone()).await["bytes"],
            16384
        );
        let mut args = args;
        args["max_bytes"] = json!(524288);
        let response = http
            .call("runtime", "read_runtime_path", args.clone())
            .await;
        assert_ne!(response["isError"], true, "{response}");
        let response_bytes = serde_json::to_vec(&response).unwrap().len();
        eprintln!("512 KiB {encoding} MCP response: {response_bytes} JSON bytes");
        assert!(response_bytes < 8 * 1024 * 1024);
        let read = &response["structuredContent"];
        assert_eq!(read["bytes"], 524288);
        assert_eq!(read["encoding"], encoding);
        if encoding == "base64" {
            assert_eq!(read["content"].as_str().unwrap().len(), 699052);
        }
        assert!(serde_json::to_vec(&read).unwrap().len() < 3200000);
        args["max_bytes"] = json!(524289);
        assert_eq!(
            http.call("runtime", "read_runtime_path", args).await["isError"],
            true
        );
    }
    let id = Uuid::new_v4();
    http.value("run_process",project(json!({"request_id":id,"purpose":"output limit","executable":"/usr/bin/head","args":["-c","70000","/dev/zero"],"cwd":dir.path(),"lifetime_seconds":5}))).await;
    let run = http.settle(id).await["result"].clone();
    let args = project(
        json!({"action":"output","process_id":run["run_id"],"stream":"stdout","limit":65536}),
    );
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let out = http.value("observe_process", args.clone()).await;
            if out["state"] != "running" {
                assert_eq!(out["output"]["text"].as_str().unwrap().len(), 65536);
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    let mut args = args;
    args["limit"] = json!(65537);
    assert_eq!(
        http.call("runtime", "observe_process", args).await["isError"],
        true
    );
    provider.abort();
}
