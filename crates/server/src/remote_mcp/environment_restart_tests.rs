//! A real OS-process crash/restart of the runtime HTTP MCP host, not router recreation.
//! Uses production router/process/receipt code and mock authentication; not the full server binary.
use std::{
    path::{Path, PathBuf},
    process::{Child, Command},
    sync::Arc,
    time::Duration,
};

use serde_json::json;
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
fn subprocess(test: &str, root: &Path) -> Host {
    use std::os::unix::process::CommandExt;
    let mut command = Command::new(std::env::current_exe().unwrap());
    // Also terminate the fixture host if its controller fails or times out.
    unsafe {
        command.pre_exec(|| {
            if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL) != 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    Host(
        command
            .args(["--exact", test, "--ignored", "--nocapture"])
            .env("LVK_RUNTIME_RESTART_FIXTURE", root)
            .env("LVK_INSTANCE_DIR", root.join("instance"))
            .spawn()
            .unwrap(),
    )
}

#[test]
fn environment_os_process_restart_with_live_child() {
    let tmp = tempfile::tempdir().unwrap();
    let mut controller = subprocess(
        "remote_mcp::environment_restart_tests::restart_controller",
        tmp.path(),
    );
    let start = std::time::Instant::now();
    loop {
        if let Some(status) = controller.0.try_wait().unwrap() {
            assert!(
                status.success(),
                "isolated restart controller failed: {status}"
            );
            break;
        }
        assert!(
            start.elapsed() < Duration::from_secs(45),
            "restart controller timed out"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[tokio::test]
#[ignore = "subprocess fixture; invoked by environment_os_process_restart_with_live_child"]
async fn restart_host() {
    let root = PathBuf::from(std::env::var_os("LVK_RUNTIME_RESTART_FIXTURE").unwrap());
    let (mut remote, auth, _provider) = tests::fixture().await;
    remote.pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            sqlx::sqlite::SqliteConnectOptions::new()
                .filename(root.join("receipt.sqlite"))
                .create_if_missing(true),
        )
        .await
        .unwrap();
    sqlx::migrate!("../db/migrations")
        .run(&remote.pool)
        .await
        .unwrap();
    sqlx::query("INSERT OR IGNORE INTO projects(id,name) VALUES(?,'Restart fixture')")
        .bind(Uuid::from_u128(1))
        .execute(&remote.pool)
        .await
        .unwrap();
    let region = grant(&root.join("app"), PathBuf::from("/bin/sh"));
    let mut region = region;
    region.executables.push(PathBuf::from("/bin/sleep"));
    let mut other = region.clone();
    other.id = "other-grant".into();
    remote.environment = Some(Arc::new(Environment::new(vec![region, other]).unwrap()));
    let http = serve(remote, auth).await;
    std::fs::write(root.join("ready"), &http.base).unwrap();
    std::future::pending::<()>().await;
}

#[tokio::test]
#[ignore = "isolated subreaper controller; invoked by environment_os_process_restart_with_live_child"]
async fn restart_controller() {
    // Affect only this disposable test process, never the main parallel test runner.
    assert_eq!(
        unsafe { libc::prctl(libc::PR_SET_CHILD_SUBREAPER, 1, 0, 0, 0) },
        0
    );
    let root = PathBuf::from(std::env::var_os("LVK_RUNTIME_RESTART_FIXTURE").unwrap());
    std::fs::create_dir(root.join("app")).unwrap();
    std::fs::write(
        root.join("app/start.sh"),
        "printf 'launch\\n' >> launches; exec /bin/sleep 30\n",
    )
    .unwrap();
    let (mut host, http) = start_host(&root).await;
    let request = Uuid::new_v4();
    let input = json!({"region":"example-api","request_id":request,"purpose":"Isolated crash test","executable":"/bin/sh","args":["start.sh"],"cwd":".","service":true,"lifetime_seconds":60});
    http.value("run_process", input.clone()).await;
    let before = http.settle(request).await;
    assert_eq!(before["status"], "succeeded", "{before}");
    let run = &before["result"];
    let identity = run["identity"].as_str().unwrap();
    let id = run["run_id"].as_str().unwrap();
    let _child_cleanup = super::environment_process::TestProcessCleanup::new(identity).unwrap();
    let sleep = Path::new("/bin/sleep").canonicalize().unwrap();
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let observed = http
                .value(
                    "inspect_process",
                    json!({"region":"example-api","identity":identity}),
                )
                .await;
            if observed["executable"] == json!(sleep) {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    host.0.kill().unwrap();
    assert!(!host.0.wait().unwrap().success());
    drop(http);
    let (_new_host, restored) = start_host(&root).await;
    let observed = restored
        .value(
            "inspect_process",
            json!({"region":"example-api","identity":identity}),
        )
        .await;
    assert_eq!(observed["identity"], identity);
    assert_eq!(observed["pid"], run["pid"]);
    let after = restored.settle(request).await;
    assert_eq!(after["operation_id"], before["operation_id"]);
    assert_eq!(after["result"], before["result"]); // Historical startup, not current health.
    let repeated = restored.value("run_process", input).await;
    assert_eq!(repeated["result"]["run_id"], id);
    assert_eq!(
        std::fs::read_to_string(root.join("app/launches")).unwrap(),
        "launch\n"
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
    let wrong_identity = format!("{}:0", identity.rsplit_once(':').unwrap().0);
    for (region, selected) in [
        ("other-grant", identity),
        ("example-api", wrong_identity.as_str()),
    ] {
        let req = Uuid::new_v4();
        restored.value("stop_process", json!({"region":region,"identity":selected,"request_id":req,"purpose":"Verify identity and durable ownership"})).await;
        assert_eq!(restored.settle(req).await["status"], "failed");
        assert_eq!(
            restored
                .value(
                    "inspect_process",
                    json!({"region":"example-api","identity":identity})
                )
                .await["identity"],
            identity
        );
    }
    let stop = Uuid::new_v4();
    restored.value("stop_process", json!({"region":"example-api","identity":id,"request_id":stop,"purpose":"Stop verified surviving child"})).await;
    let stopped = restored.settle(stop).await;
    assert_eq!(stopped["status"], "succeeded", "{stopped}");
    assert_eq!(stopped["result"]["stop_requested"], true);
    assert_eq!(stopped["result"]["stopped"], false); // Receipt is not termination proof.
    // The controller adopted the orphan and can independently confirm termination.
    let pid = run["pid"].as_u64().unwrap() as i32;
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let mut status = 0;
            let result = unsafe { libc::waitpid(pid, &mut status, libc::WNOHANG) };
            assert!(result >= 0);
            if result == pid {
                assert!(libc::WIFSIGNALED(status));
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    assert_eq!(
        restored
            .call(
                "runtime",
                "inspect_process",
                json!({"region":"example-api","identity":identity})
            )
            .await["isError"],
        true
    );
    assert_eq!(
        restored
            .call(
                "runtime",
                "read_process_output",
                json!({"region":"example-api","process_id":id,"stream":"stderr"})
            )
            .await["isError"],
        true
    );
    let again = Uuid::new_v4();
    restored.value("stop_process", json!({"region":"example-api","identity":id,"request_id":again,"purpose":"Do not pretend an unavailable old identity is live"})).await;
    assert_eq!(restored.settle(again).await["status"], "failed");
}
async fn start_host(root: &Path) -> (Host, Http) {
    let _ = std::fs::remove_file(root.join("ready"));
    let mut host = subprocess("remote_mcp::environment_restart_tests::restart_host", root);
    let base = tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            assert!(
                host.0.try_wait().unwrap().is_none(),
                "test host exited early"
            );
            if let Ok(base) = std::fs::read_to_string(root.join("ready")) {
                break base;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    (
        host,
        Http {
            base,
            task: tokio::spawn(async {}),
        },
    )
}
