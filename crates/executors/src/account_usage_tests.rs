//! Real subprocess regression fixture; no real provider/authentication.
use std::{path::Path, time::Duration};

use serde_json::{Value, json};

use crate::{
    account_usage::Request,
    executors::{claude::ClaudeCode, codex::Codex},
};

fn config(root: &Path, mode: &str) -> Value {
    let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/passive_usage.py");
    json!({"base_command_override":shlex::try_quote(script.to_str().unwrap()).unwrap(),"env":{"LVK_USAGE_FIXTURE":root,"LVK_USAGE_FIXTURE_MODE":mode}})
}
fn assert_reaped(root: &Path) {
    for file in ["pid", "child_pid", "version_pid", "version_child_pid"] {
        if !root.join(file).exists() {
            continue;
        }
        let pid = std::fs::read_to_string(root.join(file)).unwrap();
        // A killed adopted child can briefly remain a zombie under container
        // init; it cannot execute. The owned leader must have been reaped.
        if let Ok(stat) = std::fs::read_to_string(format!("/proc/{pid}/stat")) {
            assert!(file.ends_with("child_pid"), "leader was not reaped: {pid}");
            assert!(
                stat.split_once(") ").unwrap().1.starts_with('Z'),
                "descendant still alive: {pid}"
            );
        }
    }
    for file in ["cwd", "version_cwd"] {
        if let Ok(cwd) = std::fs::read_to_string(root.join(file)) {
            assert!(
                !Path::new(&cwd).exists(),
                "management directory retained after cleanup"
            );
        }
    }
}
async fn wait_started(root: &Path) {
    tokio::time::timeout(Duration::from_secs(5), async {
        while !root.join("child_pid").exists() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn passive_usage_subprocesses_only_read_and_reap_success_failure_cancel_timeout() {
    let root = tempfile::tempdir().unwrap();
    let codex: Codex = serde_json::from_value(config(root.path(), "success")).unwrap();
    let value = codex.read_account(None, true).await;
    assert_eq!(value["quota"]["status"], "fresh", "{value}");
    assert_eq!(value["usage"]["status"], "fresh");
    assert_reaped(root.path());
    let frames = std::fs::read_to_string(root.path().join("frames")).unwrap();
    assert!(
        !frames.contains("thread/start") && !frames.contains("turn/") && !frames.contains("resume")
    );

    let root = tempfile::tempdir().unwrap();
    let claude: ClaudeCode = serde_json::from_value(config(root.path(), "success")).unwrap();
    let value = claude.read_account_usage(false, true).await;
    assert_eq!(value["quota"]["status"], "fresh", "{value}");
    assert_eq!(
        value["usage"]["scope"],
        "information_connection_session_only"
    );
    assert!(!value.to_string().contains("never-export"));
    assert_reaped(root.path());
    tokio::time::sleep(Duration::from_secs(5)).await;

    let root = tempfile::tempdir().unwrap();
    let claude: ClaudeCode = serde_json::from_value(config(root.path(), "unsupported")).unwrap();
    let value = claude.read_account_usage(true, true).await;
    assert_eq!(value["status"], "unsupported");
    assert!(!value.to_string().contains("secret-provider-error"));
    assert_reaped(root.path());

    let root = tempfile::tempdir().unwrap();
    let codex: Codex = serde_json::from_value(config(root.path(), "unsupported_usage")).unwrap();
    let value = codex.read_account(None, true).await;
    assert_eq!(value["quota"]["status"], "fresh");
    assert_eq!(value["usage"]["status"], "unsupported");
    assert!(!value.to_string().contains("secret-provider-error"));
    assert_reaped(root.path());
    tokio::time::sleep(Duration::from_secs(5)).await;

    let root = tempfile::tempdir().unwrap();
    let codex: Codex = serde_json::from_value(config(root.path(), "hang")).unwrap();
    let run = tokio::spawn(async move { codex.read_account(None, false).await });
    wait_started(root.path()).await;
    let duplicate: Codex = serde_json::from_value(config(root.path(), "success")).unwrap();
    assert_eq!(
        duplicate.read_account(None, false).await["status"],
        "unavailable"
    );
    run.abort();
    let _ = run.await;
    tokio::time::timeout(Duration::from_secs(15), async {
        while Path::new(&format!(
            "/proc/{}",
            std::fs::read_to_string(root.path().join("pid")).unwrap()
        ))
        .exists()
        {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    assert_reaped(root.path());
    tokio::time::sleep(Duration::from_secs(5)).await;

    let root = tempfile::tempdir().unwrap();
    let codex: Codex = serde_json::from_value(config(root.path(), "hang_read")).unwrap();
    assert_eq!(
        codex.read_account(None, true).await["quota"]["status"],
        "unavailable"
    );
    assert_reaped(root.path());
    tokio::time::sleep(Duration::from_secs(5)).await;
    let root = tempfile::tempdir().unwrap();
    let claude: ClaudeCode = serde_json::from_value(config(root.path(), "hang_read")).unwrap();
    assert_eq!(
        claude.read_account_usage(false, true).await["status"],
        "unavailable"
    );
    assert_reaped(root.path());
    tokio::time::sleep(Duration::from_secs(5)).await;
    let root = tempfile::tempdir().unwrap();
    let claude: ClaudeCode = serde_json::from_value(config(root.path(), "hang")).unwrap();
    let run = tokio::spawn(async move { claude.read_account_usage(false, false).await });
    wait_started(root.path()).await;
    run.abort();
    let _ = run.await;
    tokio::time::timeout(Duration::from_secs(15), async {
        while Path::new(&format!(
            "/proc/{}",
            std::fs::read_to_string(root.path().join("pid")).unwrap()
        ))
        .exists()
        {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    assert_reaped(root.path());
    tokio::time::sleep(Duration::from_secs(5)).await;
    let root = tempfile::tempdir().unwrap();
    let claude: ClaudeCode = serde_json::from_value(config(root.path(), "exit")).unwrap();
    assert_eq!(
        claude.read_account_usage(false, true).await["status"],
        "unavailable"
    );
    assert_reaped(root.path());
    tokio::time::sleep(Duration::from_secs(5)).await;
    let root = tempfile::tempdir().unwrap();
    let claude: ClaudeCode =
        serde_json::from_value(config(root.path(), "unknown_version")).unwrap();
    assert_eq!(
        claude.read_account_usage(false, true).await["status"],
        "unsupported"
    );
    assert!(
        !root.path().join("frames").exists(),
        "get_usage must not be sent to unknown CLI"
    );
    assert_reaped(root.path());
    tokio::time::sleep(Duration::from_secs(5)).await;
    let root = tempfile::tempdir().unwrap();
    let claude: ClaudeCode = serde_json::from_value(config(root.path(), "hang_version")).unwrap();
    assert_eq!(
        claude.read_account_usage(false, true).await["status"],
        "unavailable"
    );
    assert!(!root.path().join("frames").exists());
    assert_reaped(root.path());
}

#[test]
fn passive_request_cannot_accept_arbitrary_native_ids_or_ambiguous_accounts() {
    assert!(
        serde_json::from_value::<Request>(
            json!({"provider":"codex","account_context":"server_default","thread_id":"external"})
        )
        .is_err()
    );
    assert!(
        serde_json::from_value::<Request>(json!({"provider":"codex","account_context":"guess"}))
            .is_err()
    );
}

/// Explicitly invoked only for authorised live diagnostics. Never submitted a
/// prompt or resumed a session. Prints status labels, never quota/account data.
#[tokio::test]
#[ignore = "requires explicit authorization for official CLI network/auth reads"]
async fn live_provider_usage_observations() {
    use crate::{
        executors::{BaseCodingAgent, CodingAgent},
        profile::{ExecutorConfigs, ExecutorProfileId},
    };
    let profiles = ExecutorConfigs::get_cached();
    for provider in [BaseCodingAgent::Codex, BaseCodingAgent::ClaudeCode] {
        let profile = ExecutorProfileId::new(provider);
        let agent = profiles
            .get_coding_agent(&profile)
            .expect("default launcher configured");
        let overridden = match &agent {
            CodingAgent::Codex(a) => a.cmd.base_command_override.is_some(),
            CodingAgent::ClaudeCode(a) => a.cmd.base_command_override.is_some(),
        };
        println!(
            "LIVE {profile}: launcher_override={overridden}; authentication=official CLI existing context (kind not independently inspected)"
        );
        let value = match agent {
            CodingAgent::Codex(agent) => agent.read_account(None, true).await,
            CodingAgent::ClaudeCode(agent) => agent.read_account_usage(false, true).await,
        };
        let auth_inference = if profile.executor == BaseCodingAgent::Codex
            && value.pointer("/quota/status").and_then(Value::as_str) == Some("fresh")
        {
            "ChatGPT (inferred from official quota RPC success)"
        } else if value
            .pointer("/quota/rate_limits_available")
            .and_then(Value::as_bool)
            == Some(true)
        {
            "claude.ai subscription (inferred from official quota snapshot)"
        } else {
            "unknown; not independently inspected"
        };
        let subscription = value
            .pointer("/quota/subscription_type")
            .and_then(Value::as_str)
            .filter(|v| matches!(*v, "pro" | "max" | "team" | "enterprise"))
            .unwrap_or("not-reported");
        println!(
            "LIVE {profile}: authentication={auth_inference}; subscription_type={subscription}"
        );
        println!(
            "LIVE {profile}: overall={} quota={} usage={} subscription_limits_available={}",
            value
                .get("status")
                .and_then(Value::as_str)
                .unwrap_or("per-section"),
            value
                .pointer("/quota/status")
                .and_then(Value::as_str)
                .unwrap_or("unavailable"),
            value
                .pointer("/usage/status")
                .and_then(Value::as_str)
                .unwrap_or("unavailable"),
            value
                .pointer("/quota/rate_limits_available")
                .and_then(Value::as_bool)
                .map(|v| v.to_string())
                .unwrap_or_else(|| "not-reported".into())
        );
    }
}

#[tokio::test]
async fn passive_usage_rejects_execution_arguments_before_launch() {
    for args in [
        json!({"base_command_override":"codex exec forbidden-prompt"}),
        json!({"base_command_override":"claude --resume forbidden-session"}),
        json!({"additional_params":["--prompt","forbidden-prompt"]}),
    ] {
        assert!(!crate::command::supports_passive_launcher(
            &serde_json::from_value(args.clone()).unwrap()
        ));
        let codex: Codex = serde_json::from_value(args.clone()).unwrap();
        assert_eq!(
            codex.read_account(None, true).await["status"],
            "unsupported"
        );
        let claude: ClaudeCode = serde_json::from_value(args).unwrap();
        assert_eq!(
            claude.read_account_usage(false, true).await["status"],
            "unsupported"
        );
    }
}
