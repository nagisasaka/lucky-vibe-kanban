use std::sync::Arc;

use codex_app_server_protocol::{JSONRPCNotification, McpServerElicitationRequestParams};
use serde_json::{Value, json};
use tokio::sync::{Mutex, oneshot};
use tokio_util::sync::CancellationToken;
use workspace_utils::approvals::QuestionStatus;

use super::*;
use crate::{
    approvals::{ExecutorApprovalError, ExecutorApprovalService},
    executors::codex::{client::LogWriter, jsonrpc::ExitSignalSender},
    profile::ExecutionMode,
};

struct Human {
    requested: tokio::sync::Notify,
    answer: Mutex<Option<oneshot::Sender<ApprovalStatus>>>,
    receiver: Mutex<Option<oneshot::Receiver<ApprovalStatus>>>,
}
#[async_trait::async_trait]
impl ExecutorApprovalService for Human {
    async fn create_tool_approval(&self, name: &str) -> Result<String, ExecutorApprovalError> {
        assert!(
            name.contains("third-party")
                && name.contains("fixture")
                && name.contains("get_dev_server_status")
                && name.contains("one call only")
        );
        assert!(!name.contains("secret-value"));
        self.requested.notify_one();
        Ok("service-owned-permission".into())
    }
    async fn wait_tool_approval(
        &self,
        _: &str,
        cancel: CancellationToken,
    ) -> Result<ApprovalStatus, ExecutorApprovalError> {
        let rx = self.receiver.lock().await.take().unwrap();
        tokio::select! {biased; _=cancel.cancelled()=>Err(ExecutorApprovalError::Cancelled), v=rx=>v.map_err(|_|ExecutorApprovalError::ServiceUnavailable)}
    }
    async fn resolve_tool_approval(
        &self,
        id: &str,
        approved: bool,
        reason: Option<String>,
    ) -> Result<(), ExecutorApprovalError> {
        if id != "service-owned-permission" {
            return Err(ExecutorApprovalError::ServiceUnavailable);
        }
        self.answer
            .lock()
            .await
            .take()
            .ok_or(ExecutorApprovalError::ServiceUnavailable)?
            .send(if approved {
                ApprovalStatus::Approved
            } else {
                ApprovalStatus::Denied { reason }
            })
            .map_err(|_| ExecutorApprovalError::ServiceUnavailable)
    }
    async fn create_question_approval(
        &self,
        _: &str,
        _: usize,
    ) -> Result<String, ExecutorApprovalError> {
        Err(ExecutorApprovalError::ServiceUnavailable)
    }
    async fn wait_question_answer(
        &self,
        _: &str,
        _: CancellationToken,
    ) -> Result<QuestionStatus, ExecutorApprovalError> {
        Err(ExecutorApprovalError::ServiceUnavailable)
    }
}
fn params() -> Value {
    json!({"threadId":"root","turnId":"turn","serverName":"fixture","mode":"form","_meta":{"codex_approval_kind":"mcp_tool_call","tool_params":{"project_id":"project","workspace_id":"workspace","token":"secret-value"},"persist":["session","always"]},"message":"untrusted free-form label, never displayed as tool identity","requestedSchema":{"type":"object","properties":{}}})
}
fn item(id: &str) -> JSONRPCNotification {
    serde_json::from_value(json!({"method":"item/started","params":{"threadId":"root","turnId":"turn","item":{"type":"mcpToolCall","id":id,"server":"fixture","tool":"get_dev_server_status","status":"inProgress","arguments":params()["_meta"]["tool_params"]}}})).unwrap()
}

#[tokio::test]
async fn mcp_approval_protocol_binds_once_and_fails_closed() {
    for scenario in [
        "approved",
        "denied",
        "cancel",
        "timeout",
        "mismatch",
        "wrong-thread",
        "wrong-turn",
        "unknown",
        "schema",
        "url",
        "ambiguous",
        "replay",
        "missing",
        "noop",
        "auto-approve",
        "wrong-tool",
        "invalid",
        "stale",
        "replacement",
    ] {
        let (tx, rx) = oneshot::channel();
        let human = Arc::new(Human {
            requested: Default::default(),
            answer: Mutex::new(Some(tx)),
            receiver: Mutex::new(Some(rx)),
        });
        let service: Option<Arc<dyn ExecutorApprovalService>> = match scenario {
            "missing" => None,
            "noop" => Some(Arc::new(crate::approvals::NoopExecutorApprovalService)),
            _ => Some(human.clone()),
        };
        let cancel = CancellationToken::new();
        let client = AppServerClient::new(
            LogWriter::new(tokio::io::sink()),
            service,
            scenario == "auto-approve",
            false,
            ExecutionMode::Code,
            None,
            None,
            Default::default(),
            false,
            String::new(),
            cancel.clone(),
        );
        *client.thread_id.lock().await = Some("root".into());
        *client.turn_id.lock().await = Some("turn".into());
        client.observe_mcp_approval_item(&item("actual-item")).await;
        let mut p = params();
        match scenario {
            "wrong-tool" => p["_meta"]["tool_name"] = json!("another_tool"),
            "invalid" => {
                p["requestedSchema"] =
                    json!({"type":"object","properties":{"credential":{"type":"object"}}})
            }
            "mismatch" => p["_meta"]["tool_params"]["workspace_id"] = json!("foreign"),
            "wrong-thread" => p["threadId"] = json!("foreign"),
            "wrong-turn" => p["turnId"] = json!("foreign"),
            "unknown" => p["_meta"]["codex_approval_kind"] = json!("unknown"),
            "schema" => p["requestedSchema"]["properties"] = json!({"password":{"type":"string"}}),
            "url" => {
                p["mode"] = json!("url");
                p["url"] = json!("https://example.invalid/credential");
                p["elicitationId"] = json!("x");
            }
            "ambiguous" => client.observe_mcp_approval_item(&item("second-item")).await,
            _ => {}
        }
        // A tiny transport records the exact response and native string ID.
        let trace = tempfile::NamedTempFile::new().unwrap();
        let script = "import sys,json; f=open(sys.argv[1],'a',buffering=1)\nfor line in sys.stdin: f.write(json.dumps(json.loads(line))+'\\n')";
        let mut child = tokio::process::Command::new("python3")
            .args(["-c", script, trace.path().to_str().unwrap()])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .unwrap();
        let (exit_tx, _) = oneshot::channel();
        let peer = JsonRpcPeer::spawn(
            child.stdin.take().unwrap(),
            child.stdout.take().unwrap(),
            client.clone(),
            ExitSignalSender::new(exit_tx),
            CancellationToken::new(),
        );
        let typed = serde_json::from_value::<McpServerElicitationRequestParams>(p.clone());
        let invoke = async {
            if scenario == "invalid" {
                assert!(typed.is_err());
                let raw =
                    json!({"method":"mcpServer/elicitation/request","id":"native-73","params":p});
                crate::executors::codex::jsonrpc::JsonRpcCallbacks::on_request(
                    &*client,
                    &peer,
                    &raw.to_string(),
                    serde_json::from_value(raw.clone()).unwrap(),
                )
                .await
            } else {
                client
                    .handle_mcp_approval(
                        &peer,
                        RequestId::String("native-73".into()),
                        typed.as_ref().unwrap().clone(),
                    )
                    .await
            }
        };
        let exercise = async {
            if matches!(
                scenario,
                "approved"
                    | "denied"
                    | "cancel"
                    | "timeout"
                    | "replay"
                    | "auto-approve"
                    | "stale"
                    | "replacement"
            ) {
                human.requested.notified().await;
                assert!(
                    std::fs::read_to_string(trace.path()).unwrap().is_empty(),
                    "{scenario}: answered before explicit input"
                );
                if scenario == "replay" {
                    client
                        .handle_mcp_approval(
                            &peer,
                            RequestId::String("native-73".into()),
                            typed.as_ref().unwrap().clone(),
                        )
                        .await
                        .unwrap();
                }
                assert!(
                    human
                        .resolve_tool_approval("native-73", true, None)
                        .await
                        .is_err(),
                    "native ID must not resolve service permission"
                );
                if scenario == "stale" {
                    client.mcp_approvals.lock().unwrap().calls.clear();
                }
                if scenario == "replacement" {
                    client
                        .mcp_approvals
                        .lock()
                        .unwrap()
                        .calls
                        .get_mut("actual-item")
                        .unwrap()
                        .tool = "different".into();
                }
                match scenario {
                    "cancel" => cancel.cancel(),
                    "timeout" => {
                        human
                            .answer
                            .lock()
                            .await
                            .take()
                            .unwrap()
                            .send(ApprovalStatus::TimedOut)
                            .unwrap();
                    }
                    _ => {
                        human
                            .resolve_tool_approval(
                                "service-owned-permission",
                                scenario != "denied",
                                None,
                            )
                            .await
                            .unwrap();
                        assert!(
                            human
                                .resolve_tool_approval("service-owned-permission", true, None)
                                .await
                                .is_err()
                        );
                    }
                }
            }
        };
        tokio::time::timeout(std::time::Duration::from_secs(5), async {
            let (r, ()) = tokio::join!(invoke, exercise);
            r.unwrap();
        })
        .await
        .unwrap();
        for _ in 0..100 {
            if std::fs::metadata(trace.path()).unwrap().len() > 0 {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
        let response: Value = serde_json::from_str(
            std::fs::read_to_string(trace.path())
                .unwrap()
                .lines()
                .next()
                .unwrap(),
        )
        .unwrap();
        assert_eq!(response["id"], "native-73");
        assert_eq!(
            response["result"]["action"],
            match scenario {
                "approved" | "replay" | "auto-approve" => "accept",
                "denied" => "decline",
                _ => "cancel",
            },
            "{scenario}"
        );
        assert_eq!(
            response["result"]["_meta"],
            Value::Null,
            "no persistence grant"
        );
        assert_eq!(response["result"]["content"], Value::Null);
        if let Ok(params) = &typed
            && scenario != "wrong-thread"
        {
            // A fresh native ID for the same active item cannot create a second grant.
            assert!(client.mcp_approvals.lock().unwrap().bind(params).is_none());
        }
        // Same native ID can never create another permission after resolution.
        if scenario != "invalid" {
            client
                .handle_mcp_approval(&peer, RequestId::String("native-73".into()), typed.unwrap())
                .await
                .unwrap();
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        assert_eq!(
            std::fs::read_to_string(trace.path())
                .unwrap()
                .lines()
                .count(),
            1,
            "{scenario}: repeated native response"
        );
        child.kill().await.unwrap();
        child.wait().await.unwrap();
    }
}
