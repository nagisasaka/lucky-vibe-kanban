use std::{collections::HashMap, sync::Arc, time::Duration};

use async_trait::async_trait;
use db::{self, DBService, models::execution_process::ExecutionProcess};
use executors::approvals::{ExecutorApprovalError, ExecutorApprovalService};
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;
use utils::approvals::{
    ApprovalOutcome, ApprovalRequest, ApprovalResponse, ApprovalStatus, QuestionStatus,
};
use uuid::Uuid;

use crate::services::{approvals::Approvals, notification::NotificationService};

type ApprovalWaiter = futures::future::Shared<futures::future::BoxFuture<'static, ApprovalOutcome>>;

pub struct ExecutorApprovalBridge {
    approvals: Approvals,
    notifications: Option<(DBService, NotificationService)>,
    execution_process_id: Uuid,
    /// Waiters stored between create and wait phases, keyed by approval_id.
    waiters: Mutex<HashMap<String, ApprovalWaiter>>,
}

impl ExecutorApprovalBridge {
    pub fn new(
        approvals: Approvals,
        db: DBService,
        notification_service: NotificationService,
        execution_process_id: Uuid,
    ) -> Arc<Self> {
        Arc::new(Self {
            approvals,
            notifications: Some((db, notification_service)),
            execution_process_id,
            waiters: Mutex::new(HashMap::new()),
        })
    }

    /// The process host owns waiters across application reconnects. Its canonical
    /// event stream supplies the UI, so it does not open the application's DB.
    pub fn for_process_host(approvals: Approvals, run_attempt_id: Uuid) -> Arc<Self> {
        Arc::new(Self {
            approvals,
            notifications: None,
            execution_process_id: run_attempt_id,
            waiters: Mutex::new(HashMap::new()),
        })
    }

    async fn create_internal(
        &self,
        tool_name: &str,
        is_question: bool,
        question_count: Option<usize>,
        timeout: Option<Duration>,
    ) -> Result<String, ExecutorApprovalError> {
        let request = ApprovalRequest::new_with_timeout(
            tool_name.to_string(),
            self.execution_process_id,
            timeout,
        );

        let (request, waiter) = self
            .approvals
            .create_with_waiter(request, is_question)
            .await
            .map_err(ExecutorApprovalError::request_failed)?;

        let approval_id = request.id.clone();

        // Store waiter for the wait phase
        self.waiters
            .lock()
            .await
            .insert(approval_id.clone(), waiter);

        let Some((db, notification_service)) = &self.notifications else {
            return Ok(approval_id);
        };

        let (workspace_name, workspace_id) =
            ExecutionProcess::load_context(&db.pool, self.execution_process_id)
                .await
                .map(|ctx| {
                    let name = ctx
                        .workspace
                        .name
                        .unwrap_or_else(|| ctx.workspace.branch.clone());
                    (name, Some(ctx.workspace.id))
                })
                .unwrap_or_else(|_| ("Unknown workspace".to_string(), None));

        let (title, message) = if let Some(count) = question_count {
            if count == 1 {
                (
                    format!("Question Asked: {}", workspace_name),
                    "1 question requires an answer".to_string(),
                )
            } else {
                (
                    format!("Question Asked: {}", workspace_name),
                    format!("{} questions require answers", count),
                )
            }
        } else {
            (
                format!("Approval Needed: {}", workspace_name),
                format!("Tool '{}' requires approval", tool_name),
            )
        };

        notification_service
            .notify(&title, &message, workspace_id)
            .await;

        Ok(approval_id)
    }

    async fn wait_internal(
        &self,
        approval_id: &str,
        cancel: CancellationToken,
    ) -> Result<ApprovalOutcome, ExecutorApprovalError> {
        let waiter = self
            .waiters
            .lock()
            .await
            .remove(approval_id)
            .ok_or_else(|| {
                ExecutorApprovalError::request_failed(format!(
                    "no waiter found for approval_id={}",
                    approval_id
                ))
            })?;

        let outcome = tokio::select! {
            biased;
            _ = cancel.cancelled() => {
                tracing::info!("Approval request cancelled for approval_id={}", approval_id);
                self.approvals.cancel(approval_id).await;
                return Err(ExecutorApprovalError::Cancelled);
            }
            outcome = waiter => outcome,
        };

        Ok(outcome)
    }
}

#[async_trait]
impl ExecutorApprovalService for ExecutorApprovalBridge {
    async fn resolve_tool_approval(
        &self,
        approval_id: &str,
        approved: bool,
        reason: Option<String>,
    ) -> Result<(), ExecutorApprovalError> {
        self.approvals
            .respond(
                approval_id,
                ApprovalResponse {
                    execution_process_id: self.execution_process_id,
                    status: if approved {
                        ApprovalOutcome::Approved
                    } else {
                        ApprovalOutcome::Denied { reason }
                    },
                },
            )
            .await
            .map_err(ExecutorApprovalError::request_failed)?;
        Ok(())
    }

    async fn create_tool_approval(&self, tool_name: &str) -> Result<String, ExecutorApprovalError> {
        self.create_internal(tool_name, false, None, None).await
    }

    async fn create_question_approval(
        &self,
        tool_name: &str,
        question_count: usize,
    ) -> Result<String, ExecutorApprovalError> {
        self.create_internal(tool_name, true, Some(question_count), None)
            .await
    }

    async fn create_question_approval_with_timeout(
        &self,
        tool_name: &str,
        question_count: usize,
        timeout: Option<Duration>,
    ) -> Result<String, ExecutorApprovalError> {
        self.create_internal(tool_name, true, Some(question_count), timeout)
            .await
    }

    async fn wait_tool_approval(
        &self,
        approval_id: &str,
        cancel: CancellationToken,
    ) -> Result<ApprovalStatus, ExecutorApprovalError> {
        let outcome = self.wait_internal(approval_id, cancel).await?;

        match outcome {
            ApprovalOutcome::Approved => Ok(ApprovalStatus::Approved),
            ApprovalOutcome::Denied { reason } => Ok(ApprovalStatus::Denied { reason }),
            ApprovalOutcome::TimedOut => Ok(ApprovalStatus::TimedOut),
            ApprovalOutcome::Answered { .. } => Err(ExecutorApprovalError::request_failed(
                "unexpected question response for permission request",
            )),
        }
    }

    async fn wait_question_answer(
        &self,
        approval_id: &str,
        cancel: CancellationToken,
    ) -> Result<QuestionStatus, ExecutorApprovalError> {
        let outcome = self.wait_internal(approval_id, cancel).await?;

        match outcome {
            ApprovalOutcome::Answered { answers } => Ok(QuestionStatus::Answered { answers }),
            ApprovalOutcome::TimedOut => Ok(QuestionStatus::TimedOut),
            ApprovalOutcome::Approved | ApprovalOutcome::Denied { .. } => {
                Err(ExecutorApprovalError::request_failed(
                    "unexpected permission response for question request",
                ))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn process_host_timeout_and_early_answer_use_existing_waiters() {
        let bridge = ExecutorApprovalBridge::for_process_host(Approvals::new(), Uuid::new_v4());
        let id = bridge.create_tool_approval("bash").await.unwrap();
        bridge
            .resolve_tool_approval(&id, false, Some("No".into()))
            .await
            .unwrap();
        assert!(
            matches!(bridge.wait_tool_approval(&id, CancellationToken::new()).await.unwrap(), ApprovalStatus::Denied { reason: Some(reason) } if reason == "No")
        );
        let id = bridge
            .create_internal("edit", false, None, Some(Duration::from_millis(1)))
            .await
            .unwrap();
        assert!(matches!(
            bridge
                .wait_tool_approval(&id, CancellationToken::new())
                .await
                .unwrap(),
            ApprovalStatus::TimedOut
        ));
        assert!(bridge.resolve_tool_approval(&id, true, None).await.is_err());
    }

    #[tokio::test]
    async fn cancellation_wins_a_ready_answer_and_does_not_become_approval() {
        let bridge = ExecutorApprovalBridge::for_process_host(Approvals::new(), Uuid::new_v4());
        let id = bridge.create_tool_approval("edit").await.unwrap();
        bridge.resolve_tool_approval(&id, true, None).await.unwrap();
        let cancel = CancellationToken::new();
        cancel.cancel();
        assert!(matches!(
            bridge.wait_tool_approval(&id, cancel).await,
            Err(ExecutorApprovalError::Cancelled)
        ));
        assert!(bridge.resolve_tool_approval(&id, true, None).await.is_err());
    }
}
