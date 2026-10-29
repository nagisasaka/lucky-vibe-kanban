//! Isolated real Git/SQLite/managed-process hand-off. The provider is reserved,
//! never launched. Requires a task-owned fresh LVK_INSTANCE_DIR.
use db::models::integration::{IntegrationPayload, IntegrationSelection, IntegrationSource};
use executors::{executors::BaseCodingAgent, profile::ExecutorConfig};

use super::*;

#[tokio::test]
#[ignore = "fresh isolated instance only; no provider or live service"]
async fn isolated_conversation_repair_publication_drift_and_idempotence() -> anyhow::Result<()> {
    let instance = PathBuf::from(std::env::var("LVK_INSTANCE_DIR")?);
    ensure!(
        instance
            .file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("lvk257-test-")
    );
    ensure!(!instance.join("data/db.v2.sqlite").exists());
    let shutdown = tokio_util::sync::CancellationToken::new();
    let deployment = DeploymentImpl::new(shutdown.clone()).await?;
    use tower::ServiceExt;
    for route in ["adopt-checkpoint", "decision", "intervene"] {
        let response = super::super::router()
            .with_state(deployment.clone())
            .oneshot(
                axum::http::Request::post(format!("/integrations/{}/{route}", Uuid::new_v4()))
                    .header("content-type", "application/json")
                    .body(axum::body::Body::from("{}"))?,
            )
            .await?;
        ensure!(response.status() == axum::http::StatusCode::NOT_FOUND);
    }
    let result = exercise(&deployment, &instance).await;
    shutdown.cancel();
    result
}

async fn exercise(deployment: &DeploymentImpl, instance: &std::path::Path) -> anyhow::Result<()> {
    for (index, mode) in [
        "success",
        "safe-drift",
        "drift",
        "wiki-error",
        "cancel",
        "intent-crash",
        "receipt-crash",
    ]
    .into_iter()
    .enumerate()
    {
        let drift = mode == "drift";
        let wiki_error = mode == "wiki-error";
        let cancel = mode == "cancel";
        let pool = &deployment.db().pool;
        let root = instance.join(mode);
        std::fs::create_dir_all(&root)?;
        let git = |a: &[&str]| support::git(&root, a);
        git(&["init", "-b", "main"])?;
        git(&["config", "user.name", "Fixture"])?;
        git(&["config", "user.email", "fixture@example.invalid"])?;
        std::fs::write(root.join("code"), "base")?;
        git(&["add", "."])?;
        git(&["commit", "-m", "base"])?;
        let base = git(&["rev-parse", "HEAD"])?;
        git(&["checkout", "-b", "source"])?;
        std::fs::write(root.join("code"), "source")?;
        git(&["commit", "-am", "source"])?;
        let source = git(&["rev-parse", "HEAD"])?;
        git(&["checkout", "main"])?;
        let repo = Repo::find_or_create(pool, &root, "fixture").await?;
        let project = Uuid::new_v4();
        let status = Uuid::new_v4();
        sqlx::query("INSERT INTO projects(id,name,key) VALUES(?,'conversation fixture',?)")
            .bind(project)
            .bind(format!("FIX{index}"))
            .execute(pool)
            .await?;
        for (id, name, order) in [
            (status, "Done", 0),
            (Uuid::new_v4(), "In Progress", 1),
            (Uuid::new_v4(), "In Review", 2),
        ] {
            sqlx::query("INSERT INTO local_project_statuses(id,project_id,name,color,sort_order) VALUES(?,?,?,'blue',?)").bind(id).bind(project).bind(name).bind(order).execute(pool).await?;
        }
        let id = Uuid::new_v4();
        let workspace = Uuid::new_v4();
        let mut run = IntegrationRun {
            id,
            request_key: id.to_string(),
            project_id: project,
            repository_id: repo.id,
            storage_identity: deployment
                .git()
                .storage_identity(&root)?
                .to_string_lossy()
                .into(),
            target_ref: "refs/heads/main".into(),
            status: "queued".into(),
            workspace_id: Some(workspace),
            session_id: None,
            agent_run_id: None,
            payload: sqlx::types::Json(IntegrationPayload {
                observed_target: base.clone(),
                base_commit: Some(base.clone()),
                card_id: Some(Uuid::new_v4()),
                sources: vec![IntegrationSource {
                    selection: IntegrationSelection {
                        card_id: Uuid::new_v4(),
                        workspace_id: Uuid::new_v4(),
                        expected_commit: source.clone(),
                    },
                    branch: "source".into(),
                    commit: source.clone(),
                    title: "source".into(),
                    description: None,
                    status_id: status,
                    requirements_revision: 1,
                    event_ids: vec![],
                    related_workspaces: vec![],
                    done_result: None,
                }],
                executor_config: Some(serde_json::to_value(ExecutorConfig::new(
                    BaseCodingAgent::Codex,
                ))?),
                ..Default::default()
            }),
            cancel_requested: false,
            error: None,
            created_at: String::new(),
            updated_at: String::new(),
        };
        run.create_with_card(pool, &format!("candidate-{id}"), "conversation fixture")
            .await?;
        let ws = Workspace::find_by_id(pool, workspace).await?.unwrap();
        deployment
            .git()
            .create_branch_at_commit(&root, &ws.branch, &base)?;
        let container = deployment.container().ensure_container_exists(&ws).await?;
        let checkout = PathBuf::from(&container).join(&repo.name);
        let session = Session::create(
            pool,
            &CreateSession {
                executor: Some("CODEX".into()),
                name: None,
            },
            Uuid::new_v4(),
            workspace,
        )
        .await?;
        run.session_id = Some(session.id);
        run.status = "integrating".into();
        run.save(pool).await?;
        let AgentRunPortCommand::Create { request, attempt } =
            support::worker_request(&run, container, "fixture only; never launched".into())?
        else {
            unreachable!()
        };
        let agent = request.agent_run_id;
        deployment
            .agent_run_port()
            .reserve(request.clone(), attempt.clone())
            .await?;
        run.agent_run_id = Some(agent);
        run.save(pool).await?;
        support::git(&checkout, &["merge", "--ff-only", &source])?;
        let call = |request_id, action: &str, sha: &str, command: Option<&str>| WorkerRequest {
            workspace_id: workspace,
            agent_run_id: agent,
            request_id,
            action: action.into(),
            expected_commit: Some(sha.into()),
            code_commit: None,
            wiki_commit: None,
            command: command.map(str::to_owned),
            decision_actor: None,
            reason: "fixture decision".into(),
            remaining_risk: "fixture only".into(),
        };
        let failed = call(Uuid::new_v4(), "verify", &source, Some("test -f repaired"));
        action(deployment, failed).await?;
        ensure!(IntegrationRun::find(pool, id).await?.payload.evidence[0].result == "failed");
        std::fs::write(checkout.join("repaired"), "repair")?;
        support::git(&checkout, &["add", "."])?;
        support::git(&checkout, &["commit", "-m", "repair"])?;
        let candidate = support::git(&checkout, &["rev-parse", "HEAD"])?;
        let passed = call(
            Uuid::new_v4(),
            "verify",
            &candidate,
            Some("test -f repaired"),
        );
        action(deployment, passed.clone()).await?;
        action(deployment, passed.clone()).await?; // no second command
        let mut wrong = passed;
        wrong.command = Some("exit 9".into());
        ensure!(action(deployment, wrong).await.is_err());
        let observed = IntegrationRun::find(pool, id).await?;
        ensure!(
            observed.payload.evidence.len() == 2 && observed.payload.evidence[1].result == "passed"
        );
        let mut outside = call(Uuid::new_v4(), "skip", &candidate, None);
        outside.agent_run_id = Uuid::new_v4();
        ensure!(action(deployment, outside).await.is_err());
        if drift {
            std::fs::write(root.join("drift"), "later target")?;
            git(&["add", "."])?;
            git(&["commit", "-m", "target drift"])?;
        }
        if mode == "safe-drift" {
            git(&["merge", "--ff-only", &source])?;
        }
        let code_candidate = candidate.clone();
        let candidate = if wiki_error {
            std::fs::create_dir_all(checkout.join("openwiki"))?;
            std::fs::write(checkout.join("openwiki/index.md"), "fixture Wiki")?;
            support::git(&checkout, &["add", "openwiki"])?;
            support::git(&checkout, &["commit", "-m", "Wiki only"])?;
            let mut configured = IntegrationRun::find(pool, id).await?;
            configured.payload.wiki_enabled = true;
            configured.save(pool).await?;
            support::git(&checkout, &["rev-parse", "HEAD"])?
        } else {
            candidate
        };
        let mut publish = call(Uuid::new_v4(), "publish", &candidate, None);
        publish.code_commit = Some(code_candidate.clone());
        if wiki_error {
            publish.wiki_commit = Some(candidate.clone());
        }
        action(deployment, publish.clone()).await?;
        action(deployment, publish).await?;
        ensure!(git(&["rev-parse", "main"])? != candidate); // acceptance never publishes while worker lives
        if cancel {
            crate::remote_mcp::integration_write_tests::exercise_cancel(
                deployment,
                &IntegrationRun::find(pool, id).await?,
            )
            .await?;
        }
        use executors::runtime::*;
        for (index, status) in [
            AgentRunStatus::Starting,
            AgentRunStatus::Running,
            if cancel {
                AgentRunStatus::Cancelled
            } else {
                AgentRunStatus::Succeeded
            },
        ]
        .into_iter()
        .enumerate()
        {
            db::models::agent_runtime::AgentEventRecord::append_and_project(
                pool,
                &AgentEventEnvelope {
                    schema_version: AGENT_EVENT_SCHEMA_VERSION,
                    payload_version: AGENT_EVENT_PAYLOAD_VERSION,
                    event_id: Uuid::new_v4(),
                    session_id: request.session_id,
                    agent_run_id: agent,
                    turn_id: request.turn_id,
                    run_attempt_id: attempt.run_attempt_id,
                    run_attempt_number: 1,
                    sequence: index as u64 + 1,
                    correlation_id: id,
                    orchestration_run_id: None,
                    orchestration_node_execution_id: None,
                    timestamp: chrono::Utc::now(),
                    native_refs: vec![],
                    payload: AgentEventPayload::LifecycleChanged { status },
                },
            )
            .await?;
        }
        sqlx::query("UPDATE agent_process_registry SET registry_status='exited',pid=999999997,process_started_at=datetime('now','subsec'),observed_exited_at=datetime('now','subsec') WHERE run_attempt_id IN (SELECT id FROM agent_run_attempts WHERE agent_run_id=?)").bind(agent).execute(pool).await?;
        let mut next = IntegrationRun::find(pool, id).await?;
        if mode == "intent-crash" {
            next.status = "validating".into();
            next.save(pool).await?;
            next.payload.publication_intent = true;
            next.status = "publishing".into();
            let mut tx = pool.begin().await?;
            IntegrationRun::record_publication_intent(&mut tx, &next).await?;
            tx.commit().await?;
            runtime::recover_publication(deployment, IntegrationRun::find(pool, id).await?).await?;
            let stopped = IntegrationRun::find(pool, id).await?;
            ensure!(
                stopped.status == "blocked"
                    && !stopped.payload.published
                    && !stopped.payload.publication_intent
            );
            ensure!(git(&["rev-parse", "main"])? == base);
            continue;
        }
        if mode == "receipt-crash" {
            sqlx::query(&format!("CREATE TRIGGER fixture_receipt_failure BEFORE UPDATE ON integration_runs WHEN NEW.id=X'{}' AND json_extract(NEW.payload,'$.published')=1 BEGIN SELECT RAISE(FAIL,'injected receipt failure'); END",id.simple())).execute(pool).await?;
        }
        let result = advance(deployment, next).await;
        if mode == "receipt-crash" {
            ensure!(result.is_err() && git(&["rev-parse", "main"])? == candidate);
            let uncertain = IntegrationRun::find(pool, id).await?;
            ensure!(uncertain.payload.publication_intent && !uncertain.payload.published);
            sqlx::query("DROP TRIGGER fixture_receipt_failure")
                .execute(pool)
                .await?;
            runtime::recover_publication(deployment, uncertain).await?;
        } else if drift || wiki_error {
            ensure!(result.is_err());
            stop(
                deployment,
                IntegrationRun::find(pool, id).await?,
                "main更新により未反映".into(),
            )
            .await?;
        } else {
            result?;
        }
        let end = IntegrationRun::find(pool, id).await?;
        ensure!(end.payload.evidence[0].result == "failed");
        if drift || wiki_error {
            ensure!(
                end.status == "blocked"
                    && !end.payload.published
                    && git(&["rev-parse", "main"])? != candidate
            );
            let ws = Workspace::find_by_id(pool, workspace).await?.unwrap();
            ensure!(!ws.archived);
        } else if cancel {
            ensure!(
                end.status == "cancelled"
                    && !end.payload.published
                    && git(&["rev-parse", "main"])? == base
            );
            let mut resumed = end.clone();
            resume(deployment, &mut resumed).await?;
            ensure!(
                resumed.status == "integrating"
                    && !resumed.cancel_requested
                    && resumed.payload.publication_request.is_none()
            );
            ensure!(
                AgentRunRecord::find(pool, agent).await?.unwrap().status
                    == AgentRunStatus::Cancelled
            );
            let count: i64 =
                sqlx::query_scalar("SELECT COUNT(*) FROM agent_runs WHERE session_id=?")
                    .bind(session.id)
                    .fetch_one(pool)
                    .await?;
            ensure!(
                count == 1,
                "Resume admission must not automatically create another Run"
            );
        } else {
            ensure!(
                end.status == "succeeded"
                    && end.payload.published
                    && git(&["rev-parse", "main"])? == candidate
            );
            let ws = Workspace::find_by_id(pool, workspace).await?.unwrap();
            let detail: Option<String> = sqlx::query_scalar(
                "SELECT attempt_result FROM generated_issue_workspaces WHERE workspace_id=?",
            )
            .bind(workspace)
            .fetch_one(pool)
            .await?;
            ensure!(ws.archived, "archive receipt: {detail:?}");
        }
    }
    anyhow::Ok(())
}
