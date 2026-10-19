use std::sync::atomic::{AtomicUsize, Ordering};

use db::models::{
    agent_runtime::AgentRunRecord,
    session::{CreateSession, Session},
    workspace::{CreateWorkspace, Workspace},
};
use executors::runtime::*;

use super::*;

struct TestPort {
    pool: SqlitePool,
    calls: AtomicUsize,
    entered: tokio::sync::Notify,
    release: tokio::sync::Semaphore,
    pause: bool,
    missing: bool,
}
#[async_trait::async_trait]
impl AgentRunPort for TestPort {
    async fn create(
        &self,
        request: AgentRunRequestEnvelope,
        attempt: RunAttemptRequest,
    ) -> Result<Uuid, AgentRunPortError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.entered.notify_one();
        if self.pause {
            self.release.acquire().await.unwrap().forget();
        }
        AgentRunRecord::persist_identity_before_launch(&self.pool, &request, &attempt)
            .await
            .unwrap();
        Ok(request.agent_run_id)
    }
    async fn query(&self, id: Uuid) -> Result<AgentRunPortSnapshot, AgentRunPortError> {
        if self.missing {
            return Err(AgentRunPortError::NotFound(id));
        }
        let row = AgentRunRecord::find(&self.pool, id)
            .await
            .unwrap()
            .ok_or(AgentRunPortError::NotFound(id))?;
        Ok(AgentRunPortSnapshot {
            agent_run_id: id,
            state: RunState::pending(&row.request_envelope.0),
        })
    }
    async fn control(&self, _: AgentRunPortCommandEnvelope) -> Result<(), AgentRunPortError> {
        panic!("unexpected control")
    }
    async fn subscribe(&self, _: Uuid) -> Result<AgentEventStream, AgentRunPortError> {
        panic!("unexpected subscribe")
    }
}

async fn fixture(pause: bool, missing: bool) -> (OrchestrationService<TestPort>, Arc<TestPort>) {
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::migrate!("../db/migrations").run(&pool).await.unwrap();
    let port = Arc::new(TestPort {
        pool: pool.clone(),
        calls: AtomicUsize::new(0),
        entered: tokio::sync::Notify::new(),
        release: tokio::sync::Semaphore::new(0),
        pause,
        missing,
    });
    (OrchestrationService::new(pool, port.clone()), port)
}

async fn enqueue(service: &OrchestrationService<TestPort>) -> (Uuid, Uuid, Uuid) {
    let pool = &service.pool;
    let workspace = Workspace::create(
        pool,
        &CreateWorkspace {
            branch: Uuid::new_v4().to_string(),
            name: None,
        },
        Uuid::new_v4(),
    )
    .await
    .unwrap();
    let session = Session::create(
        pool,
        &CreateSession {
            executor: None,
            name: None,
        },
        Uuid::new_v4(),
        workspace.id,
    )
    .await
    .unwrap();
    let run = Uuid::new_v4();
    let plan = OrchestrationPlanSnapshot {
        schema_version: 1,
        plan_id: run,
        source_definition_id: run,
        source_definition_version: "test".into(),
        product_kind: OrchestrationProductKind::Integration,
        workspace_mode: WorkspaceMode::IsolatedWorktree,
        created_at: Utc::now(),
        nodes: vec![OrchestrationPlanNode {
            node_key: "integrate".into(),
            stable_order: 0,
            dependencies: vec![],
            join: OrchestrationJoinPolicy::All,
            failure_policy: OrchestrationFailurePolicy::FailFast,
            remaining_upstreams: RemainingUpstreamsPolicy::Continue,
            each_downstream_execution: EachDownstreamExecution::Serial,
            retry: OrchestrationRetryPolicy::default(),
            runtime_profile_id: None,
            provider_id: None,
            provider_config: None,
            requires_product_validation: true,
        }],
    };
    service
        .start_run(run, run, &run.to_string(), run, &plan)
        .await
        .unwrap();
    let (request, attempt) = requests(session.id, workspace.id, &run.to_string());
    let agent = request.agent_run_id;
    let command = AgentRunPortCommandEnvelope {
        schema_version: 1,
        command_id: Uuid::new_v4(),
        idempotency_key: run.to_string(),
        agent_run_id: agent,
        orchestration_run_id: Some(run),
        orchestration_node_execution_id: Some(stable_node_execution_id(run, "integrate", 0)),
        correlation_id: run,
        created_at: Utc::now(),
        command: AgentRunPortCommand::Create { request, attempt },
    };
    let outbox = service.enqueue_command(command).await.unwrap();
    (run, outbox, agent)
}

#[tokio::test]
async fn global_delivery_reproduces_false_start_but_targeted_delivery_accepts_own_agent() {
    let (service, port) = fixture(false, false).await;
    let (old, old_box, _) = enqueue(&service).await;
    sqlx::query("UPDATE orchestration_runs SET status='failed' WHERE id=?")
        .bind(old)
        .execute(&service.pool)
        .await
        .unwrap();
    let (_, unrelated, _) = enqueue(&service).await;
    let (_, own, agent) = enqueue(&service).await;
    // The old caller treated this unrelated successful discard as Started.
    assert!(service.deliver_next().await.unwrap());
    assert!(
        AgentRunRecord::find(&service.pool, agent)
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(port.calls.load(Ordering::SeqCst), 0);
    // Also leave another unrelated live command ahead of our own command.
    sqlx::query("UPDATE orchestration_outbox SET delivery_status='pending' WHERE id=?")
        .bind(old_box)
        .execute(&service.pool)
        .await
        .unwrap();
    let state = service.deliver_create(own, agent).await.unwrap();
    assert_eq!(state.agent_run_id, agent);
    assert_eq!(state.status, AgentRunStatus::Pending);
    assert_eq!(port.calls.load(Ordering::SeqCst), 1);
    service.deliver_create(own, agent).await.unwrap();
    assert_eq!(
        port.calls.load(Ordering::SeqCst),
        1,
        "replay must not create twice"
    );
    let old_status: String =
        sqlx::query_scalar("SELECT delivery_status FROM orchestration_outbox WHERE id=?")
            .bind(old_box)
            .fetch_one(&service.pool)
            .await
            .unwrap();
    assert_eq!(old_status, "pending");
    let unrelated_status: String =
        sqlx::query_scalar("SELECT delivery_status FROM orchestration_outbox WHERE id=?")
            .bind(unrelated)
            .fetch_one(&service.pool)
            .await
            .unwrap();
    assert_eq!(unrelated_status, "pending");
}

#[tokio::test]
async fn concurrent_dispatch_waits_for_acceptance_without_duplicate_create() {
    let (service, port) = fixture(true, false).await;
    let (_, outbox, agent) = enqueue(&service).await;
    let other = OrchestrationService::new(service.pool.clone(), port.clone());
    let delivery = tokio::spawn(async move { other.deliver_next().await });
    port.entered.notified().await;
    let waiting = tokio::spawn(async move { service.deliver_create(outbox, agent).await });
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    assert!(
        !waiting.is_finished(),
        "unaccepted Create must not report Started"
    );
    port.release.add_permits(1);
    delivery.await.unwrap().unwrap();
    assert_eq!(waiting.await.unwrap().unwrap().agent_run_id, agent);
    assert_eq!(port.calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn discarded_create_and_missing_agent_are_not_success() {
    let (service, port) = fixture(false, true).await;
    let (run, outbox, agent) = enqueue(&service).await;
    sqlx::query("UPDATE orchestration_runs SET status='cancelled' WHERE id=?")
        .bind(run)
        .execute(&service.pool)
        .await
        .unwrap();
    assert!(service.deliver_create(outbox, agent).await.is_err());
    assert_eq!(port.calls.load(Ordering::SeqCst), 0);
    let (_, outbox, agent) = enqueue(&service).await;
    assert!(
        matches!(service.deliver_create(outbox, agent).await, Err(OrchestrationServiceError::Port(AgentRunPortError::NotFound(id))) if id == agent)
    );
}

#[tokio::test]
async fn restart_reclaims_stale_delivery_and_identity_mismatch_is_rejected() {
    let (service, port) = fixture(false, false).await;
    let (_, outbox, agent) = enqueue(&service).await;
    assert!(
        service
            .deliver_create(outbox, Uuid::new_v4())
            .await
            .is_err()
    );
    assert_eq!(port.calls.load(Ordering::SeqCst), 0);
    sqlx::query("UPDATE orchestration_outbox SET delivery_status='delivering',updated_at='2000-01-01T00:00:00Z' WHERE id=?").bind(outbox).execute(&service.pool).await.unwrap();
    service.deliver_create(outbox, agent).await.unwrap();
    assert_eq!(port.calls.load(Ordering::SeqCst), 1);
}

fn requests(
    session_id: Uuid,
    workspace_id: Uuid,
    idempotency_key: &str,
) -> (AgentRunRequestEnvelope, RunAttemptRequest) {
    let now = Utc::now();
    let workspace = WorkspaceReference {
        workspace_id,
        mode: WorkspaceMode::SharedWorkspace,
        path: "C:/workspace".to_string(),
    };
    let request = AgentRunRequestEnvelope {
        schema_version: AGENT_REQUEST_SCHEMA_VERSION,
        payload_version: AGENT_REQUEST_PAYLOAD_VERSION,
        request_id: Uuid::new_v4(),
        idempotency_key: idempotency_key.to_string(),
        session_id,
        agent_run_id: Uuid::new_v4(),
        turn_id: Uuid::new_v4(),
        correlation_id: Uuid::new_v4(),
        intent: AgentRunIntent::Initial,
        runtime_profile_id: "CODEX:default".to_string(),
        provider_id: "codex".to_string(),
        workspace: workspace.clone(),
        input: CanonicalMessage {
            message_id: Uuid::new_v4(),
            role: AgentRuntimeMessageRole::User,
            content: "Implement WS1".to_string(),
        },
        created_at: now,
    };
    let attempt = RunAttemptRequest {
        schema_version: AGENT_REQUEST_SCHEMA_VERSION,
        payload_version: AGENT_REQUEST_PAYLOAD_VERSION,
        request_id: Uuid::new_v4(),
        idempotency_key: format!("{idempotency_key}:attempt:1"),
        session_id,
        agent_run_id: request.agent_run_id,
        turn_id: request.turn_id,
        run_attempt_id: Uuid::new_v4(),
        attempt_number: 1,
        correlation_id: request.correlation_id,
        mode: RunAttemptMode::Launch,
        transport: AgentTransportKind::AppServerJsonrpc,
        runtime_profile_id: request.runtime_profile_id.clone(),
        provider_id: request.provider_id.clone(),
        workspace,
        capability_snapshot: CapabilitySnapshot {
            schema_version: 1,
            runtime_profile_id: request.runtime_profile_id.clone(),
            provider_id: request.provider_id.clone(),
            runtime_version: Some("1.0.0".to_string()),
            protocol_version: Some("app-server-v2".to_string()),
            adapter_version: "1".to_string(),
            resolved_at: now,
            capabilities: vec![CapabilitySnapshotEntry {
                capability: AgentCapability::SessionResume,
                state: CapabilityState::Native,
                source: CapabilitySource::VersionProbe,
                emulation_policy: None,
                evidence: None,
            }],
        },
        executor_config: executors::profile::ExecutorConfig {
            executor: executors::executors::BaseCodingAgent::Codex,
            variant: Some("default".to_string()),
            model_id: None,
            agent_id: None,
            reasoning_id: None,
            permission_policy: None,
            execution_mode: None,
            goal_token_budget: None,
            goal_max_concurrent_agents: None,
        },
        selected_skills: None,
        reset_to_message_id: None,
        provider_session: None,
        created_at: now,
    };

    (request, attempt)
}

#[tokio::test]
async fn accepted_create_replay_repairs_link_without_new_agent_identity() {
    let (service, port) = fixture(false, false).await;
    let (run, outbox, agent) = enqueue(&service).await;
    service.deliver_create(outbox, agent).await.unwrap();
    // Crash after port acceptance but before link/receipt persistence.
    sqlx::query("DELETE FROM orchestration_agent_run_links WHERE orchestration_run_id=?")
        .bind(run)
        .execute(&service.pool)
        .await
        .unwrap();
    sqlx::query("UPDATE orchestration_outbox SET delivery_status='delivering',updated_at='2000-01-01T00:00:00Z' WHERE id=?").bind(outbox).execute(&service.pool).await.unwrap();
    service.deliver_create(outbox, agent).await.unwrap();
    assert_eq!(
        port.calls.load(Ordering::SeqCst),
        2,
        "redelivery uses the same idempotent request"
    );
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM agent_runs")
        .fetch_one(&service.pool)
        .await
        .unwrap();
    assert_eq!(count, 1);
}

#[tokio::test]
async fn cancellation_during_create_does_not_report_started() {
    let (service, port) = fixture(true, false).await;
    let (run, outbox, agent) = enqueue(&service).await;
    let pool = service.pool.clone();
    let delivery = tokio::spawn(async move { service.deliver_create(outbox, agent).await });
    port.entered.notified().await;
    sqlx::query("UPDATE orchestration_runs SET status='cancelling' WHERE id=?")
        .bind(run)
        .execute(&pool)
        .await
        .unwrap();
    port.release.add_permits(1);
    assert!(
        matches!(delivery.await.unwrap(), Err(OrchestrationServiceError::DispatchStopped(id)) if id == run)
    );
    assert!(
        AgentRunRecord::find(&pool, agent).await.unwrap().is_some(),
        "accepted identity remains available for cancellation reconciliation"
    );
}

#[tokio::test]
async fn explicit_integration_resume_reuses_discarded_create_and_skip_policy() {
    let (service, port) = fixture(false, false).await;
    let (run, outbox, agent) = enqueue(&service).await;
    let node = stable_node_execution_id(run, "integrate", 0);
    service
        .complete_product_node(run, node, OrchestrationNodeStatus::Failed)
        .await
        .unwrap();
    service.deliver_next().await.unwrap();
    assert_eq!(port.calls.load(Ordering::SeqCst), 0);
    let project = Uuid::new_v4();
    sqlx::query("INSERT INTO projects(id,name) VALUES (?,'resume test')")
        .bind(project)
        .execute(&service.pool)
        .await
        .unwrap();
    let repo = db::models::repo::Repo::find_or_create(
        &service.pool,
        std::path::Path::new("/resume-test"),
        "resume-test",
    )
    .await
    .unwrap();
    // The formal intervention transaction records operator authority before
    // the orchestration can reopen a failed run. No global preference change.
    sqlx::query("INSERT INTO integration_runs(id,request_key,project_id,repository_id,storage_identity,target_ref,status,payload) VALUES (?,?,?,?,?,'refs/heads/main','integrating',?)")
        .bind(run).bind(run.to_string()).bind(project).bind(repo.id).bind("/resume-test/.git")
        .bind(r#"{"publication_intent":false,"intervention":{"pending":true},"skip_validation":true}"#)
        .execute(&service.pool).await.unwrap();
    service.resume_integration(run, node).await.unwrap();
    sqlx::query(
        "UPDATE orchestration_outbox SET delivery_status='pending',available_at=? WHERE id=?",
    )
    .bind(Utc::now())
    .bind(outbox)
    .execute(&service.pool)
    .await
    .unwrap();
    service.deliver_create(outbox, agent).await.unwrap();
    assert_eq!(port.calls.load(Ordering::SeqCst), 1);
    let skip: bool = sqlx::query_scalar(
        "SELECT json_extract(payload,'$.skip_validation') FROM integration_runs WHERE id=?",
    )
    .bind(run)
    .fetch_one(&service.pool)
    .await
    .unwrap();
    assert!(skip);
}
