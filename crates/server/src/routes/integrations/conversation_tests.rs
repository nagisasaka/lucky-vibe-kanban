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
        "wiki-resume",
        "wiki-success",
        "wiki-noop",
        "wiki-unsettled",
        "wiki-missing-journal",
        "wiki-tampered-journal",
        "wiki-before-journal",
        "wiki-receipt-crash",
        "cancel",
        "wiki-cancel",
        "intent-crash",
        "receipt-crash",
    ]
    .into_iter()
    .enumerate()
    {
        eprintln!("isolated integration scenario: {mode}");
        let drift = mode == "drift";
        let wiki_error = mode == "wiki-error";
        let wiki_resume = mode == "wiki-resume";
        let cancel = mode.ends_with("cancel");
        let wiki = mode.starts_with("wiki-");
        let pool = &deployment.db().pool;
        let root = instance.join(mode);
        std::fs::create_dir_all(&root)?;
        let git = |a: &[&str]| support::git(&root, a);
        git(&["init", "-b", "main"])?;
        git(&["config", "user.name", "Fixture"])?;
        git(&["config", "user.email", "fixture@example.invalid"])?;
        std::fs::write(root.join("code"), "base")?;
        std::fs::write(root.join("AGENTS.md"), "# User rules\n")?;
        std::fs::write(root.join("CLAUDE.md"), "@AGENTS.md\n")?;
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
        if wiki {
            let mut configured = IntegrationRun::find(pool, id).await?;
            configured.payload.wiki_enabled = true;
            configured.save(pool).await?;
            // Fixture provisioning only: no live repository or DB is used.
            let persistent =
                utils::path::shared_resources_dir(&repo.name, repo.id).join("persistent");
            std::fs::create_dir_all(&persistent)?;
            let store = RepositoryMemoryStore::at_persistent(&persistent)?;
            store.save_state(&store.state()?)?;
            if mode == "wiki-before-journal" {
                record_wiki_audit(
                    pool,
                    &request,
                    &attempt,
                    &checkout,
                    Uuid::new_v4(),
                    false,
                    true,
                    false,
                )
                .await?;
            }
            // Neither a failed old code SHA nor a later Wiki HEAD is an origin.
            let rejected = call(Uuid::new_v4(), "prepare_wiki", &source, None);
            ensure!(action(deployment, rejected).await.is_err());
            ensure!(store.integration_wiki_setup(id)?.is_none());
            let prepare = call(Uuid::new_v4(), "prepare_wiki", &code_candidate, None);
            if mode != "wiki-missing-journal" {
                action(deployment, prepare.clone()).await?;
                if mode == "wiki-success" {
                    // Crash after durable journal, before acknowledgement: retry
                    // resumes the same receipt without recapturing originals.
                    let mut interrupted = IntegrationRun::find(pool, id).await?;
                    interrupted
                        .payload
                        .history
                        .iter_mut()
                        .find(|v| v["request_id"] == prepare.request_id.to_string())
                        .unwrap()["state"] = serde_json::json!("accepted");
                    interrupted.save(pool).await?;
                }
                action(deployment, prepare).await?;
            }
            let snippet =
                include_str!("../../../../services/src/services/openwiki/setup/agents-0.7.2.txt");
            std::fs::write(
                checkout.join("AGENTS.md"),
                format!("# User rules\n\n{snippet}\n"),
            )?;
        }
        let candidate = if wiki && mode != "wiki-noop" {
            std::fs::create_dir_all(checkout.join("openwiki"))?;
            std::fs::write(checkout.join("openwiki/index.md"), "fixture Wiki")?;
            support::git(&checkout, &["add", "openwiki"])?;
            support::git(&checkout, &["commit", "-m", "Wiki only"])?;
            support::git(&checkout, &["rev-parse", "HEAD"])?
        } else {
            candidate
        };
        let mut publish = call(Uuid::new_v4(), "publish", &candidate, None);
        publish.code_commit = Some(code_candidate.clone());
        if wiki {
            publish.wiki_commit = Some(candidate.clone());
        }
        if mode == "wiki-missing-journal" {
            ensure!(action(deployment, publish).await.is_err());
            ensure!(
                std::fs::read_to_string(checkout.join("AGENTS.md"))?.contains("OPENWIKI:START")
            );
            ensure!(git(&["rev-parse", "main"])? == base);
            // This reserved, unlaunched fixture agent is settled below as well.
            sqlx::query("UPDATE agent_runs SET status='failed' WHERE id=?")
                .bind(agent)
                .execute(pool)
                .await?;
            sqlx::query("UPDATE agent_run_attempts SET status='failed' WHERE agent_run_id=?")
                .bind(agent)
                .execute(pool)
                .await?;
            sqlx::query("UPDATE agent_process_registry SET registry_status='not_started' WHERE run_attempt_id=?").bind(attempt.run_attempt_id).execute(pool).await?;
            continue;
        }
        if !wiki_resume {
            if mode == "wiki-success" {
                // Crash between the initial receipt and complete publication
                // request: replay must finish admission, without a second entry.
                let mut interrupted = IntegrationRun::find(pool, id).await?;
                interrupted.payload.history.push(serde_json::json!({"request_id":publish.request_id,"input":publish,"state":"accepted","candidate_commit":candidate}));
                interrupted.payload.result_commit = Some(candidate.clone());
                interrupted.save(pool).await?;
            }
            action(deployment, publish.clone()).await?;
            ensure!(
                IntegrationRun::find(pool, id)
                    .await?
                    .payload
                    .publication_request
                    .is_some()
            );
            action(deployment, publish).await?;
        }
        ensure!(git(&["rev-parse", "main"])? != candidate); // acceptance never publishes while worker lives
        if mode == "wiki-unsettled" {
            advance(deployment, IntegrationRun::find(pool, id).await?).await?;
            ensure!(
                std::fs::read_to_string(checkout.join("AGENTS.md"))?.contains("OPENWIKI:START")
            );
            ensure!(!IntegrationRun::find(pool, id).await?.payload.published);
        }
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
        if mode == "wiki-unsettled" {
            sqlx::query("UPDATE agent_process_registry SET registry_status='running',pid=999999997,process_started_at=datetime('now','subsec') WHERE run_attempt_id=?").bind(attempt.run_attempt_id).execute(pool).await?;
            advance(deployment, IntegrationRun::find(pool, id).await?).await?;
            ensure!(
                std::fs::read_to_string(checkout.join("AGENTS.md"))?.contains("OPENWIKI:START")
            );
            ensure!(!IntegrationRun::find(pool, id).await?.payload.published);
        }
        sqlx::query("UPDATE agent_process_registry SET registry_status='exited',pid=999999997,process_started_at=datetime('now','subsec'),observed_exited_at=datetime('now','subsec') WHERE run_attempt_id IN (SELECT id FROM agent_run_attempts WHERE agent_run_id=?)").bind(agent).execute(pool).await?;
        if wiki_resume {
            let wiki_id = Uuid::new_v4();
            record_wiki_audit(
                pool, &request, &attempt, &checkout, wiki_id, false, false, false,
            )
            .await?;
            advance(deployment, IntegrationRun::find(pool, id).await?).await?;
            let mut stopped = IntegrationRun::find(pool, id).await?;
            ensure!(stopped.status == "blocked" && !stopped.payload.published);
            resume(deployment, &mut stopped).await?;
            // A later successful Run in the same conversation owns the finish.
            let mut followup = request.clone();
            followup.agent_run_id = Uuid::new_v4();
            followup.request_id = Uuid::new_v4();
            followup.turn_id = Uuid::new_v4();
            followup.idempotency_key = Uuid::new_v4().to_string();
            followup.intent = AgentRunIntent::FollowUp;
            followup.created_at = chrono::Utc::now();
            let mut followup_attempt = attempt.clone();
            followup_attempt.agent_run_id = followup.agent_run_id;
            followup_attempt.turn_id = followup.turn_id;
            followup_attempt.run_attempt_id = Uuid::new_v4();
            followup_attempt.request_id = Uuid::new_v4();
            followup_attempt.idempotency_key = Uuid::new_v4().to_string();
            followup_attempt.created_at = followup.created_at;
            deployment
                .agent_run_port()
                .reserve(followup.clone(), followup_attempt.clone())
                .await?;
            let mut continued_prepare = call(Uuid::new_v4(), "prepare_wiki", &code_candidate, None);
            continued_prepare.agent_run_id = followup.agent_run_id;
            action(deployment, continued_prepare).await?;
            let mut publish = call(Uuid::new_v4(), "publish", &candidate, None);
            publish.agent_run_id = followup.agent_run_id;
            publish.code_commit = Some(code_candidate.clone());
            publish.wiki_commit = Some(candidate.clone());
            action(deployment, publish).await?;
            sqlx::query("UPDATE agent_runs SET status='succeeded' WHERE id=?")
                .bind(followup.agent_run_id)
                .execute(pool)
                .await?;
            sqlx::query("UPDATE agent_run_attempts SET status='succeeded' WHERE id=?")
                .bind(followup_attempt.run_attempt_id)
                .execute(pool)
                .await?;
            sqlx::query("UPDATE agent_process_registry SET registry_status='exited',pid=999999997,process_started_at=datetime('now','subsec'),observed_exited_at=datetime('now','subsec') WHERE run_attempt_id=?").bind(followup_attempt.run_attempt_id).execute(pool).await?;
            record_wiki_audit(
                pool,
                &followup,
                &followup_attempt,
                &checkout,
                wiki_id,
                true,
                true,
                false,
            )
            .await?;
        }
        if wiki && !wiki_error && !wiki_resume && !cancel && mode != "wiki-before-journal" {
            record_wiki_audit(
                pool,
                &request,
                &attempt,
                &checkout,
                Uuid::new_v4(),
                false,
                true,
                mode == "wiki-noop",
            )
            .await?;
            if mode == "wiki-tampered-journal" {
                let store =
                    RepositoryMemoryStore::existing_for_repository(&repo.name, repo.id)?.unwrap();
                let mut journal = store.integration_wiki_setup(id)?.unwrap();
                journal.prepared_at = chrono::Utc::now() + chrono::Duration::seconds(1);
                store.save_integration_wiki_setup(&journal)?;
            }
        }
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
        if mode.ends_with("receipt-crash") {
            sqlx::query(&format!("CREATE TRIGGER fixture_receipt_failure BEFORE UPDATE ON integration_runs WHEN NEW.id=X'{}' AND json_extract(NEW.payload,'$.published')=1 BEGIN SELECT RAISE(FAIL,'injected receipt failure'); END",id.simple())).execute(pool).await?;
        }
        let result = advance(deployment, next).await;
        if mode.ends_with("receipt-crash") {
            ensure!(result.is_err() && git(&["rev-parse", "main"])? == candidate);
            let uncertain = IntegrationRun::find(pool, id).await?;
            ensure!(uncertain.payload.publication_intent && !uncertain.payload.published);
            sqlx::query("DROP TRIGGER fixture_receipt_failure")
                .execute(pool)
                .await?;
            runtime::recover_publication(deployment, uncertain).await?;
        } else if drift
            || wiki_error
            || matches!(mode, "wiki-tampered-journal" | "wiki-before-journal")
        {
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
        if drift || wiki_error || matches!(mode, "wiki-tampered-journal" | "wiki-before-journal") {
            ensure!(
                end.status == "blocked"
                    && !end.payload.published
                    && git(&["rev-parse", "main"])? != candidate
            );
            let ws = Workspace::find_by_id(pool, workspace).await?.unwrap();
            ensure!(!ws.archived);
        } else if cancel {
            if wiki {
                ensure!(
                    std::fs::read_to_string(checkout.join("AGENTS.md"))?.contains("OPENWIKI:START")
                );
                let store =
                    RepositoryMemoryStore::existing_for_repository(&repo.name, repo.id)?.unwrap();
                ensure!(
                    store.integration_wiki_setup(id)?.unwrap().checkpoint.phase
                        == utils::repository_memory::WikiSetupPhase::Prepared
                );
            }
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
            if wiki {
                ensure!(std::fs::read_to_string(root.join("AGENTS.md"))? == "# User rules\n");
                deployment.git().require_clean_source(&root, &candidate)?;
            }
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

/// Audits belong to the isolated instance and are removed with that instance.
#[allow(clippy::too_many_arguments)]
async fn record_wiki_audit(
    pool: &sqlx::SqlitePool,
    request: &executors::runtime::AgentRunRequestEnvelope,
    attempt: &executors::runtime::RunAttemptRequest,
    root: &std::path::Path,
    wiki_id: Uuid,
    resumed: bool,
    finished: bool,
    noop: bool,
) -> anyhow::Result<()> {
    use executors::runtime::*;
    use serde_json::json;
    let mut writer = NativeAuditWriter::create_in(
        utils::assets::asset_dir(),
        NativeAuditMetadata {
            session_id: request.session_id,
            agent_run_id: request.agent_run_id,
            turn_id: request.turn_id,
            run_attempt_id: attempt.run_attempt_id,
            run_attempt_number: attempt.attempt_number,
            provider_id: "codex".into(),
            runtime_profile_id: request.runtime_profile_id.clone(),
            workspace_path: request.workspace.path.clone(),
            runtime_version: None,
            protocol_version: None,
            adapter_version: "fixture".into(),
            mapper_version: "fixture".into(),
            created_at: chrono::Utc::now(),
        },
    )?;
    let mut frames =
        vec![json!({"id":3,"result":{"thread":{"id":"root"},"cwd":request.workspace.path}})];
    let mut calls = vec![
        (
            "openwiki_list_workspaces",
            json!({"root":root}),
            json!({"workspaces":[]}),
        ),
        (
            "openwiki_search",
            json!({"root":root,"query":"contract"}),
            json!({"results":[]}),
        ),
        (
            "openwiki_begin",
            json!({"root":root,"mode":"update"}),
            if noop {
                json!({"root":root,"mode":"update","status":"noop"})
            } else {
                json!({"root":root,"mode":"update","runId":wiki_id,"status":"active","resumed":resumed})
            },
        ),
    ];
    if finished && !noop {
        calls.push((
            "openwiki_finish",
            json!({"runId":wiki_id}),
            json!({"status":"complete"}),
        ));
    }
    calls.push((
        "openwiki_read",
        json!({"root":root,"page":"openwiki/quickstart.md","sections":["contract"]}),
        json!({"sections":[]}),
    ));
    calls.push((
        "openwiki_list_wikis",
        json!({"root":root,"workspace":"fixture"}),
        json!({"wikis":[]}),
    ));
    for (tool, args, result) in calls {
        let call_id = Uuid::new_v4();
        for method in ["item/started", "item/completed"] {
            frames.push(json!({"method":method,"params":{"threadId":"root","item":{
                "id":call_id,"type":"mcpToolCall","server":"openwiki","tool":tool,"arguments":args,
                "result":{"structuredContent":result},"error":null,"status":"completed"
            }}}));
        }
    }
    for frame in frames {
        writer.append_bytes(
            NativeAuditDirection::Output,
            NativeAuditChannel::Stdout,
            "application/json",
            Uuid::new_v4(),
            &serde_json::to_vec(&frame)?,
            None,
        )?;
    }
    let manifest = writer.close()?;
    sqlx::query("INSERT INTO native_audit_streams (id,session_id,agent_run_id,run_attempt_id,audit_schema_version,adapter_version,mapper_version,manifest_relative_path,frames_relative_path,integrity_status) VALUES(?,?,?,?,1,'fixture','fixture',?,?,'complete')")
        .bind(attempt.run_attempt_id).bind(request.session_id).bind(request.agent_run_id).bind(attempt.run_attempt_id)
        .bind(manifest.manifest_relative_path).bind(manifest.frames_relative_path).execute(pool).await?;
    Ok(())
}
