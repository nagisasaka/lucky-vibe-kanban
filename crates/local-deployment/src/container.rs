use std::{
    collections::HashMap,
    io,
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant},
};

use anyhow::anyhow;
use async_trait::async_trait;
use command_group::AsyncGroupChild;
use db::{
    DBService,
    models::{
        execution_process::{
            ExecutionContext, ExecutionProcess, ExecutionProcessRunReason, ExecutionProcessStatus,
        },
        execution_process_repo_state::ExecutionProcessRepoState,
        repo::Repo,
        scratch::DraftFollowUpData,
        session::{Session, SessionError},
        workspace::{Workspace, WorkspaceKind},
        workspace_repo::WorkspaceRepo,
    },
};
use executors::{
    actions::{Executable, ExecutorAction},
    env::{ExecutionEnv, RepoContext},
    executors::{
        CancellationToken, ExecutorExitSignal,
        provider_adapter::{DirectProvider, require_capability},
    },
    profile::runtime_profile_ids_match,
    provider_policy::direct_provider_capability_snapshot,
    runtime::{
        AGENT_REQUEST_PAYLOAD_VERSION, AGENT_REQUEST_SCHEMA_VERSION, AgentCapability,
        AgentRunIntent, AgentRunPort, AgentRunPortError, AgentRunRequestEnvelope, AgentRunStatus,
        AgentRuntimeMessageRole, CanonicalMessage, ProviderSessionReference, RunAttemptMode,
        RunAttemptRequest, WorkspaceMode, WorkspaceReference,
    },
};
use futures::{FutureExt, TryStreamExt, stream::select};
use git::GitService;
use services::services::{
    analytics::AnalyticsContext,
    approvals::Approvals,
    config::Config,
    container::{ContainerError, ContainerRef, ContainerService},
    diff_stream::{self, DiffStreamHandle},
    execution_process::{ExecutionCompletedEvent, publish_execution_completed},
    file::FileService,
    notification::NotificationService,
    queued_message::QueuedMessageService,
};
use sqlx::{SqlitePool, types::Json};
use tokio::{sync::RwLock, task::JoinHandle};
use tokio_util::io::ReaderStream;
use utils::{
    log_msg::LogMsg,
    msg_store::MsgStore,
    text::{git_branch_id, short_uuid},
};
use uuid::Uuid;
use workspace_manager::{RepoWorkspaceInput, WorkspaceError, WorkspaceManager};

use crate::{
    agent_process_registry::{AgentProcessRegistry, RegisteredAgentProcess},
    agent_run_port::{AgentRunTerminalEvent, LocalAgentRunPort},
    agent_runtime_supervisor, command, copy, process_supervisor,
};

const WORKSPACE_TOUCH_DEBOUNCE: Duration = Duration::from_mins(2);

fn trace_launch_diagnostic(
    execution_process_id: Uuid,
    outcome: &agent_runtime_supervisor::AgentRuntimeSupervisorOutcome,
) {
    tracing::warn!(
        execution_process_id = %execution_process_id,
        lifecycle = ?outcome.lifecycle,
        process_status = ?outcome.process_status,
        error_kind = ?outcome.runtime_error.as_ref().map(|error| error.kind),
        launch_phase = ?outcome.runtime_error.as_ref().and_then(|error| error.launch_phase),
        "classified agent runtime launch failure"
    );
}

pub(crate) async fn should_disable_default_commit_for_workspace(
    pool: &SqlitePool,
    workspace_id: Uuid,
) -> Result<bool, sqlx::Error> {
    use db::models::arena_group::{ArenaLifecycleStatus, ArenaMode};

    if sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM workspaces w LEFT JOIN integration_runs i ON i.workspace_id=w.id WHERE w.id=? AND (w.usage='execution_only' OR i.workspace_id IS NOT NULL))",
    )
    .bind(workspace_id)
    .fetch_one(pool)
    .await?
    {
        return Ok(true);
    }

    let group = Workspace::find_arena_group_for_workspace(pool, workspace_id).await?;

    Ok(matches!(
        group.map(|g| (g.mode, g.lifecycle_status)),
        Some((ArenaMode::Design, ArenaLifecycleStatus::Open))
    ))
}

#[derive(Clone)]
pub struct LocalContainerService {
    db: DBService,
    agent_run_port: LocalAgentRunPort,
    child_store: Arc<RwLock<HashMap<Uuid, Arc<RwLock<AsyncGroupChild>>>>>,
    cancellation_tokens: Arc<RwLock<HashMap<Uuid, CancellationToken>>>,
    msg_stores: Arc<RwLock<HashMap<Uuid, Arc<MsgStore>>>>,
    /// Tracks background tasks that stream logs to the database.
    /// When stopping execution, we await these to ensure logs are fully persisted.
    db_stream_handles: Arc<RwLock<HashMap<Uuid, JoinHandle<()>>>>,
    exit_monitor_handles: Arc<RwLock<HashMap<Uuid, JoinHandle<()>>>>,
    agent_process_registry: AgentProcessRegistry,
    workspace_touch_times: Arc<RwLock<HashMap<Uuid, Instant>>>,
    config: Arc<RwLock<Config>>,
    git: GitService,
    file_service: FileService,
    queued_message_service: QueuedMessageService,
    notification_service: NotificationService,
}

impl LocalContainerService {
    #[allow(clippy::too_many_arguments)]
    pub async fn new(
        db: DBService,
        agent_run_port: LocalAgentRunPort,
        msg_stores: Arc<RwLock<HashMap<Uuid, Arc<MsgStore>>>>,
        config: Arc<RwLock<Config>>,
        git: GitService,
        file_service: FileService,
        _analytics: Option<AnalyticsContext>,
        _approvals: Approvals,
        queued_message_service: QueuedMessageService,
    ) -> Self {
        let child_store = Arc::new(RwLock::new(HashMap::new()));
        let cancellation_tokens = Arc::new(RwLock::new(HashMap::new()));
        let db_stream_handles = Arc::new(RwLock::new(HashMap::new()));
        let exit_monitor_handles = Arc::new(RwLock::new(HashMap::new()));
        let agent_process_registry = AgentProcessRegistry::default();
        let workspace_touch_times = Arc::new(RwLock::new(HashMap::new()));
        let notification_service = NotificationService::new(config.clone());

        let container = LocalContainerService {
            db,
            agent_run_port,
            child_store,
            cancellation_tokens,
            msg_stores,
            db_stream_handles,
            exit_monitor_handles,
            agent_process_registry,
            workspace_touch_times,
            config,
            git,
            file_service,
            queued_message_service,
            notification_service,
        };

        // A service restart must observe and re-associate persisted processes;
        // it is not an implicit shutdown policy. Only an explicit AgentRun
        // control command may terminate a managed provider process.
        container
            .reconcile_registered_agent_processes("startup")
            .await;

        container
    }

    fn map_workspace_manager_error(err: WorkspaceError) -> ContainerError {
        match err {
            WorkspaceError::Database(err) => ContainerError::Sqlx(err),
            WorkspaceError::Worktree(err) => ContainerError::Worktree(err),
            WorkspaceError::GitService(err) => ContainerError::GitServiceError(err),
            WorkspaceError::Io(err) => ContainerError::Io(err),
            WorkspaceError::NoRepositories => {
                ContainerError::Other(anyhow!("No repositories provided"))
            }
            WorkspaceError::Repo(err) => ContainerError::Other(anyhow!(err)),
            WorkspaceError::WorkspaceNotFound => {
                ContainerError::Other(anyhow!("Workspace not found"))
            }
            WorkspaceError::RepoAlreadyAttached => {
                ContainerError::Other(anyhow!("Repository already attached to workspace"))
            }
            WorkspaceError::BranchNotFound { repo_name, branch } => ContainerError::Other(anyhow!(
                "Branch '{}' does not exist in repository '{}'",
                branch,
                repo_name
            )),
            WorkspaceError::PartialCreation(msg) => ContainerError::Other(anyhow!(msg)),
            WorkspaceError::ActiveAgentRuns { count } => {
                ContainerError::Other(anyhow!("Workspace has {count} active agent run(s)"))
            }
            WorkspaceError::OrchestrationLinkedAgentRuns { count } => ContainerError::Other(
                anyhow!("Workspace has {count} agent run(s) referenced by orchestration"),
            ),
        }
    }

    async fn workspace_repo_inputs(
        &self,
        workspace_id: Uuid,
    ) -> Result<(Vec<Repo>, Vec<RepoWorkspaceInput>), ContainerError> {
        let workspace_repos =
            WorkspaceRepo::find_by_workspace_id(&self.db.pool, workspace_id).await?;
        if workspace_repos.is_empty() {
            return Err(ContainerError::Other(anyhow!(
                "Workspace has no repositories configured"
            )));
        }

        let repositories =
            WorkspaceRepo::find_repos_for_workspace(&self.db.pool, workspace_id).await?;
        let target_branches: HashMap<_, _> = workspace_repos
            .iter()
            .map(|wr| (wr.repo_id, wr.target_branch.clone()))
            .collect();

        let workspace_inputs: Vec<RepoWorkspaceInput> = repositories
            .iter()
            .map(|repo| {
                let target_branch = target_branches.get(&repo.id).cloned().ok_or_else(|| {
                    ContainerError::Other(anyhow!(
                        "Missing target branch mapping for repo {} in workspace {}",
                        repo.id,
                        workspace_id
                    ))
                })?;
                Ok(RepoWorkspaceInput::new(repo.clone(), target_branch))
            })
            .collect::<Result<_, ContainerError>>()?;

        Ok((repositories, workspace_inputs))
    }

    async fn get_child_from_store(&self, id: &Uuid) -> Option<Arc<RwLock<AsyncGroupChild>>> {
        let map = self.child_store.read().await;
        map.get(id).cloned()
    }

    /// A terminal ExecutionProcess row precedes finalisation/log draining and
    /// process-group cleanup. Publication/admission must observe both owners,
    /// including the durable process registry after a service restart.
    async fn scripts_settled_inner(&self, workspace: Uuid) -> Result<bool, ContainerError> {
        let ids: Vec<Uuid> = sqlx::query_scalar("SELECT p.id FROM execution_processes p JOIN sessions s ON s.id=p.session_id WHERE s.workspace_id=?")
            .bind(workspace).fetch_all(&self.db.pool).await?;
        let entries = self.agent_process_registry.entries().await?;
        let children = self.child_store.read().await;
        Ok(ids.iter().all(|id| {
            !children.contains_key(id) && !entries.iter().any(|entry| entry.runtime_id == *id)
        }))
    }

    async fn add_child_to_store(&self, id: Uuid, exec: AsyncGroupChild) {
        let mut map = self.child_store.write().await;
        map.insert(id, Arc::new(RwLock::new(exec)));
    }

    async fn remove_child_from_store(&self, id: &Uuid) {
        let mut map = self.child_store.write().await;
        map.remove(id);
    }

    async fn reconcile_registered_agent_processes(&self, reason: &'static str) {
        match self.agent_process_registry.reconcile().await {
            Ok(report) => {
                let mut action_counts = [0usize; 4];
                for observation in &report.observations {
                    // The child store is intentionally empty after a service
                    // restart. An alive process therefore enters read-only
                    // observation until the AgentRun supervisor can attach a
                    // provider transport; it is never treated as completed.
                    let has_live_handle = self
                        .get_child_from_store(&observation.process.runtime_id)
                        .await
                        .is_some();
                    let process_observation = match observation.presence {
                        crate::agent_process_registry::RegisteredProcessPresence::Alive => {
                            process_supervisor::ProcessObservation::Alive
                        }
                        crate::agent_process_registry::RegisteredProcessPresence::Exited => {
                            process_supervisor::ProcessObservation::Exited(None)
                        }
                        crate::agent_process_registry::RegisteredProcessPresence::Unreachable => {
                            process_supervisor::ProcessObservation::TemporarilyUnreachable
                        }
                    };
                    let action = process_supervisor::reconciliation_action(
                        process_observation,
                        has_live_handle,
                    );
                    let action_index = match action {
                        process_supervisor::ReconciliationAction::Attach => 0,
                        process_supervisor::ReconciliationAction::ObserveReadOnly => 1,
                        process_supervisor::ReconciliationAction::ConfirmExit => 2,
                        process_supervisor::ReconciliationAction::PreserveForRetry => 3,
                    };
                    action_counts[action_index] += 1;
                    tracing::debug!(
                        reason,
                        runtime_id = %observation.process.runtime_id,
                        pid = observation.process.pid,
                        presence = ?observation.presence,
                        action = ?action,
                        "observed registered agent process during restart reconciliation"
                    );
                }
                let alive = report.alive().count();
                let exited = report.exited().count();
                let unreachable = report.observations.len().saturating_sub(alive + exited);
                if !report.observations.is_empty() {
                    tracing::info!(
                        reason,
                        observed = report.observations.len(),
                        alive,
                        exited,
                        unreachable,
                        attach = action_counts[0],
                        observe_read_only = action_counts[1],
                        confirm_exit = action_counts[2],
                        preserve_for_retry = action_counts[3],
                        "reconciled registered agent processes without cleanup"
                    );
                }
            }
            Err(error) => {
                tracing::warn!(
                    reason,
                    error = %error,
                    "failed to reconcile registered agent process registry"
                );
            }
        }
    }

    pub(crate) fn spawn_agent_run_terminal_monitor(&self) {
        let mut terminal_events = self.agent_run_port.subscribe_terminal_events();
        let container = self.clone();
        tokio::spawn(async move {
            loop {
                match terminal_events.recv().await {
                    Ok(event) => container.handle_agent_run_terminal(event).await,
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(skipped)) => {
                        tracing::warn!(
                            skipped,
                            "AgentRun terminal monitor lagged; queued follow-ups may require reconciliation"
                        );
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                }
            }
        });
    }

    async fn handle_agent_run_terminal(&self, event: AgentRunTerminalEvent) {
        // Integration owns its result commit and follow-ups. Ordinary terminal
        // finalisation must not create an unvalidated R' or consume user queues.
        match Session::find_by_id(&self.db.pool, event.session_id).await {
            Ok(Some(session)) => {
                match Workspace::find_by_id(&self.db.pool, session.workspace_id).await {
                    Ok(Some(workspace)) if workspace.is_execution_only() => return,
                    Ok(Some(_)) => {}
                    Ok(None) => return,
                    Err(error) => {
                        tracing::error!(%error,"Cannot resolve Integration finalisation owner");
                        return;
                    }
                }
            }
            Ok(None) => return,
            Err(error) => {
                tracing::error!(%error,"Cannot resolve terminal Session");
                return;
            }
        }
        let _queue_lock = self
            .queued_message_service
            .lock_session(event.session_id)
            .await;
        // A manual resume can win the lock before an old terminal notification.
        // That notification must not consume another entry or pause the new run.
        match sqlx::query_scalar::<_, Uuid>(
            "SELECT id FROM agent_runs WHERE session_id = ? ORDER BY created_at DESC, rowid DESC LIMIT 1",
        ).bind(event.session_id).fetch_optional(&self.db.pool).await {
            Ok(Some(latest)) if latest == event.agent_run_id => {}
            Ok(_) => return,
            Err(error) => {
                self.queued_message_service.pause(event.session_id);
                tracing::error!(%error, "Cannot verify queued follow-up completion");
                return;
            }
        }
        if event.status == AgentRunStatus::Succeeded
            && let Err(error) = self
                .complete_repository_memory(event.session_id, event.agent_run_id)
                .await
        {
            self.queued_message_service.pause(event.session_id);
            // Preserve source/draft for retry. Do not start a queued turn
            // which could overwrite this run's source before finalisation.
            tracing::error!(agent_run_id = %event.agent_run_id, %error, "Repository memory completion failed; source retained, queued follow-up not consumed");
            return;
        }
        if event.status != AgentRunStatus::Succeeded {
            self.queued_message_service.pause(event.session_id);
            return;
        }
        let Some(queued_message) = self.queued_message_service.take_queued(event.session_id) else {
            return;
        };

        // Queue owns its submitted message, not the composer's newer draft.
        // Draft acknowledgement is conditional and occurs in the client.

        match self
            .start_queued_follow_up(event.session_id, &queued_message.data)
            .await
        {
            Ok(next_agent_run_id) => tracing::info!(
                agent_run_id = %event.agent_run_id,
                next_agent_run_id = %next_agent_run_id,
                session_id = %event.session_id,
                "started queued follow-up after canonical AgentRun completion"
            ),
            Err(error) => {
                self.queued_message_service
                    .restore_and_pause(queued_message);
                tracing::error!(
                    agent_run_id = %event.agent_run_id,
                    session_id = %event.session_id,
                    %error,
                    "failed to start queued follow-up after canonical AgentRun completion"
                );
            }
        }
    }

    async fn complete_repository_memory(
        &self,
        session_id: Uuid,
        run_id: Uuid,
    ) -> anyhow::Result<()> {
        let Some(session) = Session::find_by_id(&self.db.pool, session_id).await? else {
            return Ok(());
        };
        if Workspace::find_by_id(&self.db.pool, session.workspace_id)
            .await?
            .is_some_and(|w| w.is_direct_folder())
        {
            return Ok(());
        }
        if should_disable_default_commit_for_workspace(&self.db.pool, session.workspace_id).await? {
            // Design Arena deliberately leaves source uncommitted until the
            // existing promotion lifecycle chooses an implementation.
            return Ok(());
        }
        let repos =
            WorkspaceRepo::find_repos_for_workspace(&self.db.pool, session.workspace_id).await?;
        for repo in repos {
            let Some(store) =
                db::models::repository_state::RepositoryMemoryStore::existing_for_repository(
                    &self.db.pool,
                    &repo.name,
                    repo.id,
                )
                .await?
            else {
                continue;
            };
            if let Some(run) = store.run(run_id)? {
                let (owned_store, owned_run) = (store.clone(), run.clone());
                let include_manifest = self.config.read().await.manifest_in_commit_body;
                tokio::task::spawn_blocking(move || {
                    tokio::runtime::Handle::current().block_on(
                        services::services::repository_memory::complete_coding_run(
                            &owned_store,
                            &owned_run,
                            include_manifest,
                        ),
                    )
                })
                .await??;
            }
        }
        Ok(())
    }

    async fn remove_registered_agent_process(
        &self,
        execution_process_id: Uuid,
        reason: &'static str,
    ) {
        if let Err(error) = self
            .agent_process_registry
            .remove_runtime(execution_process_id)
            .await
        {
            tracing::warn!(
                reason,
                execution_process_id = %execution_process_id,
                error = %error,
                "failed to remove agent process registry entry"
            );
        }
    }

    async fn cleanup_registered_agent_execution(
        &self,
        execution_process_id: Uuid,
        reason: &'static str,
    ) {
        match self
            .agent_process_registry
            .cleanup_runtime(execution_process_id)
            .await
        {
            Ok(report) if !report.is_empty() => {
                tracing::info!(
                    reason,
                    execution_process_id = %execution_process_id,
                    attempted = report.attempted,
                    removed = report.removed,
                    survivors = report.survivors,
                    "cleaned registered agent process tree for execution"
                );
            }
            Ok(_) => {}
            Err(error) => {
                tracing::warn!(
                    reason,
                    execution_process_id = %execution_process_id,
                    error = %error,
                    "failed to clean agent process registry entry"
                );
            }
        }
    }

    async fn add_cancellation_token(&self, id: Uuid, token: CancellationToken) {
        let mut map = self.cancellation_tokens.write().await;
        map.insert(id, token);
    }

    async fn take_cancellation_token(&self, id: &Uuid) -> Option<CancellationToken> {
        let mut map = self.cancellation_tokens.write().await;
        map.remove(id)
    }

    async fn add_db_stream_handle(&self, id: Uuid, handle: JoinHandle<()>) {
        let mut map = self.db_stream_handles.write().await;
        map.insert(id, handle);
    }

    async fn take_db_stream_handle(&self, id: &Uuid) -> Option<JoinHandle<()>> {
        let mut map = self.db_stream_handles.write().await;
        map.remove(id)
    }

    async fn add_exit_monitor_handle(&self, id: Uuid, handle: JoinHandle<()>) {
        let mut map = self.exit_monitor_handles.write().await;
        map.insert(id, handle);
    }

    async fn take_exit_monitor_handle(&self, id: &Uuid) -> Option<JoinHandle<()>> {
        let mut map = self.exit_monitor_handles.write().await;
        map.remove(id)
    }

    async fn cleanup_workspace(&self, workspace: &Workspace) {
        let _admission = services::services::integration_admission::MUTATIONS
            .lock()
            .await;
        match services::services::workspace_usage::cleanup_allowed(&self.db.pool, workspace.id)
            .await
        {
            Ok(true) => {}
            outcome => {
                tracing::info!(workspace_id=%workspace.id, ?outcome, "Retaining execution-owned workspace");
                return;
            }
        }
        if let Err(error) =
            db::models::integration::guard_workspace(&self.db.pool, workspace.id).await
        {
            tracing::info!(workspace_id=%workspace.id,%error,"Retaining reserved Integration source/worktree");
            return;
        }
        if !workspace.can_delete_container_path() {
            tracing::info!(
                "Skipping filesystem cleanup for external workspace {}",
                workspace.id
            );
            return;
        }

        let Some(container_ref) = &workspace.container_ref else {
            return;
        };
        let workspace_dir = PathBuf::from(container_ref);

        let repositories =
            match WorkspaceRepo::find_repos_for_workspace(&self.db.pool, workspace.id).await {
                Ok(repos) if !repos.is_empty() => repos,
                result => {
                    tracing::warn!(
                        "Workspace {} cleanup lacks repository ownership; directory retained: {:?}",
                        workspace.id,
                        result
                    );
                    return;
                }
            };
        if let Err(error) = WorkspaceManager::cleanup_workspace(&workspace_dir, &repositories).await
        {
            tracing::warn!(
                "Workspace {} cleanup stopped; files retained: {}",
                workspace.id,
                error
            );
            return;
        }

        let _ = Workspace::mark_worktree_deleted(&self.db.pool, workspace.id).await;
    }

    /// Record the current HEAD commit for each repository as the "after" state.
    /// Errors are silently ignored since this runs after the main execution completes
    /// and failure should not block process finalization.
    async fn update_after_head_commits(&self, exec_id: Uuid) {
        if let Ok(ctx) = ExecutionProcess::load_context(&self.db.pool, exec_id).await {
            if ctx.workspace.is_direct_folder() {
                return;
            }
            let workspace_root = self.workspace_to_current_dir(&ctx.workspace);
            for repo in &ctx.repos {
                let repo_path = repo.worktree_dir(&workspace_root);
                if let Ok(head) = self.git().get_head_info(&repo_path) {
                    let _ = ExecutionProcessRepoState::update_after_head_commit(
                        &self.db.pool,
                        exec_id,
                        repo.id,
                        &head.oid,
                    )
                    .await;
                }
            }
        }
    }

    /// Get the commit message based on the execution run reason.
    async fn get_commit_message(&self, ctx: &ExecutionContext) -> String {
        match ctx.execution_process.run_reason {
            ExecutionProcessRunReason::CleanupScript => {
                format!("Cleanup script changes for workspace {}", ctx.workspace.id)
            }
            _ => format!(
                "Changes from execution process {}",
                ctx.execution_process.id
            ),
        }
    }

    /// Check which repos have uncommitted changes. Fails if any repo is inaccessible.
    fn check_repos_for_changes(
        &self,
        workspace_root: &Path,
        repos: &[Repo],
    ) -> Result<Vec<(Repo, PathBuf)>, ContainerError> {
        let git = GitService::new();
        let mut repos_with_changes = Vec::new();

        for repo in repos {
            let worktree_path = repo.worktree_dir(workspace_root);

            match git.get_worktree_status(&worktree_path) {
                Ok(ws) if !ws.entries.is_empty() => {
                    repos_with_changes.push((repo.clone(), worktree_path));
                }
                Ok(_) => {
                    tracing::debug!("No changes in repo '{}'", repo.name);
                }
                Err(e) => {
                    return Err(ContainerError::Other(anyhow!(
                        "Pre-flight check failed for repo '{}': {}",
                        repo.name,
                        e
                    )));
                }
            }
        }

        Ok(repos_with_changes)
    }

    /// Commit changes to each repo. Logs failures but continues with other repos.
    fn commit_repos(&self, repos_with_changes: Vec<(Repo, PathBuf)>, message: &str) -> bool {
        let mut any_committed = false;

        for (repo, worktree_path) in repos_with_changes {
            tracing::debug!(
                "Committing changes for repo '{}' at {:?}",
                repo.name,
                &worktree_path
            );

            match self.git().commit(&worktree_path, message) {
                Ok(true) => {
                    any_committed = true;
                    tracing::info!("Committed changes in repo '{}'", repo.name);
                }
                Ok(false) => {
                    tracing::warn!("No changes committed in repo '{}' (unexpected)", repo.name);
                }
                Err(e) => {
                    tracing::warn!("Failed to commit in repo '{}': {}", repo.name, e);
                }
            }
        }

        any_committed
    }

    /// Spawn a background task that polls the child process for completion and
    /// cleans up the execution entry when it exits.
    fn spawn_exit_monitor(
        &self,
        exec_id: &Uuid,
        exit_signal: Option<ExecutorExitSignal>,
    ) -> JoinHandle<()> {
        let exec_id = *exec_id;
        let child_store = self.child_store.clone();
        let msg_stores = self.msg_stores.clone();
        let db = self.db.clone();
        let container = self.clone();

        let mut process_exit_rx = self.spawn_os_exit_watcher(exec_id);

        tokio::spawn(async move {
            let mut exit_signal_future = exit_signal
                .map(|rx| rx.boxed()) // wait for result
                .unwrap_or_else(|| std::future::pending().boxed()); // no signal, stall forever

            let terminal_outcome: agent_runtime_supervisor::AgentRuntimeSupervisorOutcome;

            // Wait for process to exit, or exit signal from executor
            tokio::select! {
                // Exit signal with result.
                // Some coding agent processes do not automatically exit after processing the user request; instead the executor
                // signals when processing has finished to gracefully kill the process.
                exit_result = &mut exit_signal_future => {
                    // Executor signaled completion: kill group and use the provided result
                    if let Some(child_lock) = child_store.read().await.get(&exec_id).cloned() {
                        let mut child = child_lock.write().await ;
                        if let Err(err) = command::kill_process_group(&mut child).await {
                            tracing::error!("Failed to kill process group after exit signal: {} {}", exec_id, err);
                        }
                    }

                    terminal_outcome = match exit_result {
                        Ok(result) => agent_runtime_supervisor::classify_executor_exit_result(result),
                        Err(_) => agent_runtime_supervisor::classify_executor_exit_channel_closed(),
                    };
                }
                // Process exit
                exit_status_result = &mut process_exit_rx => {
                    terminal_outcome = match exit_status_result {
                        Ok(Ok(exit_status)) => agent_runtime_supervisor::classify_process_exit(exit_status),
                        Ok(Err(error)) => agent_runtime_supervisor::classify_watcher_error(error.to_string()),
                        Err(error) => agent_runtime_supervisor::classify_watcher_error(error.to_string()),
                    };
                }
            }

            let status = terminal_outcome
                .process_status
                .clone()
                .expect("terminal agent runtime outcome should include process status");
            let exit_code = terminal_outcome.exit_code;
            tracing::debug!(
                execution_process_id = %exec_id,
                lifecycle = ?terminal_outcome.lifecycle,
                process_status = ?status,
                exit_code = ?exit_code,
                error_kind = ?terminal_outcome.runtime_error.as_ref().map(|error| error.kind),
                "classified agent runtime terminal outcome"
            );

            if !ExecutionProcess::was_stopped(&db.pool, exec_id).await
                && let Err(e) =
                    ExecutionProcess::update_completion(&db.pool, exec_id, status, exit_code).await
            {
                tracing::error!("Failed to update execution process completion: {}", e);
            }

            // Notify any subscribers (e.g. workflow runtime) that this execution
            // process has reached a terminal state. This enables event-driven
            // workflow progression without relying on HTTP polling.
            //
            // We publish the event unconditionally here (even if was_stopped was
            // true) because the DB already holds a terminal status in that case
            // and any workflow watcher should still reconcile.  The session_id is
            // looked up from the DB to keep this path self-contained.
            if let Ok(Some(ep)) = ExecutionProcess::find_by_id(&db.pool, exec_id).await {
                publish_execution_completed(ExecutionCompletedEvent {
                    execution_process_id: exec_id,
                    session_id: ep.session_id,
                });
            }

            if let Ok(ctx) = ExecutionProcess::load_context(&db.pool, exec_id).await {
                let success = matches!(
                    ctx.execution_process.status,
                    ExecutionProcessStatus::Completed
                ) && exit_code == Some(0);

                let cleanup_done = matches!(
                    ctx.execution_process.run_reason,
                    ExecutionProcessRunReason::CleanupScript
                ) && !matches!(
                    ctx.execution_process.status,
                    ExecutionProcessStatus::Running
                );

                if success || cleanup_done {
                    // Commit changes (if any) and get feedback about whether changes were made
                    match container.try_commit_changes(&ctx).await {
                        Ok(_) => {}
                        Err(e) => {
                            tracing::error!("Failed to commit changes after execution: {}", e);
                            // Treat commit failures as if changes were made to be safe
                        }
                    };

                    // Script actions are the only ExecutionProcess actions. Any
                    // canonical AgentRun is finalized by AgentRunPort and never
                    // enters this legacy process monitor.
                    if let Err(e) = container.try_start_next_action(&ctx).await {
                        tracing::error!("Failed to start next action after completion: {}", e);
                    }
                }

                if container.should_finalize(&ctx) {
                    container.finalize_task(&ctx).await;
                }
            }

            // Now that commit/next-action/finalization steps for this process are complete,
            // capture the HEAD OID as the definitive "after" state (best-effort).
            container.update_after_head_commits(exec_id).await;

            // Wait for DB persistence to complete before cleaning up MsgStore
            let db_stream_handle = container.take_db_stream_handle(&exec_id).await;
            if let Some(msg_arc) = msg_stores.write().await.remove(&exec_id) {
                msg_arc.push_finished();
            }
            if let Some(handle) = db_stream_handle {
                let _ = tokio::time::timeout(Duration::from_secs(5), handle).await;
            }

            // SIGKILL any orphaned children (e.g. MCP servers) still in the
            // process group. The executor itself is already done — either it
            // exited naturally or was killed in the exit-signal branch above.
            if let Some(child_lock) = child_store.read().await.get(&exec_id).cloned() {
                let mut child = child_lock.write().await;
                let _ = child.start_kill();
            }
            container
                .remove_registered_agent_process(exec_id, "exit_monitor")
                .await;
            child_store.write().await.remove(&exec_id);
        })
    }

    fn spawn_os_exit_watcher(
        &self,
        exec_id: Uuid,
    ) -> tokio::sync::oneshot::Receiver<std::io::Result<std::process::ExitStatus>> {
        let (tx, rx) = tokio::sync::oneshot::channel::<std::io::Result<std::process::ExitStatus>>();
        let child_store = self.child_store.clone();
        tokio::spawn(async move {
            loop {
                let child_lock = {
                    let map = child_store.read().await;
                    map.get(&exec_id).cloned()
                };
                if let Some(child_lock) = child_lock {
                    let mut child_handler = child_lock.write().await;
                    match child_handler.try_wait() {
                        Ok(Some(status)) => {
                            let _ = tx.send(Ok(status));
                            break;
                        }
                        Ok(None) => {}
                        Err(e) => {
                            let _ = tx.send(Err(e));
                            break;
                        }
                    }
                } else {
                    let _ = tx.send(Err(io::Error::other(format!(
                        "Child handle missing for {exec_id}"
                    ))));
                    break;
                }
                tokio::time::sleep(Duration::from_millis(250)).await;
            }
        });
        rx
    }

    fn dir_name_from_workspace(workspace_id: &Uuid, task_title: &str) -> String {
        let task_title_id = git_branch_id(task_title);
        format!("{}-{}", short_uuid(workspace_id), task_title_id)
    }

    async fn track_child_msgs_in_store(
        &self,
        id: Uuid,
        child: &mut AsyncGroupChild,
    ) -> Result<(), ContainerError> {
        let store = self
            .get_msg_store_by_id(&id)
            .await
            .ok_or_else(|| ContainerError::Other(anyhow!("MsgStore not found for execution")))?;
        let out = child.inner().stdout.take().expect("no stdout");
        let err = child.inner().stderr.take().expect("no stderr");

        // Map stdout bytes -> LogMsg::Stdout
        let out = ReaderStream::new(out)
            .map_ok(|chunk| LogMsg::Stdout(String::from_utf8_lossy(&chunk).into_owned()));

        // Map stderr bytes -> LogMsg::Stderr
        let err = ReaderStream::new(err)
            .map_ok(|chunk| LogMsg::Stderr(String::from_utf8_lossy(&chunk).into_owned()));

        // If you have a JSON Patch source, map it to LogMsg::JsonPatch too, then select all three.

        // Merge and forward into the store
        let merged = select(out, err); // Stream<Item = Result<LogMsg, io::Error>>
        store.clone().spawn_forwarder(merged);
        Ok(())
    }

    /// Create a live diff log stream for ongoing attempts for WebSocket
    /// Returns a stream that owns the filesystem watcher - when dropped, watcher is cleaned up
    async fn create_live_diff_stream(
        &self,
        args: diff_stream::DiffStreamArgs,
    ) -> Result<DiffStreamHandle, ContainerError> {
        diff_stream::create(args)
            .await
            .map_err(|e| ContainerError::Other(anyhow!("{e}")))
    }

    /// Copy project files and workspace attachments to the workspace.
    /// Skips files that already exist (fast no-op if all exist).
    async fn copy_files_and_images(
        &self,
        workspace_dir: &Path,
        workspace: &Workspace,
    ) -> Result<(), ContainerError> {
        let repos = WorkspaceRepo::find_repos_with_copy_files(&self.db.pool, workspace.id).await?;

        for repo in &repos {
            if let Some(copy_files) = &repo.copy_files
                && !copy_files.trim().is_empty()
            {
                let worktree_path = repo.worktree_dir(workspace_dir);
                self.copy_project_files(&repo.path, &worktree_path, copy_files)
                    .await
                    .unwrap_or_else(|e| {
                        tracing::warn!(
                            "Failed to copy project files for repo '{}': {}",
                            repo.name,
                            e
                        );
                    });
            }
        }

        self.file_service
            .copy_files_by_workspace_to_worktree(workspace_dir, workspace.id, None)
            .await
            .map_err(|e| ContainerError::Other(e.into()))?;

        Ok(())
    }

    async fn prepare_shared_resources(
        workspace_dir: &Path,
        repos: &[Repo],
        workspace: &Workspace,
    ) -> Result<(), ContainerError> {
        let owner = workspace
            .execution_owner
            .as_ref()
            .map(|owner| owner.kind.clone());
        let roots: Vec<_> = repos
            .iter()
            .map(|repo| (repo.worktree_dir(workspace_dir), repo.name.clone(), repo.id))
            .collect();
        tokio::task::spawn_blocking(move || {
            for (root, name, id) in roots {
                workspace_manager::shared_resources::ensure_for_workspace(
                    &root,
                    &name,
                    id,
                    owner.as_deref(),
                )?;
            }
            Ok::<_, std::io::Error>(())
        })
        .await
        .map_err(|error| ContainerError::Other(anyhow!(error)))??;
        Ok(())
    }

    /// Maintain LVK's non-Git parent instructions without changing repository files.
    /// Claude understands scoped @imports; Codex is directed to read child files.
    async fn create_workspace_config_files(
        workspace_dir: &Path,
        repos: &[Repo],
    ) -> Result<(), ContainerError> {
        const MARKER: &str = "<!-- LVK WORKSPACE INSTRUCTIONS v1 -->";
        let mut imports = Vec::new();
        let mut content = format!(
            "{MARKER}\n# Workspace instructions\n\nThis non-Git directory is the Workspace's common working root. Each child repository is a separate Git worktree. Run Git commands inside the relevant repository, not at this root.\n\nBefore working in any repository, read its AGENTS.md and every lower AGENTS.md applicable to the target path. Imported repository instructions apply ONLY to their own repository and subtree, never to siblings. The parent does not replace repository instructions. Do not edit user instruction files to add LVK policy. AGENTS.md is LVK's standard; existing CLAUDE.md compatibility is not guaranteed.\n\nWorkspace attachments are relative to this root under .lvk/attachments. Each repository owns its own .lvk/shared and openwiki; never combine or move those stores into this root.\n\n{}\n\n## Repositories and scoped instructions\n",
            utils::path::SHARED_RESOURCE_GUIDANCE
        );
        for repo in repos {
            content.push_str(&format!(
                "\n### {} — only ./{} and its descendants\n",
                repo.display_name,
                repo.worktree_name()
            ));
            if repo.worktree_dir(workspace_dir).join("AGENTS.md").is_file() {
                let line = format!("@{}/AGENTS.md", repo.worktree_name().replace(' ', "\\ "));
                imports.push(format!("@{}/AGENTS.md", repo.worktree_name()));
                content.push_str(&format!("{line}\n"));
            } else {
                content.push_str("No root AGENTS.md was found. Still check target-path instructions before making changes.\n");
            }
        }
        let agents = workspace_dir.join("AGENTS.md");
        let legacy_agents = imports.join("\n") + "\n";
        let old = tokio::fs::read_to_string(&agents).await.ok();
        // Only our marked file or the exact previous generated template may be
        // refreshed. Preserve custom root files and every child repository file.
        if old
            .as_ref()
            .is_none_or(|s| s.starts_with(MARKER) || *s == legacy_agents)
        {
            if agents
                .symlink_metadata()
                .is_ok_and(|m| m.file_type().is_symlink())
            {
                return Ok(());
            }
            if old.as_deref() != Some(content.as_str()) {
                // Readers starting another Session must see the complete old or
                // new parent, never a file truncated during a refresh.
                let temporary = workspace_dir.join(format!(".AGENTS-{}.tmp", Uuid::new_v4()));
                let result: std::io::Result<()> = async {
                    let mut file = tokio::fs::OpenOptions::new()
                        .write(true)
                        .create_new(true)
                        .open(&temporary)
                        .await?;
                    tokio::io::AsyncWriteExt::write_all(&mut file, content.as_bytes()).await?;
                    drop(file);
                    tokio::fs::rename(&temporary, &agents).await
                }
                .await;
                if result.is_err() {
                    let _ = tokio::fs::remove_file(&temporary).await;
                }
                result?;
            }
        }
        // The old generator emitted only one import per selected child. A
        // child may since have migrated to AGENTS or disappeared; existence is
        // not provenance. Recognize only whole lines for known saved placements.
        let claude = workspace_dir.join("CLAUDE.md");
        if claude
            .symlink_metadata()
            .is_ok_and(|m| m.is_file() && !m.file_type().is_symlink())
            && let Ok(content) = tokio::fs::read_to_string(&claude).await
        {
            let known: std::collections::HashSet<_> = repos
                .iter()
                .map(|r| format!("@{}/CLAUDE.md", r.worktree_name()))
                .collect();
            let lines: Vec<_> = content.lines().collect();
            let unique: std::collections::HashSet<_> = lines.iter().copied().collect();
            let legacy = content == lines.join("\n") + "\n"
                && !lines.is_empty()
                && unique.len() == lines.len()
                && lines.iter().all(|line| known.contains(*line));
            if legacy {
                let lvk = utils::lvk_directory::LvkDirectory::open(workspace_dir)?;
                lvk.verify()?;
                let mut name = "legacy-workspace-CLAUDE.md".to_owned();
                let backup = loop {
                    let backup = lvk.anchored_path().join(&name);
                    // Unlike rename, hard_link never replaces an existing file
                    // or dangling symlink. Both names stay until backup succeeds.
                    match tokio::fs::hard_link(&claude, &backup).await {
                        Ok(()) => break backup,
                        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                            name = format!("legacy-workspace-CLAUDE-{}.md", Uuid::new_v4());
                        }
                        Err(error) => return Err(error.into()),
                    }
                };
                if !backup.symlink_metadata()?.is_file()
                    || backup.symlink_metadata()?.file_type().is_symlink()
                    || tokio::fs::read_to_string(&backup).await? != content
                    || claude.symlink_metadata()?.file_type().is_symlink()
                    || tokio::fs::read_to_string(&claude).await? != content
                {
                    return Err(ContainerError::Other(anyhow!(
                        "Parent CLAUDE.md changed during retirement; files preserved"
                    )));
                }
                tokio::fs::remove_file(&claude).await?;
            } else {
                tracing::debug!(
                    "Preserving unrecognized/custom parent CLAUDE.md; it may suppress native AGENTS loading"
                );
            }
        }

        Ok(())
    }

    /// Start a canonical AgentRun from a queued message.
    async fn start_queued_follow_up(
        &self,
        session_id: Uuid,
        queued_data: &DraftFollowUpData,
    ) -> Result<Uuid, ContainerError> {
        let session = Session::find_by_id(&self.db.pool, session_id)
            .await?
            .ok_or(SessionError::NotFound)?;
        let workspace = Workspace::find_by_id(&self.db.pool, session.workspace_id)
            .await?
            .ok_or(SessionError::WorkspaceNotFound)?;
        let executor_profile_id = queued_data.executor_config.profile_id();

        if let Some(expected) = session.executor.clone() {
            let actual = executor_profile_id.executor.to_string();
            if expected != actual {
                return Err(SessionError::ExecutorMismatch { expected, actual }.into());
            }
        }

        if session.executor.is_none() {
            Session::update_executor(
                &self.db.pool,
                session.id,
                &executor_profile_id.executor.to_string(),
            )
            .await?;
        }

        let provider = DirectProvider::from_base_agent(queued_data.executor_config.executor)
            .ok_or_else(|| {
                ContainerError::Other(anyhow!(
                    "{} is not a V1 direct Agent Runtime provider",
                    queued_data.executor_config.executor
                ))
            })?;
        let runtime_profile_id = executor_profile_id.cache_key();
        let provider_session = sqlx::query_as::<_, (String, Json<ProviderSessionReference>)>(
            r#"
            SELECT runtime_profile_id, session_reference
            FROM agent_provider_sessions
            WHERE session_id = ? AND provider_id = ?
            LIMIT 1
            "#,
        )
        .bind(session.id)
        .bind(provider.id())
        .fetch_optional(&self.db.pool)
        .await?
        .map(|(bound_profile_id, reference)| {
            if !runtime_profile_ids_match(&bound_profile_id, &runtime_profile_id) {
                return Err(ContainerError::Other(anyhow!(
                    "Session is bound to runtime profile {bound_profile_id}; create a new VK session for {runtime_profile_id}"
                )));
            }
            Ok(reference.0)
        })
        .transpose()?;
        let lifecycle = services::services::workspace_lifecycle::lock(workspace.id).await;
        let workspace = Workspace::find_by_id(&self.db.pool, workspace.id)
            .await?
            .ok_or(SessionError::WorkspaceNotFound)?;
        if workspace.archived {
            return Err(ContainerError::Other(anyhow!(
                "Workspace was archived; queued follow-up is paused until explicit resume"
            )));
        }
        let workspace_path = self
            .ensure_container_exists_locked(&workspace, &lifecycle)
            .await?;
        drop(lifecycle);
        let (request, attempt) = build_queued_agent_run_requests(
            session.id,
            &workspace,
            workspace_path,
            queued_data,
            provider_session,
        )
        .map_err(|error| ContainerError::Other(anyhow!(error)))?;

        self.agent_run_port
            .create(request, attempt)
            .await
            .map_err(|error| ContainerError::Other(anyhow!(error)))
    }
}

fn build_queued_agent_run_requests(
    session_id: Uuid,
    workspace: &Workspace,
    workspace_path: String,
    queued_data: &DraftFollowUpData,
    provider_session: Option<ProviderSessionReference>,
) -> Result<(AgentRunRequestEnvelope, RunAttemptRequest), AgentRunPortError> {
    let provider = DirectProvider::from_base_agent(queued_data.executor_config.executor)
        .ok_or_else(|| {
            AgentRunPortError::Rejected(format!(
                "{} is not a V1 direct Agent Runtime provider",
                queued_data.executor_config.executor
            ))
        })?;
    let runtime_profile_id = queued_data.executor_config.profile_id().cache_key();
    let capability_snapshot =
        direct_provider_capability_snapshot(provider, runtime_profile_id.clone());
    let intent = if provider_session.is_some() {
        require_capability(
            provider,
            &capability_snapshot,
            AgentCapability::SessionResume,
            false,
        )
        .map_err(|error| AgentRunPortError::Rejected(error.to_string()))?;
        AgentRunIntent::FollowUp
    } else {
        AgentRunIntent::Initial
    };
    let request_id = Uuid::new_v4();
    let agent_run_id = Uuid::new_v4();
    let turn_id = Uuid::new_v4();
    let correlation_id = Uuid::new_v4();
    let created_at = chrono::Utc::now();
    let idempotency_key = format!("session:{session_id}:request:{request_id}:queued");
    let workspace = WorkspaceReference {
        workspace_id: workspace.id,
        mode: match workspace.workspace_kind {
            WorkspaceKind::DirectFolder => WorkspaceMode::SharedWorkspace,
            WorkspaceKind::Worktree => WorkspaceMode::IsolatedWorktree,
        },
        path: workspace_path,
    };
    let request = AgentRunRequestEnvelope {
        schema_version: AGENT_REQUEST_SCHEMA_VERSION,
        payload_version: AGENT_REQUEST_PAYLOAD_VERSION,
        request_id,
        idempotency_key: idempotency_key.clone(),
        session_id,
        agent_run_id,
        turn_id,
        correlation_id,
        intent,
        runtime_profile_id: runtime_profile_id.clone(),
        provider_id: provider.id().to_string(),
        workspace: workspace.clone(),
        input: CanonicalMessage {
            message_id: Uuid::new_v4(),
            role: AgentRuntimeMessageRole::User,
            content: queued_data.message.clone(),
        },
        created_at,
    };
    let attempt = RunAttemptRequest {
        schema_version: AGENT_REQUEST_SCHEMA_VERSION,
        payload_version: AGENT_REQUEST_PAYLOAD_VERSION,
        request_id,
        idempotency_key: format!("{idempotency_key}:attempt:1"),
        session_id,
        agent_run_id,
        turn_id,
        run_attempt_id: Uuid::new_v4(),
        attempt_number: 1,
        correlation_id,
        mode: RunAttemptMode::Launch,
        transport: provider.transport(),
        runtime_profile_id,
        provider_id: provider.id().to_string(),
        workspace,
        capability_snapshot,
        executor_config: queued_data.executor_config.clone(),
        selected_skills: queued_data
            .selected_skills
            .clone()
            .filter(|skills| !skills.is_empty()),
        reset_to_message_id: None,
        provider_session,
        created_at,
    };

    Ok((request, attempt))
}

#[async_trait]
impl ContainerService for LocalContainerService {
    async fn script_running(&self, execution: Uuid) -> Result<Option<bool>, ContainerError> {
        use crate::agent_process_registry::RegisteredProcessPresence;
        Ok(
            match self
                .agent_process_registry
                .observe_runtime(execution)
                .await?
            {
                Some(RegisteredProcessPresence::Alive) => Some(true),
                Some(RegisteredProcessPresence::Exited) => Some(false),
                Some(RegisteredProcessPresence::Unreachable) | None => None,
            },
        )
    }

    async fn script_settled(&self, execution: Uuid) -> Result<bool, ContainerError> {
        Ok(self.get_child_from_store(&execution).await.is_none()
            && self
                .agent_process_registry
                .peek_runtime(execution)
                .await?
                .is_none())
    }
    async fn scripts_settled(&self, workspace: Uuid) -> Result<bool, ContainerError> {
        self.scripts_settled_inner(workspace).await
    }
    fn msg_stores(&self) -> &Arc<RwLock<HashMap<Uuid, Arc<MsgStore>>>> {
        &self.msg_stores
    }

    fn db(&self) -> &DBService {
        &self.db
    }

    fn git(&self) -> &GitService {
        &self.git
    }

    fn notification_service(&self) -> &NotificationService {
        &self.notification_service
    }

    async fn touch(&self, workspace: &Workspace) -> Result<(), ContainerError> {
        let now = Instant::now();

        // We debounce touches to avoid excessive database writes, which in SQLites causes DB locks
        let should_debounce = |last_touch: &Instant| -> bool {
            now.duration_since(*last_touch) < WORKSPACE_TOUCH_DEBOUNCE
        };

        // Quick check with read lock
        if self
            .workspace_touch_times
            .read()
            .await
            .get(&workspace.id)
            .is_some_and(should_debounce)
        {
            return Ok(());
        }

        let mut map = self.workspace_touch_times.write().await;
        // Clean up stale entries older than the debounce window, reduce memory usage over time
        map.retain(|_, time| should_debounce(time));
        // check in case another thread has touched already
        if map.get(&workspace.id).is_some_and(should_debounce) {
            return Ok(());
        }
        map.insert(workspace.id, now);
        drop(map);

        Workspace::touch(&self.db.pool, workspace.id).await?;
        Ok(())
    }

    async fn store_db_stream_handle(&self, id: Uuid, handle: JoinHandle<()>) {
        self.add_db_stream_handle(id, handle).await;
    }

    async fn take_db_stream_handle(&self, id: &Uuid) -> Option<JoinHandle<()>> {
        LocalContainerService::take_db_stream_handle(self, id).await
    }

    async fn git_branch_prefix(&self) -> String {
        self.config.read().await.git_branch_prefix.clone()
    }

    fn workspace_to_current_dir(&self, workspace: &Workspace) -> PathBuf {
        PathBuf::from(workspace.container_ref.clone().unwrap_or_default())
    }

    async fn create(&self, workspace: &Workspace) -> Result<ContainerRef, ContainerError> {
        if workspace.is_direct_folder() {
            let container_ref = workspace
                .container_ref
                .as_deref()
                .filter(|value| !value.is_empty())
                .ok_or_else(|| {
                    ContainerError::Other(anyhow!(
                        "Direct folder workspace is missing a container path"
                    ))
                })?;
            let container_path = PathBuf::from(container_ref);
            let metadata = tokio::fs::metadata(&container_path).await?;
            if !metadata.is_dir() {
                return Err(ContainerError::Other(anyhow!(
                    "Direct folder workspace path is not a directory: {}",
                    container_path.display()
                )));
            }
            return Ok(container_ref.to_string());
        }

        let label = workspace.name.as_deref().unwrap_or("workspace");
        let workspace_dir_name =
            LocalContainerService::dir_name_from_workspace(&workspace.id, label);
        let workspace_dir = WorkspaceManager::get_workspace_base_dir().join(&workspace_dir_name);

        let (repositories, workspace_inputs) = self.workspace_repo_inputs(workspace.id).await?;

        let created_workspace = WorkspaceManager::create_workspace(
            &workspace_dir,
            &workspace_inputs,
            &workspace.branch,
        )
        .await
        .map_err(Self::map_workspace_manager_error)?;

        // Copy project files and images to workspace
        self.copy_files_and_images(&created_workspace.workspace_dir, workspace)
            .await?;

        Self::create_workspace_config_files(&created_workspace.workspace_dir, &repositories)
            .await?;
        Self::prepare_shared_resources(&created_workspace.workspace_dir, &repositories, workspace)
            .await?;

        Workspace::update_container_ref(
            &self.db.pool,
            workspace.id,
            &created_workspace.workspace_dir.to_string_lossy(),
        )
        .await?;

        Ok(created_workspace
            .workspace_dir
            .to_string_lossy()
            .to_string())
    }

    async fn delete(&self, workspace: &Workspace) -> Result<(), ContainerError> {
        self.try_stop(workspace, true).await;
        if workspace.can_delete_container_path() {
            self.cleanup_workspace(workspace).await;
        }
        Ok(())
    }

    async fn ensure_container_exists_locked(
        &self,
        workspace: &Workspace,
        guard: &services::services::workspace_lifecycle::Guard,
    ) -> Result<ContainerRef, ContainerError> {
        assert_eq!(guard.workspace_id, workspace.id);
        self.touch(workspace).await?;
        if workspace.is_direct_folder() {
            let container_ref = workspace
                .container_ref
                .as_deref()
                .filter(|value| !value.is_empty())
                .ok_or_else(|| {
                    ContainerError::Other(anyhow!(
                        "Direct folder workspace is missing a container path"
                    ))
                })?;
            let container_path = PathBuf::from(container_ref);
            let metadata = tokio::fs::metadata(&container_path).await?;
            if !metadata.is_dir() {
                return Err(ContainerError::Other(anyhow!(
                    "Direct folder workspace path is not a directory: {}",
                    container_path.display()
                )));
            }
            if workspace.worktree_deleted {
                Workspace::clear_worktree_deleted(&self.db.pool, workspace.id).await?;
            }
            return Ok(container_ref.to_string());
        }

        let (repositories, workspace_inputs) = self.workspace_repo_inputs(workspace.id).await?;

        let workspace_dir = if let Some(container_ref) = &workspace.container_ref {
            PathBuf::from(container_ref)
        } else {
            let label = workspace.name.as_deref().unwrap_or("workspace");
            let workspace_dir_name =
                LocalContainerService::dir_name_from_workspace(&workspace.id, label);
            WorkspaceManager::get_workspace_base_dir().join(&workspace_dir_name)
        };

        WorkspaceManager::ensure_workspace_exists(
            &workspace_dir,
            &workspace_inputs,
            &workspace.branch,
        )
        .await
        .map_err(Self::map_workspace_manager_error)?;

        if workspace.container_ref.is_none() {
            Workspace::update_container_ref(
                &self.db.pool,
                workspace.id,
                &workspace_dir.to_string_lossy(),
            )
            .await?;
        }

        if workspace.worktree_deleted {
            Workspace::clear_worktree_deleted(&self.db.pool, workspace.id).await?;
        }

        // Copy project files and images (fast no-op if already exist)
        self.copy_files_and_images(&workspace_dir, workspace)
            .await?;

        Self::create_workspace_config_files(&workspace_dir, &repositories).await?;
        Self::prepare_shared_resources(&workspace_dir, &repositories, workspace).await?;

        Ok(workspace_dir.to_string_lossy().to_string())
    }

    async fn is_container_clean(&self, workspace: &Workspace) -> Result<bool, ContainerError> {
        if workspace.is_direct_folder() {
            return Ok(true);
        }
        let Some(container_ref) = &workspace.container_ref else {
            return Ok(true);
        };

        let workspace_dir = PathBuf::from(container_ref);
        if !workspace_dir.exists() {
            return Ok(true);
        }

        let repositories =
            WorkspaceRepo::find_repos_for_workspace(&self.db.pool, workspace.id).await?;

        for repo in &repositories {
            let worktree_path = repo.worktree_dir(&workspace_dir);
            if worktree_path.exists() {
                let (uncommitted, untracked) =
                    self.git().get_worktree_change_counts(&worktree_path)?;
                if uncommitted > 0 || untracked > 0 {
                    return Ok(false);
                }
            }
        }

        Ok(true)
    }

    async fn start_execution_inner(
        &self,
        workspace: &Workspace,
        execution_process: &ExecutionProcess,
        executor_action: &ExecutorAction,
    ) -> Result<(), ContainerError> {
        services::services::workspace_usage::validate_script_owner(
            &self.db.pool,
            workspace.id,
            execution_process.session_id,
            executor_action,
            &execution_process.run_reason,
        )
        .await
        .map_err(ContainerError::Other)?;
        if !matches!(
            executor_action.typ(),
            executors::actions::ExecutorActionType::ScriptRequest(_)
        ) {
            return Err(ContainerError::Other(anyhow!(
                "coding-agent ExecutionProcess actions were removed; use the canonical AgentRun API"
            )));
        }

        // Get the worktree path
        let container_ref = workspace
            .container_ref
            .as_ref()
            .ok_or(ContainerError::Other(anyhow!(
                "Container ref not found for workspace"
            )))?;
        let current_dir = PathBuf::from(container_ref);
        let starting = agent_runtime_supervisor::classify_starting();
        tracing::debug!(
            execution_process_id = %execution_process.id,
            lifecycle = ?starting.lifecycle,
            "classified agent runtime start outcome"
        );

        let repos = WorkspaceRepo::find_repos_for_workspace(&self.db.pool, workspace.id).await?;
        let repo_names: Vec<String> = repos.iter().map(|r| r.worktree_name().to_owned()).collect();
        let repo_context = RepoContext::new(current_dir.clone(), repo_names);

        let mut env = ExecutionEnv::new(repo_context);

        // Always inject workspace/session context
        env.insert("VK_WORKSPACE_ID", workspace.id.to_string());
        env.insert("VK_WORKSPACE_BRANCH", &workspace.branch);
        env.insert(
            "LVK_SHARED_RESOURCE_ROOTS",
            serde_json::to_string(
                &repos
                    .iter()
                    .flat_map(|repo| {
                        workspace_manager::shared_resources::writable_roots(&repo.name, repo.id)
                    })
                    .collect::<Vec<_>>(),
            )
            .map_err(|error| ContainerError::Other(anyhow!(error)))?,
        );

        // Create the child and stream, add to execution tracker with timeout
        let spawn_result = tokio::time::timeout(
            Duration::from_secs(30),
            executor_action.spawn(
                &current_dir,
                Arc::new(executors::approvals::NoopExecutorApprovalService {}),
                &env,
            ),
        )
        .await;

        let mut spawned = match spawn_result {
            Ok(Ok(spawned)) => spawned,
            Ok(Err(error)) => {
                let outcome =
                    agent_runtime_supervisor::classify_launch_executor_error(&error, None);
                trace_launch_diagnostic(execution_process.id, &outcome);
                return Err(ContainerError::ExecutorError(error));
            }
            Err(_) => {
                let outcome = agent_runtime_supervisor::classify_launch_timeout();
                trace_launch_diagnostic(execution_process.id, &outcome);
                return Err(ContainerError::Other(anyhow!(
                    "Timeout: process took more than 30 seconds to start"
                )));
            }
        };

        let pid = match spawned.child.inner().id() {
            Some(pid) => pid,
            None => {
                let _ = command::kill_process_group(&mut spawned.child).await;
                return Err(ContainerError::Other(anyhow!(
                    "Spawned execution process has no OS pid"
                )));
            }
        };
        let registered_process = RegisteredAgentProcess::new(
            execution_process.id,
            Some(execution_process.session_id),
            Some(workspace.id),
            None,
            pid,
            Some(pid),
            None,
        );
        if let Err(error) = self
            .agent_process_registry
            .register(registered_process)
            .await
        {
            tracing::warn!(
                execution_process_id = %execution_process.id,
                session_id = %execution_process.session_id,
                workspace_id = %workspace.id,
                pid,
                error = %error,
                "failed to register spawned agent process; killing child"
            );
            let _ = command::kill_process_group(&mut spawned.child).await;
            return Err(ContainerError::Io(error));
        }

        if let Err(e) = self
            .track_child_msgs_in_store(execution_process.id, &mut spawned.child)
            .await
        {
            let _ = command::kill_process_group(&mut spawned.child).await;
            self.remove_registered_agent_process(execution_process.id, "spawn_tracking_failed")
                .await;
            return Err(e);
        }

        self.add_child_to_store(execution_process.id, spawned.child)
            .await;
        let running = agent_runtime_supervisor::classify_running();
        tracing::debug!(
            execution_process_id = %execution_process.id,
            lifecycle = ?running.lifecycle,
            "classified agent runtime running outcome"
        );

        // Store cancellation token for graceful shutdown
        if let Some(cancel) = spawned.cancel {
            self.add_cancellation_token(execution_process.id, cancel)
                .await;
        }

        // Spawn unified exit monitor: watches OS exit and optional executor signal
        let hn = self.spawn_exit_monitor(&execution_process.id, spawned.exit_signal);
        self.add_exit_monitor_handle(execution_process.id, hn).await;

        Ok(())
    }

    async fn stop_execution(
        &self,
        execution_process: &ExecutionProcess,
        status: ExecutionProcessStatus,
    ) -> Result<(), ContainerError> {
        let child = self.get_child_from_store(&execution_process.id).await;
        let exit_code = if status == ExecutionProcessStatus::Completed {
            Some(0)
        } else {
            None
        };
        let cancelling = agent_runtime_supervisor::classify_cancellation_requested();
        tracing::debug!(
            execution_process_id = %execution_process.id,
            lifecycle = ?cancelling.lifecycle,
            requested_status = ?status,
            "classified agent runtime cancellation request"
        );

        ExecutionProcess::update_completion(
            &self.db.pool,
            execution_process.id,
            status.clone(),
            exit_code,
        )
        .await?;

        // Try graceful cancellation first, then force kill
        if let Some(cancel) = self.take_cancellation_token(&execution_process.id).await {
            cancel.cancel();

            // Wait for exit monitor to finish gracefully
            if let Some(monitor_handle) = self.take_exit_monitor_handle(&execution_process.id).await
            {
                match tokio::time::timeout(Duration::from_secs(5), monitor_handle).await {
                    Ok(_) => {
                        tracing::debug!("Process {} exited gracefully", execution_process.id);
                    }
                    Err(_) => {
                        tracing::debug!(
                            "Graceful shutdown timed out for process {}, force killing",
                            execution_process.id
                        );
                    }
                }
            }
        }

        if let Some(child) = child {
            {
                let mut child_guard = child.write().await;
                if let Err(e) = command::kill_process_group(&mut child_guard).await {
                    tracing::error!(
                        "Failed to stop execution process {}: {}",
                        execution_process.id,
                        e
                    );
                    return Err(e);
                }
            }
            self.remove_child_from_store(&execution_process.id).await;
            self.remove_registered_agent_process(execution_process.id, "explicit_stop")
                .await;
        } else {
            tracing::warn!(
                execution_process_id = %execution_process.id,
                session_id = %execution_process.session_id,
                requested_status = ?status,
                "execution process has no in-memory child handle; cleaning stale registry entry"
            );
            self.cleanup_registered_agent_execution(execution_process.id, "missing_child_stop")
                .await;
        }

        // Mark the process finished in the MsgStore and wait for DB persistence
        let db_stream_handle = self.take_db_stream_handle(&execution_process.id).await;
        if let Some(msg) = self.msg_stores.write().await.remove(&execution_process.id) {
            msg.push_finished();
        }
        if let Some(handle) = db_stream_handle {
            let _ = tokio::time::timeout(Duration::from_secs(5), handle).await;
        }

        tracing::debug!(
            "Execution process {} stopped successfully",
            execution_process.id
        );

        // Record after-head commit OID (best-effort)
        self.update_after_head_commits(execution_process.id).await;

        Ok(())
    }

    async fn stream_diff(
        &self,
        workspace: &Workspace,
        stats_only: bool,
    ) -> Result<futures::stream::BoxStream<'static, Result<LogMsg, std::io::Error>>, ContainerError>
    {
        if workspace.is_direct_folder() {
            return Ok(Box::pin(futures::stream::empty()));
        }
        let workspace_repos =
            WorkspaceRepo::find_by_workspace_id(&self.db.pool, workspace.id).await?;
        let target_branches: HashMap<_, _> = workspace_repos
            .iter()
            .map(|wr| (wr.repo_id, wr.target_branch.clone()))
            .collect();

        let repositories =
            WorkspaceRepo::find_repos_for_workspace(&self.db.pool, workspace.id).await?;

        let mut streams = Vec::new();

        let lifecycle = services::services::workspace_lifecycle::lock(workspace.id).await;
        let workspace = Workspace::find_by_id(&self.db.pool, workspace.id)
            .await?
            .ok_or_else(|| ContainerError::Other(anyhow!("Workspace missing")))?;
        let archived = workspace.archived;
        let workspace_root = if archived {
            PathBuf::from(workspace.container_ref.as_deref().unwrap_or(""))
        } else if workspace.is_execution_only() {
            services::services::workspace_usage::inspection_root(&workspace)
                .map_err(ContainerError::Other)?
        } else {
            PathBuf::from(
                self.ensure_container_exists_locked(&workspace, &lifecycle)
                    .await?,
            )
        };
        drop(lifecycle);

        for repo in repositories {
            let worktree_path = repo.worktree_dir(&workspace_root);
            let branch = &workspace.branch;

            let Some(target_branch) = target_branches.get(&repo.id) else {
                tracing::warn!(
                    "Skipping diff stream for repo {}: no target branch configured",
                    repo.name
                );
                continue;
            };

            let base_commit = match self
                .git()
                .get_base_commit(&repo.path, branch, target_branch)
            {
                Ok(c) => c,
                Err(e) => {
                    tracing::warn!(
                        "Skipping diff stream for repo {}: failed to get base commit: {}",
                        repo.name,
                        e
                    );
                    continue;
                }
            };

            let args = diff_stream::DiffStreamArgs {
                git_service: self.git().clone(),
                db: self.db().clone(),
                workspace_id: workspace.id,
                repo_id: repo.id,
                repo_path: repo.path.clone(),
                worktree_path: worktree_path.clone(),
                branch: branch.to_string(),
                target_branch: target_branch.clone(),
                base_commit: base_commit.clone(),
                stats_only,
                path_prefix: Some(repo.worktree_name().to_owned()),
            };
            let stream: futures::stream::BoxStream<'static, Result<LogMsg, std::io::Error>> =
                if archived && (workspace.worktree_deleted || !args.worktree_path.is_dir()) {
                    Box::pin(futures::stream::iter(
                        diff_stream::archived_snapshot(&args)
                            .map_err(|error| ContainerError::Other(anyhow!(error)))?
                            .into_iter()
                            .map(Ok),
                    ))
                } else {
                    Box::pin(self.create_live_diff_stream(args).await?)
                };
            streams.push(stream);
        }

        if streams.is_empty() {
            return Ok(Box::pin(futures::stream::empty()));
        }

        // Merge all streams into one
        Ok(Box::pin(futures::stream::select_all(streams)))
    }

    async fn try_commit_changes(&self, ctx: &ExecutionContext) -> Result<bool, ContainerError> {
        if ctx.workspace.is_direct_folder() {
            return Ok(false);
        }
        if !matches!(
            ctx.execution_process.run_reason,
            ExecutionProcessRunReason::CleanupScript,
        ) {
            return Ok(false);
        }

        if should_disable_default_commit_for_workspace(&self.db.pool, ctx.workspace.id).await? {
            tracing::info!(
                workspace_id = %ctx.workspace.id,
                "Skipping automatic commit for open Design Arena workspace"
            );
            return Ok(false);
        }

        let message = self.get_commit_message(ctx).await;

        let container_ref = ctx
            .workspace
            .container_ref
            .as_ref()
            .ok_or_else(|| ContainerError::Other(anyhow!("Container reference not found")))?;
        let workspace_root = PathBuf::from(container_ref);

        let repos_with_changes = self.check_repos_for_changes(&workspace_root, &ctx.repos)?;
        if repos_with_changes.is_empty() {
            tracing::debug!("No changes to commit in any repository");
            return Ok(false);
        }

        Ok(self.commit_repos(repos_with_changes, &message))
    }

    /// Copy files from the original project directory to the worktree.
    /// Skips files that already exist at target with same size.
    async fn copy_project_files(
        &self,
        source_dir: &Path,
        target_dir: &Path,
        copy_files: &str,
    ) -> Result<(), ContainerError> {
        let source_dir = source_dir.to_path_buf();
        let target_dir = target_dir.to_path_buf();
        let copy_files = copy_files.to_string();

        tokio::time::timeout(
            std::time::Duration::from_secs(30),
            tokio::task::spawn_blocking(move || {
                copy::copy_project_files_impl(&source_dir, &target_dir, &copy_files)
            }),
        )
        .await
        .map_err(|_| ContainerError::Other(anyhow!("Copy project files timed out after 30s")))?
        .map_err(|e| ContainerError::Other(anyhow!("Copy files task failed: {e}")))?
    }

    async fn kill_all_running_processes(&self) -> Result<(), ContainerError> {
        tracing::info!("Killing all running processes");
        let running_processes = ExecutionProcess::find_running(&self.db.pool).await?;

        tracing::info!(
            "Found {} running processes to kill",
            running_processes.len()
        );

        for process in running_processes {
            tracing::info!(
                "Killing process: id={}, run_reason={:?}",
                process.id,
                process.run_reason
            );
            if let Err(error) = self
                .stop_execution(&process, ExecutionProcessStatus::Killed)
                .await
            {
                tracing::error!(
                    "Failed to cleanly kill running execution process {:?}: {:?}",
                    process,
                    error
                );
            } else {
                tracing::info!("Successfully killed process: id={}", process.id);
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use db::models::{
        arena_group::{ArenaLifecycleStatus, ArenaMode, ArenaStatus},
        workspace::ContainerOwnership,
    };
    use executors::{
        executors::BaseCodingAgent, profile::ExecutorConfig,
        runtime::PROVIDER_SESSION_REFERENCE_SCHEMA_VERSION,
    };
    use sqlx::sqlite::SqlitePoolOptions;

    use super::*;

    #[tokio::test]
    async fn same_named_repositories_use_persisted_paths_for_instructions_ensure_and_cleanup() {
        use db::models::{
            workspace::{CreateWorkspace, Workspace},
            workspace_repo::{CreateWorkspaceRepo, WorkspaceRepo},
        };
        let tmp = tempfile::tempdir().unwrap();
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        db::run_migrations(&pool).await.unwrap();
        let workspace_id = Uuid::new_v4();
        Workspace::create(
            &pool,
            &CreateWorkspace {
                branch: "placement-test".into(),
                name: None,
            },
            workspace_id,
        )
        .await
        .unwrap();
        let mut selections = Vec::new();
        for parent in ["first", "second"] {
            let source = tmp.path().join(parent).join("same repo");
            std::fs::create_dir_all(source.join(".agents/skills/test")).unwrap();
            std::fs::write(
                source.join("AGENTS.md"),
                format!("Only this repository: {parent}\n"),
            )
            .unwrap();
            std::fs::write(
                source.join(".agents/skills/test/SKILL.md"),
                "---\nname: test\ndescription: synthetic\n---\nSynthetic only.\n",
            )
            .unwrap();
            for args in [
                vec!["init", "-b", "main"],
                vec!["add", "."],
                vec![
                    "-c",
                    "user.name=Fixture",
                    "-c",
                    "user.email=fixture@example.invalid",
                    "commit",
                    "-m",
                    "fixture",
                ],
            ] {
                let output = std::process::Command::new("git")
                    .arg("-C")
                    .arg(&source)
                    .args(args)
                    .output()
                    .unwrap();
                assert!(
                    output.status.success(),
                    "{}",
                    String::from_utf8_lossy(&output.stderr)
                );
            }
            let repo = Repo::find_or_create(&pool, &source, parent).await.unwrap();
            selections.push(CreateWorkspaceRepo {
                repo_id: repo.id,
                target_branch: "main".into(),
            });
        }
        WorkspaceRepo::create_many(&pool, workspace_id, &selections)
            .await
            .unwrap();
        let repos = WorkspaceRepo::find_repos_for_workspace(&pool, workspace_id)
            .await
            .unwrap();
        let inputs: Vec<_> = repos
            .iter()
            .cloned()
            .map(|r| RepoWorkspaceInput::new(r, "main".into()))
            .collect();
        let root = tmp.path().join("workspace");
        WorkspaceManager::create_workspace(&root, &inputs, "placement-test")
            .await
            .unwrap();
        LocalContainerService::create_workspace_config_files(&root, &repos)
            .await
            .unwrap();
        let parent = std::fs::read_to_string(root.join("AGENTS.md")).unwrap();
        assert!(!root.join(".git").exists());
        assert_ne!(repos[0].worktree_name(), repos[1].worktree_name());
        for repo in &repos {
            let checkout = repo.worktree_dir(&root);
            assert!(checkout.join(".agents/skills/test/SKILL.md").is_file());
            assert_eq!(
                std::fs::read_to_string(checkout.join("AGENTS.md")).unwrap(),
                std::fs::read_to_string(repo.path.join("AGENTS.md")).unwrap()
            );
            assert!(parent.contains(&format!(
                "@{}/AGENTS.md",
                repo.worktree_name().replace(' ', "\\ ")
            )));
            std::fs::write(checkout.join("local.txt"), repo.id.to_string()).unwrap();
            assert_eq!(
                GitService::new()
                    .get_worktree_status(&checkout)
                    .unwrap()
                    .untracked,
                1
            );
        }
        WorkspaceManager::ensure_workspace_exists(&root, &inputs, "placement-test")
            .await
            .unwrap();
        for repo in &repos {
            assert_eq!(
                std::fs::read_to_string(repo.worktree_dir(&root).join("local.txt")).unwrap(),
                repo.id.to_string()
            );
            assert!(!repo.path.join("local.txt").exists());
        }
        WorkspaceManager::cleanup_workspace_strict(&root, &repos)
            .await
            .unwrap();
        assert!(!root.exists());
        assert!(repos.iter().all(|r| r.path.join("AGENTS.md").is_file()));
        pool.close().await;
    }

    #[tokio::test]
    async fn common_root_contains_single_and_multiple_real_worktrees() {
        let tmp = tempfile::tempdir().unwrap();
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!("../db/migrations").run(&pool).await.unwrap();
        let mut inputs = Vec::new();
        for name in ["a", "b"] {
            let source = tmp.path().join("sources").join(name);
            std::fs::create_dir_all(&source).unwrap();
            for args in [
                vec!["init", "-b", "main"],
                vec![
                    "-c",
                    "user.name=Fixture",
                    "-c",
                    "user.email=fixture@example.invalid",
                    "commit",
                    "--allow-empty",
                    "-m",
                    "synthetic fixture",
                ],
            ] {
                let output = std::process::Command::new("git")
                    .arg("-C")
                    .arg(&source)
                    .args(args)
                    .output()
                    .unwrap();
                assert!(
                    output.status.success(),
                    "{}",
                    String::from_utf8_lossy(&output.stderr)
                );
            }
            let repo = Repo::find_or_create(&pool, &source, name).await.unwrap();
            inputs.push(RepoWorkspaceInput::new(repo, "main".into()));
        }
        for count in [1, 2] {
            let root = tmp.path().join(format!("work-{count}"));
            let created = WorkspaceManager::create_workspace(
                &root,
                &inputs[..count],
                &format!("fixture-{count}"),
            )
            .await
            .unwrap();
            assert_eq!(created.workspace_dir, root);
            assert!(!root.join(".git").exists());
            assert_eq!(created.worktrees.len(), count);
            for worktree in created.worktrees {
                assert!(worktree.worktree_path.starts_with(&root));
                assert!(worktree.worktree_path.join(".git").is_file());
            }
        }
    }

    #[tokio::test]
    async fn existing_output_survives_repository_addition_and_resume() {
        use db::models::{
            workspace::{CreateWorkspace, Workspace},
            workspace_repo::{CreateWorkspaceRepo, WorkspaceRepo},
        };
        let temp = tempfile::tempdir().unwrap();
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        db::run_migrations(&pool).await.unwrap();
        let source = temp.path().join("source/output");
        std::fs::create_dir_all(&source).unwrap();
        std::fs::write(
            source.join("AGENTS.md"),
            "Only output repository instructions",
        )
        .unwrap();
        for args in [
            vec!["init", "-b", "main"],
            vec!["add", "."],
            vec![
                "-c",
                "user.name=Fixture",
                "-c",
                "user.email=fixture@example.invalid",
                "commit",
                "-m",
                "fixture",
            ],
        ] {
            let output = std::process::Command::new("git")
                .arg("-C")
                .arg(&source)
                .args(args)
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
        let repo = Repo::find_or_create(&pool, &source, "Output")
            .await
            .unwrap();
        let root = temp.path().join("workspace");
        std::fs::create_dir_all(root.join("output")).unwrap();
        std::fs::write(root.join("output/user-result.txt"), "user result").unwrap();
        let id = Uuid::new_v4();
        Workspace::create(
            &pool,
            &CreateWorkspace {
                branch: "fixture".into(),
                name: None,
            },
            id,
        )
        .await
        .unwrap();
        Workspace::update_container_ref(&pool, id, root.to_str().unwrap())
            .await
            .unwrap();
        WorkspaceRepo::create_many(
            &pool,
            id,
            &[CreateWorkspaceRepo {
                repo_id: repo.id,
                target_branch: "main".into(),
            }],
        )
        .await
        .unwrap();
        let placed = WorkspaceRepo::find_repo_for_workspace(&pool, id, repo.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            placed.worktree_name(),
            format!("output--{}", repo.id.simple())
        );
        let inputs = [RepoWorkspaceInput::new(placed.clone(), "main".into())];
        for _ in 0..2 {
            WorkspaceManager::ensure_workspace_exists(&root, &inputs, "fixture")
                .await
                .unwrap();
            LocalContainerService::create_workspace_config_files(
                &root,
                std::slice::from_ref(&placed),
            )
            .await
            .unwrap();
            assert_eq!(
                std::fs::read_to_string(root.join("output/user-result.txt")).unwrap(),
                "user result"
            );
            assert!(placed.worktree_dir(&root).join(".git").is_file());
        }
        worktree_manager::WorktreeManager::cleanup_worktree(
            &worktree_manager::WorktreeCleanup::new(
                placed.worktree_dir(&root),
                Some(source.clone()),
            ),
        )
        .await
        .unwrap();
        assert_eq!(
            std::fs::read_to_string(root.join("output/user-result.txt")).unwrap(),
            "user result"
        );
        assert_eq!(
            std::fs::read_to_string(source.join("AGENTS.md")).unwrap(),
            "Only output repository instructions"
        );
        pool.close().await;
    }

    #[tokio::test]
    async fn unknown_repository_path_stops_enclosing_workspace_cleanup() {
        let temp = tempfile::tempdir().unwrap();
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        db::run_migrations(&pool).await.unwrap();
        let source = temp.path().join("source/output");
        std::fs::create_dir_all(&source).unwrap();
        let repo = Repo::find_or_create(&pool, &source, "Output")
            .await
            .unwrap();
        let root = temp.path().join("workspace");
        std::fs::create_dir_all(root.join("output")).unwrap();
        std::fs::write(root.join("output/user-result.txt"), "preserve").unwrap();
        std::fs::write(root.join("root-result.txt"), "also preserve").unwrap();
        assert!(
            WorkspaceManager::cleanup_workspace(&root, std::slice::from_ref(&repo))
                .await
                .is_err()
        );
        assert!(
            WorkspaceManager::cleanup_workspace_strict(&root, &[repo])
                .await
                .is_err()
        );
        assert_eq!(
            std::fs::read_to_string(root.join("output/user-result.txt")).unwrap(),
            "preserve"
        );
        assert_eq!(
            std::fs::read_to_string(root.join("root-result.txt")).unwrap(),
            "also preserve"
        );
        pool.close().await;
    }

    #[tokio::test]
    async fn legacy_parent_retirement_ignores_missing_children_and_never_clobbers_backups() {
        let tmp = tempfile::tempdir().unwrap();
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        db::run_migrations(&pool).await.unwrap();
        let child = tmp.path().join("a");
        std::fs::create_dir(&child).unwrap();
        std::fs::write(
            child.join("AGENTS.md"),
            "User AGENTS; old CLAUDE has migrated",
        )
        .unwrap();
        let repo = Repo::find_or_create(&pool, &child, "a").await.unwrap();
        let repos = [repo];
        std::fs::create_dir(tmp.path().join(".lvk")).unwrap();
        let backup = tmp.path().join(".lvk/legacy-workspace-CLAUDE.md");
        std::fs::write(&backup, "previous backup").unwrap();
        for _ in 0..2 {
            std::fs::write(tmp.path().join("CLAUDE.md"), "@a/CLAUDE.md\n").unwrap();
            LocalContainerService::create_workspace_config_files(tmp.path(), &repos)
                .await
                .unwrap();
            assert!(!tmp.path().join("CLAUDE.md").exists());
        }
        assert_eq!(std::fs::read_to_string(&backup).unwrap(), "previous backup");
        assert_eq!(
            std::fs::read_dir(tmp.path().join(".lvk")).unwrap().count(),
            3
        );
        assert_eq!(
            std::fs::read_to_string(child.join("AGENTS.md")).unwrap(),
            "User AGENTS; old CLAUDE has migrated"
        );
        // Missing child directory, not only a migrated child instruction file.
        std::fs::remove_file(child.join("AGENTS.md")).unwrap();
        std::fs::remove_dir(&child).unwrap();
        std::fs::write(tmp.path().join("CLAUDE.md"), "@a/CLAUDE.md\n").unwrap();
        LocalContainerService::create_workspace_config_files(tmp.path(), &repos)
            .await
            .unwrap();
        assert!(!tmp.path().join("CLAUDE.md").exists());
        for custom in [
            "Custom parent\n@a/CLAUDE.md\n",
            "@unknown/CLAUDE.md\n",
            "@a/CLAUDE.md\n@a/CLAUDE.md\n",
        ] {
            std::fs::write(tmp.path().join("CLAUDE.md"), custom).unwrap();
            LocalContainerService::create_workspace_config_files(tmp.path(), &repos)
                .await
                .unwrap();
            assert_eq!(
                std::fs::read_to_string(tmp.path().join("CLAUDE.md")).unwrap(),
                custom
            );
        }
        #[cfg(unix)]
        {
            std::fs::remove_file(tmp.path().join("CLAUDE.md")).unwrap();
            let target = tmp.path().join("user-instructions");
            std::fs::write(&target, "@a/CLAUDE.md\n").unwrap();
            std::os::unix::fs::symlink(&target, tmp.path().join("CLAUDE.md")).unwrap();
            LocalContainerService::create_workspace_config_files(tmp.path(), &repos)
                .await
                .unwrap();
            assert!(
                tmp.path()
                    .join("CLAUDE.md")
                    .symlink_metadata()
                    .unwrap()
                    .file_type()
                    .is_symlink()
            );
            assert_eq!(std::fs::read_to_string(target).unwrap(), "@a/CLAUDE.md\n");
        }
        #[cfg(unix)]
        {
            let fresh = tempfile::tempdir().unwrap();
            std::fs::create_dir(fresh.path().join(".lvk")).unwrap();
            let backup = fresh.path().join(".lvk/legacy-workspace-CLAUDE.md");
            let missing = fresh.path().join("missing-backup-target");
            std::os::unix::fs::symlink(&missing, &backup).unwrap();
            std::fs::write(fresh.path().join("CLAUDE.md"), "@a/CLAUDE.md\n").unwrap();
            LocalContainerService::create_workspace_config_files(fresh.path(), &repos)
                .await
                .unwrap();
            assert!(backup.symlink_metadata().unwrap().file_type().is_symlink());
            assert!(!missing.exists());
            assert!(!fresh.path().join("CLAUDE.md").exists());
            assert_eq!(
                std::fs::read_dir(fresh.path().join(".lvk"))
                    .unwrap()
                    .count(),
                2
            );
        }
        pool.close().await;
    }

    #[tokio::test]
    async fn parent_instructions_preserve_children_and_custom_files() {
        let tmp = tempfile::tempdir().unwrap();
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!("../db/migrations").run(&pool).await.unwrap();
        let mut repos = Vec::new();
        for name in ["a", "b"] {
            let child = tmp.path().join(name);
            std::fs::create_dir(&child).unwrap();
            std::fs::write(child.join("AGENTS.md"), format!("Only {name} marker")).unwrap();
            std::fs::write(child.join("CLAUDE.md"), "User Claude rules").unwrap();
            repos.push(Repo::find_or_create(&pool, &child, name).await.unwrap());
        }
        // Even legacy direct-Git workspaces never initialize repository memory.
        let direct = runtime_workspace(WorkspaceKind::DirectFolder);
        assert!(
            services::services::repository_memory::begin_coding_run(
                &pool,
                &repos[0],
                &direct,
                Uuid::new_v4(),
                tmp.path(),
                "main",
                true
            )
            .await
            .unwrap()
            .is_none()
        );
        assert!(
            services::services::repository_memory::prepare_workspace_source(
                &pool,
                &git::GitService::new(),
                &repos[0],
                &direct,
                tmp.path(),
                "main",
                false
            )
            .await
            .is_err()
        );
        // Exact previous LVK-generated parent is backed up, never child files.
        std::fs::write(tmp.path().join("CLAUDE.md"), "@a/CLAUDE.md\n@b/CLAUDE.md\n").unwrap();
        LocalContainerService::create_workspace_config_files(tmp.path(), &repos)
            .await
            .unwrap();
        let generated = std::fs::read_to_string(tmp.path().join("AGENTS.md")).unwrap();
        assert!(generated.contains("@a/AGENTS.md"));
        assert!(generated.contains("@b/AGENTS.md"));
        assert!(generated.contains("ONLY to their own repository"));
        assert!(!tmp.path().join("CLAUDE.md").exists());
        assert!(tmp.path().join(".lvk/legacy-workspace-CLAUDE.md").is_file());
        assert_eq!(
            std::fs::read_to_string(tmp.path().join("a/CLAUDE.md")).unwrap(),
            "User Claude rules"
        );
        assert_eq!(
            std::fs::read_to_string(tmp.path().join("a/AGENTS.md")).unwrap(),
            "Only a marker"
        );
        std::fs::write(tmp.path().join("AGENTS.md"), "Custom parent").unwrap();
        std::fs::write(tmp.path().join("CLAUDE.md"), "Custom Claude parent").unwrap();
        LocalContainerService::create_workspace_config_files(tmp.path(), &repos[..1])
            .await
            .unwrap();
        assert_eq!(
            std::fs::read_to_string(tmp.path().join("AGENTS.md")).unwrap(),
            "Custom parent"
        );
        assert_eq!(
            std::fs::read_to_string(tmp.path().join("CLAUDE.md")).unwrap(),
            "Custom Claude parent"
        );
    }

    async fn setup_container_policy_pool() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("connect sqlite");

        for statement in [
            "CREATE TABLE integration_runs (workspace_id BLOB)",
            r#"
            CREATE TABLE arena_groups (
                id BLOB PRIMARY KEY,
                issue_id BLOB NOT NULL,
                project_id BLOB NOT NULL,
                prompt TEXT NOT NULL,
                base_branch TEXT NOT NULL,
                mode TEXT NOT NULL,
                lifecycle_status TEXT NOT NULL,
                promoted_workspace_id BLOB,
                implementation_workspace_id BLOB,
                promoted_at TEXT,
                closed_at TEXT,
                created_at TEXT NOT NULL DEFAULT (datetime('now', 'subsec')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now', 'subsec'))
            )
            "#,
            r#"
            CREATE TABLE workspaces (
                id BLOB PRIMARY KEY,
                usage TEXT NOT NULL DEFAULT 'interactive',
                arena_group_id BLOB
            )
            "#,
        ] {
            sqlx::query(statement)
                .execute(&pool)
                .await
                .expect("create schema");
        }

        pool
    }

    async fn insert_workspace_in_arena(
        pool: &SqlitePool,
        mode: ArenaMode,
        lifecycle_status: ArenaLifecycleStatus,
    ) -> Uuid {
        let group_id = Uuid::new_v4();
        let workspace_id = Uuid::new_v4();

        sqlx::query(
            r#"INSERT INTO arena_groups
                  (id, issue_id, project_id, prompt, base_branch, mode, lifecycle_status)
               VALUES (?, ?, ?, ?, ?, ?, ?)"#,
        )
        .bind(group_id)
        .bind(Uuid::new_v4())
        .bind(Uuid::new_v4())
        .bind("Compare designs")
        .bind("main")
        .bind(mode)
        .bind(lifecycle_status)
        .execute(pool)
        .await
        .expect("insert arena group");

        sqlx::query("INSERT INTO workspaces (id, arena_group_id) VALUES (?, ?)")
            .bind(workspace_id)
            .bind(group_id)
            .execute(pool)
            .await
            .expect("insert workspace");

        workspace_id
    }

    fn runtime_workspace(workspace_kind: WorkspaceKind) -> Workspace {
        let now = Utc::now();
        Workspace {
            id: Uuid::new_v4(),
            task_id: None,
            container_ref: Some("C:\\runtime-workspace".to_string()),
            workspace_kind,
            container_ownership: ContainerOwnership::Managed,
            usage: Default::default(),
            execution_owner: None,
            branch: "main".to_string(),
            setup_completed_at: None,
            created_at: now,
            updated_at: now,
            archived: false,
            pinned: false,
            name: None,
            worktree_deleted: false,
            arena_group_id: None,
            arena_status: ArenaStatus::Active,
        }
    }

    fn queued_follow_up_data() -> DraftFollowUpData {
        let mut executor_config = ExecutorConfig::new(BaseCodingAgent::Codex);
        executor_config.variant = Some("queued-profile".to_string());
        executor_config.model_id = Some("gpt-5.4".to_string());
        executor_config.agent_id = Some("default".to_string());
        executor_config.reasoning_id = Some("high".to_string());
        DraftFollowUpData {
            message: "Continue the canonical task".to_string(),
            executor_config,
            selected_skills: None,
        }
    }

    #[test]
    fn queued_request_without_provider_session_is_canonical_initial() {
        let session_id = Uuid::new_v4();
        let workspace = runtime_workspace(WorkspaceKind::Worktree);
        let queued_data = queued_follow_up_data();

        let (request, attempt) = build_queued_agent_run_requests(
            session_id,
            &workspace,
            "C:\\runtime-workspace".to_string(),
            &queued_data,
            None,
        )
        .expect("build queued AgentRun request");

        request.validate_current().expect("valid request envelope");
        attempt
            .validate_for_run(&request)
            .expect("valid attempt envelope");
        assert_eq!(request.intent, AgentRunIntent::Initial);
        assert_eq!(request.input.content, queued_data.message);
        assert_eq!(attempt.executor_config, queued_data.executor_config);
        assert_eq!(attempt.selected_skills, None);
        assert_eq!(attempt.provider_session, None);
        assert_eq!(request.workspace.mode, WorkspaceMode::IsolatedWorktree);
        assert_eq!(request.workspace, attempt.workspace);
    }

    #[test]
    fn queued_request_normalizes_empty_selected_skills() {
        let mut data = queued_follow_up_data();
        data.selected_skills = Some(Vec::new());
        let (request, attempt) = build_queued_agent_run_requests(
            Uuid::new_v4(),
            &runtime_workspace(WorkspaceKind::DirectFolder),
            "/tmp/queue-test".into(),
            &data,
            None,
        )
        .unwrap();
        attempt.validate_for_run(&request).unwrap();
        assert_eq!(attempt.selected_skills, None);
    }

    #[test]
    fn queued_request_with_provider_session_is_native_follow_up() {
        let session_id = Uuid::new_v4();
        let workspace = runtime_workspace(WorkspaceKind::DirectFolder);
        let queued_data = queued_follow_up_data();
        let runtime_profile_id = queued_data.executor_config.profile_id().cache_key();
        let provider_session = ProviderSessionReference {
            schema_version: PROVIDER_SESSION_REFERENCE_SCHEMA_VERSION,
            provider_id: DirectProvider::Codex.id().to_string(),
            runtime_profile_id,
            provider_session_id: "native-session-1".to_string(),
            observed_at: Utc::now(),
            metadata: Some(serde_json::json!({ "source": "test" })),
        };

        let (request, attempt) = build_queued_agent_run_requests(
            session_id,
            &workspace,
            "C:\\shared-workspace".to_string(),
            &queued_data,
            Some(provider_session.clone()),
        )
        .expect("build queued follow-up request");

        request.validate_current().expect("valid request envelope");
        attempt
            .validate_for_run(&request)
            .expect("valid attempt envelope");
        assert_eq!(request.intent, AgentRunIntent::FollowUp);
        assert_eq!(attempt.provider_session, Some(provider_session));
        assert_eq!(attempt.executor_config, queued_data.executor_config);
        assert_eq!(attempt.selected_skills, None);
        assert_eq!(request.workspace.mode, WorkspaceMode::SharedWorkspace);
    }

    #[test]
    fn queued_request_maps_both_workspace_collaboration_modes() {
        let queued_data = queued_follow_up_data();
        let cases = [
            (WorkspaceKind::DirectFolder, WorkspaceMode::SharedWorkspace),
            (WorkspaceKind::Worktree, WorkspaceMode::IsolatedWorktree),
        ];

        for (workspace_kind, expected_mode) in cases {
            let workspace = runtime_workspace(workspace_kind);
            let (request, attempt) = build_queued_agent_run_requests(
                Uuid::new_v4(),
                &workspace,
                "C:\\workspace".to_string(),
                &queued_data,
                None,
            )
            .expect("build workspace-mode request");

            assert_eq!(request.workspace.mode, expected_mode);
            assert_eq!(attempt.workspace.mode, expected_mode);
        }
    }

    #[tokio::test]
    async fn open_design_arena_workspace_disables_default_commit() {
        let pool = setup_container_policy_pool().await;
        let workspace_id =
            insert_workspace_in_arena(&pool, ArenaMode::Design, ArenaLifecycleStatus::Open).await;

        let disabled = should_disable_default_commit_for_workspace(&pool, workspace_id)
            .await
            .expect("policy lookup");

        assert!(disabled);
    }

    #[tokio::test]
    async fn implementation_started_workspace_allows_normal_commit_policy() {
        let pool = setup_container_policy_pool().await;
        let workspace_id = insert_workspace_in_arena(
            &pool,
            ArenaMode::Design,
            ArenaLifecycleStatus::ImplementationStarted,
        )
        .await;

        let disabled = should_disable_default_commit_for_workspace(&pool, workspace_id)
            .await
            .expect("policy lookup");

        assert!(!disabled);
    }

    #[tokio::test]
    async fn conversation_worker_never_gets_a_terminal_automatic_commit() {
        let pool = setup_container_policy_pool().await;
        let workspace = insert_workspace_in_arena(
            &pool,
            ArenaMode::Design,
            ArenaLifecycleStatus::ImplementationStarted,
        )
        .await;
        sqlx::query("INSERT INTO integration_runs(workspace_id) VALUES(?)")
            .bind(workspace)
            .execute(&pool)
            .await
            .unwrap();
        assert!(
            should_disable_default_commit_for_workspace(&pool, workspace)
                .await
                .unwrap()
        );
    }

    #[tokio::test]
    async fn execution_reservation_disables_commits_until_released() {
        let pool = setup_container_policy_pool().await;
        let workspace_id = insert_workspace_in_arena(
            &pool,
            ArenaMode::Design,
            ArenaLifecycleStatus::ImplementationStarted,
        )
        .await;
        assert!(
            !should_disable_default_commit_for_workspace(&pool, workspace_id)
                .await
                .unwrap()
        );
        sqlx::query("UPDATE workspaces SET usage='execution_only' WHERE id=?")
            .bind(workspace_id)
            .execute(&pool)
            .await
            .unwrap();
        // The execution owner controls reserved work; the ordinary finalizer
        // must not append a commit while that reservation is active.
        assert!(
            should_disable_default_commit_for_workspace(&pool, workspace_id)
                .await
                .unwrap()
        );
        sqlx::query("UPDATE workspaces SET usage='interactive' WHERE id=?")
            .bind(workspace_id)
            .execute(&pool)
            .await
            .unwrap();
        // An ordinary workspace returns to its normal policy after owner release.
        assert!(
            !should_disable_default_commit_for_workspace(&pool, workspace_id)
                .await
                .unwrap()
        );
    }
}
