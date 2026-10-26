//! Passive account RPC compatibility projection for rust-v0.164.0.
//! No thread lifecycle, rollout access, billing inference or model requests.
use std::{
    collections::BTreeMap,
    io,
    sync::{Arc, LazyLock},
    time::{Duration, Instant},
};

use async_trait::async_trait;
use codex_app_server_protocol::{
    JSONRPCError, JSONRPCNotification, JSONRPCRequest, JSONRPCResponse,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::sync::{Mutex, Semaphore};

use super::{
    Codex,
    jsonrpc::{JsonRpcCallbacks, JsonRpcControlFlow, JsonRpcPeer},
};
use crate::executors::ExecutorError;

static SLOT: LazyLock<Arc<Semaphore>> = LazyLock::new(|| Arc::new(Semaphore::new(1)));
static LAST_START: Mutex<Option<Instant>> = Mutex::const_new(None);
pub(super) fn poison_admission() {
    SLOT.close();
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Quota {
    pub ordinary_usage_allowed: Option<bool>,
    pub rate_limits: Option<Bucket>,
    pub rate_limits_by_limit_id: Option<BTreeMap<String, Bucket>>,
}
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Bucket {
    pub limit_id: Option<String>,
    pub limit_name: Option<String>,
    pub normal_model_slug: Option<String>,
    pub primary: Option<Window>,
    pub secondary: Option<Window>,
    pub credits: Option<Credits>,
    pub plan_type: Option<String>,
    pub spend_control_reached: Option<bool>,
    pub individual_limit: Option<SpendControlLimit>,
    pub rate_limit_reached_type: Option<String>,
}
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpendControlLimit {
    pub limit: String,
    pub used: String,
    pub remaining_percent: i32,
    pub resets_at: i64,
}
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Window {
    pub used_percent: u32,
    pub window_duration_mins: Option<u64>,
    pub resets_at: Option<i64>,
}
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Credits {
    pub has_credits: bool,
    pub unlimited: bool,
    pub balance: Option<String>,
}
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Usage {
    pub summary: Summary,
    pub daily_usage_buckets: Option<Vec<DailyBucket>>,
    pub thread_usage: Option<ThreadUsage>,
}
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    pub lifetime_tokens: Option<u64>,
    pub peak_daily_tokens: Option<u64>,
    pub longest_running_turn_sec: Option<u64>,
    pub current_streak_days: Option<u64>,
    pub longest_streak_days: Option<u64>,
}
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DailyBucket {
    pub start_date: String,
    pub tokens: u64,
}
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadUsage {
    pub thread_id: String,
    pub estimated_usage_credits_micros: u64,
    pub estimated_usage_usd_micros: Option<u64>,
    pub groups: Vec<UsageGroup>,
}
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageGroup {
    pub model: Option<String>,
    pub reasoning_effort: Option<String>,
    pub speed: Option<String>,
    pub estimated_usage_credits_micros: u64,
    pub net_new_input_tokens: Option<u64>,
    pub cached_input_tokens: Option<u64>,
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub total_tokens: Option<u64>,
}

fn unavailable(reason: &str) -> Value {
    json!({"status":"unavailable","reason":reason,"data":null})
}
fn project<T: Serialize>(result: Result<T, ExecutorError>) -> Value {
    match result {
        Ok(data) => json!({"status":"fresh","observedAt":chrono::Utc::now(),"data":data}),
        Err(ExecutorError::Io(error)) if error.kind() == io::ErrorKind::Unsupported => {
            json!({"status":"unsupported","reason":"Installed CLI does not implement this official RPC","data":null})
        }
        Err(ExecutorError::Io(error)) if error.kind() == io::ErrorKind::TimedOut => {
            unavailable("Official RPC timed out")
        }
        // Never expose provider error text (it can contain account identifiers,
        // paths, URLs or credentials). A method/auth/schema failure is not zero.
        Err(_) => unavailable(
            "Official RPC unavailable: CLI version, authentication, scope or response incompatible",
        ),
    }
}

impl Codex {
    /// Server-selected launcher/profile only. cwd must be an LVK-owned empty
    /// management directory, never a Project or Workspace directory.
    pub async fn read_account(&self, thread_id: Option<String>, include_usage: bool) -> Value {
        if !crate::command::supports_passive_launcher(&self.cmd) || self.oss == Some(true) {
            return json!({"status":"unsupported","reason":"Passive read requires a CLI executable without base arguments, extra parameters or OSS mode","data":null});
        }
        let Ok(permit) = SLOT.clone().try_acquire_owned() else {
            return unavailable(
                "An account read is already in progress; no duplicate process started",
            );
        };
        {
            let mut last = LAST_START.lock().await;
            if last.is_some_and(|t| t.elapsed() < Duration::from_secs(5)) {
                return unavailable("Account reads are limited to one start per five seconds");
            }
            *last = Some(Instant::now());
        }
        let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
        let Ok(directory) = tempfile::Builder::new()
            .prefix("lvk-account-read-")
            .tempdir()
        else {
            return unavailable("Could not create an owned management directory");
        };
        let cwd = directory.path().to_path_buf();
        let result = self
            .with_information_app_server(
                &cwd,
                move |client| async move {
                    let mut quota = project(
                        client
                            .account_read_rpc::<Quota>(
                                "account/rateLimits/read",
                                json!({"excludeResetCreditDetails":true}),
                            )
                            .await.and_then(|quota:Quota| {
                                if quota.rate_limits_by_limit_id.as_ref().is_some_and(|b|b.len()>64) {
                                    Err(ExecutorError::Io(io::Error::new(io::ErrorKind::InvalidData,"Too many quota buckets")))
                                } else {Ok(quota)}
                            }),
                    );
                    if quota["status"] == "fresh" && quota["data"]["rateLimits"].is_null() && quota["data"]["rateLimitsByLimitId"].is_null() {
                        quota["status"] = json!("partial");
                        quota["reason"] = json!("Provider did not report quota buckets");
                    }
                    let usage = if include_usage {
                        let result = client
                            .account_read_rpc::<Usage>(
                                "account/usage/read",
                                json!({"threadId":thread_id}),
                            )
                            .await;
                        let result = result.and_then(|usage| {
                            if usage
                                .daily_usage_buckets
                                .as_ref()
                                .is_some_and(|v| v.len() > 3660)
                                || usage.thread_usage.as_ref().is_some_and(|u| {
                                    u.groups.len() > 256 || Some(&u.thread_id) != thread_id.as_ref()
                                })
                            {
                                Err(ExecutorError::Io(io::Error::other(
                                    "Usage scope or size mismatch",
                                )))
                            } else {
                                Ok(usage)
                            }
                        });
                        let mut result=project(result);
                        result["summary_scope"]=json!("account");
                        result["thread_usage_scope"]=json!(if thread_id.is_some() {"requested_existing_native_thread; parent/child inclusion unknown"} else {"not_requested"});
                        if thread_id.is_some() && result["status"]=="fresh" && result["data"]["threadUsage"].is_null() {
                            result["status"]=json!("partial");
                            result["reason"]=json!("Provider did not return usage for the existing thread; account summary is separate");
                        }
                        result
                    } else {
                        json!({"status":"not_requested","data":null})
                    };
                    Ok(json!({"quota":quota,"usage":usage}))
                },
                Some((permit, directory, deadline)),
            )
            .await;
        match result {
            Ok(data) => data,
            Err(_) => {
                unavailable("Account connection initialization/read/cleanup failed or timed out")
            }
        }
    }
}

/// Deliberately isolated from execution callbacks: notifications cannot launch
/// follow-up turns, approvals, plans or checkpoint compaction. No raw logging.
pub(super) struct PassiveCallbacks;
#[async_trait]
impl JsonRpcCallbacks for PassiveCallbacks {
    async fn on_request(
        &self,
        _: &JsonRpcPeer,
        _: &str,
        _: JSONRPCRequest,
    ) -> Result<(), ExecutorError> {
        Err(ExecutorError::Io(io::Error::other(
            "Unexpected request on passive connection",
        )))
    }
    async fn on_response(
        &self,
        _: &JsonRpcPeer,
        _: &str,
        _: &JSONRPCResponse,
    ) -> Result<(), ExecutorError> {
        Ok(())
    }
    async fn on_error(
        &self,
        _: &JsonRpcPeer,
        _: &str,
        _: &JSONRPCError,
    ) -> Result<(), ExecutorError> {
        Ok(())
    }
    async fn on_notification(
        &self,
        _: &JsonRpcPeer,
        _: &str,
        _: JSONRPCNotification,
    ) -> Result<JsonRpcControlFlow, ExecutorError> {
        Ok(JsonRpcControlFlow::Continue)
    }
    async fn on_non_json(&self, _: &str) -> Result<(), ExecutorError> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn quota_preserves_buckets_nulls_units_and_drops_unknown_private_fields() {
        let q: Quota=serde_json::from_value(json!({"rateLimits":{"primary":{"usedPercent":17,"windowDurationMins":73,"resetsAt":null},"secret":"omit"},"rateLimitsByLimitId":{"other":{"secondary":{"usedPercent":101,"windowDurationMins":10000}}},"accountId":"private"})).unwrap();
        let value = serde_json::to_value(q).unwrap();
        assert_eq!(value["rateLimits"]["primary"]["windowDurationMins"], 73);
        assert!(value["rateLimits"]["primary"]["resetsAt"].is_null());
        assert_eq!(
            value["rateLimitsByLimitId"]["other"]["secondary"]["usedPercent"],
            101
        );
        assert!(!value.to_string().contains("private"));
        assert!(
            serde_json::from_value::<Quota>(json!({"rateLimits":{"primary":{"usedPercent":-1}}}))
                .is_err()
        );
    }
    #[test]
    fn usage_null_is_not_zero_and_cost_units_are_micros() {
        let u:Usage=serde_json::from_value(json!({"summary":{},"threadUsage":{"threadId":"native","estimatedUsageCreditsMicros":1000000,"estimatedUsageUsdMicros":null,"groups":[]}})).unwrap();
        assert_eq!(
            u.thread_usage.unwrap().estimated_usage_credits_micros,
            1000000
        );
        assert!(u.summary.lifetime_tokens.is_none());
        assert!(
            !project::<Usage>(Err(ExecutorError::Io(io::Error::other("secret"))))
                .to_string()
                .contains("secret")
        );
    }
}
