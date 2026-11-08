//! Experimental official Agent SDK 0.3.301 get_usage wire projection.
//! get_usage always skips behavior/transcript scanning. Quota is percent, not
//! the 0..1 utilization used by rate_limit_event notifications.
use std::{
    collections::BTreeMap,
    sync::{Arc, LazyLock},
    time::{Duration, Instant},
};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::{
    process::Command,
    sync::{Mutex, Semaphore, TryAcquireError},
};
use tokio_util::sync::CancellationToken;
use workspace_utils::command_ext::GroupSpawnNoWindowExt;

use super::{ClaudeCode, protocol::ProtocolPeer};
use crate::{
    env::{ExecutionEnv, RepoContext},
    executors::ExecutorError,
};

static SLOT: LazyLock<Arc<Semaphore>> = LazyLock::new(|| Arc::new(Semaphore::new(1)));
static LAST_START: Mutex<Option<Instant>> = Mutex::const_new(None);

#[derive(Debug, Serialize, Deserialize)]
pub struct Usage {
    pub session: SessionUsage,
    pub subscription_type: Option<String>,
    pub rate_limits_available: bool,
    pub rate_limits: Option<RateLimits>,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct SessionUsage {
    pub total_cost_usd: Option<f64>,
    pub total_api_duration_ms: Option<u64>,
    pub total_duration_ms: Option<u64>,
    pub model_usage: Option<BTreeMap<String, ModelUsage>>,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelUsage {
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub thinking_tokens: Option<u64>,
    pub cache_read_input_tokens: Option<u64>,
    pub cache_creation_input_tokens: Option<u64>,
    pub cost_usd: Option<f64>,
    pub canonical_model: Option<String>,
    pub provider: Option<String>,
    pub cost_basis: Option<String>,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct RateLimits {
    pub five_hour: Option<Window>,
    pub seven_day: Option<Window>,
    pub seven_day_oauth_apps: Option<Window>,
    pub seven_day_opus: Option<Window>,
    pub seven_day_sonnet: Option<Window>,
    pub model_scoped: Option<Vec<ModelWindow>>,
    pub extra_usage: Option<ExtraUsage>,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct Window {
    pub utilization: Option<f64>,
    pub resets_at: Option<String>,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct ModelWindow {
    pub display_name: String,
    pub utilization: Option<f64>,
    pub resets_at: Option<String>,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct ExtraUsage {
    pub is_enabled: bool,
    pub monthly_limit: Option<f64>,
    pub used_credits: Option<f64>,
    pub utilization: Option<f64>,
    pub currency: Option<String>,
}
impl Usage {
    pub fn validate(&self) -> bool {
        let percent =
            |n: Option<f64>| n.is_none_or(|n| n.is_finite() && (0.0..=100.0).contains(&n));
        self.session
            .total_cost_usd
            .is_none_or(|n| n.is_finite() && n >= 0.0)
            && self.session.model_usage.as_ref().is_none_or(|m| {
                m.len() <= 256
                    && m.values()
                        .all(|m| m.cost_usd.is_none_or(|n| n.is_finite() && n >= 0.0))
            })
            && self.rate_limits.as_ref().is_none_or(|r| {
                [
                    &r.five_hour,
                    &r.seven_day,
                    &r.seven_day_oauth_apps,
                    &r.seven_day_opus,
                    &r.seven_day_sonnet,
                ]
                .iter()
                .all(|w| w.as_ref().is_none_or(|w| percent(w.utilization)))
                    && r.model_scoped
                        .as_ref()
                        .is_none_or(|m| m.len() <= 256 && m.iter().all(|m| percent(m.utilization)))
                    && r.extra_usage
                        .as_ref()
                        .is_none_or(|e| percent(e.utilization))
            })
    }
}
fn unavailable(reason: &str) -> Value {
    unavailable_code("read_failed", reason)
}
fn unavailable_code(code: &str, reason: &str) -> Value {
    json!({"status":"unavailable","code":code,"reason":reason,"data":null})
}
fn admit(slot: &Arc<Semaphore>) -> Result<tokio::sync::OwnedSemaphorePermit, Value> {
    slot.clone().try_acquire_owned().map_err(|error| match error {
        TryAcquireError::Closed => unavailable_code("cleanup_unconfirmed", "Claude account reads are disabled because process cleanup could not be confirmed; operator recovery is required"),
        TryAcquireError::NoPermits => unavailable_code("in_progress", "Claude account read already in progress; no duplicate process started"),
    })
}
impl ClaudeCode {
    pub async fn read_account_usage(&self, existing_session: bool, include_usage: bool) -> Value {
        if self.claude_code_router == Some(true)
            || !crate::command::supports_passive_launcher(&self.cmd)
            || self.validate_subscription_profile().is_err()
        {
            return json!({"status":"unsupported","reason":"Passive reads require the official CLI executable without router, base arguments, extra parameters or incompatible authentication overrides","data":null});
        }
        let permit = match admit(&SLOT) {
            Ok(permit) => permit,
            Err(value) => return value,
        };
        {
            let mut last = LAST_START.lock().await;
            if last.is_some_and(|t| t.elapsed() < Duration::from_secs(5)) {
                return unavailable_code(
                    "rate_limited",
                    "Claude reads are limited to one start per five seconds",
                );
            }
            *last = Some(Instant::now());
        }
        let deadline = tokio::time::Instant::now() + Duration::from_secs(20);
        let Ok(directory) = tempfile::Builder::new()
            .prefix("lvk-claude-usage-")
            .tempdir()
        else {
            return unavailable("Could not create management directory");
        };
        let parts =
            super::command_adapter::ClaudeCodeCommandAdapter::new(self).build_passive_usage();
        let Ok(parts) = parts.and_then(|b| b.build_initial()) else {
            return unavailable("Invalid passive launcher");
        };
        let Ok((program, args)) = parts.into_resolved_passive().await else {
            return unavailable("Claude CLI is not installed");
        };
        let build_command = || {
            let mut command = Command::new(&program);
            command
                .args(&args)
                .current_dir(directory.path())
                .stdin(std::process::Stdio::piped())
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::null())
                .kill_on_drop(true);
            ExecutionEnv::new(RepoContext::default())
                .with_profile(&self.cmd)
                .apply_to_command(&mut command);
            command
        };
        let command = build_command();
        let mut version_probe = build_command();
        version_probe
            .arg("--version")
            .stdin(std::process::Stdio::null());
        let cancel = CancellationToken::new();
        let _on_drop = cancel.clone().drop_guard();
        let read = async move {
            let _owned = (permit, directory);
            verify_passive_version(version_probe, cancel.clone(), deadline).await?;
            if cancel.is_cancelled() || deadline <= tokio::time::Instant::now() {
                return Err(ExecutorError::Io(std::io::Error::new(
                    std::io::ErrorKind::Interrupted,
                    "Account startup cancelled or timed out",
                )));
            }
            let mut command = command;
            let mut child = command.group_spawn_no_window()?;
            let mut ownership = match workspace_utils::process::OwnedProcessGroup::capture(
                &mut child,
                "claude-account-read",
            ) {
                Ok(owner) => owner,
                Err(error) => {
                    SLOT.close();
                    let _ = workspace_utils::process::kill_process_group(&mut child).await;
                    return Err(ExecutorError::Io(error));
                }
            };
            let reader_cancel = CancellationToken::new();
            let mut reader = None;
            use futures::FutureExt;
            let result=std::panic::AssertUnwindSafe(async {
                let stdin=child.inner().stdin.take().ok_or_else(||std::io::Error::other("Missing control stdin"))?;
                let stdout=child.inner().stdout.take().ok_or_else(||std::io::Error::other("Missing control stdout"))?;
                let (peer,task)=ProtocolPeer::spawn_passive(stdin,stdout,reader_cancel.clone());
                reader=Some(task);
                tokio::select! {
                    _=cancel.cancelled()=>Err(ExecutorError::Io(std::io::Error::new(std::io::ErrorKind::Interrupted,"Account read cancelled"))),
                    result=tokio::time::timeout_at(deadline.min(tokio::time::Instant::now()+Duration::from_secs(15)),async {
                        peer.initialize_passive().await?;
                        let usage=peer.get_usage().await?;
                        if !usage.validate() {return Err(ExecutorError::Io(std::io::Error::other("Incompatible usage response")))}
                        Ok(usage)
                    })=>result.map_err(|_|ExecutorError::Io(std::io::Error::new(std::io::ErrorKind::TimedOut,"Account read timed out")))?,
                }
            }).catch_unwind().await.unwrap_or_else(|_|Err(ExecutorError::Io(std::io::Error::other("Account read panic"))));
            reader_cancel.cancel();
            let cleanup = ownership.terminate(&mut child).await;
            if let Some(mut task) = reader
                && tokio::time::timeout(Duration::from_secs(2), &mut task)
                    .await
                    .is_err()
            {
                task.abort();
                if tokio::time::timeout(Duration::from_secs(1), task)
                    .await
                    .is_err()
                {
                    SLOT.close();
                    return Err(ExecutorError::Io(std::io::Error::new(
                        std::io::ErrorKind::TimedOut,
                        "Control reader cleanup timed out",
                    )));
                }
            }
            if cleanup.is_err() {
                SLOT.close();
            }
            cleanup?;
            result
        };
        let result = tokio::spawn(read).await;
        if SLOT.is_closed() {
            return unavailable_code(
                "cleanup_unconfirmed",
                "Claude account reads are disabled because process cleanup could not be confirmed; operator recovery is required",
            );
        }
        match result {
            Ok(Err(ExecutorError::Io(ref error)))
                if error.kind() == std::io::ErrorKind::TimedOut =>
            {
                unavailable_code(
                    "timeout",
                    "Claude account initialization or usage read timed out; owned process cleanup completed",
                )
            }
            Ok(Ok(usage)) => {
                json!({"quota":{"status":if usage.rate_limits_available && usage.rate_limits.is_some() {"fresh"}else{"partial"},"observedAt":chrono::Utc::now(),"rate_limits_available":usage.rate_limits_available,"subscription_type":usage.subscription_type,"data":usage.rate_limits,"reason":if !usage.rate_limits_available {Some("Subscription limits do not apply or profile scope is missing")}else if usage.rate_limits.is_none(){Some("Provider did not return a quota snapshot")}else{None}},"usage":if existing_session {json!({"status":"unsupported","reason":"No safely reusable existing session control handle; passive connection is never resumed to obtain usage","data":null})}else if include_usage {json!({"status":"fresh","observedAt":chrono::Utc::now(),"scope":"information_connection_session_only","data":usage.session,"reason":"This short-lived connection's session totals are not Workspace usage"})}else{json!({"status":"not_requested","data":null})}})
            }
            Ok(Err(ExecutorError::Io(error)))
                if error.kind() == std::io::ErrorKind::Unsupported =>
            {
                json!({"status":"unsupported","reason":"Claude passive usage requires verified CLI 2.1.308 and supported official control methods","data":null})
            }
            _ => unavailable(
                "Claude initialization/get_usage/cleanup failed, timed out, or is unsupported by this CLI/authentication",
            ),
        }
    }
}

/// An experimental method may exist while ignoring a newly added option.
/// Probe only public version metadata and fail closed before get_usage on an
/// unverified CLI. This list can expand after deliberate compatibility testing.
async fn verify_passive_version(
    mut command: Command,
    cancel: CancellationToken,
    deadline: tokio::time::Instant,
) -> Result<(), ExecutorError> {
    use tokio::io::AsyncReadExt;
    if cancel.is_cancelled() || deadline <= tokio::time::Instant::now() {
        return Err(ExecutorError::Io(std::io::Error::new(
            std::io::ErrorKind::Interrupted,
            "Version probe cancelled or timed out",
        )));
    }
    let mut child = command.group_spawn_no_window()?;
    let mut ownership = match workspace_utils::process::OwnedProcessGroup::capture(
        &mut child,
        "claude-usage-version",
    ) {
        Ok(owner) => owner,
        Err(error) => {
            SLOT.close();
            let _ = workspace_utils::process::kill_process_group(&mut child).await;
            return Err(ExecutorError::Io(error));
        }
    };
    let read = async {
        let stdout = child
            .inner()
            .stdout
            .take()
            .ok_or_else(|| std::io::Error::other("Missing version stdout"))?;
        let mut output = Vec::new();
        stdout.take(1024).read_to_end(&mut output).await?;
        let status = ownership.wait(&mut child).await?;
        if status.success()
            && std::str::from_utf8(&output).ok().map(str::trim) == Some("2.1.308 (Claude Code)")
        {
            Ok(())
        } else {
            Err(ExecutorError::Io(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "Unverified Claude CLI version",
            )))
        }
    };
    let result = tokio::select! {
        _=cancel.cancelled()=>Err(ExecutorError::Io(std::io::Error::new(std::io::ErrorKind::Interrupted,"Version probe cancelled"))),
        result=tokio::time::timeout_at(deadline.min(tokio::time::Instant::now()+Duration::from_secs(5)),read)=>match result {
            Ok(result)=>result,
            Err(_)=>Err(ExecutorError::Io(std::io::Error::new(std::io::ErrorKind::TimedOut,"Version probe timed out"))),
        },
    };
    let cleanup = ownership.terminate(&mut child).await;
    if cleanup.is_err() {
        SLOT.close();
    }
    cleanup?;
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn admission_distinguishes_busy_from_unconfirmed_cleanup_without_reopening() {
        let slot = Arc::new(Semaphore::new(1));
        let permit = admit(&slot).unwrap();
        assert_eq!(admit(&slot).unwrap_err()["code"], "in_progress");
        slot.close();
        drop(permit);
        assert_eq!(admit(&slot).unwrap_err()["code"], "cleanup_unconfirmed");
        assert!(slot.is_closed());
    }
    #[test]
    fn experimental_usage_is_nullable_typed_and_never_exports_behaviors() {
        let mut value = json!({"session":{"total_cost_usd":null},"rate_limits_available":false,"rate_limits":null,"behaviors":{"secret":"discard"}});
        let usage: Usage = serde_json::from_value(value.clone()).unwrap();
        assert!(usage.validate());
        assert!(!serde_json::to_string(&usage).unwrap().contains("secret"));
        value["rate_limits"] = json!({"five_hour":{"utilization":101,"resets_at":null}});
        assert!(!serde_json::from_value::<Usage>(value).unwrap().validate());
    }
}
