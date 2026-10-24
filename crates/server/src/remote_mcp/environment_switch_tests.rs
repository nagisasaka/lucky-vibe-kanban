//! Production HTTP MCP + real Python switch engine in disposable subprocesses.
//! The engine terminates the calling production-MCP fixture itself, then starts
//! a C/SQLite target. A fresh MCP host reads the durable outcome. Authentication
//! uses the existing mock IdP; this is not real Auth0 or the production server UI.
use std::{
    path::{Path, PathBuf},
    process::{Child, Command},
    sync::Arc,
    time::Duration,
};

use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use super::{
    environment::Environment,
    environment_tests::{Http, grant, serve},
    tests,
};

struct Host(Child);
impl Drop for Host {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
struct Driver(Child);
impl Drop for Driver {
    fn drop(&mut self) {
        if self.0.try_wait().ok().flatten().is_some() {
            return;
        }
        // The driver owns fixture cleanup; never SIGKILL it and orphan a switch.
        unsafe {
            libc::kill(self.0.id() as i32, libc::SIGTERM);
        }
        let until = std::time::Instant::now() + Duration::from_secs(180);
        while std::time::Instant::now() < until {
            if self.0.try_wait().ok().flatten().is_some() {
                return;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        eprintln!("Fixture cleanup did not settle; operator inspection required");
    }
}
fn engine() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../deploy/developer/switch/start_binary.py")
        .canonicalize()
        .unwrap()
}
fn host(root: &Path, fixture: &Value, restored: bool) -> Host {
    Host(
        Command::new(if restored {
            std::env::current_exe().unwrap()
        } else {
            Path::new(fixture["instance"].as_str().unwrap()).join("releases/initial/server")
        })
        .args([
            "--exact",
            "remote_mcp::environment_switch_tests::switch_host",
            "--ignored",
            "--nocapture",
        ])
        .current_dir(fixture["instance"].as_str().unwrap())
        .env("DISABLE_WORKTREE_CLEANUP", "1")
        .env("LVK_SWITCH_STATUS_ONLY", if restored { "1" } else { "0" })
        .env("LVK_SWITCH_HTTP_FIXTURE", root)
        .env("LVK_INSTANCE_DIR", fixture["instance"].as_str().unwrap())
        .env(
            "LVK_LAUNCH_CONFIG_SHA256",
            format!(
                "{:x}",
                Sha256::digest(
                    std::fs::read(
                        Path::new(fixture["instance"].as_str().unwrap()).join("launch.json")
                    )
                    .unwrap()
                )
            ),
        )
        .env(
            "BACKEND_PORT",
            fixture["port"].as_u64().unwrap().to_string(),
        )
        .env(
            "VIBE_KANBAN_AGENT_PROCESS_HOST",
            Path::new(fixture["instance"].as_str().unwrap())
                .join("releases/initial/agent-process-host"),
        )
        .spawn()
        .unwrap(),
    )
}
async fn wait_file(path: &Path) -> String {
    tokio::time::timeout(Duration::from_secs(20), async {
        loop {
            if let Ok(value) = std::fs::read_to_string(path) {
                if !value.is_empty() {
                    break value;
                }
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap()
}
async fn start(root: &Path, fixture: &Value, restored: bool) -> (Host, Http) {
    let _ = std::fs::remove_file(root.join("ready"));
    let child = host(root, fixture, restored);
    let base = wait_file(&root.join("ready")).await;
    (
        child,
        Http {
            base,
            task: tokio::spawn(async {}),
        },
    )
}

#[tokio::test]
#[ignore = "isolated HTTP host launched by environment_switch_http_disconnect_and_reconnect"]
async fn switch_host() {
    let root = PathBuf::from(std::env::var_os("LVK_SWITCH_HTTP_FIXTURE").unwrap());
    let (mut remote, auth, _provider) = tests::fixture().await;
    remote.pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            sqlx::sqlite::SqliteConnectOptions::new()
                .filename(root.join("receipts.sqlite"))
                .create_if_missing(true),
        )
        .await
        .unwrap();
    sqlx::migrate!("../db/migrations")
        .run(&remote.pool)
        .await
        .unwrap();
    sqlx::query("INSERT OR IGNORE INTO projects(id,name) VALUES(?,'Switch fixture')")
        .bind(Uuid::from_u128(1))
        .execute(&remote.pool)
        .await
        .unwrap();
    let instance = PathBuf::from(std::env::var_os("LVK_INSTANCE_DIR").unwrap());
    let first = grant(&instance);
    let mut second = first.clone();
    second.id = "other-grant".into();
    remote.environment = Some(Arc::new(Environment::new(vec![first, second]).unwrap()));
    let base = if std::env::var("LVK_SWITCH_STATUS_ONLY").unwrap() == "1" {
        let http = serve(remote, auth).await;
        let base = http.base.clone();
        std::mem::forget(http);
        base
    } else {
        let config: Value =
            serde_json::from_slice(&std::fs::read(instance.join("launch.json")).unwrap()).unwrap();
        let listener =
            tokio::net::TcpListener::bind(("127.0.0.1", config["port"].as_u64().unwrap() as u16))
                .await
                .unwrap();
        super::record_local_listener(listener.local_addr().unwrap());
        let base = format!("http://{}", listener.local_addr().unwrap());
        let app = super::router(remote, auth)
            .route(
                "/api/health",
                axum::routing::get(|| async { axum::Json(json!({"success":true})) }),
            )
            .route(
                "/api/resource-coordination/snapshot",
                axum::routing::get(|| async {
                    axum::Json(
                        json!({"success":true,"data":{"resources":[],"holders":[],"queue":[]}}),
                    )
                }),
            );
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        std::fs::write(instance.join("server.pid"), std::process::id().to_string()).unwrap();
        base
    };
    std::fs::write(root.join("ready"), &base).unwrap();
    std::future::pending::<()>().await;
}

#[tokio::test]
async fn environment_switch_http_disconnect_and_reconnect() {
    let root = tempfile::tempdir().unwrap();
    let mut driver = Driver(
        Command::new("/usr/bin/python3")
            .arg(engine().with_file_name("mcp_fixture.py"))
            .arg(root.path())
            .arg(std::env::current_exe().unwrap())
            .env_remove("LVK_INSTANCE_DIR")
            .spawn()
            .unwrap(),
    );
    let fixture: Value =
        serde_json::from_str(&wait_file(&root.path().join("fixture.json")).await).unwrap();
    let (mut old, http) = start(root.path(), &fixture, false).await;
    let id = Uuid::new_v4();
    let mut input = fixture["selection"].clone();
    input["request_id"] = json!(id);
    input["region"] = json!("example-api");
    input["purpose"] = json!("Isolated restart disconnect test");
    input["no_new_work"] = json!(true);
    let receipt = http.value("self_restart", input.clone()).await;
    // The engine, not the test controller, stops its own MCP caller via pidfd.
    let initial = if receipt["status"] == "accepted" || receipt["status"] == "running" {
        http.settle(id).await
    } else {
        receipt
    };
    assert_eq!(initial["status"], "succeeded", "{initial}");
    tokio::time::timeout(Duration::from_secs(20), async {
        loop {
            if let Some(status) = old.0.try_wait().unwrap() {
                assert!(!status.success());
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    drop(http);
    let result_file = Path::new(fixture["instance"].as_str().unwrap())
        .join("switch-operations")
        .join(id.to_string())
        .join("result.json");
    tokio::time::timeout(Duration::from_secs(20), async {
        loop {
            let result: Value =
                serde_json::from_str(&std::fs::read_to_string(&result_file).unwrap()).unwrap();
            if result["status"] == "succeeded" {
                break;
            }
            assert_ne!(result["status"], "failed", "{result}");
            tokio::time::sleep(Duration::from_millis(30)).await;
        }
    })
    .await
    .unwrap();
    let (new, restored) = start(root.path(), &fixture, true).await;
    let outcome = restored
        .value(
            "get_self_restart_operation",
            json!({"region":"example-api","request_id":id}),
        )
        .await;
    assert_eq!(outcome["status"], "succeeded", "{outcome}");
    assert_eq!(outcome["operation_id"], id.to_string());
    let repeated = restored.value("self_restart", input).await;
    assert_eq!(repeated["operation_id"], initial["operation_id"]);
    let other = restored
        .value(
            "get_self_restart_operation",
            json!({"region":"other-grant","request_id":id}),
        )
        .await;
    assert_eq!(other["status"], "refused");
    assert_eq!(
        restored
            .rpc(
                "runtime-read",
                "tools/call",
                json!({"name":"self_restart","arguments":{}})
            )
            .await
            .0,
        403
    );
    drop(restored);
    drop(new);
    // Graceful driver exit performs pidfd/instance-checked fixture cleanup.
    unsafe {
        libc::kill(driver.0.id() as i32, libc::SIGTERM);
    }
    let status = driver.0.wait().unwrap();
    assert!(status.success());
}
