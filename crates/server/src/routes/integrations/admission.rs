use std::collections::HashSet;

use anyhow::{Context, ensure};
use db::models::{
    integration::{IntegrationPayload, IntegrationRun, IntegrationSelection, IntegrationSource},
    repo::Repo,
    workspace::Workspace,
    workspace_repo::WorkspaceRepo,
};
use deployment::Deployment;
use executors::{
    executors::BaseCodingAgent,
    model_selector::PermissionPolicy,
    profile::{ExecutionMode, ExecutorConfig},
};
use services::services::container::ContainerService;
use sqlx::{Row, SqlitePool};
use utils::repository_memory::RepositoryMemoryStore;
use uuid::Uuid;

use super::CreateIntegrationRequest;
use crate::DeploymentImpl;

#[derive(Debug, thiserror::Error)]
pub(super) enum SelectionChanged {
    #[error(
        "TARGET_CHANGED: target HEAD changed since selection; inspect and explicitly reselect it"
    )]
    Target,
    #[error(
        "SOURCE_CHANGED: adopted Workspace HEAD changed since selection; explicitly reselect the observed commit"
    )]
    Source,
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum AdmissionFailure {
    #[error("{message}")]
    Rejected {
        code: &'static str,
        message: &'static str,
    },
    #[error("Admission outcome unavailable; inspect the request before retrying")]
    Uncertain,
}

pub(super) async fn idle(deployment: &DeploymentImpl, workspace: Uuid) -> anyhow::Result<()> {
    let pool = &deployment.db().pool;
    services::services::resource_coordination::guard_workspace_idle(pool, workspace).await?;
    let active: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM agent_runs WHERE workspace_id=? AND status NOT IN ('succeeded','failed','cancelled','crashed','audit_failed')) OR EXISTS(SELECT 1 FROM execution_processes p JOIN sessions s ON s.id=p.session_id WHERE s.workspace_id=? AND p.status='running') OR EXISTS(SELECT 1 FROM agent_process_registry p JOIN agent_run_attempts a ON a.id=p.run_attempt_id JOIN agent_runs r ON r.id=a.agent_run_id WHERE r.workspace_id=? AND p.registry_status IN ('spawned','running','unreachable'))")
        .bind(workspace).bind(workspace).bind(workspace).fetch_one(pool).await?;
    ensure!(
        !active,
        "Workspace {workspace} has an active or unconfirmed writer"
    );
    ensure!(
        deployment.container().scripts_settled(workspace).await?,
        "Workspace {workspace} has unfinished script finalisation/process cleanup"
    );
    Ok(())
}

pub(super) async fn source(
    deployment: &DeploymentImpl,
    request: &CreateIntegrationRequest,
    selected: &IntegrationSelection,
) -> anyhow::Result<IntegrationSource> {
    inspect_source(deployment, request, selected, true).await
}

async fn inspect_source(
    deployment: &DeploymentImpl,
    request: &CreateIntegrationRequest,
    selected: &IntegrationSelection,
    inspect_memory: bool,
) -> anyhow::Result<IntegrationSource> {
    let pool = &deployment.db().pool;
    let row=sqlx::query("SELECT i.title,i.description,i.status_id,i.integration_revision FROM local_issues i JOIN local_workspace_links l ON l.issue_id=i.id WHERE i.id=? AND i.project_id=? AND l.workspace_id=?")
        .bind(selected.card_id).bind(request.project_id).bind(selected.workspace_id).fetch_optional(pool).await?.context("Adopted Workspace must be currently linked to the selected local Card/project")?;
    let repo = Repo::find_by_id(pool, request.repository_id)
        .await?
        .context("Repository missing")?;
    let workspace = Workspace::find_by_id(pool, selected.workspace_id)
        .await?
        .context("Workspace missing")?;
    WorkspaceRepo::find_by_workspace_and_repo_id(pool, workspace.id, repo.id)
        .await?
        .context("Selected Workspace does not contain this repository")?;
    let commit = deployment
        .git()
        .get_branch_oid(&repo.path, &workspace.branch)?;
    if commit != selected.expected_commit {
        return Err(SelectionChanged::Source.into());
    }
    // Only this immutable commit is selected. Uncommitted work, Goals, other
    // Workspaces and other repositories are not part of this operation.
    let mut event_ids = Vec::new();
    if inspect_memory
        && let Some(store) =
            RepositoryMemoryStore::read_only_configured_for_repository(&repo.name, repo.id)?
    {
        let mut after = None;
        loop {
            let page = match store.discover_events(repo.id, after.as_deref(), 100) {
                Ok(page) => page,
                Err(error) => {
                    tracing::warn!(%error, "Integration memory discovery unavailable");
                    break;
                }
            };
            for event in page.records {
                if event.workspace_id == workspace.id
                    && deployment
                        .git()
                        .is_ancestor(&repo.path, &event.source_commit, &commit)
                        .unwrap_or(false)
                {
                    event_ids.push(event.event_id);
                }
            }
            after = page.next_after;
            if after.is_none() {
                break;
            }
        }
    }
    event_ids.sort();
    Ok(IntegrationSource {
        selection: selected.clone(),
        branch: workspace.branch,
        commit,
        title: row.try_get("title")?,
        description: row.try_get("description")?,
        status_id: row.try_get("status_id")?,
        requirements_revision: row.try_get("integration_revision")?,
        event_ids,
        related_workspaces: vec![selected.workspace_id],
        done_result: None,
    })
}

/// Capture the local target used by the formal frozen candidate. The caller
/// holds the existing admission mutation lock; publication rechecks it later.
pub(super) fn observe_target(
    git: &git::GitService,
    repository: &std::path::Path,
    branch: &str,
    expected: Option<&str>,
) -> anyhow::Result<String> {
    let observed = git.get_branch_oid(repository, branch)?;
    if expected.is_some_and(|expected| expected != observed) {
        return Err(SelectionChanged::Target.into());
    }
    Ok(observed)
}

pub(super) async fn submit(
    deployment: &DeploymentImpl,
    request: CreateIntegrationRequest,
) -> anyhow::Result<IntegrationRun> {
    submit_with_expected_target(deployment, request, None).await
}

pub(super) async fn submit_with_expected_target(
    deployment: &DeploymentImpl,
    request: CreateIntegrationRequest,
    expected_target: Option<&str>,
) -> anyhow::Result<IntegrationRun> {
    let _admission = services::services::integration_admission::MUTATIONS
        .lock()
        .await;
    let pool = &deployment.db().pool;
    ensure!(
        !request.selections.is_empty() && request.selections.len() <= 100,
        "Select one or more Cards (request safety bound exceeded)"
    );
    if let Some(config) = &request.executor_config {
        ensure!(
            config.execution_mode.unwrap_or_default() == ExecutionMode::Code
                && config.permission_policy != Some(PermissionPolicy::Plan),
            "Formal Integration requires Code mode; independent Goal/Plan continuations cannot publish a host-validated result"
        );
    }
    let cards: HashSet<_> = request.selections.iter().map(|s| s.card_id).collect();
    let workspaces: HashSet<_> = request.selections.iter().map(|s| s.workspace_id).collect();
    ensure!(
        cards.len() == request.selections.len() && workspaces.len() == cards.len(),
        "Use exactly one adopted Workspace per Card"
    );
    let target = request
        .target_branch
        .strip_prefix("refs/heads/")
        .unwrap_or(&request.target_branch);
    ensure!(
        !target.starts_with("refs/") && !target.starts_with('-'),
        "Choose a local target branch"
    );
    let target_ref = format!("refs/heads/{target}");
    let key = request.request_id.to_string();
    if let Some(run) =
        sqlx::query_as::<_, IntegrationRun>("SELECT * FROM integration_runs WHERE request_key=?")
            .bind(&key)
            .fetch_optional(pool)
            .await?
    {
        matches_request(&run, &request, &target_ref, expected_target)?;
        return Ok(run);
    }
    let repo = Repo::find_by_id(pool, request.repository_id)
        .await?
        .context("Repository missing")?;
    let registered = super::project_repository_ids(pool, request.project_id)
        .await?
        .contains(&repo.id);
    ensure!(registered, "Repository must belong to this local project");
    let observed_target = observe_target(deployment.git(), &repo.path, target, expected_target)?;
    let storage = deployment
        .git()
        .storage_identity(&repo.path)?
        .to_string_lossy()
        .into_owned();
    let mut sources = Vec::new();
    for selection in &request.selections {
        sources.push(source(deployment, &request, selection).await?);
    }
    let id = Uuid::new_v4();
    let card_id = Uuid::new_v4();
    let workspace_id = Uuid::new_v4();
    let name = format!(
        "Auto merge: {} → {}",
        sources
            .iter()
            .map(|s| s.title.as_str())
            .collect::<Vec<_>>()
            .join(" · "),
        target
    );
    let branch = deployment
        .container()
        .git_branch_from_workspace(&workspace_id, &name)
        .await;
    let mut payload = IntegrationPayload {
        mechanical: true,
        sources,
        observed_target,
        card_id: Some(card_id),
        sequential: true,
        skip_validation: request
            .resolved_skip_validation(deployment.config().read().await.integration_skip_validation),
        executor_config: Some(serde_json::to_value(
            request
                .executor_config
                .clone()
                .unwrap_or_else(|| ExecutorConfig::new(BaseCodingAgent::Codex)),
        )?),
        ..Default::default()
    };
    payload.validation = super::mechanical::validation_plan(
        &repo.path,
        &payload.observed_target,
        payload.skip_validation,
    )?;
    let run = IntegrationRun {
        id,
        request_key: key,
        project_id: request.project_id,
        repository_id: repo.id,
        storage_identity: storage,
        target_ref,
        status: "queued".into(),
        workspace_id: Some(workspace_id),
        session_id: None,
        agent_run_id: None,
        payload: sqlx::types::Json(payload),
        cancel_requested: false,
        error: None,
        created_at: String::new(),
        updated_at: String::new(),
    };
    run.create_with_card(pool, &branch, &name).await?;
    Ok(IntegrationRun::find(pool, id).await?)
}

pub(super) async fn verify_sources(
    deployment: &DeploymentImpl,
    run: &IntegrationRun,
) -> anyhow::Result<()> {
    let repo = Repo::find_by_id(&deployment.db().pool, run.repository_id)
        .await?
        .context("Repository missing")?;
    for frozen in &run.payload.sources {
        ensure!(
            deployment
                .git()
                .is_ancestor(&repo.path, &frozen.commit, &frozen.commit)?,
            "Selected commit is unavailable"
        );
    }
    Ok(())
}

pub(super) async fn release(pool: &SqlitePool, id: Uuid) -> anyhow::Result<()> {
    sqlx::query("DELETE FROM integration_reservations WHERE run_id=?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub(super) async fn target_right(pool: &SqlitePool, run: &IntegrationRun) -> anyhow::Result<bool> {
    // Conservative repository-wide serialisation, keyed by canonical Git
    // storage, even when the repository is registered under another ID.
    let first:Option<Uuid>=sqlx::query_scalar("SELECT id FROM integration_runs WHERE storage_identity=? AND status IN ('queued','preparing','integrating','validating','publishing','post_processing','cancelling','recovery_required') ORDER BY created_at,id LIMIT 1")
        .bind(&run.storage_identity).fetch_optional(pool).await?;
    if first != Some(run.id) {
        return Ok(false);
    }
    sqlx::query("INSERT INTO integration_reservations(resource_kind,resource_key,run_id) VALUES('target',?,?) ON CONFLICT(resource_kind,resource_key) DO NOTHING")
        .bind(&run.storage_identity).bind(run.id).execute(pool).await?;
    let owner:Uuid=sqlx::query_scalar("SELECT run_id FROM integration_reservations WHERE resource_kind='target' AND resource_key=?").bind(&run.storage_identity).fetch_one(pool).await?;
    Ok(owner == run.id)
}

fn matches_request(
    run: &IntegrationRun,
    request: &CreateIntegrationRequest,
    target_ref: &str,
    expected_target: Option<&str>,
) -> anyhow::Result<()> {
    ensure!(
        run.project_id == request.project_id
            && run.repository_id == request.repository_id
            && run.target_ref == target_ref
            && request
                .skip_validation
                .is_none_or(|choice| run.payload.skip_validation == choice)
            && expected_target.is_none_or(|expected| expected == run.payload.observed_target)
            && run.payload.executor_config
                == Some(serde_json::to_value(
                    request
                        .executor_config
                        .clone()
                        .unwrap_or_else(|| ExecutorConfig::new(BaseCodingAgent::Codex))
                )?)
            && run.payload.sources.len() == request.selections.len()
            && run
                .payload
                .sources
                .iter()
                .zip(&request.selections)
                .all(|(s, r)| s.selection.card_id == r.card_id
                    && s.selection.workspace_id == r.workspace_id
                    && s.selection.expected_commit == r.expected_commit),
        "Idempotency conflict: explicit selection changed"
    );
    Ok(())
}

/// Called only after synchronous admission has returned an error. No admission
/// task remains running behind that return. Recheck its durable reservation under
/// the same mutation lock before distinguishing rejection from lost acknowledgement.
pub(super) async fn reconcile_failed_admission(
    pool: &SqlitePool,
    request: &CreateIntegrationRequest,
    expected_target: &str,
    error: &anyhow::Error,
) -> Result<IntegrationRun, AdmissionFailure> {
    let _admission = services::services::integration_admission::MUTATIONS
        .lock()
        .await;
    let record =
        sqlx::query_as::<_, IntegrationRun>("SELECT * FROM integration_runs WHERE request_key=?")
            .bind(request.request_id.to_string())
            .fetch_optional(pool)
            .await
            .map_err(|_| AdmissionFailure::Uncertain)?;
    if let Some(run) = record {
        let target = request
            .target_branch
            .strip_prefix("refs/heads/")
            .unwrap_or(&request.target_branch);
        matches_request(
            &run,
            request,
            &format!("refs/heads/{target}"),
            Some(expected_target),
        )
        .map_err(|_| AdmissionFailure::Uncertain)?;
        return Ok(run);
    }
    let (code, message) = match error.downcast_ref::<SelectionChanged>() {
        Some(SelectionChanged::Target) => (
            "target_changed",
            "Target HEAD changed; refresh candidates and explicitly reselect the target",
        ),
        Some(SelectionChanged::Source) => (
            "source_changed",
            "Source HEAD changed; refresh and explicitly reselect the adopted Workspace",
        ),
        None if matches!(
            error.downcast_ref::<git::GitServiceError>(),
            Some(git::GitServiceError::WorktreeDirty(..))
        ) =>
        {
            (
                "source_dirty",
                "Source has uncommitted changes; finish the work before Integration",
            )
        }
        None => (
            "integration_not_admitted",
            "Formal Integration was not admitted; inspect source readiness, writer state and the selected target before a new request",
        ),
    };
    Err(AdmissionFailure::Rejected { code, message })
}
