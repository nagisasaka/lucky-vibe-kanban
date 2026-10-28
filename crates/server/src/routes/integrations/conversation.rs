//! Conversation-owned work; the host only observes evidence and settles publication.
use std::path::PathBuf;

use anyhow::{Context, ensure};
use axum::{Json, extract::State};
use db::models::{
    agent_runtime::AgentRunRecord,
    execution_process::{ExecutionProcess, ExecutionProcessRunReason, ExecutionProcessStatus},
    integration::{IntegrationEvidence, IntegrationPublicationRequest, IntegrationRun},
    repo::Repo,
    session::{CreateSession, Session},
    workspace::Workspace,
};
use deployment::Deployment;
use executors::{
    actions::{
        ExecutorAction, ExecutorActionType,
        script::{ScriptContext, ScriptRequest, ScriptRequestLanguage},
    },
    runtime::{AgentRunPort, AgentRunPortCommand},
};
use serde::{Deserialize, Serialize};
use services::services::container::ContainerService;
use utils::{repository_memory::RepositoryMemoryStore, response::ApiResponse};
use uuid::Uuid;

use super::{admission, mechanical, runtime};
use crate::{DeploymentImpl, error::ApiError};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct WorkerRequest {
    pub workspace_id: Uuid,
    pub agent_run_id: Uuid,
    pub request_id: Uuid,
    /// inspect, verify, skip, or publish. No progress planner lives here.
    pub action: String,
    pub expected_commit: Option<String>,
    pub code_commit: Option<String>,
    pub wiki_commit: Option<String>,
    pub command: Option<String>,
    /// Attribution of a skip decision, not an execution-approval identity.
    pub decision_actor: Option<String>,
    #[serde(default)]
    pub reason: String,
    #[serde(default)]
    pub remaining_risk: String,
}

pub(crate) async fn worker(
    State(deployment): State<DeploymentImpl>,
    Json(request): Json<WorkerRequest>,
) -> Result<Json<ApiResponse<serde_json::Value>>, ApiError> {
    action(&deployment, request)
        .await
        .map(|v| Json(ApiResponse::success(v)))
        .map_err(super::error)
}

/// Bound to the calling runtime, not a user-selected integration or repository.
pub(crate) async fn action(
    deployment: &DeploymentImpl,
    request: WorkerRequest,
) -> anyhow::Result<serde_json::Value> {
    let pool = &deployment.db().pool;
    let mut run: IntegrationRun = sqlx::query_as("SELECT * FROM integration_runs WHERE workspace_id=? AND json_extract(payload,'$.conversational')=1")
        .bind(request.workspace_id).fetch_one(pool).await?;
    let _operation = runtime::reserve_intervention(run.id)?;
    let agent = AgentRunRecord::find(pool, request.agent_run_id)
        .await?
        .context("Worker missing")?;
    let latest: Uuid = sqlx::query_scalar(
        "SELECT id FROM agent_runs WHERE session_id=? ORDER BY created_at DESC,id DESC LIMIT 1",
    )
    .bind(agent.session_id)
    .fetch_one(pool)
    .await?;
    ensure!(latest == agent.id, "Stale worker");
    ensure!(
        agent.workspace_id == request.workspace_id
            && Some(agent.session_id) == run.session_id
            && agent.correlation_id == run.id,
        "Worker ownership mismatch"
    );
    // Immutable request receipt in existing history, persisted before any process side effect.
    let input = serde_json::to_value(&request)?;
    if let Some(old) = run
        .payload
        .history
        .iter()
        .find(|v| v["request_id"] == request.request_id.to_string())
    {
        ensure!(old["input"] == input, "Idempotency conflict");
        let receipt = old.clone();
        if request.action == "verify"
            && !run
                .payload
                .evidence
                .iter()
                .any(|e| e.request_id == request.request_id)
            && let Some(process) = receipt["process_id"]
                .as_str()
                .and_then(|id| Uuid::parse_str(id).ok())
        {
            let process = ExecutionProcess::find_by_id(pool, process)
                .await?
                .context("Process receipt missing")?;
            if process.status != ExecutionProcessStatus::Running
                && deployment
                    .container()
                    .scripts_settled(request.workspace_id)
                    .await?
            {
                let expected = request
                    .expected_commit
                    .as_deref()
                    .context("Missing candidate")?;
                let (_, _, _, root) = runtime::context(deployment, &run).await?;
                deployment.git().require_clean_source(&root, expected)?;
                let (before, after): (Option<String>, Option<String>) = sqlx::query_as("SELECT before_head_commit,after_head_commit FROM execution_process_repo_states WHERE execution_process_id=? AND repo_id=?").bind(process.id).bind(run.repository_id).fetch_one(pool).await?;
                ensure!(
                    before.as_deref() == Some(expected) && after.as_deref() == Some(expected),
                    "Verification SHA mismatch"
                );
                run.payload.evidence.push(IntegrationEvidence {
                    request_id: request.request_id,
                    agent_run_id: agent.id,
                    code_commit: expected.into(),
                    command: request.command,
                    process_id: Some(process.id),
                    result: if process.status == ExecutionProcessStatus::Completed
                        && process.exit_code == Some(0)
                    {
                        "passed"
                    } else {
                        "failed"
                    }
                    .into(),
                    actor: "host_execution".into(),
                    reason: request.reason,
                    remaining_risk: request.remaining_risk,
                });
                let entry = run
                    .payload
                    .history
                    .iter_mut()
                    .find(|v| v["request_id"] == request.request_id.to_string())
                    .unwrap();
                entry["state"] = serde_json::json!("recorded");
                run.save(pool).await?;
                return Ok(
                    serde_json::json!({"operation":receipt,"evidence":run.payload.evidence.last(),"logs":log_tail(deployment,process.id).await}),
                );
            }
        }
        if let Some(evidence) = run
            .payload
            .evidence
            .iter()
            .find(|e| e.request_id == request.request_id)
        {
            let logs = if let Some(process) = evidence.process_id {
                log_tail(deployment, process).await
            } else {
                serde_json::Value::Null
            };
            return Ok(serde_json::json!({"operation":receipt,"evidence":evidence,"logs":logs}));
        }
        return Ok(receipt);
    }
    ensure!(
        !agent.status.is_terminal()
            && !run.cancel_requested
            && !run.payload.publication_intent
            && run.payload.publication_request.is_none(),
        "Worker no longer owns an editable integration"
    );
    ensure!(
        run.status == "integrating",
        "Integration is not accepting worker operations"
    );
    let (repo, workspace, session, root) = runtime::context(deployment, &run).await?;
    mechanical::require_private_branch(&root, &workspace.branch, &run.target_ref)?;
    if request.action == "inspect" {
        return Ok(
            serde_json::json!({"integration_id":run.id,"sources":run.payload.sources,"base_commit":run.payload.observed_target,"candidate_commit":run.payload.result_commit,"evidence":run.payload.evidence,"wiki_enabled":run.payload.wiki_enabled,"history":run.payload.history.iter().map(|entry|serde_json::json!({"request_id":entry["request_id"],"state":entry["state"],"process_id":entry["process_id"],"candidate_commit":entry["candidate_commit"],"event":entry["event"]})).collect::<Vec<_>>(),"publication":run.payload.publication_request}),
        );
    }
    let expected = request
        .expected_commit
        .as_deref()
        .context("expected_commit required")?;
    deployment.git().require_clean_source(&root, expected)?;
    ensure!(
        deployment
            .git()
            .is_ancestor(&root, &run.payload.observed_target, expected)?,
        "Candidate lost the admitted base"
    );
    for source in &run.payload.sources {
        ensure!(
            deployment
                .git()
                .is_ancestor(&root, &source.commit, expected)?,
            "Candidate lost a selected source"
        );
    }
    let user_skip = run.payload.skip_validation;
    if !run.payload.wiki_enabled {
        services::services::repository_memory::guard_normal_workspace(
            deployment.git(),
            &root,
            &run.payload.observed_target,
        )?;
    }
    let receipt = serde_json::json!({"request_id":request.request_id,"input":input,"state":"accepted","candidate_commit":expected});
    ensure!(
        matches!(request.action.as_str(), "verify" | "skip" | "publish"),
        "Unknown worker action"
    );
    if request.action != "publish" {
        ensure!(
            !request.reason.trim().is_empty(),
            "Record the verification/skip reason"
        );
        if request.action == "verify" {
            ensure!(
                request
                    .command
                    .as_ref()
                    .is_some_and(|s| !s.trim().is_empty()),
                "command required"
            );
        }
        if request.action == "skip" {
            ensure!(
                request
                    .decision_actor
                    .as_deref()
                    .is_none_or(|actor| matches!(actor, "user" | "agent")),
                "Unknown skip decision actor"
            );
            ensure!(
                !request.remaining_risk.trim().is_empty(),
                "Record remaining risk when skipping"
            );
        }
    }
    run.payload.history.push(receipt);
    run.payload.result_commit = Some(expected.into());
    run.save(pool).await?;
    match request.action.as_str() {
        "verify" => {
            let command = request.command.clone().unwrap();
            let action = ExecutorAction::new(
                ExecutorActionType::ScriptRequest(ScriptRequest {
                    script: command.clone(),
                    language: ScriptRequestLanguage::Bash,
                    context: ScriptContext::IntegrationValidation,
                    working_dir: Some(repo.name.clone()),
                }),
                None,
            );
            let process = deployment
                .container()
                .start_execution(
                    &workspace,
                    &session,
                    &action,
                    &ExecutionProcessRunReason::IntegrationValidation,
                )
                .await?;
            // Persist process identity before waiting; redelivery never launches another process.
            run.payload.history.last_mut().unwrap()["process_id"] = serde_json::json!(process.id);
            run.save(pool).await?;
            let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(3600);
            let finished = loop {
                let p = ExecutionProcess::find_by_id(pool, process.id)
                    .await?
                    .context("Verification process missing")?;
                if p.status != ExecutionProcessStatus::Running
                    && deployment.container().scripts_settled(workspace.id).await?
                {
                    break p;
                }
                if IntegrationRun::find(pool, run.id).await?.cancel_requested
                    || AgentRunRecord::find(pool, agent.id)
                        .await?
                        .is_none_or(|a| a.status.is_terminal())
                    || tokio::time::Instant::now() > deadline
                {
                    deployment
                        .container()
                        .stop_execution(&p, ExecutionProcessStatus::Killed)
                        .await?;
                    anyhow::bail!(
                        "Verification cancelled/timed out; inspect retained process evidence"
                    );
                }
                tokio::time::sleep(std::time::Duration::from_millis(200)).await;
            };
            deployment.git().require_clean_source(&root, expected)?;
            let (before, after): (Option<String>, Option<String>) = sqlx::query_as("SELECT before_head_commit,after_head_commit FROM execution_process_repo_states WHERE execution_process_id=? AND repo_id=?")
                .bind(process.id).bind(repo.id).fetch_one(pool).await?;
            ensure!(
                before.as_deref() == Some(expected) && after.as_deref() == Some(expected),
                "Verification SHA mismatch"
            );
            let result = if finished.status == ExecutionProcessStatus::Completed
                && finished.exit_code == Some(0)
            {
                "passed"
            } else {
                "failed"
            };
            run.payload.evidence.push(IntegrationEvidence {
                request_id: request.request_id,
                agent_run_id: agent.id,
                code_commit: expected.into(),
                command: Some(command),
                process_id: Some(process.id),
                result: result.into(),
                actor: "host_execution".into(),
                reason: request.reason,
                remaining_risk: request.remaining_risk,
            });
        }
        "skip" => run.payload.evidence.push(IntegrationEvidence {
            request_id: request.request_id,
            agent_run_id: agent.id,
            code_commit: expected.into(),
            command: request.command,
            process_id: None,
            result: "skipped".into(),
            actor: request
                .decision_actor
                .unwrap_or_else(|| if user_skip { "user" } else { "agent" }.into()),
            reason: request.reason,
            remaining_risk: request.remaining_risk,
        }),
        "publish" => {
            ensure!(
                !request.reason.trim().is_empty(),
                "Describe the integrated changes and any conflict repairs before publication"
            );
            run.payload.semantic_summary = Some(request.reason.clone());
            ensure!(
                !deployment.queued_message_service().has_queued(session.id),
                "A user instruction is queued; finish this Run without publication so it can continue in the same conversation"
            );
            ensure!(
                crate::routes::sessions::questions::pending(pool, session.id)
                    .await?
                    .is_empty(),
                "Answer outstanding questions before ending this Run for publication"
            );
            let code = request
                .code_commit
                .clone()
                .context("code_commit required")?;
            ensure!(
                run.payload.evidence.iter().any(|e| e.code_commit == code),
                "Record code verification or a reasoned skip first"
            );
            ensure!(
                deployment.git().is_ancestor(&root, &code, expected)?,
                "Code candidate is not in final candidate"
            );
            if run.payload.wiki_enabled {
                // Part of the existing publication identity check, not a new
                // Wiki validation stage: never relabel a repaired code tree as
                // the Wiki continuation of an earlier tested code SHA.
                let paths = mechanical::git(
                    &root,
                    &["diff", "--no-renames", "--name-only", "-z", &code, expected],
                )?;
                ensure!(
                    paths
                        .split('\0')
                        .filter(|p| !p.is_empty())
                        .all(services::services::repository_memory::canonical_wiki_path),
                    "Final candidate does not preserve the recorded code tree; record the repaired code candidate before Wiki publication"
                );
                ensure!(
                    request.wiki_commit.as_deref() == Some(expected),
                    "Wiki completion must identify the final candidate (or unchanged code SHA)"
                );
            } else {
                ensure!(
                    code == expected && request.wiki_commit.is_none(),
                    "Wiki disabled: code SHA must be the final SHA"
                );
            }
            run.payload.publication_request = Some(IntegrationPublicationRequest {
                request_id: request.request_id,
                agent_run_id: agent.id,
                code_commit: code,
                wiki_commit: request.wiki_commit,
                candidate_commit: expected.into(),
            });
            run.payload.history.last_mut().unwrap()["state"] =
                serde_json::json!("pending_writer_settlement");
        }
        _ => unreachable!(),
    }
    if request.action != "publish" {
        run.payload.history.last_mut().unwrap()["state"] = serde_json::json!("recorded");
    }
    run.save(pool).await?;
    let logs = if let Some(process) = run
        .payload
        .evidence
        .last()
        .filter(|e| e.request_id == request.request_id)
        .and_then(|e| e.process_id)
    {
        log_tail(deployment, process).await
    } else {
        serde_json::Value::Null
    };
    Ok(
        serde_json::json!({"logs":logs,"operation":run.payload.history.last(),"evidence":run.payload.evidence.last(),"instruction":if request.action == "publish" { "公開要求を保存しました。追加の質問を残さず、このRunを終了してください。Run/child/writer収束後にLVKが公開し、結果を会話に記録します。" } else { "結果を確認し、必要なら同じ会話で修復してください。" }}),
    )
}

pub(super) async fn advance(
    deployment: &DeploymentImpl,
    mut run: IntegrationRun,
) -> anyhow::Result<()> {
    let pool = &deployment.db().pool;
    if run.cancel_requested {
        let latest: Option<Uuid> = sqlx::query_scalar(
            "SELECT id FROM agent_runs WHERE session_id=? ORDER BY created_at DESC,id DESC LIMIT 1",
        )
        .bind(run.session_id)
        .fetch_optional(pool)
        .await?;
        if latest.is_some() {
            run.agent_run_id = latest;
            run.save(pool).await?;
        }
        return runtime::cancel(deployment, run).await;
    }
    if matches!(run.status.as_str(), "publishing" | "post_processing") {
        return runtime::recover_publication(deployment, run).await;
    }
    if run.status == "queued" {
        // No repository-wide queue: independent work starts at the admitted SHA.
        run.payload.base_commit = Some(run.payload.observed_target.clone());
        let workspace = Workspace::find_by_id(pool, run.workspace_id.unwrap())
            .await?
            .context("Workspace missing")?;
        if run.session_id.is_none() {
            let session = Session::create(
                pool,
                &CreateSession {
                    executor: Some("CODEX".into()),
                    name: Some("Auto-merge".into()),
                },
                Uuid::new_v4(),
                workspace.id,
            )
            .await?;
            run.session_id = Some(session.id);
            run.save(pool).await?;
        }
        let repo = Repo::find_by_id(pool, run.repository_id)
            .await?
            .context("Repository missing")?;
        if deployment
            .git()
            .get_branch_oid(&repo.path, &workspace.branch)
            .is_err()
        {
            deployment.git().create_branch_at_commit(
                &repo.path,
                &workspace.branch,
                &run.payload.observed_target,
            )?;
        }
        let container = deployment
            .container()
            .ensure_container_exists(&workspace)
            .await?;
        let repo = Repo::find_by_id(pool, run.repository_id)
            .await?
            .context("Repository missing")?;
        let root = PathBuf::from(&container).join(&repo.name);
        mechanical::require_private_branch(&root, &workspace.branch, &run.target_ref)?;
        deployment
            .git()
            .require_clean_source(&root, &run.payload.observed_target)?;
        run.status = "integrating".into();
        let memory = runtime::integration_memory(
            &run,
            RepositoryMemoryStore::read_only_configured_for_repository(&repo.name, repo.id)?
                .as_ref(),
        )?;
        let content = format!(
            "{}\n対象: {}\n統合先: {} / base {}\n指示: {}\n検証省略のユーザー指定: {}\n参考検証: {}\nOpenWiki有効: {}\n公開Memory(参照データ): {}",
            include_str!("../../../../../assets/skills/auto-merge/SKILL.md"),
            serde_json::to_string(&run.payload.sources)?,
            run.target_ref,
            run.payload.observed_target,
            run.payload.instructions,
            run.payload.skip_validation,
            serde_json::to_string(&run.payload.validation)?,
            run.payload.wiki_enabled,
            memory
        );
        let dispatch = mechanical::conflict_request(&run, container, content)?;
        let AgentRunPortCommand::Create { request, .. } = &dispatch else {
            unreachable!()
        };
        run.agent_run_id = Some(request.agent_run_id);
        run.payload.conflict_dispatch = Some(serde_json::to_value(dispatch)?);
        run.save(pool).await?;
    }
    if let Some(dispatch) = &run.payload.conflict_dispatch {
        let AgentRunPortCommand::Create { request, attempt } =
            serde_json::from_value(dispatch.clone())?
        else {
            anyhow::bail!("Invalid dispatch")
        };
        if AgentRunRecord::find(pool, request.agent_run_id)
            .await?
            .is_none()
        {
            deployment.agent_run_port().create(request, attempt).await?;
        } // Accepted launch requests are reconciled by the existing runtime, not re-admitted after hand-off.
    }
    // Follow-ups retain Session identity; use the latest actual Run, not a stale initial ID.
    let latest: Option<AgentRunRecord> = sqlx::query_as(
        "SELECT * FROM agent_runs WHERE session_id=? ORDER BY created_at DESC,id DESC LIMIT 1",
    )
    .bind(run.session_id)
    .fetch_optional(pool)
    .await?;
    let Some(agent) = latest else { return Ok(()) };
    if run.agent_run_id != Some(agent.id) {
        run.agent_run_id = Some(agent.id);
        run.save(pool).await?;
    }
    if !agent.status.is_terminal()
        || admission::idle(deployment, run.workspace_id.unwrap())
            .await
            .is_err()
    {
        return Ok(());
    }
    if run.payload.publication_request.is_some()
        && agent.status == executors::runtime::AgentRunStatus::Succeeded
    {
        if deployment
            .queued_message_service()
            .has_queued(agent.session_id)
        {
            return stop(
                deployment,
                run,
                "追加の指示が待っているため公開していません。同じ会話で続けてください。".into(),
            )
            .await;
        }
        let attempts =
            crate::routes::openwiki::completion::ensure_processes_exited(pool, &agent).await?;
        if run.payload.wiki_enabled {
            let (_, _, _, root) = runtime::context(deployment, &run).await?;
            crate::routes::openwiki::completion::writer_completion_proof(
                pool,
                &agent,
                &attempts,
                run.workspace_id.unwrap(),
                std::path::Path::new(&agent.workspace_path),
                &root,
                services::services::openwiki::completion::WriterPhase::Sync,
            )
            .await?
            .validate()?;
        }
        run.status = "validating".into(); // publication hand-off, not a host validation plan
        run.save(pool).await?;
        runtime::publish(deployment, run).await
    } else {
        stop(deployment, run, "担当Runが終了しました。候補と実施・省略した検証を保持しました。未公開です。追加の指示で同じ会話から再開できます。".into()).await
    }
}

pub(super) async fn stop(
    deployment: &DeploymentImpl,
    mut run: IntegrationRun,
    reason: String,
) -> anyhow::Result<()> {
    // Never conceal a possible Git publication behind a generic unpublished stop.
    if run.payload.publication_intent {
        return runtime::recover_publication(deployment, run).await;
    }
    admission::idle(deployment, run.workspace_id.unwrap()).await?;
    run.status = "blocked".into();
    if run.payload.stop.is_none() {
        run.payload.stop = Some(db::models::integration::IntegrationStop {
            stage: "conversation".into(),
            category: "unpublished".into(),
            next_action:
                "候補・検証記録と担当Runの説明を確認し、同じ会話へ次の指示を送ってください。".into(),
        });
    }
    run.error = Some(reason.clone());
    let previous = run.payload.publication_request.clone();
    run.payload
        .history
        .push(serde_json::json!({"event":"publication_not_applied","request":previous}));
    run.payload.publication_request = None;
    let candidate = run.payload.result_commit.clone();
    run.payload.history.push(serde_json::json!({"event":"stopped_unpublished","message":reason,"candidate_commit":candidate}));
    run.save(&deployment.db().pool).await?;
    admission::release(&deployment.db().pool, run.id).await?;
    Ok(())
}

/// Only a new user instruction enters here. No drift-following or auto-resume loop.
pub(crate) async fn resume(
    deployment: &DeploymentImpl,
    run: &mut IntegrationRun,
) -> anyhow::Result<()> {
    if run.payload.published {
        return Ok(());
    }
    if run.status == "cancelled" && !run.payload.publication_intent {
        admission::idle(deployment, run.workspace_id.unwrap()).await?;
        let previous = run.payload.publication_request.take();
        run.payload.history.push(serde_json::json!({"event":"cancelled_publication_request_retained","request":previous}));
    }
    ensure!(
        !run.payload.publication_intent
            && (!run.cancel_requested || run.status == "cancelled")
            && run.payload.publication_request.is_none(),
        "Reconcile publication/cancellation before resuming"
    );
    if matches!(run.status.as_str(), "blocked" | "cancelled") {
        admission::idle(deployment, run.workspace_id.unwrap()).await?;
        let (_, workspace, _, root) = runtime::context(deployment, run).await?;
        mechanical::require_private_branch(&root, &workspace.branch, &run.target_ref)?;
        // Inspect, never reset retained edits/index or reuse old pass on a new SHA.
        mechanical::git(&root, &["status", "--porcelain"])?;
        let previous_status = run.status.clone();
        if run.cancel_requested {
            sqlx::query(
                "UPDATE integration_runs SET cancel_requested=0 WHERE id=? AND status='cancelled'",
            )
            .bind(run.id)
            .execute(&deployment.db().pool)
            .await?;
            run.cancel_requested = false;
        }
        run.payload
            .history
            .push(serde_json::json!({"event":"explicit_resume","previous_status":previous_status}));
        run.status = "integrating".into();
        run.error = None;
        run.payload.stop = None;
        run.save(&deployment.db().pool).await?;
    }
    Ok(())
}

/// Evidence is checked on the CODE SHA, never re-labelled as a test on the
/// final Wiki commit. No mandatory command list and no command syntax allowlist.
pub(super) async fn verify_evidence(
    pool: &sqlx::SqlitePool,
    run: &IntegrationRun,
) -> anyhow::Result<()> {
    let request = run
        .payload
        .publication_request
        .as_ref()
        .context("Missing publication request")?;
    ensure!(
        run.payload.result_commit.as_deref() == Some(&request.candidate_commit),
        "Publication candidate changed"
    );
    let evidence: Vec<_> = run
        .payload
        .evidence
        .iter()
        .filter(|e| e.code_commit == request.code_commit)
        .collect();
    ensure!(
        !evidence.is_empty(),
        "Missing code verification or reasoned skip"
    );
    for (index, item) in evidence.iter().enumerate() {
        ensure!(!item.reason.trim().is_empty(), "Missing evidence reason");
        if item.result == "skipped" {
            ensure!(
                item.process_id.is_none()
                    && !item.remaining_risk.trim().is_empty()
                    && matches!(item.actor.as_str(), "user" | "agent"),
                "Invalid skip provenance"
            );
            continue;
        }
        ensure!(
            matches!(item.result.as_str(), "passed" | "failed") && item.actor == "host_execution",
            "Unconfirmed test claim"
        );
        let process =
            ExecutionProcess::find_by_id(pool, item.process_id.context("Missing process")?)
                .await?
                .context("Process evidence missing")?;
        ensure!(
            Some(process.session_id) == run.session_id
                && process.status != ExecutionProcessStatus::Running
                && !process.dropped
                && process.run_reason == ExecutionProcessRunReason::IntegrationValidation,
            "Process identity mismatch"
        );
        if item.result == "passed" {
            ensure!(
                process.status == ExecutionProcessStatus::Completed && process.exit_code == Some(0),
                "Process did not pass"
            );
        } else {
            ensure!(
                evidence
                    .iter()
                    .skip(index + 1)
                    .any(
                        |later| (later.result == "passed" && later.command == item.command)
                            || (later.result == "skipped"
                                && (later.command.is_none() || later.command == item.command))
                    ),
                "Failed verification requires repair or a reasoned skip"
            );
        }
        let (before, after): (Option<String>, Option<String>) = sqlx::query_as("SELECT before_head_commit,after_head_commit FROM execution_process_repo_states WHERE execution_process_id=? AND repo_id=?").bind(process.id).bind(run.repository_id).fetch_one(pool).await?;
        ensure!(
            before.as_deref() == Some(&request.code_commit)
                && after.as_deref() == Some(&request.code_commit),
            "Verification SHA mismatch"
        );
        let action = process
            .executor_action()
            .context("Missing validation action")?;
        let ExecutorActionType::ScriptRequest(script) = action.typ();
        ensure!(
            Some(&script.script) == item.command.as_ref()
                && script.context == ScriptContext::IntegrationValidation,
            "Verification command mismatch"
        );
    }
    Ok(())
}

#[cfg(test)]
#[path = "conversation_tests.rs"]
mod tests;

pub(super) fn record_wiki_result(repo: &Repo, run: &IntegrationRun) -> anyhow::Result<String> {
    if !run.payload.wiki_enabled {
        return Ok("disabled".into());
    }
    let request = run
        .payload
        .publication_request
        .as_ref()
        .context("Missing Wiki publication binding")?;
    if let Some(store) = RepositoryMemoryStore::existing_for_repository(&repo.name, repo.id)? {
        let _lock = store.try_lock()?;
        for source in &run.payload.sources {
            for event in &source.event_ids {
                store.acknowledge(&utils::repository_memory::WikiReconciliationReceipt {
                    event_id: *event,
                    reconciled_at: chrono::Utc::now(),
                    target_commit: request.code_commit.clone(),
                    wiki_commit: request.wiki_commit.clone(),
                    result: if request.wiki_commit.as_deref() == Some(&request.code_commit) {
                        utils::repository_memory::ReconciliationResult::NoOp
                    } else {
                        utils::repository_memory::ReconciliationResult::Updated
                    },
                    error: None,
                })?;
            }
        }
        let mut state = store.state()?;
        if state.active_run_id.is_none()
            && state.bootstrap.is_none()
            && state.target_branch.as_deref() == run.target_ref.strip_prefix("refs/heads/")
        {
            state.source_commit = Some(request.code_commit.clone());
            state.wiki_commit = request.wiki_commit.clone();
            state.last_success = Some(chrono::Utc::now());
            state.status = utils::repository_memory::RepositoryWikiStatus::Current;
            state.error = None;
            store.save_state(&state)?;
        }
    }
    Ok("Wiki included in published candidate".into())
}

/// Read an existing managed-process log, keeping only a bounded tail for the
/// worker. Full logs remain in the normal execution-process store/UI.
async fn log_tail(deployment: &DeploymentImpl, id: Uuid) -> serde_json::Value {
    use futures_util::StreamExt;
    use utils::log_msg::LogMsg;
    let mut tail = String::new();
    let Some(mut stream) = deployment.container().stream_raw_logs(&id).await else {
        return serde_json::json!({"available":false,"process_id":id});
    };
    let mut truncated = false;
    while let Ok(Some(item)) =
        tokio::time::timeout(std::time::Duration::from_secs(2), stream.next()).await
    {
        match item {
            Ok(LogMsg::Stdout(line) | LogMsg::Stderr(line)) => {
                tail.push_str(&line);
                if tail.len() > 16384 {
                    let mut cut = tail.len() - 16384;
                    while !tail.is_char_boundary(cut) {
                        cut += 1;
                    }
                    tail.drain(..cut);
                    truncated = true;
                }
            }
            Ok(LogMsg::Finished) => break,
            Err(_) => break,
            _ => {}
        }
    }
    let process = ExecutionProcess::find_by_id(&deployment.db().pool, id)
        .await
        .ok()
        .flatten();
    serde_json::json!({"available":true,"process_id":id,"exit_code":process.as_ref().and_then(|p|p.exit_code),"status":process.map(|p|p.status),"tail":tail,"truncated":truncated})
}
