use std::sync::Arc;

use tokio_util::sync::CancellationToken;
use workspace_utils::approvals::ApprovalStatus;

use crate::{
    approvals::{ExecutorApprovalError, ExecutorApprovalService},
    executors::{
        ExecutorError,
        claude::{
            ClaudeJson,
            types::{PermissionResult, PermissionUpdate},
        },
        codex::client::LogWriter,
    },
};

const EXIT_PLAN_MODE_NAME: &str = "ExitPlanMode";
const ENTER_PLAN_MODE_NAME: &str = "EnterPlanMode";
const ASK_USER_QUESTION_NAME: &str = "AskUserQuestion";
pub const AUTO_APPROVE_CALLBACK_ID: &str = "AUTO_APPROVE_CALLBACK_ID";
// Prefix for denial messages from the user, mirrors claude code CLI behavior
const TOOL_DENY_PREFIX: &str = "The user doesn't want to proceed with this tool use. The tool use was rejected (eg. if it was a file edit, the new_string was NOT written to the file). To tell you how to proceed, the user said: ";

/// Claude Agent client with control protocol support
pub struct ClaudeAgentClient {
    log_writer: LogWriter,
    approvals: Option<Arc<dyn ExecutorApprovalService>>,
    auto_approve: bool, // true when approvals is None
    cancel: CancellationToken,
    pending: Arc<tokio::sync::Mutex<usize>>,
}

impl ClaudeAgentClient {
    /// Create a new client with optional approval service
    pub fn new(
        log_writer: LogWriter,
        approvals: Option<Arc<dyn ExecutorApprovalService>>,
        cancel: CancellationToken,
    ) -> Arc<Self> {
        let auto_approve = approvals.is_none();
        Arc::new(Self {
            log_writer,
            approvals,
            auto_approve,
            cancel,
            pending: Default::default(),
        })
    }

    pub(super) fn with_cancel(&self, cancel: CancellationToken) -> Arc<Self> {
        Arc::new(Self {
            log_writer: self.log_writer.clone(),
            approvals: self.approvals.clone(),
            auto_approve: self.auto_approve,
            cancel,
            pending: self.pending.clone(),
        })
    }

    pub(super) fn is_cancelled(&self) -> bool {
        self.cancel.is_cancelled()
    }

    async fn change_pending(&self, add: bool) -> Result<(), ExecutorError> {
        let mut pending = self.pending.lock().await;
        if add {
            *pending += 1;
        } else {
            *pending -= 1;
        }
        self.log_writer
            .log_raw(
                &serde_json::json!({
                    "method":"lucky-vibe/approval/status", "params":{"pending":*pending}
                })
                .to_string(),
            )
            .await
    }

    async fn handle_approval(
        &self,
        tool_use_id: String,
        tool_name: String,
        tool_input: serde_json::Value,
    ) -> Result<PermissionResult, ExecutorError> {
        let approval_service = self
            .approvals
            .as_ref()
            .ok_or(ExecutorApprovalError::ServiceUnavailable)?;

        // Canonical approval details must exist even when the CLI has not
        // emitted an assistant tool_use block yet.
        self.log_writer
            .log_raw(
                &serde_json::json!({
                    "type": "tool_call", "id": tool_use_id, "name": tool_name,
                    "arguments": tool_input, "approval_required": true
                })
                .to_string(),
            )
            .await?;
        let approval_id = match approval_service.create_tool_approval(&tool_name).await {
            Ok(id) => id,
            Err(err) => {
                self.handle_approval_error(&tool_name, &tool_use_id, None, &err)
                    .await?;
                return Err(err.into());
            }
        };

        let _ = self
            .log_writer
            .log_raw(&serde_json::to_string(&ClaudeJson::ApprovalRequested {
                tool_call_id: tool_use_id.clone(),
                tool_name: tool_name.clone(),
                approval_id: approval_id.clone(),
            })?)
            .await;

        self.change_pending(true).await?;
        let outcome = approval_service
            .wait_tool_approval(&approval_id, self.cancel.clone())
            .await;
        self.change_pending(false).await?;
        let status = match outcome {
            Ok(s) => s,
            Err(err) => {
                self.handle_approval_error(&tool_name, &tool_use_id, Some(&approval_id), &err)
                    .await?;
                return Err(err.into());
            }
        };

        self.log_writer
            .log_raw(&serde_json::to_string(&ClaudeJson::ApprovalResponse {
                tool_call_id: tool_use_id.clone(),
                tool_name: tool_name.clone(),
                approval_status: status.clone(),
            })?)
            .await?;

        self.log_writer
            .log_raw(
                &serde_json::json!({
                    "type": "approval_resolved", "approval_id": approval_id,
                    "approved": matches!(status, ApprovalStatus::Approved),
                    "reason": match &status {
                        ApprovalStatus::Denied { reason } => reason.clone(),
                        ApprovalStatus::TimedOut => Some("Approval request timed out".into()),
                        ApprovalStatus::Pending => Some("Approval still pending".into()),
                        ApprovalStatus::Approved => None,
                    }
                })
                .to_string(),
            )
            .await?;
        match status {
            ApprovalStatus::Approved => Ok(PermissionResult::Allow {
                updated_input: tool_input,
                updated_permissions: None,
            }),
            ApprovalStatus::Denied { reason } => Ok(PermissionResult::Deny {
                message: format!("{}{}", TOOL_DENY_PREFIX, reason.unwrap_or_default()),
                interrupt: Some(false),
            }),
            ApprovalStatus::TimedOut => Ok(PermissionResult::Deny {
                message: "Approval request timed out".to_string(),
                interrupt: Some(true),
            }),
            ApprovalStatus::Pending => Ok(PermissionResult::Deny {
                message: "Approval still pending (unexpected)".to_string(),
                interrupt: Some(false),
            }),
        }
    }

    async fn handle_approval_error(
        &self,
        tool_name: &str,
        tool_use_id: &str,
        approval_id: Option<&str>,
        err: &ExecutorApprovalError,
    ) -> Result<(), ExecutorError> {
        if let Some(id) = approval_id {
            let _ = self
                .log_writer
                .log_raw(
                    &serde_json::json!({
                        "type":"approval_resolved", "approval_id":id, "approved":false,
                        "reason":format!("Approval service error: {err}")
                    })
                    .to_string(),
                )
                .await;
        }
        if !matches!(err, ExecutorApprovalError::Cancelled) {
            tracing::error!(
                "Claude approval failed for tool={} call_id={}: {err}",
                tool_name,
                tool_use_id
            );
        }
        let _ = self
            .log_writer
            .log_raw(&serde_json::to_string(&ClaudeJson::ApprovalResponse {
                tool_call_id: tool_use_id.to_string(),
                tool_name: tool_name.to_string(),
                approval_status: ApprovalStatus::Denied {
                    reason: Some(format!("Approval service error: {err}")),
                },
            })?)
            .await;
        Ok(())
    }

    pub async fn resolve_approval(
        &self,
        id: &str,
        approved: bool,
        reason: Option<String>,
    ) -> Result<(), ExecutorError> {
        self.approvals
            .as_ref()
            .ok_or(ExecutorApprovalError::ServiceUnavailable)?
            .resolve_tool_approval(id, approved, reason)
            .await?;
        Ok(())
    }

    pub async fn on_can_use_tool(
        &self,
        tool_name: String,
        input: serde_json::Value,
        _permission_suggestions: Option<Vec<PermissionUpdate>>,
        tool_use_id: Option<String>,
    ) -> Result<PermissionResult, ExecutorError> {
        if matches!(
            tool_name.as_str(),
            ASK_USER_QUESTION_NAME | EXIT_PLAN_MODE_NAME | ENTER_PLAN_MODE_NAME
        ) {
            return Ok(PermissionResult::Deny {
                message: "Claude plan transitions and question UI are not connected".into(),
                interrupt: Some(false),
            });
        }
        if self.auto_approve {
            Ok(PermissionResult::Allow {
                updated_input: input,
                updated_permissions: None,
            })
        } else if let Some(latest_tool_use_id) = tool_use_id.filter(|id| !id.trim().is_empty()) {
            self.handle_approval(latest_tool_use_id, tool_name, input)
                .await
        } else {
            Ok(PermissionResult::Deny {
                message: "Interactive approval requires a tool_use_id".into(),
                interrupt: Some(false),
            })
        }
    }

    pub async fn on_hook_callback(
        &self,
        callback_id: String,
        _input: serde_json::Value,
        _tool_use_id: Option<String>,
    ) -> Result<serde_json::Value, ExecutorError> {
        if self.auto_approve {
            Ok(serde_json::json!({
                "hookSpecificOutput": {
                    "hookEventName": "PreToolUse",
                    "permissionDecision": "allow",
                    "permissionDecisionReason": "Auto-approved by SDK"
                }
            }))
        } else {
            match callback_id.as_str() {
                AUTO_APPROVE_CALLBACK_ID => Ok(serde_json::json!({
                    "hookSpecificOutput": {
                        "hookEventName": "PreToolUse",
                        "permissionDecision": "deny",
                        "permissionDecisionReason": "Plan auto-approval callback is unsupported"
                    }
                })),
                _ => {
                    // Hook callbacks is only used to forward approval requests to can_use_tool.
                    // This works because `ask` decision in hook callback triggers a can_use_tool request
                    // https://docs.claude.com/en/api/agent-sdk/permissions#permission-flow-diagram
                    Ok(serde_json::json!({
                        "hookSpecificOutput": {
                            "hookEventName": "PreToolUse",
                            "permissionDecision": "ask",
                            "permissionDecisionReason": "Forwarding to canusetool service"
                        }
                    }))
                }
            }
        }
    }

    pub async fn log_message(&self, line: &str) -> Result<(), ExecutorError> {
        self.log_writer.log_raw(line).await
    }
}

#[cfg(test)]
mod tests {
    use async_trait::async_trait;
    use workspace_utils::approvals::QuestionStatus;

    use super::*;

    struct OutcomeFixture(ApprovalStatus);
    #[async_trait]
    impl ExecutorApprovalService for OutcomeFixture {
        async fn create_tool_approval(&self, _: &str) -> Result<String, ExecutorApprovalError> {
            Ok("fixture-approval".into())
        }
        async fn create_question_approval(
            &self,
            _: &str,
            _: usize,
        ) -> Result<String, ExecutorApprovalError> {
            Err(ExecutorApprovalError::ServiceUnavailable)
        }
        async fn wait_tool_approval(
            &self,
            _: &str,
            _: CancellationToken,
        ) -> Result<ApprovalStatus, ExecutorApprovalError> {
            Ok(self.0.clone())
        }
        async fn wait_question_answer(
            &self,
            _: &str,
            _: CancellationToken,
        ) -> Result<QuestionStatus, ExecutorApprovalError> {
            Err(ExecutorApprovalError::ServiceUnavailable)
        }
    }

    #[tokio::test]
    async fn timeout_and_unexpected_pending_never_allow_or_change_mode() {
        for status in [ApprovalStatus::TimedOut, ApprovalStatus::Pending] {
            let (writer, _reader) = tokio::io::duplex(16384);
            let client = ClaudeAgentClient::new(
                LogWriter::new(writer),
                Some(Arc::new(OutcomeFixture(status))),
                CancellationToken::new(),
            );
            assert!(matches!(
                client
                    .on_can_use_tool(
                        "Bash".into(),
                        serde_json::json!({"command":"must not execute"}),
                        None,
                        Some("tool-id".into())
                    )
                    .await
                    .unwrap(),
                PermissionResult::Deny { .. }
            ));
        }
    }

    #[tokio::test]
    async fn missing_identity_unavailable_service_plan_and_questions_fail_closed() {
        let (writer, _reader) = tokio::io::duplex(16384);
        let client = ClaudeAgentClient::new(
            LogWriter::new(writer),
            Some(Arc::new(
                crate::approvals::UnavailableExecutorApprovalService,
            )),
            CancellationToken::new(),
        );
        for id in [None, Some(String::new()), Some("  ".into())] {
            assert!(matches!(
                client
                    .on_can_use_tool("Bash".into(), serde_json::json!({}), None, id)
                    .await
                    .unwrap(),
                PermissionResult::Deny { .. }
            ));
        }
        assert!(
            client
                .on_can_use_tool(
                    "Bash".into(),
                    serde_json::json!({}),
                    None,
                    Some("tool-id".into())
                )
                .await
                .is_err()
        );
        for tool in [
            EXIT_PLAN_MODE_NAME,
            ENTER_PLAN_MODE_NAME,
            ASK_USER_QUESTION_NAME,
        ] {
            assert!(matches!(
                client
                    .on_can_use_tool(
                        tool.into(),
                        serde_json::json!({}),
                        None,
                        Some("tool-id".into())
                    )
                    .await
                    .unwrap(),
                PermissionResult::Deny { .. }
            ));
        }
        let hook = client
            .on_hook_callback(AUTO_APPROVE_CALLBACK_ID.into(), serde_json::json!({}), None)
            .await
            .unwrap();
        assert_eq!(hook["hookSpecificOutput"]["permissionDecision"], "deny");
    }
}
