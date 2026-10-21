//! Permission waiters live with the provider, not the reconnecting web server.
use std::sync::Arc;

use async_trait::async_trait;
use executors::approvals::{
    ExecutorApprovalError, ExecutorApprovalService, NoopExecutorApprovalService,
};
use services::services::approvals::{Approvals, executor_approvals::ExecutorApprovalBridge};
use tokio_util::sync::CancellationToken;
use utils::approvals::{ApprovalStatus, QuestionStatus};
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
        NoopExecutorApprovalService
            .create_question_approval(tool, count)
            .await
    }

    async fn wait_question_answer(
        &self,
        id: &str,
        cancel: CancellationToken,
    ) -> Result<QuestionStatus, ExecutorApprovalError> {
        NoopExecutorApprovalService
            .wait_question_answer(id, cancel)
            .await
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
    async fn legacy_plan_and_question_contracts_are_unchanged() {
        let service = ProcessHostApprovals::new(Uuid::new_v4());
        let plan = service.create_tool_approval("plan").await.unwrap();
        assert!(matches!(
            service
                .wait_tool_approval(&plan, CancellationToken::new())
                .await
                .unwrap(),
            ApprovalStatus::Approved
        ));
        let question = service
            .create_question_approval("question", 1)
            .await
            .unwrap();
        assert!(matches!(
            service
                .wait_question_answer(&question, CancellationToken::new())
                .await,
            Err(ExecutorApprovalError::ServiceUnavailable)
        ));
    }
}
