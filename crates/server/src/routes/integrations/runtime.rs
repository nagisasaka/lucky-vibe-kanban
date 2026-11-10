use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    sync::{LazyLock, Mutex},
    time::Duration,
};

use anyhow::{Context, bail, ensure};
use db::models::{
    execution_process::{ExecutionProcess, ExecutionProcessStatus},
    integration::{IntegrationRun, resource_key},
    repo::Repo,
    repository_state::RepositoryMemoryStore,
    session::Session,
    workspace::Workspace,
};
use deployment::Deployment;
use services::services::container::ContainerService;
use sqlx::SqlitePool;
use uuid::Uuid;

use super::admission;
use crate::DeploymentImpl;

// Only task de-duplication; correctness lives in durable reservations and
// owner identities, not this ephemeral set or a dispatcher TTL.
static IN_FLIGHT: LazyLock<Mutex<HashSet<Uuid>>> = LazyLock::new(|| Mutex::new(HashSet::new()));
pub(crate) struct IntegrationActionGuard(Uuid);
impl Drop for IntegrationActionGuard {
    fn drop(&mut self) {
        IN_FLIGHT
            .lock()
            .expect("Auto-merge task set")
            .remove(&self.0);
    }
}
pub(crate) fn reserve_action(id: Uuid) -> anyhow::Result<IntegrationActionGuard> {
    ensure!(
        IN_FLIGHT.lock().expect("Auto-merge task set").insert(id),
        "Auto-merge is busy; retry after the current operation settles"
    );
    Ok(IntegrationActionGuard(id))
}

#[cfg(test)]
#[path = "runtime_tests.rs"]
mod tests;

pub(super) async fn request_recovery(
    deployment: &DeploymentImpl,
    run: IntegrationRun,
) -> anyhow::Result<()> {
    ensure!(
        IN_FLIGHT
            .lock()
            .expect("Auto-merge task set")
            .insert(run.id),
        "This Auto-merge is already being reconciled"
    );
    let id = run.id;
    let result = async {
        if run.payload.published {
            finish(deployment, run).await
        } else {
            recover_publication(deployment, run).await
        }
    }
    .await;
    IN_FLIGHT.lock().expect("Auto-merge task set").remove(&id);
    result
}

// Admission freezes event IDs after checking source ancestry. Resolve only those
// immutable public records, never another Workspace's private memory or history.
pub(super) fn integration_memory(
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

pub fn spawn_monitor(deployment: DeploymentImpl) {
    tokio::spawn(async move {
        loop {
            let runs = IntegrationRun::active(&deployment.db().pool).await;
            match runs {
                Ok(runs) => {
                    for run in runs {
                        if !IN_FLIGHT
                            .lock()
                            .expect("Auto-merge task set")
                            .insert(run.id)
                        {
                            continue;
                        }
                        let deployment = deployment.clone();
                        tokio::spawn(async move {
                            let id = run.id;
                            if let Err(error) = super::conversation::advance(&deployment, run).await
                            {
                                tracing::warn!(run_id=%id,%error,"Auto-merge held");
                                if let Err(error) =
                                    hold(&deployment, id, format!("{error:#}")).await
                                {
                                    tracing::error!(run_id=%id,%error,"Auto-merge recovery required; reservations retained");
                                }
                            }
                            IN_FLIGHT.lock().expect("Auto-merge task set").remove(&id);
                        });
                    }
                }
                Err(error) => tracing::warn!(%error,"Auto-merge monitor unavailable"),
            }
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    });
}

fn short_target(run: &IntegrationRun) -> &str {
    run.target_ref
        .strip_prefix("refs/heads/")
        .unwrap_or(&run.target_ref)
}

pub(super) async fn context(
    deployment: &DeploymentImpl,
    run: &IntegrationRun,
) -> anyhow::Result<(Repo, Workspace, Session, PathBuf)> {
    let pool = &deployment.db().pool;
    let repo = db::models::workspace_repo::WorkspaceRepo::find_repo_for_workspace(
        pool,
        run.workspace_id.context("Auto-merge Workspace missing")?,
        run.repository_id,
    )
    .await?
    .context("Repository missing")?;
    let workspace = Workspace::find_by_id(
        pool,
        run.workspace_id.context("Auto-merge Workspace missing")?,
    )
    .await?
    .context("Workspace missing")?;
    let session = Session::find_by_id(pool, run.session_id.context("Auto-merge Session missing")?)
        .await?
        .context("Session missing")?;
    let root = PathBuf::from(
        workspace
            .container_ref
            .as_ref()
            .context("Workspace checkout missing")?,
    )
    .join(repo.worktree_name());
    Ok((repo, workspace, session, root))
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

pub(super) async fn cancel(
    deployment: &DeploymentImpl,
    mut run: IntegrationRun,
) -> anyhow::Result<()> {
    ensure!(
        !run.payload.publication_intent,
        "Publication boundary passed; cancellation cannot undo Git"
    );
    run.status = "cancelling".into();
    run.agent_run_id = sqlx::query_scalar(
        "SELECT id FROM agent_runs WHERE session_id=? ORDER BY created_at DESC,id DESC LIMIT 1",
    )
    .bind(run.session_id)
    .fetch_optional(&deployment.db().pool)
    .await?;
    run.save(&deployment.db().pool).await?;
    stop_validation(deployment, &run).await?;
    if let Some(agent) = run.agent_run_id {
        let accepted =
            db::models::agent_runtime::AgentRunRecord::find(&deployment.db().pool, agent)
                .await?
                .context("Accepted Agent disappeared")?;
        ensure!(
            accepted.workspace_id == run.workspace_id.context("Missing workspace")?
                && Some(accepted.session_id) == run.session_id
                && accepted.correlation_id == run.id,
            "Cancellation worker ownership mismatch"
        );
        services::services::agent_runtime::AgentRunCommandService::new(
            &deployment.db().pool,
            deployment.agent_run_port(),
        )
        .dispatch(executors::runtime::AgentRunPortCommandEnvelope {
            schema_version: executors::runtime::ORCHESTRATION_COMMAND_SCHEMA_VERSION,
            command_id: Uuid::from_u128(agent.as_u128() ^ 0x269fcd3f937a48a087adbc4ee766ac0e),
            idempotency_key: format!("integration:{}:cancel-worker:{agent}", run.id),
            agent_run_id: agent,
            orchestration_run_id: None,
            orchestration_node_execution_id: None,
            correlation_id: run.id,
            created_at: accepted.request_envelope.created_at,
            command: executors::runtime::AgentRunPortCommand::Cancel {
                reason: "Auto-merge cancellation requested".into(),
            },
        })
        .await?;
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
    let mut conversation = IntegrationRun::find(&deployment.db().pool, id).await?;
    if conversation.payload.publication_intent {
        conversation
            .payload
            .history
            .push(serde_json::json!({"event":"publication_error","message":error}));
        conversation.save(&deployment.db().pool).await?;
        return match recover_publication(deployment, conversation.clone()).await {
            Ok(()) => Ok(()),
            Err(recovery) => {
                conversation.status = "recovery_required".into();
                conversation.error = Some(format!(
                    "公開結果の照合が必要です。未公開とは判定できないため、Gitの再実行や予約解放は行いません。{recovery:#}"
                ));
                let event = serde_json::json!({"event":"publication_recovery_required","operation_error":error,"reconciliation_error":format!("{recovery:#}")});
                conversation.payload.history.push(event);
                conversation.save(&deployment.db().pool).await?;
                Ok(())
            }
        };
    }
    super::conversation::stop(deployment, conversation, error).await
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

pub(super) async fn publish(
    deployment: &DeploymentImpl,
    mut run: IntegrationRun,
) -> anyhow::Result<()> {
    let admission_guard = services::services::integration_admission::MUTATIONS
        .lock()
        .await;
    let pool = &deployment.db().pool;
    let (repo, workspace, _, root) = context(deployment, &run).await?;
    super::support::require_private_branch(&root, &workspace.branch, &run.target_ref)?;
    admission::verify_sources(deployment, &run).await?;
    admission::idle(deployment, run.workspace_id.unwrap()).await?;
    super::conversation::verify_evidence(pool, &run).await?;
    let current = deployment
        .git()
        .get_branch_oid(&repo.path, short_target(&run))?;
    if !deployment.git().is_ancestor(
        &repo.path,
        &current,
        run.payload
            .result_commit
            .as_deref()
            .context("Missing candidate")?,
    )? {
        run.payload.stop=Some(db::models::integration::IntegrationStop {stage:"publication".into(),category:"target_changed".into(),next_action:"main更新により未反映です。候補と検証記録を保持しました。新たな指示でまとめ直すか、同じ会話を再開できます。".into()});
        run.save(pool).await?;
        bail!("main更新により未反映。候補を保存しました。");
    }
    // Publish the unchanged candidate using CAS against the observed current target.
    run.payload.base_commit = Some(current);
    let base = run.payload.base_commit.clone().context("Missing B")?;
    let result = run.payload.result_commit.clone().context("Missing R")?;
    let (targets, target_writers) = managed_targets(&deployment.db().pool, &repo, &run).await?;
    for ws in &target_writers {
        admission::idle(deployment, *ws).await?;
    }
    if run.payload.wiki_enabled {
        let request = run
            .payload
            .publication_request
            .as_ref()
            .context("Missing publication request")?;
        let store = RepositoryMemoryStore::existing_for_repository(
            &deployment.db().pool,
            &repo.name,
            repo.id,
        )
        .await?
        .context("Missing setup journal store")?;
        super::conversation::validate_prepared_wiki_proof(pool, &run, &store).await?;
        services::services::openwiki::setup::integration::restore(
            &store,
            &super::conversation::wiki_setup_identity(&run, &root, &request.code_commit)?,
            &result,
        )?;
    }
    deployment.git().require_clean_source(&root, &result)?;
    ensure!(
        deployment
            .git()
            .get_branch_oid(&repo.path, short_target(&run))?
            == base,
        "TARGET_CHANGED: target advanced; start a new Auto-merge against it"
    );
    run.payload.publication_intent = true;
    run.status = "publishing".into();
    // Atomic cancellation boundary. This durable intent precedes any Git write.
    let mut tx = pool.begin_with("BEGIN IMMEDIATE").await?;
    sqlx::query("INSERT INTO integration_reservations VALUES('target',?,?) ON CONFLICT(resource_kind,resource_key) DO NOTHING")
            .bind(&run.storage_identity).bind(run.id).execute(&mut *tx).await?;
    let owner: Uuid=sqlx::query_scalar("SELECT run_id FROM integration_reservations WHERE resource_kind='target' AND resource_key=?")
            .bind(&run.storage_identity).fetch_one(&mut *tx).await?;
    ensure!(
        owner == run.id,
        "Target is owned by an unsettled integration"
    );
    for ws in target_writers {
        sqlx::query("INSERT INTO integration_reservations VALUES('workspace',?,?) ON CONFLICT(resource_kind,resource_key) DO NOTHING")
            .bind(resource_key(ws)).bind(run.id).execute(&mut *tx).await?;
        let owner:Uuid=sqlx::query_scalar("SELECT run_id FROM integration_reservations WHERE resource_kind='workspace' AND resource_key=?").bind(resource_key(ws)).fetch_one(&mut *tx).await?;
        ensure!(
            owner == run.id,
            "Target Workspace reserved by another Auto-merge"
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
    let event = serde_json::json!({"event":"target_published","id":run.id,"candidate_commit":run.payload.result_commit});
    run.payload.history.push(event);
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
    super::conversation::verify_evidence(&deployment.db().pool, &run).await?;
    let (targets, writers) = managed_targets(&deployment.db().pool, &repo, &run).await?;
    if !run.payload.published {
        let (_, _, _, root) = context(deployment, &run).await?;
        deployment.git().require_clean_source(&root, result)?;
        admission::idle(
            deployment,
            run.workspace_id.context("Missing Auto-merge Workspace")?,
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
            let request = run.payload.publication_request.clone();
            let event = serde_json::json!({"event":"publication_not_applied","request":request,"base":base,"candidate":result});
            run.payload.history.push(event);
            run.payload.publication_intent = false;
            run.payload.publication_request = None;
            return Box::pin(super::conversation::stop(
                    deployment,
                    run,
                    "公開は未適用です。公開intentと実Gitの照合結果を保持しました。同じ会話へ次の指示を送ってください。".into(),
                ))
                .await;
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
    // Card effects are independently retryable from Wiki receipt bookkeeping. They
    // must not be undone or indefinitely reserved when Wiki bookkeeping fails.
    run = complete_cards(deployment, run).await?;
    let repo = db::models::workspace_repo::WorkspaceRepo::find_repo_for_workspace(
        pool,
        run.workspace_id.context("Auto-merge Workspace missing")?,
        run.repository_id,
    )
    .await?
    .context("Repository missing")?;
    let mut tx = pool.begin_with("BEGIN IMMEDIATE").await?;
    let wiki_result = super::conversation::record_wiki_result(pool, &mut tx, &repo, &run).await;
    run.status = "succeeded".into();
    match wiki_result {
        Ok(result) => {
            run.payload.wiki_result = Some(result);
            run.save_on(&mut tx).await?;
            tx.commit().await?;
        }
        Err(error) => {
            tx.rollback().await?;
            run.payload.wiki_result = Some(format!("Wiki receipt pending: {error:#}"));
            run.save(pool).await?;
        }
    }
    admission::release(pool, run.id).await?;
    if let Some(workspace) = Workspace::find_by_id(pool, run.workspace_id.unwrap()).await? {
        crate::routes::local_remote::issue_archive::archive_generated(deployment, workspace).await;
    }
    Ok(())
}

async fn complete_cards(
    deployment: &DeploymentImpl,
    run: IntegrationRun,
) -> anyhow::Result<IntegrationRun> {
    let mut heads = std::collections::HashMap::new();
    for source in &run.payload.sources {
        if source.done_result.is_some() {
            continue;
        }
        let repo = db::models::workspace_repo::WorkspaceRepo::find_repo_for_workspace(
            &deployment.db().pool,
            source.selection.workspace_id,
            run.repository_id,
        )
        .await?
        .context("Source repository missing during Done reconciliation")?;
        let workspace = Workspace::find_by_id(&deployment.db().pool, source.selection.workspace_id)
            .await?
            .context("Source Workspace missing during Done reconciliation")?;
        let container = workspace
            .container_ref
            .as_ref()
            .context("Source checkout missing during Done reconciliation")?;
        let root = super::repository_checkout(
            Path::new(container),
            workspace.is_direct_folder(),
            &repo.path,
            repo.worktree_name(),
        );
        let head = super::support::git(&root, &["rev-parse", "HEAD"])?;
        heads.insert(workspace.id, head);
    }
    let result = IntegrationRun::complete_cards(&deployment.db().pool, run.id, &heads).await?;
    admission::release(&deployment.db().pool, run.id).await?;
    Ok(result)
}
