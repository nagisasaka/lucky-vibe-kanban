use std::sync::Arc;

use tokio_util::sync::CancellationToken;
use workspace_utils::approvals::{ApprovalStatus, QuestionStatus};

use super::types::PermissionMode;
use crate::{
    approvals::{ExecutorApprovalError, ExecutorApprovalService},
    executors::{
        ExecutorError,
        claude::{
            ClaudeJson,
            types::{
                PermissionResult, PermissionUpdate, PermissionUpdateDestination,
                PermissionUpdateType,
            },
        },
        codex::client::LogWriter,
    },
};

const EXIT_PLAN_MODE_NAME: &str = "ExitPlanMode";
const ASK_USER_QUESTION_NAME: &str = "AskUserQuestion";
pub const AUTO_APPROVE_CALLBACK_ID: &str = "AUTO_APPROVE_CALLBACK_ID";
// Prefix for denial messages from the user, mirrors claude code CLI behavior
const TOOL_DENY_PREFIX: &str = "The user doesn't want to proceed with this tool use. The tool use was rejected (eg. if it was a file edit, the new_string was NOT written to the file). To tell you how to proceed, the user said: ";

/// Claude Agent client with control protocol support
pub struct ClaudeAgentClient {
    instruction_delivery:
        tokio::sync::Mutex<Option<(String, crate::instruction_delivery::InstructionDelivery)>>,
    log_writer: LogWriter,
    approvals: Option<Arc<dyn ExecutorApprovalService>>,
    auto_approve: bool, // true when approvals is None
    cancel: CancellationToken,
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
            instruction_delivery: tokio::sync::Mutex::new(None),
            log_writer,
            approvals,
            auto_approve,
            cancel,
        })
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

        let approval_id = match approval_service.create_tool_approval(&tool_name).await {
            Ok(id) => id,
            Err(err) => {
                self.handle_approval_error(&tool_name, &tool_use_id, &err)
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

        let status = match approval_service
            .wait_tool_approval(&approval_id, self.cancel.clone())
            .await
        {
            Ok(s) => s,
            Err(err) => {
                self.handle_approval_error(&tool_name, &tool_use_id, &err)
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

        match status {
            ApprovalStatus::Approved => {
                if tool_name == EXIT_PLAN_MODE_NAME {
                    Ok(PermissionResult::Allow {
                        updated_input: tool_input,
                        updated_permissions: Some(vec![PermissionUpdate {
                            update_type: PermissionUpdateType::SetMode,
                            mode: Some(PermissionMode::BypassPermissions),
                            destination: Some(PermissionUpdateDestination::Session),
                            rules: None,
                            behavior: None,
                            directories: None,
                        }]),
                    })
                } else {
                    Ok(PermissionResult::Allow {
                        updated_input: tool_input,
                        updated_permissions: None,
                    })
                }
            }
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

    async fn handle_question(
        &self,
        tool_use_id: String,
        tool_name: String,
        tool_input: serde_json::Value,
    ) -> Result<PermissionResult, ExecutorError> {
        let approval_service = self
            .approvals
            .as_ref()
            .ok_or(ExecutorApprovalError::ServiceUnavailable)?;

        let question_count = tool_input
            .get("questions")
            .and_then(|q| q.as_array())
            .map(|a| a.len())
            .unwrap_or(1);

        let approval_id = match approval_service
            .create_question_approval(&tool_name, question_count)
            .await
        {
            Ok(id) => id,
            Err(err) => {
                self.handle_question_error(&tool_use_id, &tool_name, &err)
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

        let status = match approval_service
            .wait_question_answer(&approval_id, self.cancel.clone())
            .await
        {
            Ok(s) => s,
            Err(err) => {
                self.handle_question_error(&tool_use_id, &tool_name, &err)
                    .await?;
                return Err(err.into());
            }
        };

        self.log_writer
            .log_raw(&serde_json::to_string(&ClaudeJson::QuestionResponse {
                tool_call_id: tool_use_id.clone(),
                tool_name: tool_name.clone(),
                question_status: status.clone(),
            })?)
            .await?;

        match status {
            QuestionStatus::Answered { answers } => {
                let answers_map: serde_json::Map<String, serde_json::Value> = answers
                    .iter()
                    .map(|qa| {
                        (
                            qa.question.clone(),
                            serde_json::Value::String(qa.answer.join(", ")),
                        )
                    })
                    .collect();
                let mut updated = tool_input.clone();
                if let Some(obj) = updated.as_object_mut() {
                    obj.insert(
                        "answers".to_string(),
                        serde_json::Value::Object(answers_map),
                    );
                }
                Ok(PermissionResult::Allow {
                    updated_input: updated,
                    updated_permissions: None,
                })
            }
            QuestionStatus::TimedOut => Ok(PermissionResult::Deny {
                message: "Question request timed out".to_string(),
                interrupt: Some(true),
            }),
        }
    }

    async fn handle_approval_error(
        &self,
        tool_name: &str,
        tool_use_id: &str,
        err: &ExecutorApprovalError,
    ) -> Result<(), ExecutorError> {
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

    async fn handle_question_error(
        &self,
        tool_use_id: &str,
        tool_name: &str,
        err: &ExecutorApprovalError,
    ) -> Result<(), ExecutorError> {
        if !matches!(err, ExecutorApprovalError::Cancelled) {
            tracing::error!("Claude question failed {err}",);
        }
        let _ = self
            .log_writer
            .log_raw(&serde_json::to_string(&ClaudeJson::QuestionResponse {
                tool_call_id: tool_use_id.to_string(),
                tool_name: tool_name.to_string(),
                question_status: QuestionStatus::TimedOut,
            })?)
            .await;
        Ok(())
    }

    pub async fn on_can_use_tool(
        &self,
        tool_name: String,
        input: serde_json::Value,
        _permission_suggestions: Option<Vec<PermissionUpdate>>,
        tool_use_id: Option<String>,
    ) -> Result<PermissionResult, ExecutorError> {
        if tool_name == ASK_USER_QUESTION_NAME {
            if let Some(latest_tool_use_id) = tool_use_id {
                return self
                    .handle_question(latest_tool_use_id, tool_name, input)
                    .await;
            } else {
                tracing::warn!("AskUserQuestion without tool_use_id, cannot route to approval");
                return Ok(PermissionResult::Deny {
                    message:
                        "AskUserQuestion requires user interaction but no tool_use_id was provided"
                            .to_string(),
                    interrupt: Some(false),
                });
            }
        }
        if self.auto_approve {
            Ok(PermissionResult::Allow {
                updated_input: input,
                updated_permissions: None,
            })
        } else if let Some(latest_tool_use_id) = tool_use_id {
            self.handle_approval(latest_tool_use_id, tool_name, input)
                .await
        } else {
            // Auto approve tools with no matching tool_use_id
            // tool_use_id is undocumented so this may not be possible
            tracing::warn!(
                "No tool_use_id available for tool '{}', cannot request approval",
                tool_name
            );
            Ok(PermissionResult::Allow {
                updated_input: input,
                updated_permissions: None,
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
                        "permissionDecision": "allow",
                        "permissionDecisionReason": "Approved by SDK"
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
        self.log_writer.log_raw(line).await?;
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(line)
            && value["type"] == "user"
        {
            let content = value.pointer("/message/content");
            let text = content
                .and_then(|v| v.as_str().map(str::to_owned))
                .or_else(|| {
                    let blocks = content?.as_array()?;
                    if blocks.iter().all(|v| v["type"] == "text") {
                        Some(
                            blocks
                                .iter()
                                .filter_map(|v| v["text"].as_str())
                                .collect::<String>(),
                        )
                    } else {
                        None
                    }
                });
            let mut pending = self.instruction_delivery.lock().await;
            if pending
                .as_ref()
                .is_some_and(|(expected, _)| text.as_ref() == Some(expected))
            {
                let (_, mut delivery) = pending.take().expect("matched pending delivery");
                delivery.status = "accepted".into();
                delivery.native_session_id = value["session_id"].as_str().map(str::to_owned);
                delivery.log(&self.log_writer).await?;
            }
        }
        Ok(())
    }

    pub async fn begin_instruction_delivery(
        &self,
        expected: String,
        additional: String,
    ) -> Result<(), ExecutorError> {
        if additional.is_empty() {
            return Ok(());
        }
        let delivery = crate::instruction_delivery::InstructionDelivery::new(
            "stream-json.user_suffix",
            "user",
            "full_fallback",
            None,
            additional,
        );
        delivery.log(&self.log_writer).await?;
        *self.instruction_delivery.lock().await = Some((expected, delivery));
        Ok(())
    }

    pub async fn finish_instruction_delivery(&self) -> Result<(), ExecutorError> {
        if let Some((_, mut delivery)) = self.instruction_delivery.lock().await.take() {
            delivery.status = "unconfirmed".into();
            delivery.log(&self.log_writer).await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod instruction_delivery_tests {
    use tokio::io::AsyncReadExt;

    use super::*;

    #[tokio::test]
    async fn instruction_receipt_requires_exact_native_echo_not_generic_success() {
        for acknowledged in [false, true] {
            let (writer, mut reader) = tokio::io::duplex(65536);
            let client =
                ClaudeAgentClient::new(LogWriter::new(writer), None, CancellationToken::new());
            client
                .begin_instruction_delivery("human\nfixed".into(), "fixed".into())
                .await
                .unwrap();
            client
                .log_message(r#"{"type":"result","subtype":"success","is_error":false}"#)
                .await
                .unwrap();
            client
                .log_message(r#"{"type":"user","message":{"content":"different"}}"#)
                .await
                .unwrap();
            if acknowledged {
                for _ in 0..2 {
                    client.log_message(r#"{"type":"user","session_id":"native-1","message":{"content":[{"type":"text","text":"human\nfixed"}]}}"#).await.unwrap();
                }
            }
            client.finish_instruction_delivery().await.unwrap();
            drop(client);
            let mut log = String::new();
            reader.read_to_string(&mut log).await.unwrap();
            let facts: Vec<serde_json::Value> = log
                .lines()
                .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
                .filter_map(|v| v.get("lvk_instruction_delivery").cloned())
                .collect();
            assert_eq!(facts.len(), 2);
            assert_eq!(facts[0]["status"], "attempted");
            assert_eq!(
                facts[1]["status"],
                if acknowledged {
                    "accepted"
                } else {
                    "unconfirmed"
                }
            );
            if acknowledged {
                assert_eq!(facts[1]["native_session_id"], "native-1");
            }
        }
    }
}
