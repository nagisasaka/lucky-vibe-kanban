//! Explicit dev-only handoff to the administrator-installed switch engine.
//! The MCP receipt records admission; the engine's durable result owns completion.
use std::{path::PathBuf, process::Stdio, time::Duration};

use anyhow::{Result, ensure};
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use super::{Principal, Remote, read::arguments};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Restart {
    request_id: Uuid,
    purpose: String,
    bundle: PathBuf,
    no_new_work: bool,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Status {
    request_id: Uuid,
}

fn configured() -> Result<(PathBuf, PathBuf)> {
    let instance = utils::instance::instance_dir_read_only()?
        .ok_or_else(|| anyhow::anyhow!("Developer instance required"))?;
    let engine = instance.join("switch/start_binary.py");
    let config = instance.join("launch.json");
    ensure!(
        engine.canonicalize()? == engine && config.canonicalize()? == config,
        "Self restart configuration cannot be aliased"
    );
    Ok((engine, instance))
}

fn snapshot(instance: &std::path::Path) -> Result<Value> {
    let pid = std::process::id();
    let proc = PathBuf::from(format!("/proc/{pid}"));
    let stat = std::fs::read_to_string(proc.join("stat"))?;
    let start = stat
        .rsplit_once(')')
        .ok_or_else(|| anyhow::anyhow!("Invalid process identity"))?
        .1
        .split_whitespace()
        .nth(19)
        .ok_or_else(|| anyhow::anyhow!("Missing process start"))?;
    let listener = super::LOCAL_LISTENER
        .get()
        .ok_or_else(|| anyhow::anyhow!("Local listener identity unavailable"))?;
    ensure!(
        listener.ip() == std::net::Ipv4Addr::LOCALHOST,
        "Self restart requires loopback listener"
    );
    let port = listener.port();
    let current: Value = serde_json::from_slice(&std::fs::read(instance.join("current.json"))?)?;
    Ok(json!({
        "identity": {"pid": pid, "start": start,
            "boot": std::fs::read_to_string("/proc/sys/kernel/random/boot_id")?.trim(),
            "exe": std::fs::read_link(proc.join("exe"))?, "cwd": std::env::current_dir()?},
        "instance": instance, "data": utils::assets::asset_dir_path()?, "port": port,
        "current": current,
        "launch_hash": std::env::var("LVK_LAUNCH_CONFIG_SHA256")?
    }))
}

async fn invoke(owner: &str, args: Vec<String>) -> Result<Value> {
    let (engine, instance) = configured()?;
    let handoff = if args.iter().any(|a| a == "--async") {
        Some(snapshot(&instance)?)
    } else {
        None
    };
    let mut command = tokio::process::Command::new("/usr/bin/python3");
    command
        .arg(engine)
        .arg("--instance")
        .arg(instance)
        .arg("--owner")
        .arg(owner)
        .args(args)
        .env_clear()
        .env("PATH", "/usr/local/bin:/usr/bin:/bin")
        .env("LANG", "C.UTF-8")
        .stdin(if handoff.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    // Only the short admission/status client is managed here. Its worker creates
    // a new session, owns the lock, closes inherited I/O, and persists all stages.
    if handoff.is_some() {
        command.arg("--snapshot-stdin");
    }
    let mut child = command.spawn()?;
    if let Some(handoff) = handoff {
        use tokio::io::AsyncWriteExt;
        let mut stdin = child
            .stdin
            .take()
            .ok_or_else(|| anyhow::anyhow!("Missing handoff pipe"))?;
        stdin.write_all(&serde_json::to_vec(&handoff)?).await?;
        stdin.shutdown().await?;
    }
    let output = tokio::time::timeout(Duration::from_secs(15), child.wait_with_output()).await??;
    ensure!(
        output.stdout.len() <= 65536,
        "Self restart response too large"
    );
    let result: Value = serde_json::from_slice(&output.stdout)?;
    ensure!(result.is_object(), "Invalid self restart response");
    ensure!(
        output.status.success()
            || matches!(
                result["status"].as_str(),
                Some("failed" | "interrupted" | "refused")
            ),
        "Invalid self restart engine exit"
    );
    Ok(result)
}

pub(super) async fn call(
    remote: &Remote,
    principal: &Principal,
    project: Uuid,
    name: &str,
    input: Value,
) -> Result<Value> {
    let principal_key = remote.principal_key(principal);
    let owner = format!(
        "{:x}",
        Sha256::digest(json!([principal_key, project]).to_string().as_bytes())
    );
    if name == "get_self_restart_operation" {
        let a: Status = arguments(input)?;
        return invoke(
            &owner,
            vec![
                "--status".into(),
                "--request-id".into(),
                a.request_id.to_string(),
            ],
        )
        .await;
    }
    ensure!(
        principal.allows("lvk:runtime:execute"),
        "Runtime execute permission required"
    );
    configured()?;
    let a: Restart = arguments(input.clone())?;
    ensure!(
        a.no_new_work && !a.purpose.trim().is_empty() && a.purpose.len() <= 1000,
        "Coordinate idle work and provide purpose before self restart"
    );
    let mut args = vec![
        "--async".into(),
        "--request-id".into(),
        a.request_id.to_string(),
        "--no-new-work".into(),
    ];
    ensure!(a.bundle.is_absolute(), "Bundle path must be absolute");
    args.extend([
        "--bundle".into(),
        a.bundle
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("Non-UTF8 bundle path"))?
            .into(),
    ]);
    let owned = remote.clone();
    let original = json!({"project_id":project,"arguments":input});
    let targets = json!({"project_id":project,"purpose":a.purpose,"completion":"get_self_restart_operation with the same request_id; admission is not cutover success"});
    let receipt = remote.operations().submit_and_wait(principal_key, a.request_id, project, "self_restart".into(), original, targets, move |_| async move {
        let result = async {
            let current = owned.environment.as_ref().ok_or_else(|| anyhow::anyhow!("Runtime access disabled"))?.current_settings().await?;
            ensure!(current.execution_enabled, "Program execution disabled");
            invoke(&owner,args).await
        }.await;
        match result {
            Ok(value) if value["status"] == "refused" => ("failed", value),
            Ok(value) => ("succeeded",json!({"admission":value,"cutover_success":false})),
            Err(_) => ("uncertain",json!({"message":"Self restart admission unavailable; query the same request_id before retrying"})),
        }
    }).await?;
    Ok(remote.receipt_value(receipt))
}

// Local stdio capability only. OAuth/browser callers retain the existing remote
// path; this adapter cannot select an instance, executable, owner or engine.
pub(super) fn stdio_router(pool: sqlx::SqlitePool) -> axum::Router {
    axum::Router::new()
        .route(
            "/api/self-restart/{action}",
            axum::routing::post(stdio_call),
        )
        .with_state(pool)
}

fn stdio_allowed(channel: &str, principal: &Principal, name: &str) -> bool {
    channel == "internal"
        && matches!(name, "self_restart" | "get_self_restart_operation")
        && principal.allows("lvk:read")
        && principal.allows("lvk:runtime:read")
        && (name != "self_restart" || principal.allows("lvk:runtime:execute"))
}

async fn stdio_call(
    axum::extract::State(pool): axum::extract::State<sqlx::SqlitePool>,
    axum::Extension(actor): axum::Extension<server_access::Principal>,
    axum::extract::Path(name): axum::extract::Path<String>,
    axum::Json(input): axum::Json<Value>,
) -> Result<axum::Json<utils::response::ApiResponse<Value>>, crate::error::ApiError> {
    let principal = Principal {
        subject: actor.subject,
        scopes: actor.scopes,
    };
    if !stdio_allowed(actor.channel, &principal, &name) {
        return Err(crate::error::ApiError::Forbidden(
            "Internal restart capability and runtime permissions required".into(),
        ));
    }
    let ids = sqlx::query_scalar::<_, Uuid>("SELECT id FROM projects")
        .fetch_all(&pool)
        .await?;
    let remote = Remote {
        environment: Some(std::sync::Arc::new(
            super::environment::Environment::managed(pool.clone()),
        )),
        pool,
        runtime: None,
        launch: None,
        process_instance: *super::PROCESS_INSTANCE.get_or_init(Uuid::new_v4),
        config: std::sync::Arc::new(super::config::Config {
            public_base_url: String::new(),
            issuer: actor.issuer,
            project_ids: ids.into_iter().collect(),
            #[cfg(test)]
            introspection_endpoint: String::new(),
            #[cfg(test)]
            introspection_client_id: String::new(),
            #[cfg(test)]
            introspection_secret_env: String::new(),
            #[cfg(test)]
            owner_subject: principal.subject.clone(),
        }),
    };
    // Same settings, project, request identity, durable receipt and engine code
    // as remote MCP. Never interpret a local principal as OAuth user approval.
    super::environment::call(&remote, &principal, &name, input)
        .await
        .map(|value| axum::Json(utils::response::ApiResponse::success(value)))
        .map_err(|error| crate::error::ApiError::BadRequest(super::errors::message(&error).into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stdio_requires_internal_channel_and_existing_runtime_scopes() {
        let mut principal = Principal {
            subject: "fixture".into(),
            scopes: ["lvk:read", "lvk:runtime:read", "lvk:runtime:execute"]
                .into_iter()
                .map(str::to_owned)
                .collect(),
        };
        assert!(stdio_allowed("internal", &principal, "self_restart"));
        for channel in ["web", "mcp", "local"] {
            assert!(!stdio_allowed(channel, &principal, "self_restart"));
        }
        assert!(!stdio_allowed("internal", &principal, "run_process"));
        principal.scopes.remove("lvk:runtime:execute");
        assert!(!stdio_allowed("internal", &principal, "self_restart"));
        assert!(stdio_allowed(
            "internal",
            &principal,
            "get_self_restart_operation"
        ));
        principal.scopes.remove("lvk:runtime:read");
        assert!(!stdio_allowed(
            "internal",
            &principal,
            "get_self_restart_operation"
        ));
    }

    #[tokio::test]
    async fn stdio_http_uses_real_access_gate_and_runtime_settings_without_restart() {
        let (remote, _, provider) = super::super::tests::fixture().await;
        let directory = tempfile::tempdir().unwrap();
        let access = server_access::test_support::access(
            remote.pool.clone(),
            directory.path().into(),
            json!({"keys":[]}),
        )
        .await;
        let token = server_access::test_support::internal_token(&access);
        let app = access.layer(stdio_router(remote.pool.clone()));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let client = reqwest::Client::new();
        let input = json!({"project_id":Uuid::from_u128(1),"request_id":Uuid::new_v4(),"purpose":"isolated fixture","bundle":"/not-a-live-bundle","no_new_work":true});
        let uri = format!("{base}/api/self-restart/self_restart");
        assert_eq!(
            client
                .post(&uri)
                .json(&input)
                .send()
                .await
                .unwrap()
                .status(),
            reqwest::StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            client
                .post(&uri)
                .header("x-lvk-internal-token", "wrong")
                .json(&input)
                .send()
                .await
                .unwrap()
                .status(),
            reqwest::StatusCode::UNAUTHORIZED
        );
        // Valid capability reaches the same disabled runtime setting check, not an engine.
        assert_eq!(
            client
                .post(&uri)
                .header("x-lvk-internal-token", &token)
                .json(&input)
                .send()
                .await
                .unwrap()
                .status(),
            reqwest::StatusCode::BAD_REQUEST
        );
        assert_eq!(
            client
                .post(format!("{base}/api/self-restart/run_process"))
                .header("x-lvk-internal-token", &token)
                .json(&input)
                .send()
                .await
                .unwrap()
                .status(),
            reqwest::StatusCode::FORBIDDEN
        );
        assert_eq!(
            client
                .post(&uri)
                .header("x-lvk-internal-token", &token)
                .header("Origin", "https://untrusted.fixture")
                .json(&input)
                .send()
                .await
                .unwrap()
                .status(),
            reqwest::StatusCode::FORBIDDEN
        );
        server.abort();
        provider.abort();
    }

    #[test]
    fn bundle_contract_rejects_legacy_fields() {
        let mut value = json!({"request_id":Uuid::new_v4(), "purpose":"fixture", "bundle":"/bundle", "no_new_work":true});
        assert!(serde_json::from_value::<Restart>(value.clone()).is_ok());
        value["source"] = json!("/legacy-source");
        assert!(serde_json::from_value::<Restart>(value).is_err());
    }
}
