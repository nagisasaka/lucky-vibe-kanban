//! Explicit dev-only handoff to the administrator-installed switch engine.
//! The MCP receipt records admission; the engine's durable result owns completion.
use std::{path::PathBuf, process::Stdio, time::Duration};

use anyhow::{Result, ensure};
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use super::{Principal, Remote, environment::Grant, read::arguments};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Restart {
    request_id: Uuid,
    purpose: String,
    binary: PathBuf,
    source: PathBuf,
    agent_host: PathBuf,
    frontend: PathBuf,
    #[serde(default)]
    manifest: Option<PathBuf>,
    no_new_work: bool,
    #[serde(default)]
    frontend_mode: Option<String>,
    #[serde(default)]
    retain_worktree: bool,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Status {
    request_id: Uuid,
}

fn configured(grant: &Grant) -> Result<(PathBuf, PathBuf)> {
    let engine = PathBuf::from(std::env::var("LVK_SELF_RESTART_ENGINE")?);
    let config = PathBuf::from(std::env::var("LVK_SELF_RESTART_CONFIG")?);
    let instance = PathBuf::from(std::env::var("LVK_INSTANCE_DIR")?);
    ensure!(
        engine.is_absolute() && config.is_absolute() && instance.is_absolute(),
        "Self restart requires administrator-installed absolute paths"
    );
    ensure!(
        engine.canonicalize()? == engine
            && config.canonicalize()? == config
            && instance.canonicalize()? == instance,
        "Self restart configuration cannot be aliased"
    );
    ensure!(
        grant.root == instance && config.parent() == Some(instance.as_path()),
        "Self restart requires an explicit grant for this instance"
    );
    let settings: Value = serde_json::from_slice(&std::fs::read(&config)?)?;
    ensure!(
        settings["instance"].as_str() == instance.to_str(),
        "Self restart instance mismatch"
    );
    Ok((engine, config))
}

async fn invoke(grant: &Grant, owner: &str, args: Vec<String>) -> Result<Value> {
    let (engine, config) = configured(grant)?;
    let mut command = tokio::process::Command::new("/usr/bin/python3");
    command
        .arg(engine)
        .arg("--config")
        .arg(config)
        .arg("--owner")
        .arg(owner)
        .args(args)
        .env_clear()
        .env("PATH", "/usr/local/bin:/usr/bin:/bin")
        .env("LANG", "C.UTF-8")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    // Only the short admission/status client is managed here. Its worker creates
    // a new session, owns the lock, closes inherited I/O, and persists all stages.
    let output = tokio::time::timeout(Duration::from_secs(15), command.output()).await??;
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
    grant: &Grant,
    name: &str,
    input: Value,
) -> Result<Value> {
    let principal_key = remote.principal_key(principal);
    let owner = format!(
        "{:x}",
        Sha256::digest(
            json!([principal_key, grant.id, grant.project_id])
                .to_string()
                .as_bytes()
        )
    );
    if name == "get_self_restart_operation" {
        let a: Status = arguments(input)?;
        return invoke(
            grant,
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
    ensure!(
        grant.allow_uncoordinated_execution,
        "Instance execution disabled"
    );
    configured(grant)?;
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
    for (key, path) in [
        ("--binary", a.binary),
        ("--source", a.source),
        ("--agent-host", a.agent_host),
        ("--frontend", a.frontend),
    ] {
        ensure!(path.is_absolute(), "Artifact paths must be absolute");
        args.extend([
            key.into(),
            path.to_str()
                .ok_or_else(|| anyhow::anyhow!("Non-UTF8 artifact path"))?
                .into(),
        ]);
    }
    if let Some(path) = a.manifest {
        ensure!(path.is_absolute(), "Manifest path must be absolute");
        args.extend([
            "--manifest".into(),
            path.to_str()
                .ok_or_else(|| anyhow::anyhow!("Non-UTF8 manifest path"))?
                .into(),
        ]);
    }
    if let Some(mode) = a.frontend_mode {
        ensure!(
            matches!(mode.as_str(), "external" | "embedded"),
            "Invalid frontend mode"
        );
        args.extend(["--frontend-mode".into(), mode]);
    }
    if a.retain_worktree {
        args.push("--retain-worktree".into());
    }
    let owned = remote.clone();
    let grant = grant.clone();
    let original = json!({"region":grant.id,"arguments":input});
    let targets = json!({"region":grant.id,"purpose":a.purpose,"completion":"get_self_restart_operation with the same request_id; admission is not cutover success"});
    let receipt = remote.operations().submit_and_wait(principal_key, a.request_id, grant.project_id, "self_restart".into(), original, targets, move |_| async move {
        let result = async {
            let current = owned.environment.as_ref().ok_or_else(|| anyhow::anyhow!("Runtime access disabled"))?.grant(&grant.id).await?;
            ensure!(serde_json::to_value(&current)? == serde_json::to_value(&grant)?, "Runtime grant changed");
            invoke(&grant,&owner,args).await
        }.await;
        match result {
            Ok(value) if value["status"] == "refused" => ("failed", value),
            Ok(value) => ("succeeded",json!({"admission":value,"cutover_success":false})),
            Err(_) => ("uncertain",json!({"message":"Self restart admission unavailable; query the same request_id before retrying"})),
        }
    }).await?;
    Ok(remote.receipt_value(receipt))
}
