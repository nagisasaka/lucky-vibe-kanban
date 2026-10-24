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
    bundle: PathBuf,
    no_new_work: bool,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Status {
    request_id: Uuid,
}

fn configured(grant: &Grant) -> Result<(PathBuf, PathBuf)> {
    let instance = utils::instance::instance_dir_read_only()?
        .ok_or_else(|| anyhow::anyhow!("Developer instance required"))?;
    ensure!(
        grant.root == instance,
        "Self restart requires an explicit grant for this instance"
    );
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

async fn invoke(grant: &Grant, owner: &str, args: Vec<String>) -> Result<Value> {
    let (engine, instance) = configured(grant)?;
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
    ensure!(a.bundle.is_absolute(), "Bundle path must be absolute");
    args.extend([
        "--bundle".into(),
        a.bundle
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("Non-UTF8 bundle path"))?
            .into(),
    ]);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundle_contract_rejects_legacy_fields() {
        let mut value = json!({"request_id":Uuid::new_v4(), "purpose":"fixture", "bundle":"/bundle", "no_new_work":true});
        assert!(serde_json::from_value::<Restart>(value.clone()).is_ok());
        value["source"] = json!("/legacy-source");
        assert!(serde_json::from_value::<Restart>(value).is_err());
    }
}
