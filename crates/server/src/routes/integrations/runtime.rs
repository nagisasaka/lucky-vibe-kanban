use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    sync::{Arc, LazyLock, Mutex},
    time::Duration,
};

use anyhow::{Context, bail, ensure};
use db::models::{
    execution_process::{ExecutionProcess, ExecutionProcessRunReason, ExecutionProcessStatus},
    execution_process_repo_state::ExecutionProcessRepoState,
    integration::{IntegrationRun, resource_key},
    repo::Repo,
    session::{CreateSession, Session},
    workspace::Workspace,
    workspace_repo::{CreateWorkspaceRepo, WorkspaceRepo},
};
use deployment::Deployment;
use executors::{
    actions::{
        ExecutorAction, ExecutorActionType,
        script::{ScriptContext, ScriptRequest, ScriptRequestLanguage},
    },
    runtime::{AgentRunStatus, OrchestrationNodeStatus},
};
use services::services::{container::ContainerService, orchestration::OrchestrationService};
use sqlx::SqlitePool;
use utils::repository_memory::{
    ChangeManifest, MemoryIntegration, RepositoryMemoryStore, SemanticChanges,
};
use uuid::Uuid;

use super::admission;
use crate::DeploymentImpl;

// Only task de-duplication; correctness lives in durable reservations and
// Orchestration identities, not this ephemeral set or a dispatcher TTL.
static IN_FLIGHT: LazyLock<Mutex<HashSet<Uuid>>> = LazyLock::new(|| Mutex::new(HashSet::new()));
pub(crate) struct IntegrationActionGuard(Uuid);
impl Drop for IntegrationActionGuard {
    fn drop(&mut self) {
        IN_FLIGHT
            .lock()
            .expect("Integration task set")
            .remove(&self.0);
    }
}
pub(crate) fn reserve_intervention(id: Uuid) -> anyhow::Result<IntegrationActionGuard> {
    ensure!(
        IN_FLIGHT.lock().expect("Integration task set").insert(id),
        "Integration is busy; retry after the current operation settles"
    );
    Ok(IntegrationActionGuard(id))
}

#[cfg(test)]
#[path = "runtime_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "scenario_tests.rs"]
mod scenario_tests;

pub(super) async fn request_recovery(
    deployment: &DeploymentImpl,
    mut run: IntegrationRun,
) -> anyhow::Result<()> {
    ensure!(
        IN_FLIGHT
            .lock()
            .expect("Integration task set")
            .insert(run.id),
        "This Integration is already being reconciled"
    );
    let id = run.id;
    let result = async {
        if run.status == "succeeded" && run.payload.published {
            complete_product(deployment, &run, OrchestrationNodeStatus::Succeeded).await?;
            // Git and Card receipts are already final. Retrying memory must not
            // depend on historical process rows or on the target still being here.
            run.payload.wiki_result =
                Some(match record_memory_integration(deployment, &run).await {
                    Ok(result) => result,
                    Err(error) => format!("semantic outbox pending: {error:#}"),
                });
            run.save(&deployment.db().pool).await.map_err(Into::into)
        } else {
            recover_publication(deployment, run).await
        }
    }
    .await;
    IN_FLIGHT.lock().expect("Integration task set").remove(&id);
    result
}

pub(super) async fn intervene(
    deployment: &DeploymentImpl,
    id: Uuid,
    request: super::InterventionRequest,
) -> anyhow::Result<()> {
    ensure!(request.prompt.len() <= 100_000, "Instruction too large");
    let _guard = reserve_intervention(id)?;
    intervene_inner(deployment, id, request).await
}

async fn intervene_inner(
    deployment: &DeploymentImpl,
    id: Uuid,
    request: super::InterventionRequest,
) -> anyhow::Result<()> {
    let _admission = services::services::integration_admission::MUTATIONS
        .lock()
        .await;
    let pool = &deployment.db().pool;
    let mut run = IntegrationRun::find(pool, id).await?;
    ensure!(run.project_id == request.project_id, "Project mismatch");
    super::mechanical::require_engine(&run)?;
    if let Some(old) = &run.payload.intervention
        && old.request_id == request.request_id
    {
        ensure!(old.prompt == request.prompt, "Idempotency conflict");
        return Ok(());
    }
    ensure!(
        run.status == "blocked" && !run.payload.publication_intent && !run.cancel_requested,
        "Only a blocked unpublished Integration can resume"
    );
    let (repo, ws, _, root) = context(deployment, &run).await?;
    admission::idle(deployment, ws.id).await?;
    admission::verify_sources(deployment, &run).await?;
    ensure!(
        deployment
            .git()
            .get_branch_oid(&repo.path, short_target(&run))?
            == run.payload.base_commit.as_deref().context("Missing base")?,
        "TARGET_CHANGED"
    );
    // A reviewed candidate continues mechanically. Revised resolutions require a new
    // explicit conflict job, never a Retry of a historical AgentRun.
    ensure!(
        request.prompt.is_empty() || run.payload.result_commit.is_none(),
        "A frozen candidate cannot be changed by instructions; review it or cancel"
    );
    let history = serde_json::json!({"operation":"resume", "request_id":request.request_id,
        "previous_error":run.error,"previous_stop":run.payload.stop,"agent_run_id":run.agent_run_id,
        "conflict_dispatch":run.payload.conflict_dispatch});
    run.payload.history.push(history);
    if run.payload.result_commit.is_none() {
        if let Some(agent) = run.agent_run_id {
            use executors::runtime::AgentRunPort;
            let state = deployment.agent_run_port().query(agent).await?.state;
            ensure!(
                state.status.is_terminal(),
                "Previous conflict job has not ended"
            );
        }
        // Preserve edits/index; the next conflict job receives the complete current contract.
        run.agent_run_id = None;
        run.payload.conflict_dispatch = None;
    }
    run.payload.intervention = Some(db::models::integration::IntegrationIntervention {
        request_id: request.request_id,
        prompt: request.prompt,
        created_at: chrono::Utc::now(),
        pending: false,
        retry_command: None,
        instruction_command: None,
    });
    run.payload.stop = None;
    run.error = None;
    run.status = if run.payload.steps.len() == run.payload.sources.len() {
        "validating"
    } else {
        "integrating"
    }
    .into();
    run.payload.resume_validation = run.status == "validating";
    let mut tx = pool.begin_with("BEGIN IMMEDIATE").await?;
    for (kind, key) in [
        ("target", run.storage_identity.clone()),
        ("workspace", resource_key(ws.id)),
    ] {
        sqlx::query("INSERT INTO integration_reservations VALUES(?,?,?) ON CONFLICT(resource_kind,resource_key) DO NOTHING")
            .bind(kind).bind(&key).bind(run.id).execute(&mut *tx).await?;
        let owner: Uuid = sqlx::query_scalar(
            "SELECT run_id FROM integration_reservations WHERE resource_kind=? AND resource_key=?",
        )
        .bind(kind)
        .bind(&key)
        .fetch_one(&mut *tx)
        .await?;
        ensure!(owner == run.id, "Another Integration owns the resource");
    }
    tx.commit().await?;
    // Git is only inspected here; dispatch and mutation happen on the monitor after this save.
    let _ = root;
    run.save(pool).await?;
    Ok(())
}

pub(super) async fn decide_gate(
    deployment: &DeploymentImpl,
    id: Uuid,
    request: super::GateDecisionRequest,
) -> anyhow::Result<()> {
    ensure!(
        matches!(request.decision.as_str(), "continue" | "retry" | "cancel"),
        "Invalid decision"
    );
    let _admission = services::services::integration_admission::MUTATIONS
        .lock()
        .await;
    let pool = &deployment.db().pool;
    let run = IntegrationRun::find(pool, id).await?;
    ensure!(run.project_id == request.project_id, "Project mismatch");
    if let Some(review) = run.payload.reviews.iter().find(|r| r.id == request.gate_id) {
        if review.request_id == Some(request.request_id) {
            ensure!(
                review.decision.as_deref() == Some(request.decision.as_str()),
                "Idempotency conflict"
            );
            return Ok(());
        }
        ensure!(
            !IN_FLIGHT
                .lock()
                .expect("Integration task set")
                .contains(&id),
            "Integration is still settling"
        );
        if request.decision != "cancel" {
            let (repo, workspace, _, root) = context(deployment, &run).await?;
            admission::idle(deployment, workspace.id).await?;
            deployment
                .git()
                .require_clean_source(&root, &review.candidate)?;
            ensure!(
                deployment
                    .git()
                    .get_branch_oid(&repo.path, short_target(&run))?
                    == run.payload.base_commit.as_deref().context("Missing base")?,
                "TARGET_CHANGED: select the current target again"
            );
        }
        return IntegrationRun::decide_review(
            pool,
            id,
            request.project_id,
            request.gate_id,
            request.request_id,
            &request.decision,
            &review.scope,
        )
        .await;
    }
    let gate = run
        .payload
        .gates
        .iter()
        .find(|g| g.id == request.gate_id)
        .context("Unknown confirmation")?;
    if gate.request_id == Some(request.request_id) {
        ensure!(
            gate.decision.as_deref() == Some(request.decision.as_str()),
            "Idempotency conflict"
        );
        return Ok(());
    }
    ensure!(
        run.status == "blocked" && !run.payload.publication_intent && !run.cancel_requested,
        "Integration no longer awaits this decision"
    );
    ensure!(
        gate.decision.is_none() && gate.scope == run.validation_scope(),
        "Stale confirmation"
    );
    ensure!(
        !IN_FLIGHT
            .lock()
            .expect("Integration task set")
            .contains(&id),
        "Integration is still settling; retry shortly"
    );
    if request.decision != "cancel" {
        admission::verify_sources(deployment, &run).await?;
        let (repo, workspace, _, root) = context(deployment, &run).await?;
        admission::idle(deployment, workspace.id).await?;
        deployment.git().require_clean_source(
            &root,
            run.payload
                .result_commit
                .as_deref()
                .context("Missing candidate")?,
        )?;
        ensure!(
            deployment
                .git()
                .get_branch_oid(&repo.path, short_target(&run))?
                == run.payload.base_commit.as_deref().context("Missing base")?,
            "TARGET_CHANGED: reselect against the current target; this approval cannot be reused"
        );
    }
    IntegrationRun::decide_validation_gate(
        pool,
        id,
        request.project_id,
        request.gate_id,
        request.request_id,
        &request.decision,
        &gate.scope,
    )
    .await?;
    Ok(())
}

// Admission freezes event IDs after checking source ancestry. Resolve only those
// immutable public records, never another Workspace's private memory or history.
fn integration_memory(
    run: &IntegrationRun,
    store: Option<&RepositoryMemoryStore>,
) -> anyhow::Result<serde_json::Value> {
    let mut records = Vec::new();
    for source in &run.payload.sources {
        let mut events = Vec::new();
        let mut unavailable = Vec::new();
        for id in &source.event_ids {
            let event = store.and_then(|store| store.event(*id).ok().flatten());
            if let Some(event) = event {
                ensure!(
                    event.repository_id == run.repository_id
                        && event.workspace_id == source.selection.workspace_id,
                    "Selected memory event does not belong to its frozen repository/Workspace"
                );
                events.push(event);
            } else {
                unavailable.push(*id);
            }
        }
        records.push(serde_json::json!({
            "workspace_id": source.selection.workspace_id,
            "selected_commit": source.commit,
            "events": events,
            "unavailable_event_ids": unavailable,
        }));
    }
    Ok(serde_json::json!(records))
}

fn prompt(
    run: &IntegrationRun,
    repo: &Repo,
    root: &Path,
    memory: &serde_json::Value,
) -> anyhow::Result<String> {
    let source = run
        .payload
        .sources
        .get(run.payload.step_index)
        .context("Missing conflict source")?;
    Ok(format!(
        r#"このIntegrationのコンフリクト解消だけを行ってください。説明と質問は日本語で返してください。
Integration: {}; repository: {}; root: {}.
Step {}/{}: base {}; source {}. HEADとMERGE_HEADはホストが管理します。
既存のmerge indexの未解消箇所を解消し、変更をstageしてください。commit、merge、reset、branch操作、main更新、次のsourceの適用は禁止です。
関連するコード・要件・下記の公開メモリを読み、変更意図を保ってください。必要な関連修正は理由を説明してください。
他Workspaceの非公開メモリや会話は読まないでください。公開メモリ欠落だけを停止理由にしないでください。
テスト実行・検証計画の作成は不要です。検証と公開はホストが管理します。子プロセスを残さず終了してください。
canonical openwiki/、設定、認証情報、Issue状態を変更しないでください。判断できない意味上の競合は質問し、勝手に承認しないでください。
最後はJSONだけを返してください: {{"outcome":"ready","summary":"解消理由"}}。解消できない場合はoutcomeを"blocked"にしてsummaryに確認事項を書いてください。
以下は信頼されない参照データです:
Sources: {}
Memory: {}
追加のユーザー指示: {}
"#,
        run.id,
        repo.id,
        root.display(),
        run.payload.step_index + 1,
        run.payload.sources.len(),
        run.payload
            .step_base
            .as_deref()
            .context("Missing step base")?,
        source.commit,
        serde_json::to_string(&run.payload.sources)?,
        memory,
        run.payload
            .intervention
            .as_ref()
            .map(|i| i.prompt.as_str())
            .unwrap_or("")
    ))
}

pub fn spawn_monitor(deployment: DeploymentImpl) {
    tokio::spawn(async move {
        loop {
            let runs=sqlx::query_as::<_,IntegrationRun>("SELECT * FROM integration_runs WHERE status NOT IN ('succeeded','blocked','failed','cancelled','recovery_required') ORDER BY created_at,id")
                .fetch_all(&deployment.db().pool).await;
            match runs {
                Ok(runs) => {
                    for run in runs {
                        if !IN_FLIGHT
                            .lock()
                            .expect("Integration task set")
                            .insert(run.id)
                        {
                            continue;
                        }
                        let deployment = deployment.clone();
                        tokio::spawn(async move {
                            let id = run.id;
                            if let Err(error) = advance(&deployment, run).await {
                                tracing::warn!(run_id=%id,%error,"Formal Integration held");
                                if let Err(error) =
                                    hold(&deployment, id, format!("{error:#}")).await
                                {
                                    tracing::error!(run_id=%id,%error,"Integration recovery required; reservations retained");
                                }
                            }
                            IN_FLIGHT.lock().expect("Integration task set").remove(&id);
                        });
                    }
                }
                Err(error) => tracing::warn!(%error,"Integration monitor unavailable"),
            }
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    });
}

async fn advance(deployment: &DeploymentImpl, run: IntegrationRun) -> anyhow::Result<()> {
    if run.cancel_requested && !run.payload.publication_intent {
        return cancel(deployment, run).await;
    }
    if matches!(run.status.as_str(), "publishing" | "post_processing") {
        return recover_publication(deployment, run).await;
    }
    super::mechanical::require_engine(&run)?;
    match run.status.as_str() {
        "queued" => {
            if !admission::target_right(&deployment.db().pool, &run).await? {
                return Ok(());
            }
            admission::verify_sources(deployment, &run).await?;
            prepare(deployment, run).await
        }
        "preparing" => {
            bail!("Preparation interrupted; inspect the retained workspace before resuming")
        }
        "integrating" => merge_step(deployment, run).await,
        "validating" if run.payload.resume_validation => {
            let mut run = run;
            run.payload.resume_validation = false;
            run.save(&deployment.db().pool).await?;
            validate(deployment, run).await
        }
        "validating" => {
            stop_validation(deployment, &run).await?;
            bail!("Validation interrupted; inspect retained logs and resume explicitly")
        }
        "cancelling" => cancel(deployment, run).await,
        _ => Ok(()),
    }
}

async fn merge_step(deployment: &DeploymentImpl, mut run: IntegrationRun) -> anyhow::Result<()> {
    use executors::runtime::{AgentRunPort, AgentRunPortCommand, ProjectionStatus};

    use super::mechanical;
    let pool = &deployment.db().pool;
    let (repo, ws, _, root) = context(deployment, &run).await?;
    mechanical::require_private_branch(&root, &ws.branch, &run.target_ref)?;
    ensure!(
        deployment
            .git()
            .get_branch_oid(&repo.path, short_target(&run))?
            == run.payload.base_commit.as_deref().context("Missing base")?,
        "TARGET_CHANGED"
    );
    let before = run.payload.step_base.clone().context("Missing step base")?;
    let source = run
        .payload
        .sources
        .get(run.payload.step_index)
        .context("Missing step source")?
        .commit
        .clone();
    if run.payload.result_commit.is_none() {
        if run.payload.merge_tree.is_none() {
            admission::idle(deployment, ws.id).await?;
            super::steps::prepare(&root, &mut run)?;
            run.save(pool).await?;
        }
        let tree = if run.payload.merge_conflicted {
            if run.payload.conflict_dispatch.is_none() {
                admission::idle(deployment, ws.id).await?;
                mechanical::prepare_conflict(&root, &before, &source)?;
                let store = RepositoryMemoryStore::read_only_configured_for_repository(
                    &repo.name, repo.id,
                )?;
                let memory = integration_memory(&run, store.as_ref())?;
                let dispatch = mechanical::conflict_request(
                    &run,
                    ws.container_ref.clone().context("Missing workspace path")?,
                    prompt(&run, &repo, &root, &memory)?,
                )?;
                let AgentRunPortCommand::Create { ref request, .. } = dispatch else {
                    unreachable!()
                };
                run.agent_run_id = Some(request.agent_run_id);
                run.payload.conflict_dispatch = Some(serde_json::to_value(dispatch)?);
                run.save(pool).await?; // immutable identity and full instruction precede all provider side effects
            }
            let AgentRunPortCommand::Create { request, attempt } =
                serde_json::from_value(run.payload.conflict_dispatch.clone().unwrap())?
            else {
                bail!("Invalid conflict dispatch")
            };
            let agent = deployment.agent_run_port().create(request, attempt).await?;
            ensure!(
                run.agent_run_id == Some(agent),
                "Conflict identity mismatch"
            );
            let state = deployment.agent_run_port().query(agent).await?.state;
            if !state.status.is_terminal() {
                return Ok(());
            }
            // Terminal output alone never grants a new writer. Teardown may lag behind it.
            if admission::idle(deployment, ws.id).await.is_err() {
                return Ok(());
            }
            ensure!(
                state.status == AgentRunStatus::Succeeded
                    && state.projection_status == ProjectionStatus::Current,
                "Conflict job did not finish with a trustworthy projection; inspect and resume explicitly"
            );
            let result = mechanical::resolution(
                &state.terminal_output.context("Missing resolution")?.content,
            )?;
            ensure!(
                result.outcome == "ready",
                "Conflict needs clarification: {}",
                result.summary
            );
            run.payload.semantic_summary = Some(result.summary);
            mechanical::resolution_tree(&root, &before, &source)?
        } else {
            admission::idle(deployment, ws.id).await?;
            run.payload.semantic_summary =
                Some(format!("Automatically merged pinned source {source}"));
            run.payload.merge_tree.clone().unwrap()
        };
        run.payload.result_commit = Some(mechanical::candidate(
            &root,
            &before,
            &source,
            &tree,
            run.id,
            run.payload.step_index,
        )?);
        run.save(pool).await?; // Git object exists, branch has not moved: durable installation intent
    }
    admission::idle(deployment, ws.id).await?;
    let result = run.payload.result_commit.clone().unwrap();
    mechanical::require_private_branch(&root, &ws.branch, &run.target_ref)?;
    mechanical::install(
        &root,
        &before,
        &source,
        &result,
        run.payload.merge_conflicted,
    )?;
    deployment.git().require_clean_source(&root, &result)?;
    ensure!(
        deployment.git().is_ancestor(&root, &before, &result)?
            && deployment.git().is_ancestor(&root, &source, &result)?,
        "Candidate lost merge ancestry"
    );
    services::services::repository_memory::guard_normal_workspace(
        deployment.git(),
        &root,
        run.payload.base_commit.as_deref().unwrap(),
    )?;
    if !run
        .payload
        .reviews
        .iter()
        .any(|r| r.step_index == run.payload.step_index && r.candidate == result)
    {
        super::steps::record_review(&root, &mut run)?;
        run.save(pool).await?;
    }
    if !super::steps::review_approved(&run) {
        run.status = "blocked".into();
        run.payload.stop = Some(db::models::integration::IntegrationStop {
            stage: "reviewing".into(),
            category: "merge_review".into(),
            next_action: "Review the frozen resolution diff and approve to continue".into(),
        });
        run.save(pool).await?;
        deployment
            .container()
            .notification_service()
            .notify(
                "Merge resolution needs review",
                "Review the candidate in the Integration Workspace.",
                run.workspace_id,
            )
            .await;
        return Ok(());
    }
    if run.payload.steps.len() == run.payload.step_index && run.agent_run_id.is_some() {
        let receipt = serde_json::json!({"operation":"conflict_resolved", "step_index":run.payload.step_index,
            "agent_run_id":run.agent_run_id, "candidate":result});
        run.payload.history.push(receipt);
    }
    super::steps::complete(&mut run)?;
    if run.payload.steps.len() < run.payload.sources.len() {
        run.payload.step_index += 1;
        run.payload.step_base = Some(result);
        run.payload.result_commit = None;
        run.payload.merge_tree = None;
        run.payload.merge_conflicted = false;
        run.payload.conflict_dispatch = None;
        run.agent_run_id = None;
        run.payload.intervention = None;
        run.save(pool).await?;
        return Ok(());
    }
    let summary = run.payload.semantic_summary.clone().unwrap_or_default();
    let semantics = SemanticChanges {
        goal: "Integrate the explicitly selected commits".into(),
        summary: if run.payload.skip_validation {
            format!("Unverified — validation skipped by explicit request. {summary}")
        } else {
            summary
        },
        tests: if run.payload.skip_validation {
            vec![utils::repository_memory::MemoryTest {
                command: None,
                result: utils::repository_memory::MemoryTestResult::NotRun,
                summary: Some("Integration validation explicitly skipped".into()),
            }]
        } else {
            vec![]
        },
        ..Default::default()
    };
    run.payload.semantics = Some(semantics.clone());
    run.payload.integration_manifest = Some(ChangeManifest {
        version: 1,
        event_id: run.id,
        repository_id: repo.id,
        workspace_id: ws.id,
        task_id: None,
        created_at: chrono::Utc::now(),
        base_commit: run.payload.base_commit.clone().unwrap(),
        source_commit: result,
        target_branch: Some(short_target(&run).into()),
        changed_paths: deployment
            .git()
            .get_diff_file_paths(&root, &run.payload.base_commit.as_deref().unwrap().parse()?)?
            .into_iter()
            .collect(),
        semantics,
    });
    run.status = "validating".into();
    run.save(pool).await?;
    validate(deployment, run).await
}

fn short_target(run: &IntegrationRun) -> &str {
    run.target_ref
        .strip_prefix("refs/heads/")
        .unwrap_or(&run.target_ref)
}

async fn prepare(deployment: &DeploymentImpl, mut run: IntegrationRun) -> anyhow::Result<()> {
    let pool = &deployment.db().pool;
    let repo = Repo::find_by_id(pool, run.repository_id)
        .await?
        .context("Repository missing")?;
    let target_head = deployment
        .git()
        .get_branch_oid(&repo.path, short_target(&run))?;
    ensure!(
        target_head == run.payload.observed_target,
        "TARGET_CHANGED: frozen admission target moved"
    );
    if let Some(base) = &run.payload.base_commit {
        ensure!(
            base == &target_head,
            "TARGET_CHANGED: select the current target again"
        );
    } else {
        run.payload.base_commit = Some(target_head);
    }
    run.status = "preparing".into();
    run.save(pool).await?;
    let workspace = if let Some(id) = run.workspace_id {
        Workspace::find_by_id(pool, id)
            .await?
            .context("Integration Workspace missing")?
    } else {
        let workspace = crate::routes::workspaces::create::create_workspace_record_with_owner(
            deployment,
            Some(format!("Integration: {}", repo.display_name)),
            Some(&db::models::workspace_usage::WorkspaceExecutionOwner::new(
                db::models::workspace_usage::INTEGRATION,
                repo.id,
                Some(run.id),
            )),
        )
        .await?;
        run.workspace_id = Some(workspace.id);
        run.save(pool).await?;
        WorkspaceRepo::create_many(
            pool,
            workspace.id,
            &[CreateWorkspaceRepo {
                repo_id: repo.id,
                target_branch: short_target(&run).into(),
            }],
        )
        .await?;
        workspace
    };
    let container = deployment
        .container()
        .ensure_container_exists(&workspace)
        .await?;
    let root = PathBuf::from(&container).join(&repo.name);
    super::mechanical::require_private_branch(&root, &workspace.branch, &run.target_ref)?;
    deployment
        .git()
        .require_clean_source(&root, run.payload.base_commit.as_deref().unwrap())?;
    if run.payload.sequential {
        run.payload.step_base = run.payload.base_commit.clone();
        super::steps::prepare(&root, &mut run)?;
        run.save(pool).await?;
    }
    let config: executors::profile::ExecutorConfig = serde_json::from_value(
        run.payload
            .executor_config
            .clone()
            .context("Missing executor configuration")?,
    )?;
    let session = if let Some(id) = run.session_id {
        Session::find_by_id(pool, id)
            .await?
            .context("Integration Session missing")?
    } else {
        Session::create(
            pool,
            &CreateSession {
                executor: Some(config.executor.to_string()),
                name: Some("Formal Integration".into()),
            },
            Uuid::new_v4(),
            workspace.id,
        )
        .await?
    };
    run.session_id = Some(session.id);
    run.save(pool).await?;
    sqlx::query("INSERT INTO integration_reservations VALUES('workspace',?,?) ON CONFLICT(resource_kind,resource_key) DO NOTHING")
        .bind(resource_key(workspace.id))
        .bind(run.id)
        .execute(pool)
        .await?;
    run.status = "integrating".into();
    run.save(pool).await?;
    Ok(())
}

pub(super) async fn context(
    deployment: &DeploymentImpl,
    run: &IntegrationRun,
) -> anyhow::Result<(Repo, Workspace, Session, PathBuf)> {
    let pool = &deployment.db().pool;
    let repo = Repo::find_by_id(pool, run.repository_id)
        .await?
        .context("Repository missing")?;
    let workspace = Workspace::find_by_id(
        pool,
        run.workspace_id.context("Integration Workspace missing")?,
    )
    .await?
    .context("Workspace missing")?;
    let session = Session::find_by_id(pool, run.session_id.context("Integration Session missing")?)
        .await?
        .context("Session missing")?;
    let root = PathBuf::from(
        workspace
            .container_ref
            .as_ref()
            .context("Workspace checkout missing")?,
    )
    .join(&repo.name);
    Ok((repo, workspace, session, root))
}

async fn validate(deployment: &DeploymentImpl, mut run: IntegrationRun) -> anyhow::Result<()> {
    let pool = &deployment.db().pool;
    let (repo, workspace, session, root) = context(deployment, &run).await?;
    let result = run
        .payload
        .result_commit
        .clone()
        .context("Missing frozen R")?;
    deployment.git().require_clean_source(&root, &result)?;
    if run.payload.skip_validation {
        verify_validation(pool, &run).await?;
    }
    for index in 0..run.payload.validation.len() {
        if IntegrationRun::find(pool, run.id).await?.cancel_requested {
            return cancel(deployment, run).await;
        }
        deployment.git().require_clean_source(&root, &result)?;
        let item = run.payload.validation[index].clone();
        if item.result.as_deref() == Some("passed") || run.validation_excepted(index) {
            continue;
        }
        let cwd = std::fs::canonicalize(root.join(&item.cwd))?;
        ensure!(
            cwd.starts_with(std::fs::canonicalize(&root)?),
            "Validation cwd escapes repository through a symlink"
        );
        let action = ExecutorAction::new(
            ExecutorActionType::ScriptRequest(ScriptRequest {
                script: item.command,
                language: ScriptRequestLanguage::Bash,
                context: ScriptContext::IntegrationValidation,
                working_dir: Some(
                    Path::new(&repo.name)
                        .join(&item.cwd)
                        .to_string_lossy()
                        .into_owned(),
                ),
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
        run.payload.validation[index].execution_process_id = Some(process.id);
        run.save(pool).await?;
        let deadline = tokio::time::Instant::now() + Duration::from_secs(60 * 60);
        let process = loop {
            if IntegrationRun::find(pool, run.id).await?.cancel_requested {
                deployment
                    .container()
                    .stop_execution(&process, ExecutionProcessStatus::Killed)
                    .await?;
                return cancel(deployment, run).await;
            }
            let observed = ExecutionProcess::find_by_id(pool, process.id)
                .await?
                .context("Host validation process disappeared")?;
            if observed.status != ExecutionProcessStatus::Running {
                break observed;
            }
            if tokio::time::Instant::now() > deadline {
                deployment
                    .container()
                    .stop_execution(&observed, ExecutionProcessStatus::Killed)
                    .await?;
                break ExecutionProcess::find_by_id(pool, observed.id)
                    .await?
                    .context("Timed-out validation process disappeared")?;
            }
            tokio::time::sleep(Duration::from_millis(300)).await;
        };
        let passed =
            process.status == ExecutionProcessStatus::Completed && process.exit_code == Some(0);
        // Status is saved before the normal process monitor has drained logs,
        // recorded after-HEAD and stopped its process group. Do not race that
        // hand-off or manufacture an after-HEAD from a later observation.
        let settled_deadline = tokio::time::Instant::now() + Duration::from_secs(30);
        loop {
            let states =
                ExecutionProcessRepoState::find_by_execution_process_id(pool, process.id).await?;
            let after_recorded = states
                .iter()
                .any(|state| state.repo_id == repo.id && state.after_head_commit.is_some());
            if after_recorded && deployment.container().scripts_settled(workspace.id).await? {
                break;
            }
            if IntegrationRun::find(pool, run.id).await?.cancel_requested {
                return cancel(deployment, run).await;
            }
            ensure!(
                tokio::time::Instant::now() < settled_deadline,
                "Host validation finalisation has not settled; no publication permitted"
            );
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
        run.payload.validation[index].exit_code = process.exit_code;
        run.payload.validation[index].result = Some(
            if passed {
                "passed"
            } else if process.status == ExecutionProcessStatus::Killed {
                "not_completed"
            } else if matches!(process.exit_code, Some(126 | 127)) {
                "unavailable"
            } else {
                "failed"
            }
            .into(),
        );
        run.save(pool).await?;
        deployment.git().require_clean_source(&root, &result)?;
        if !passed && item.required {
            return await_validation_decision(deployment, run, index).await;
        }
    }
    if run.payload.mechanical && !run.payload.skip_validation {
        let evidence = run.payload.validation.iter().map(|v| utils::repository_memory::MemoryTest {
            command: Some(v.command.clone()),
            result: match v.result.as_deref() {
                Some("passed") => utils::repository_memory::MemoryTestResult::Passed,
                Some("failed") | Some("unavailable") => utils::repository_memory::MemoryTestResult::Failed,
                _ => utils::repository_memory::MemoryTestResult::NotRun,
            },
            summary: Some("Host execution on the frozen final candidate; any exception remains in Integration gates".into()),
        }).collect::<Vec<_>>();
        if let Some(semantics) = &mut run.payload.semantics {
            semantics.tests = evidence.clone();
        }
        if let Some(manifest) = &mut run.payload.integration_manifest {
            manifest.semantics.tests = evidence;
        }
        run.save(pool).await?;
    }
    publish(deployment, run).await
}

async fn await_validation_decision(
    deployment: &DeploymentImpl,
    mut run: IntegrationRun,
    index: usize,
) -> anyhow::Result<()> {
    let scope = run.validation_scope();
    let evidence = run.payload.validation[index].clone();
    run.payload.gates.push(db::models::integration::IntegrationGate {
        id: Uuid::new_v4(), validation_index: index, scope,
        risk: "This required check did not pass. Continuing publishes this candidate without this check's assurance; regressions and unmet requirements may remain. This exception covers only this check and candidate.".into(),
        evidence,
        decision: None, actor: None, decided_at: None, request_id: None,
    });
    run.payload.stop = Some(db::models::integration::IntegrationStop {
        stage: "validating".into(),
        category: "validation_gate".into(),
        next_action:
            "Fix and retry, explicitly accept this check's candidate-bound risk, or cancel.".into(),
    });
    run.status = "blocked".into();
    run.error = Some("Required host validation did not pass. Review the evidence and choose retry, a candidate-bound exception, or cancel.".into());
    // Retain ownership of the candidate and target while awaiting a decision.
    run.save(&deployment.db().pool).await?;
    deployment.container().notification_service().notify(
        "Integration needs a decision", "A required check did not pass. Review the candidate and risk in its Integration Workspace.", run.workspace_id,
    ).await;
    Ok(())
}

async fn stop_validation(deployment: &DeploymentImpl, run: &IntegrationRun) -> anyhow::Result<()> {
    if let Some(workspace) = run.workspace_id {
        let ids:Vec<Uuid>=sqlx::query_scalar("SELECT p.id FROM execution_processes p JOIN sessions s ON s.id=p.session_id WHERE s.workspace_id=? AND (p.status='running' OR p.run_reason='integrationvalidation')").bind(workspace).fetch_all(&deployment.db().pool).await?;
        for id in ids {
            if let Some(process) = ExecutionProcess::find_by_id(&deployment.db().pool, id).await? {
                if process.status != ExecutionProcessStatus::Running
                    && deployment.container().script_settled(id).await?
                {
                    continue;
                }
                deployment
                    .container()
                    .stop_execution(&process, ExecutionProcessStatus::Killed)
                    .await?;
            }
        }
    }
    Ok(())
}

async fn accepted_agent_identity(
    pool: &SqlitePool,
    run: &IntegrationRun,
) -> anyhow::Result<Option<Uuid>> {
    if run.payload.mechanical {
        if let Some(agent) = run.agent_run_id {
            let accepted = db::models::agent_runtime::AgentRunRecord::find(pool, agent).await?;
            if let Some(accepted) = accepted {
                let Some(value) = &run.payload.conflict_dispatch else {
                    bail!("Missing conflict dispatch")
                };
                let executors::runtime::AgentRunPortCommand::Create { request, .. } =
                    serde_json::from_value(value.clone())?
                else {
                    bail!("Invalid conflict dispatch")
                };
                ensure!(
                    accepted.request_envelope.0 == request && request.correlation_id == run.id,
                    "Conflict dispatch identity mismatch"
                );
                return Ok(Some(agent));
            }
        }
        return Ok(None);
    }
    let linked: Option<Uuid> = sqlx::query_scalar("SELECT agent_run_id FROM orchestration_agent_run_links WHERE orchestration_run_id=? ORDER BY created_at DESC LIMIT 1")
        .bind(run.id).fetch_optional(pool).await?;
    if linked.is_some() {
        return Ok(linked);
    }
    let envelope: Option<sqlx::types::Json<executors::runtime::AgentRunPortCommandEnvelope>> = sqlx::query_scalar(
        "SELECT command_envelope FROM orchestration_outbox WHERE orchestration_run_id=? AND json_extract(command_envelope,'$.command.type')='create' ORDER BY created_at LIMIT 1"
    ).bind(run.id).fetch_optional(pool).await?;
    if let Some(envelope) = envelope
        && let executors::runtime::AgentRunPortCommand::Create { request, .. } = &envelope.command
        && let Some(accepted) =
            db::models::agent_runtime::AgentRunRecord::find(pool, request.agent_run_id).await?
    {
        ensure!(
            accepted.request_envelope.0 == *request
                && request.correlation_id == run.id
                && Some(request.workspace.workspace_id) == run.workspace_id
                && Some(request.session_id) == run.session_id,
            "Accepted dispatch identity mismatch; recovery requires inspection"
        );
        return Ok(Some(request.agent_run_id));
    }
    Ok(None)
}

// Older dispatch code saved a planned identity before Create was accepted.
// Only clear that identity with durable proof of a stopped parent and an
// unaccepted, settled delivery. An in-flight delivery still needs reconciliation.
async fn cancellation_agent(
    pool: &SqlitePool,
    run: &IntegrationRun,
) -> anyhow::Result<Option<Uuid>> {
    let accepted = accepted_agent_identity(pool, run).await?;
    if let Some(agent) = accepted {
        ensure!(
            run.agent_run_id.is_none_or(|saved| saved == agent),
            "Accepted Agent identity mismatch"
        );
        return Ok(Some(agent));
    }
    if run.agent_run_id.is_none() || run.payload.mechanical {
        return Ok(None);
    }
    let agent = run.agent_run_id.unwrap();
    ensure!(
        db::models::agent_runtime::AgentRunRecord::find(pool, agent)
            .await?
            .is_none(),
        "Accepted Agent requires reconciliation"
    );
    let unaccepted: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM orchestration_outbox o JOIN orchestration_runs r ON r.id=o.orchestration_run_id WHERE r.id=? AND r.status IN ('failed','cancelled','succeeded') AND o.delivery_status!='delivering' AND json_extract(o.command_envelope,'$.command.type')='create' AND json_extract(o.command_envelope,'$.agent_run_id')=?)",
    ).bind(run.id).bind(agent.to_string()).fetch_one(pool).await?;
    ensure!(
        unaccepted,
        "Dispatch acceptance is unresolved; reconcile before cancellation"
    );
    Ok(None)
}

async fn cancel(deployment: &DeploymentImpl, mut run: IntegrationRun) -> anyhow::Result<()> {
    ensure!(
        !run.payload.publication_intent,
        "Publication boundary passed; cancellation cannot undo Git"
    );
    run.status = "cancelling".into();
    run.agent_run_id = cancellation_agent(&deployment.db().pool, &run).await?;
    run.save(&deployment.db().pool).await?;
    stop_validation(deployment, &run).await?;
    if let Some(agent) = run.agent_run_id {
        let linked: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM orchestration_agent_run_links WHERE orchestration_run_id=? AND agent_run_id=?)")
            .bind(run.id).bind(agent).fetch_one(&deployment.db().pool).await?;
        if !linked {
            let accepted =
                db::models::agent_runtime::AgentRunRecord::find(&deployment.db().pool, agent)
                    .await?
                    .context("Accepted Agent disappeared")?;
            services::services::agent_runtime::AgentRunCommandService::new(
                &deployment.db().pool,
                deployment.agent_run_port(),
            )
            .dispatch(executors::runtime::AgentRunPortCommandEnvelope {
                schema_version: executors::runtime::ORCHESTRATION_COMMAND_SCHEMA_VERSION,
                command_id: Uuid::from_u128(agent.as_u128() ^ 0x269fcd3f937a48a087adbc4ee766ac0e),
                idempotency_key: format!("integration:{}:cancel-unlinked:{agent}", run.id),
                agent_run_id: agent,
                orchestration_run_id: None,
                orchestration_node_execution_id: None,
                correlation_id: run.id,
                created_at: accepted.request_envelope.created_at,
                command: executors::runtime::AgentRunPortCommand::Cancel {
                    reason: "Integration stopped before dispatch link was recorded".into(),
                },
            })
            .await?;
        }
    }
    let has_orchestration: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM orchestration_runs WHERE id=?)")
            .bind(run.id)
            .fetch_one(&deployment.db().pool)
            .await?;
    if has_orchestration {
        let orchestration = OrchestrationService::new(
            deployment.db().pool.clone(),
            Arc::new(deployment.agent_run_port().clone()),
        );
        orchestration.cancel(run.id, run.id).await?;
        while orchestration.deliver_next().await? {}
        if let Some(agent) = run.agent_run_id
            && !orchestration
                .query_agent_run(agent)
                .await?
                .status
                .is_terminal()
        {
            return Ok(());
        }
    }
    if let Some(ws) = run.workspace_id {
        admission::idle(deployment, ws).await?;
    }
    let requested = IntegrationRun::find(&deployment.db().pool, run.id)
        .await?
        .cancel_requested;
    run.status = if run.error.is_some() && !requested {
        "blocked"
    } else {
        "cancelled"
    }
    .into();
    run.save(&deployment.db().pool).await?;
    admission::release(&deployment.db().pool, run.id).await
}

async fn hold(deployment: &DeploymentImpl, id: Uuid, error: String) -> anyhow::Result<()> {
    let mut run = IntegrationRun::find(&deployment.db().pool, id).await?;
    if !run.payload.mechanical && !run.payload.publication_intent {
        run.status = "blocked".into();
        run.error = Some(error);
        run.save(&deployment.db().pool).await?;
        return Ok(());
    }
    run.payload.stop = Some(db::models::integration::IntegrationStop {
        stage: run.status.clone(), category: if error.contains("database is locked") { "storage_contention" } else if error.contains("TARGET_CHANGED") || error.contains("SOURCE_CHANGED") { "selection_changed" } else { "technical_recovery" }.into(),
        next_action: if run.payload.publication_intent {
            "Reconcile publication before any further execution. Never replay Git publication blindly."
        } else {
            "Inspect the saved error and execution logs. Repair storage, permissions or the execution environment, then resume this Integration. If source or target changed, cancel and explicitly reselect. Technical failures cannot be waived."
        }.into(),
    });
    run.error = Some(error);
    if run.payload.publication_intent {
        run.status = "recovery_required".into();
        run.save(&deployment.db().pool).await?;
        return Ok(());
    }
    // A dispatch could have succeeded just before identity was copied to the
    // product. Recover the durable Orchestration link before releasing anything.
    if run.agent_run_id.is_none() {
        run.agent_run_id = accepted_agent_identity(&deployment.db().pool, &run).await?;
    }
    stop_validation(deployment, &run).await?;
    // Keep writer reservations on technical failure. Only explicit Cancel stops a conflict job.
    let settled = if let Some(ws) = run.workspace_id {
        admission::idle(deployment, ws).await.is_ok()
    } else {
        true
    };
    run.status = if run.cancel_requested {
        "cancelled"
    } else {
        "blocked"
    }
    .into();
    run.save(&deployment.db().pool).await?;
    complete_product(deployment, &run, OrchestrationNodeStatus::Failed).await?;
    if settled {
        admission::release(&deployment.db().pool, id).await?;
    }
    Ok(())
}

async fn complete_product(
    deployment: &DeploymentImpl,
    run: &IntegrationRun,
    status: OrchestrationNodeStatus,
) -> anyhow::Result<()> {
    if run.payload.mechanical {
        return Ok(()); // an adopted legacy product keeps its terminal audit outcome
    }
    let node: Option<Uuid> = sqlx::query_scalar("SELECT id FROM orchestration_node_executions WHERE orchestration_run_id=? AND node_key='integrate'")
        .bind(run.id).fetch_optional(&deployment.db().pool).await?;
    if let Some(node) = node {
        OrchestrationService::new(
            deployment.db().pool.clone(),
            Arc::new(deployment.agent_run_port().clone()),
        )
        .complete_product_node(run.id, node, status)
        .await?;
    }
    Ok(())
}

async fn managed_targets(
    pool: &SqlitePool,
    repo: &Repo,
    run: &IntegrationRun,
) -> anyhow::Result<(Vec<PathBuf>, Vec<Uuid>)> {
    let checkouts: HashSet<_> = git::GitCli::new()
        .list_worktrees(&repo.path)?
        .into_iter()
        .filter(|w| {
            w.branch.as_deref() == Some(run.target_ref.as_str())
                || w.branch.as_deref() == Some(short_target(run))
        })
        .map(|w| std::fs::canonicalize(w.path))
        .collect::<Result<_, _>>()?;
    let paths:Vec<(Uuid,String,String,String,String)>=sqlx::query_as("SELECT w.id,w.container_ref,w.workspace_kind,r.name,r.path FROM workspaces w JOIN workspace_repos wr ON wr.workspace_id=w.id JOIN repos r ON r.id=wr.repo_id WHERE w.container_ref IS NOT NULL AND w.worktree_deleted=0")
        .fetch_all(pool).await?;
    let mut result = vec![repo.path.clone()];
    let mut writers = HashSet::new();
    // Only check writers on worktrees actually holding the target in publish;
    // independent normal development remains allowed.
    for (id, path, kind, name, repository_path) in paths {
        let root = super::repository_checkout(
            Path::new(&path),
            kind == "direct_folder",
            Path::new(&repository_path),
            &name,
        );
        if std::fs::canonicalize(&root).is_ok_and(|p| checkouts.contains(&p)) {
            result.push(root);
            writers.insert(id);
        }
    }
    Ok((result, writers.into_iter().collect()))
}

async fn verify_validation(pool: &SqlitePool, run: &IntegrationRun) -> anyhow::Result<()> {
    let repo = Repo::find_by_id(pool, run.repository_id)
        .await?
        .context("Validation repository missing")?;
    let result = run.payload.result_commit.as_deref().context("Missing R")?;
    if run.payload.skip_validation {
        ensure!(
            run.payload.validation.is_empty()
                && run.payload.gates.is_empty()
                && run.payload.steps.iter().all(|step| step.validation_skipped
                    && step.validation.is_empty()
                    && step.gates.is_empty()),
            "Skipped validation cannot contain execution evidence or validated steps"
        );
        return Ok(());
    }
    ensure!(
        !run.payload.steps.iter().any(|step| step.validation_skipped),
        "Required validation cannot reuse unverified steps"
    );
    ensure!(
        run.payload.validation.iter().any(|v| v.required),
        "VALIDATION_UNAVAILABLE"
    );
    for (index, item) in run.payload.validation.iter().enumerate() {
        let excepted = run.validation_excepted(index);
        let process = ExecutionProcess::find_by_id(
            pool,
            item.execution_process_id
                .context("Unexecuted required validation")?,
        )
        .await?
        .context("Validation evidence missing")?;
        ensure!(
            process.session_id == run.session_id.context("Missing Session")?
                && process.status != ExecutionProcessStatus::Running
                && (!item.required
                    || excepted
                    || (process.status == ExecutionProcessStatus::Completed
                        && process.exit_code == Some(0)))
                && !process.dropped
                && process.run_reason == ExecutionProcessRunReason::IntegrationValidation,
            "Required host validation did not pass"
        );
        let (before, after):(Option<String>,Option<String>)=sqlx::query_as("SELECT before_head_commit,after_head_commit FROM execution_process_repo_states WHERE execution_process_id=? AND repo_id=?").bind(process.id).bind(run.repository_id).fetch_one(pool).await?;
        ensure!(
            before.as_deref() == Some(result) && after.as_deref() == Some(result),
            "Validation was executed on a different commit"
        );
        let action = process
            .executor_action()
            .context("Validation action missing")?;
        let ExecutorActionType::ScriptRequest(script) = action.typ();
        ensure!(
            script.script == item.command
                && script.context == ScriptContext::IntegrationValidation
                && script.language == ScriptRequestLanguage::Bash
                && script.working_dir.as_deref()
                    == Some(
                        Path::new(&repo.name)
                            .join(&item.cwd)
                            .to_string_lossy()
                            .as_ref()
                    )
                && action.next_action().is_none(),
            "Validation plan/evidence mismatch"
        );
    }
    Ok(())
}

async fn publish(deployment: &DeploymentImpl, mut run: IntegrationRun) -> anyhow::Result<()> {
    let admission_guard = services::services::integration_admission::MUTATIONS
        .lock()
        .await;
    let pool = &deployment.db().pool;
    let (repo, _, _, root) = context(deployment, &run).await?;
    admission::verify_sources(deployment, &run).await?;
    admission::idle(deployment, run.workspace_id.unwrap()).await?;
    verify_validation(pool, &run).await?;
    let base = run.payload.base_commit.clone().context("Missing B")?;
    let result = run.payload.result_commit.clone().context("Missing R")?;
    deployment.git().require_clean_source(&root, &result)?;
    let (targets, target_writers) = managed_targets(&deployment.db().pool, &repo, &run).await?;
    for ws in &target_writers {
        admission::idle(deployment, *ws).await?;
    }
    ensure!(
        deployment
            .git()
            .get_branch_oid(&repo.path, short_target(&run))?
            == base,
        "TARGET_CHANGED: target advanced; start a new Integration against it"
    );
    if run.payload.sequential {
        ensure!(
            run.payload.steps.len() == run.payload.sources.len()
                && super::steps::review_approved(&run),
            "Sequential validation/review is incomplete"
        );
    }
    run.payload.publication_intent = true;
    run.status = "publishing".into();
    // Atomic cancellation boundary. This durable intent precedes any Git write.
    let mut tx = pool.begin_with("BEGIN IMMEDIATE").await?;
    for ws in target_writers {
        sqlx::query("INSERT INTO integration_reservations VALUES('workspace',?,?) ON CONFLICT(resource_kind,resource_key) DO NOTHING")
            .bind(resource_key(ws)).bind(run.id).execute(&mut *tx).await?;
        let owner:Uuid=sqlx::query_scalar("SELECT run_id FROM integration_reservations WHERE resource_kind='workspace' AND resource_key=?").bind(resource_key(ws)).fetch_one(&mut *tx).await?;
        ensure!(
            owner == run.id,
            "Target Workspace reserved by another Integration"
        );
        let busy:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM agent_runs WHERE workspace_id=? AND status NOT IN ('succeeded','failed','cancelled','crashed','audit_failed')) OR EXISTS(SELECT 1 FROM execution_processes p JOIN sessions s ON s.id=p.session_id WHERE s.workspace_id=? AND p.status='running')")
            .bind(ws).bind(ws).fetch_one(&mut *tx).await?;
        ensure!(!busy, "Target writer started before publication");
    }
    IntegrationRun::record_publication_intent(&mut tx, &run).await?;
    tx.commit().await?;
    publish_candidate(deployment.git(), pool, &repo, &mut run, &targets).await?;
    drop(admission_guard);
    finish(deployment, run).await
}

/// The real Git/DB hand-off, deliberately not one fictitious transaction. The
/// durable intent above remains authoritative if recording the receipt fails.
async fn publish_candidate(
    git: &git::GitService,
    pool: &SqlitePool,
    repo: &Repo,
    run: &mut IntegrationRun,
    targets: &[PathBuf],
) -> anyhow::Result<()> {
    ensure!(
        run.payload.publication_intent && !run.payload.published,
        "Publication requires an unconsumed durable intent"
    );
    git.promote_exact(
        &repo.path,
        &run.target_ref,
        run.payload.base_commit.as_deref().context("Missing B")?,
        run.payload.result_commit.as_deref().context("Missing R")?,
        targets,
        &run.id.to_string(),
    )?;
    run.payload.published = true;
    run.status = "post_processing".into();
    run.save(pool).await?;
    Ok(())
}

pub(super) async fn recover_publication(
    deployment: &DeploymentImpl,
    mut run: IntegrationRun,
) -> anyhow::Result<()> {
    ensure!(
        run.payload.publication_intent,
        "No durable publication intent; refusing to guess or remerge"
    );
    let admission_guard = services::services::integration_admission::MUTATIONS
        .lock()
        .await;
    let repo = Repo::find_by_id(&deployment.db().pool, run.repository_id)
        .await?
        .context("Repository missing")?;
    let base = run.payload.base_commit.as_deref().context("Missing B")?;
    let result = run.payload.result_commit.as_deref().context("Missing R")?;
    verify_validation(&deployment.db().pool, &run).await?;
    let (targets, writers) = managed_targets(&deployment.db().pool, &repo, &run).await?;
    if !run.payload.published {
        let (_, _, _, root) = context(deployment, &run).await?;
        deployment.git().require_clean_source(&root, result)?;
        admission::idle(
            deployment,
            run.workspace_id.context("Missing integration Workspace")?,
        )
        .await?;
        for ws in writers {
            admission::idle(deployment, ws).await?;
        }
    }
    match deployment.git().inspect_exact_publication(
        &repo.path,
        &run.target_ref,
        base,
        result,
        run.payload.published,
        &targets,
    )? {
        git::publication::ExactPublicationState::Applied => {}
        git::publication::ExactPublicationState::NotApplied => {
            run.status = "blocked".into();
            run.error=Some("Publication did not apply: target ref, index and files still match B. No Git writes were replayed; start a new Integration after inspection.".into());
            run.save(&deployment.db().pool).await?;
            complete_product(deployment, &run, OrchestrationNodeStatus::Failed).await?;
            admission::release(&deployment.db().pool, run.id).await?;
            return Ok(());
        }
    }
    run.payload.published = true;
    run.status = "post_processing".into();
    run.error = None;
    run.save(&deployment.db().pool).await?;
    drop(admission_guard);
    finish(deployment, run).await
}

async fn finish(deployment: &DeploymentImpl, mut run: IntegrationRun) -> anyhow::Result<()> {
    let pool = &deployment.db().pool;
    ensure!(
        run.payload.published,
        "Post-processing requires confirmed Git publication"
    );
    // Card effects are independently retryable from semantic outbox/Wiki. They
    // must not be undone or indefinitely reserved when a Wiki file is damaged.
    run = complete_cards(deployment, run).await?;
    let wiki_result = record_memory_integration(deployment, &run).await;
    run.payload.wiki_result = Some(match wiki_result {
        Ok(result) => result,
        Err(error) => format!("semantic outbox pending: {error:#}"),
    });
    run.save(pool).await?;
    complete_product(deployment, &run, OrchestrationNodeStatus::Succeeded).await?;
    run.status = "succeeded".into();
    run.save(pool).await?;
    admission::release(pool, run.id).await?;
    Ok(())
}

async fn complete_cards(
    deployment: &DeploymentImpl,
    run: IntegrationRun,
) -> anyhow::Result<IntegrationRun> {
    let result = IntegrationRun::complete_cards(&deployment.db().pool, run.id).await?;
    admission::release(&deployment.db().pool, run.id).await?;
    Ok(result)
}

async fn record_memory_integration(
    deployment: &DeploymentImpl,
    run: &IntegrationRun,
) -> anyhow::Result<String> {
    let pool = &deployment.db().pool;
    let repo = Repo::find_by_id(pool, run.repository_id)
        .await?
        .context("Repository missing")?;
    if let Some(store) = RepositoryMemoryStore::existing_for_repository(&repo.name, repo.id)? {
        record_memory_integration_at(deployment.git(), &repo, run, &store)
    } else {
        Ok("repository memory not configured".into())
    }
}

fn record_memory_integration_at(
    git: &git::GitService,
    repo: &Repo,
    run: &IntegrationRun,
    store: &RepositoryMemoryStore,
) -> anyhow::Result<String> {
    let state = store.state()?;
    if !state.enabled {
        // Git/Done and the frozen semantic proposal are already durable in
        // Integration. Disabling Memory must not create new memory records or
        // make completion depend on old unavailable semantic inputs.
        return Ok("disabled".into());
    }
    let result = run.payload.result_commit.clone().context("Missing R")?;
    let _guard = store.try_integration_lock()?;
    let mut ids = Vec::new();
    for source in &run.payload.sources {
        for id in &source.event_ids {
            let event = store.event(*id)?.context(
                "Frozen Manifest missing; source retained, Wiki post-processing blocked",
            )?;
            ensure!(
                event.repository_id == repo.id
                    && event.workspace_id == source.selection.workspace_id
                    && git.is_ancestor(&repo.path, &event.source_commit, &source.commit)?,
                "Frozen Manifest/source identity mismatch"
            );
            ids.push(*id);
        }
    }
    if let Some(event) = &run.payload.integration_manifest {
        store.publish_event(event)?;
        ids.push(event.event_id);
    }
    ids.sort();
    ids.dedup();
    let integration = MemoryIntegration {
        id: run.id,
        event_ids: ids,
        target_branch: short_target(run).into(),
        before_commit: run.payload.base_commit.clone().context("Missing B")?,
        integrated_commit: Some(result),
        source_error: None,
    };
    if let Some(existing) = store
        .integrations()?
        .into_iter()
        .find(|record| record.id == run.id)
    {
        ensure!(
            existing.event_ids == integration.event_ids
                && existing.target_branch == integration.target_branch
                && existing.before_commit == integration.before_commit
                && existing.integrated_commit == integration.integrated_commit,
            "Immutable Integration input changed; refusing to replace its semantic event binding"
        );
    } else {
        store.save_integration(&integration)?;
    }
    Ok(
        if state.target_branch.as_deref() != Some(short_target(run)) {
            "recorded; Wiki configured for another target"
        } else {
            "pending existing OpenWiki reconciliation"
        }
        .into(),
    )
}
