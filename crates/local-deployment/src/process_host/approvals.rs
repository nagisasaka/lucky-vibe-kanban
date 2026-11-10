//! Interactive waiters live with the provider, not the reconnecting web server.
use std::sync::Arc;

use async_trait::async_trait;
use executors::approvals::{
    ExecutorApprovalError, ExecutorApprovalService, NoopExecutorApprovalService,
};
use services::services::approvals::{Approvals, executor_approvals::ExecutorApprovalBridge};
use tokio_util::sync::CancellationToken;
use utils::approvals::{ApprovalStatus, QuestionAnswer, QuestionStatus};
use uuid::Uuid;

pub(super) struct ProcessHostApprovals {
    bridge: Arc<ExecutorApprovalBridge>,
}

impl ProcessHostApprovals {
    pub(super) fn new(run_attempt_id: Uuid) -> Arc<Self> {
        Arc::new(Self {
            bridge: ExecutorApprovalBridge::for_process_host(Approvals::new(), run_attempt_id),
        })
    }
}

#[async_trait]
impl ExecutorApprovalService for ProcessHostApprovals {
    async fn create_tool_approval(&self, tool_name: &str) -> Result<String, ExecutorApprovalError> {
        // Plan transition is a separate legacy contract, outside command/file
        // permissions. Preserve it until its own UI/control migration.
        if tool_name == "plan" {
            return NoopExecutorApprovalService
                .create_tool_approval(tool_name)
                .await;
        }
        self.bridge.create_tool_approval(tool_name).await
    }

    async fn wait_tool_approval(
        &self,
        id: &str,
        cancel: CancellationToken,
    ) -> Result<ApprovalStatus, ExecutorApprovalError> {
        if id == "noop" {
            return NoopExecutorApprovalService
                .wait_tool_approval(id, cancel)
                .await;
        }
        self.bridge.wait_tool_approval(id, cancel).await
    }

    async fn resolve_tool_approval(
        &self,
        id: &str,
        approved: bool,
        reason: Option<String>,
    ) -> Result<(), ExecutorApprovalError> {
        self.bridge
            .resolve_tool_approval(id, approved, reason)
            .await
    }

    async fn create_question_approval(
        &self,
        tool: &str,
        count: usize,
    ) -> Result<String, ExecutorApprovalError> {
        self.bridge.create_question_approval(tool, count).await
    }

    async fn create_question_approval_with_timeout(
        &self,
        tool: &str,
        count: usize,
        timeout: Option<std::time::Duration>,
    ) -> Result<String, ExecutorApprovalError> {
        self.bridge
            .create_question_approval_with_timeout(tool, count, timeout)
            .await
    }

    async fn resolve_question_answer(
        &self,
        id: &str,
        answers: Vec<QuestionAnswer>,
    ) -> Result<(), ExecutorApprovalError> {
        self.bridge.resolve_question_answer(id, answers).await
    }

    async fn wait_question_answer(
        &self,
        id: &str,
        cancel: CancellationToken,
    ) -> Result<QuestionStatus, ExecutorApprovalError> {
        self.bridge.wait_question_answer(id, cancel).await
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    #[tokio::test]
    async fn command_and_file_wait_for_explicit_decisions_and_reject_stale_answers() {
        for tool in ["bash", "edit"] {
            for approved in [true, false] {
                let service = ProcessHostApprovals::new(Uuid::new_v4());
                let id = service.create_tool_approval(tool).await.unwrap();
                let cancel = CancellationToken::new();
                let waiting_service = service.clone();
                let waiting_id = id.clone();
                let mut waiter = tokio::spawn(async move {
                    waiting_service
                        .wait_tool_approval(&waiting_id, cancel)
                        .await
                });
                assert!(
                    tokio::time::timeout(Duration::from_millis(30), &mut waiter)
                        .await
                        .is_err()
                );
                assert!(
                    service
                        .resolve_tool_approval("foreign-id", true, None)
                        .await
                        .is_err()
                );
                service
                    .resolve_tool_approval(&id, approved, None)
                    .await
                    .unwrap();
                let outcome = waiter.await.unwrap().unwrap();
                assert_eq!(matches!(outcome, ApprovalStatus::Approved), approved);
                assert!(
                    service
                        .resolve_tool_approval(&id, !approved, None)
                        .await
                        .is_err()
                );
                assert!(
                    service
                        .wait_tool_approval(&id, CancellationToken::new())
                        .await
                        .is_err()
                );
            }
        }
    }

    #[tokio::test]
    async fn cancellation_releases_waiter_and_late_answer_cannot_approve() {
        let service = ProcessHostApprovals::new(Uuid::new_v4());
        let id = service.create_tool_approval("bash").await.unwrap();
        let cancel = CancellationToken::new();
        cancel.cancel();
        assert!(matches!(
            service.wait_tool_approval(&id, cancel).await,
            Err(ExecutorApprovalError::Cancelled)
        ));
        assert!(
            service
                .resolve_tool_approval(&id, true, None)
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn questions_are_isolated_from_permissions_attempts_and_late_answers() {
        let service = ProcessHostApprovals::new(Uuid::new_v4());
        let other = ProcessHostApprovals::new(Uuid::new_v4());
        let id = service
            .create_question_approval("question", 1)
            .await
            .unwrap();
        let answers = vec![QuestionAnswer {
            question: "q".into(),
            answer: vec!["A".into()],
        }];
        assert!(
            service
                .resolve_tool_approval(&id, true, None)
                .await
                .is_err()
        );
        assert!(
            other
                .resolve_question_answer(&id, answers.clone())
                .await
                .is_err()
        );
        service
            .resolve_question_answer(&id, answers.clone())
            .await
            .unwrap();
        assert!(
            service
                .resolve_question_answer(&id, answers.clone())
                .await
                .is_err()
        );
        assert!(matches!(
            service
                .wait_question_answer(&id, CancellationToken::new())
                .await
                .unwrap(),
            QuestionStatus::Answered { .. }
        ));
        let tool = service.create_tool_approval("bash").await.unwrap();
        assert!(
            service
                .resolve_question_answer(&tool, answers.clone())
                .await
                .is_err()
        );
        service
            .resolve_tool_approval(&tool, false, None)
            .await
            .unwrap();
        service
            .wait_tool_approval(&tool, CancellationToken::new())
            .await
            .unwrap();
        for timeout in [false, true] {
            let id = service
                .create_question_approval_with_timeout(
                    "question",
                    1,
                    timeout.then_some(Duration::ZERO),
                )
                .await
                .unwrap();
            let cancel = CancellationToken::new();
            if !timeout {
                cancel.cancel();
            }
            let result = service.wait_question_answer(&id, cancel).await;
            if timeout {
                assert!(matches!(result, Ok(QuestionStatus::TimedOut)));
            } else {
                assert!(matches!(result, Err(ExecutorApprovalError::Cancelled)));
            }
            assert!(
                service
                    .resolve_question_answer(&id, answers.clone())
                    .await
                    .is_err()
            );
        }
    }

    #[test]
    #[ignore = "native question subprocess fixture"]
    fn native_question_fixture() {
        use std::io::{BufRead, Write};

        use serde_json::{Value, json};
        let case = std::env::var("LVK_376_CASE").unwrap();
        let emit = |v: Value| {
            println!("{v}");
            std::io::stdout().flush().unwrap();
        };
        emit(
            json!({"method":"turn/started","params":{"threadId":"root","turn":{"id":"turn","status":"inProgress"}}}),
        );
        let mut questions = vec![
            json!({"id":"native-q1","header":"Color","question":"Which color?","options":[{"label":"Blue","description":"Cool"}]}),
        ];
        if case == "ui" {
            questions.push(
                json!({"id":"native-q2","header":"Other","question":"Constraints?","options":null}),
            );
        }
        let native_id = if case == "mcp" {
            json!("native-request")
        } else {
            json!(73)
        };
        let request = json!({"id":native_id,"method":"item/tool/requestUserInput","params":{
            "threadId":"root","turnId":"turn","itemId":"native-item","isBlocking":case != "nonblocking",
            "autoResolutionMs":if case == "timeout" {json!(1)} else {Value::Null},"questions":questions}});
        emit(request.clone());
        if case == "duplicate" {
            emit(request);
        }
        let line = std::io::stdin()
            .lock()
            .lines()
            .next()
            .expect("original native request was not answered")
            .unwrap();
        let response: Value = serde_json::from_str(&line).unwrap();
        assert_eq!(response["id"], native_id);
        if matches!(case.as_str(), "cancel" | "end" | "timeout") {
            assert_eq!(response["result"]["answers"], json!({}));
        } else {
            assert_eq!(
                response["result"]["answers"]["native-q1"]["answers"],
                json!(["Blue"])
            );
            if case == "ui" {
                assert_eq!(
                    response["result"]["answers"]["native-q2"]["answers"],
                    json!(["None"])
                );
            }
        }
    }

    #[tokio::test]
    async fn host_question_controls_round_trip_to_original_native_request() {
        use executors::{
            executors::{
                codex::{
                    client::{AppServerClient, LogWriter},
                    jsonrpc::{ExitSignalSender, JsonRpcCallbacks, JsonRpcPeer},
                },
                provider_adapter::DirectControl,
            },
            profile::ExecutionMode,
        };
        use serde_json::{Value, json};
        use tokio::io::{AsyncBufReadExt, BufReader};
        for case in [
            "ui",
            "mcp",
            "nonblocking",
            "duplicate",
            "cancel",
            "end",
            "timeout",
        ] {
            tokio::time::timeout(Duration::from_secs(10), async {
                let service = ProcessHostApprovals::new(Uuid::new_v4());
                let cancel = CancellationToken::new();
                let (writer, reader) = tokio::io::duplex(65536);
                let client = AppServerClient::new(LogWriter::new(writer), Some(service.clone()), false, false, ExecutionMode::Code, None, None, cancel.clone());
                client.register_session("root").await.unwrap();
                let mut child = tokio::process::Command::new(std::env::current_exe().unwrap())
                    .args(["--exact","process_host::approvals::tests::native_question_fixture","--ignored","--nocapture"])
                    .env("LVK_376_CASE", case).stdin(std::process::Stdio::piped()).stdout(std::process::Stdio::piped())
                    .kill_on_drop(true).spawn().unwrap();
                let (exit, _exit_rx) = tokio::sync::oneshot::channel();
                let peer = JsonRpcPeer::spawn(child.stdin.take().unwrap(), child.stdout.take().unwrap(), client.clone(), ExitSignalSender::new(exit), cancel.clone());
                client.connect(peer.clone());
                let mut lines = BufReader::new(reader).lines();
                let event: Value = loop {
                    let line = lines.next_line().await.unwrap().unwrap();
                    if let Ok(v) = serde_json::from_str::<Value>(&line)
                        && v["type"] == "input_requested" { break v; }
                };
                assert_eq!(event["provider_metadata"]["id"], if case == "mcp" { json!("native-request") } else { json!(73) });
                assert_eq!(event["provider_metadata"]["params"]["itemId"],"native-item");
                let id = event["input_id"].as_str().unwrap().to_string();
                loop {
                    let line = lines.next_line().await.unwrap().unwrap();
                    if let Ok(v) = serde_json::from_str::<Value>(&line)
                        && v["method"] == "lucky-vibe/question/status" {
                        assert_eq!(v["params"]["pending"], usize::from(case != "nonblocking"));
                        break;
                    }
                }
                if case == "cancel" { cancel.cancel(); }
                else if case == "end" {
                    let notification = serde_json::from_value(json!({"method":"turn/completed","params":{"threadId":"root","turn":{"id":"turn","status":"completed"}}})).unwrap();
                    client.on_notification(&peer, "{}", notification).await.unwrap();
                } else if case != "timeout" {
                    let text = if case == "ui" { json!({"answers":[{"question":"Which color?","answer":["Blue"]},{"question":"Constraints?","answer":["None"]}]}).to_string() } else { "Blue".into() };
                    assert!(client.send_direct_control(DirectControl::Input { request_id:"foreign".into(), text:text.clone() }).await.is_err());
                    if case == "ui" { assert!(client.send_direct_control(DirectControl::Input { request_id:id.clone(),text:"incomplete".into() }).await.is_err()); }
                    client.send_direct_control(DirectControl::Input { request_id:id.clone(),text:text.clone() }).await.unwrap();
                    assert!(client.send_direct_control(DirectControl::Input { request_id:id.clone(),text }).await.is_err());
                }
                if case != "cancel" {
                    loop {
                        let line = lines.next_line().await.unwrap().unwrap();
                        if let Ok(value) = serde_json::from_str::<Value>(&line) {
                            assert_ne!(value["type"], "input_requested", "duplicate native request opened another waiter");
                            if value["type"] == "input_resolved" { break; }
                        }
                    }
                }
                // Parent cancellation closes the JSON-RPC transport, so it is
                // independently covered by the host waiter cancellation test.
                if case == "cancel" {
                    loop {
                        let line = lines.next_line().await.unwrap().unwrap();
                        if serde_json::from_str::<Value>(&line).is_ok_and(|v| v["type"] == "input_resolved") { break; }
                    }
                    child.kill().await.unwrap();
                }
                let status = child.wait().await.unwrap();
                if case != "cancel" { assert!(status.success(), "{case}: {status}"); }
                assert!(service.resolve_question_answer(&id, vec![]).await.is_err());
                cancel.cancel();
            }).await.unwrap();
        }
    }

    #[tokio::test]
    async fn legacy_plan_contract_is_unchanged() {
        let service = ProcessHostApprovals::new(Uuid::new_v4());
        let plan = service.create_tool_approval("plan").await.unwrap();
        assert!(matches!(
            service
                .wait_tool_approval(&plan, CancellationToken::new())
                .await
                .unwrap(),
            ApprovalStatus::Approved
        ));
    }
}
