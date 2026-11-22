//! Workspace usage policy shared by admission, dispatch and inspection.
//! Product owners retain their existing state machines; attribution is not a
//! capability supplied by clients and it never expires with a dispatcher lease.
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use anyhow::{Context, ensure};
use db::models::{
    repo::Repo,
    repository_state::RepositoryMemoryStore,
    workspace::Workspace,
    workspace_repo::WorkspaceRepo,
    workspace_usage::{INTEGRATION, OPENWIKI_BOOTSTRAP, OPENWIKI_SYNC, WorkspaceExecutionOwner},
};
use executors::runtime::AgentRunRequestEnvelope;
use sqlx::{SqlitePool, types::Json};
use utils::repository_memory::RepositoryWikiStatus;
use uuid::Uuid;

mod lifecycle;
pub use lifecycle::{
    allows_wiki_instruction, ensure_wiki_board_link, ensure_wiki_board_link_for_project,
    release_completed,
};

/// Only existing, host-owned records establish historical usage. Ordinary
/// workflows, unlinked workspaces and names are deliberately not evidence.
pub async fn backfill(pool: &SqlitePool) -> anyhow::Result<()> {
    let mut evidence: BTreeMap<Uuid, Vec<WorkspaceExecutionOwner>> = BTreeMap::new();
    let rows: Vec<(Uuid, Uuid, Uuid, String)> = sqlx::query_as(
        "SELECT workspace_id,id,repository_id,'integration' FROM integration_runs WHERE workspace_id IS NOT NULL
         UNION ALL SELECT workspace_id,id,repository_id,'openwiki_bootstrap' FROM workflow_runs
         WHERE trigger_source='openwiki_bootstrap' AND issue_id IS NULL AND attempt_id IS NULL
           AND repository_id IS NOT NULL AND workspace_id IS NOT NULL"
    ).fetch_all(pool).await?;
    for (ws, run, repo, kind) in rows {
        evidence
            .entry(ws)
            .or_default()
            .push(WorkspaceExecutionOwner::new(&kind, repo, Some(run)));
    }
    let mut unproven = 0;
    for (id,) in sqlx::query_as::<_, (Uuid,)>(
        "SELECT id FROM workspaces WHERE usage='interactive' AND execution_owner IS NULL",
    )
    .fetch_all(pool)
    .await?
    {
        if evidence.contains_key(&id) {
            continue;
        }
        let workspace = Workspace::find_by_id(pool, id)
            .await?
            .context("Workspace disappeared during usage backfill")?;
        for repo in WorkspaceRepo::find_repos_for_workspace(pool, id).await? {
            let store = match RepositoryMemoryStore::read_only_configured_for_repository(
                pool, &repo.name, repo.id,
            )
            .await
            {
                Ok(Some(store)) => store,
                Ok(None) => continue,
                Err(error) => {
                    tracing::warn!(%id, %error, "Unreadable historical repository memory; preserving usage");
                    continue;
                }
            };
            let setup = match store.wiki_setup(id) {
                Ok(Some(setup)) => setup,
                Ok(None) => continue,
                Err(error) => {
                    tracing::warn!(%id, %error, "Unreadable historical Wiki setup; preserving usage");
                    continue;
                }
            };
            let expected = workspace
                .container_ref
                .as_ref()
                .map(|root| Path::new(root).join(repo.worktree_name()));
            if setup.workspace_id != id
                || expected.as_deref() != Some(Path::new(&setup.repository_path))
            {
                tracing::warn!(%id, "Wiki setup path identity mismatch; preserving usage");
                continue;
            }
            let runs: Vec<(Uuid,)> =
                sqlx::query_as("SELECT id FROM agent_runs WHERE workspace_id=?")
                    .bind(id)
                    .fetch_all(pool)
                    .await?;
            // A setup checkpoint proves internal purpose, but multiple unrelated
            // runs do not establish which one published. Keep that outcome unknown.
            let run_id = if runs.len() == 1 {
                Some(runs[0].0)
            } else {
                None
            };
            evidence
                .entry(id)
                .or_default()
                .push(WorkspaceExecutionOwner::new(OPENWIKI_SYNC, repo.id, run_id));
        }
        if !evidence.contains_key(&id) {
            unproven += 1;
        }
    }
    let mut classified = 0;
    let mut conflicts = 0;
    for (id, mut owners) in evidence {
        let mut unique = Vec::new();
        for owner in owners {
            if !unique.contains(&owner) {
                unique.push(owner);
            }
        }
        owners = unique;
        let owner = if owners.len() == 1 {
            owners.remove(0)
        } else {
            conflicts += 1;
            tracing::error!(%id, ?owners, "Conflicting execution ownership; inspection only");
            WorkspaceExecutionOwner {
                kind: "conflicting_evidence".into(),
                run_id: None,
                repository_id: None,
                result: None,
            }
        };
        classified += sqlx::query("UPDATE workspaces SET usage='execution_only',execution_owner=? WHERE id=? AND usage='interactive' AND execution_owner IS NULL")
            .bind(Json(owner)).bind(id).execute(pool).await?.rows_affected();
    }
    tracing::info!(
        classified,
        conflicts,
        without_internal_evidence = unproven,
        "Workspace usage backfill (unproven workspaces remain interactive; not inferred from names)"
    );
    // Backfill links once at startup. The release monitor must not recreate a
    // card that the operator intentionally unlinks or deletes later.
    let ids: Vec<Uuid> = sqlx::query_scalar("SELECT id FROM workspaces WHERE usage='execution_only' AND json_extract(execution_owner,'$.kind') IN ('openwiki_sync','openwiki_bootstrap')")
        .fetch_all(pool).await?;
    for id in ids {
        if let Some(workspace) = Workspace::find_by_id(pool, id).await?
            && let Err(error) = ensure_wiki_board_link(pool, &workspace).await
        {
            tracing::warn!(workspace_id=%id, %error, "Historical Wiki Board link unavailable");
        }
    }
    // Migration can only see owners already stored at that time. Reuse the
    // startup reconciliation's recovered evidence for older linked Issues too.
    // This never runs from Board reads and does not recreate normal tags.
    let mut tx = pool.begin_with("BEGIN IMMEDIATE").await?;
    let links: Vec<(Uuid, Uuid)> = sqlx::query_as(
        "SELECT DISTINCT i.project_id,i.id FROM workspaces w
         JOIN local_workspace_links l ON l.workspace_id=w.id
         JOIN local_issues i ON i.id=l.issue_id AND i.project_id=l.project_id
         WHERE json_extract(w.execution_owner,'$.kind') IN ('openwiki_bootstrap','openwiki_sync','integration')"
    ).fetch_all(&mut *tx).await?;
    for (project, issue) in links {
        db::models::board_tag::attach_system(&mut tx, project, issue).await?;
    }
    tx.commit().await?;
    Ok(())
}

/// Reserve/dispatch checks complement (never replace) the narrower phase and
/// executor validation performed by the owning runtime.
pub async fn validate_agent_owner(
    pool: &SqlitePool,
    workspace: &Workspace,
    request: &AgentRunRequestEnvelope,
    launching: bool,
) -> anyhow::Result<()> {
    if !workspace.is_execution_only() {
        return Ok(());
    }
    let owner = workspace
        .execution_owner
        .as_ref()
        .context("Execution-only workspace has no owner; inspect its history")?;
    ensure!(
        owner.result.is_none(),
        "Execution workspace has a terminal owner result"
    );
    let repo_id = owner
        .repository_id
        .context("Execution repository identity is missing")?;
    ensure!(
        WorkspaceRepo::find_by_workspace_and_repo_id(pool, workspace.id, repo_id)
            .await?
            .is_some(),
        "Execution owner repository membership mismatch"
    );
    match owner.kind.as_str() {
        OPENWIKI_BOOTSTRAP | OPENWIKI_SYNC => {
            let repo = Repo::find_by_id(pool, repo_id)
                .await?
                .context("Execution repository missing")?;
            let store = RepositoryMemoryStore::read_only_configured_for_repository(
                pool, &repo.name, repo.id,
            )
            .await?
            .context("Maintenance owner missing")?;
            let state = store.state().await?;
            ensure!(
                state.maintenance_workspace_id == Some(workspace.id)
                    && (state.bootstrap.is_some()
                        || state.maintenance_session_id == Some(request.session_id)),
                "Maintenance execution identity mismatch"
            );
            ensure!(
                matches!(
                    state.status,
                    RepositoryWikiStatus::Initializing | RepositoryWikiStatus::Reconciling
                ),
                "Maintenance is not accepting Agent dispatch"
            );
            if owner.kind == OPENWIKI_BOOTSTRAP {
                let bootstrap = state.bootstrap.context("Bootstrap ownership missing")?;
                ensure!(
                    Some(bootstrap.workflow_run_id) == owner.run_id,
                    "Bootstrap run does not match workspace owner"
                );
                let (_, run_id, correlation, initial_run) =
                    bootstrap_delegation(pool, workspace.id, request.session_id)
                        .await?
                        .context("Bootstrap node has not delegated this Session")?;
                ensure!(
                    run_id == bootstrap.workflow_run_id
                        && request.correlation_id == correlation
                        && (request.agent_run_id == initial_run
                            || request.intent == executors::runtime::AgentRunIntent::FollowUp),
                    "Bootstrap execution identity mismatch"
                );
            } else {
                ensure!(
                    state.bootstrap.is_none(),
                    "Sync cannot borrow Bootstrap ownership"
                );
                ensure!(
                    owner.run_id.is_none_or(|id| id == request.agent_run_id)
                        && state
                            .active_run_id
                            .is_none_or(|id| id == request.agent_run_id),
                    "Sync run identity mismatch"
                );
                if launching {
                    ensure!(
                        owner.run_id == Some(request.agent_run_id)
                            && state.active_run_id == owner.run_id,
                        "Sync ownership must be bound before launch"
                    );
                }
            }
        }
        _ => anyhow::bail!(
            "Unknown execution owner {}; inspect history instead of starting arbitrary work",
            owner.kind
        ),
    }
    Ok(())
}

/// Existing Workflow node records are the sole Bootstrap delegation authority.
pub async fn bootstrap_delegation(
    pool: &SqlitePool,
    workspace_id: Uuid,
    session_id: Uuid,
) -> anyhow::Result<Option<(String, Uuid, Uuid, Uuid)>> {
    Ok(sqlx::query_as("SELECT ne.node_id,wr.id,wr.orchestration_run_id,ne.agent_run_id FROM node_executions ne JOIN workflow_runs wr ON wr.id=ne.run_id WHERE wr.workspace_id=? AND ne.session_id=? AND wr.trigger_source='openwiki_bootstrap' AND wr.status IN ('running','awaiting_human') AND ne.status IN ('pending','running') ORDER BY ne.iteration DESC LIMIT 1")
        .bind(workspace_id).bind(session_id).fetch_optional(pool).await?)
}

/// No mkdir, worktree restoration, scripts, or database changes. Callers retain
/// their repository membership and path/symlink checks beneath this root.
pub fn inspection_root(workspace: &Workspace) -> anyhow::Result<PathBuf> {
    ensure!(
        !workspace.worktree_deleted,
        "Execution files were cleaned up; saved logs are still available"
    );
    let root = PathBuf::from(
        workspace
            .container_ref
            .as_ref()
            .context("Execution files have not been prepared")?,
    );
    ensure!(
        root.is_dir(),
        "Execution files are unavailable; viewing does not recreate a worktree"
    );
    Ok(root)
}

pub async fn validate_script_owner(
    pool: &SqlitePool,
    workspace_id: Uuid,
    _session_id: Uuid,
    _action: &executors::actions::ExecutorAction,
    _reason: &db::models::execution_process::ExecutionProcessRunReason,
) -> anyhow::Result<()> {
    let ws = Workspace::find_by_id(pool, workspace_id)
        .await?
        .context("Workspace missing")?;
    ensure!(
        !ws.is_execution_only(),
        "Execution-only workspace does not permit arbitrary scripts"
    );
    Ok(())
}

pub async fn cleanup_allowed(pool: &SqlitePool, workspace_id: Uuid) -> anyhow::Result<bool> {
    let ws = Workspace::find_by_id(pool, workspace_id)
        .await?
        .context("Workspace missing")?;
    if !ws.is_execution_only() {
        return Ok(true);
    }
    let busy: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM agent_runs WHERE workspace_id=? AND status NOT IN ('succeeded','failed','cancelled','crashed','audit_failed')) OR EXISTS(SELECT 1 FROM agent_process_registry p JOIN agent_run_attempts a ON a.id=p.run_attempt_id JOIN agent_runs r ON r.id=a.agent_run_id WHERE r.workspace_id=? AND p.registry_status IN ('reserved','spawned','running','unreachable')) OR EXISTS(SELECT 1 FROM execution_processes p JOIN sessions s ON s.id=p.session_id WHERE s.workspace_id=? AND p.status='running')")
        .bind(workspace_id).bind(workspace_id).bind(workspace_id).fetch_one(pool).await?;
    if busy {
        return Ok(false);
    }
    let Some(owner) = ws.execution_owner else {
        return Ok(false);
    };
    match owner.kind.as_str() {
        INTEGRATION => {
            let Some(id) = owner.run_id else {
                return Ok(owner.result.is_some());
            };
            let run = db::models::integration::IntegrationRun::find(pool, id).await?;
            Ok(run.workspace_id == Some(workspace_id)
                && matches!(
                    run.status.as_str(),
                    "succeeded" | "failed" | "cancelled" | "blocked"
                ))
        }
        OPENWIKI_BOOTSTRAP | OPENWIKI_SYNC => {
            let Some(repo_id) = owner.repository_id else {
                return Ok(false);
            };
            let Some(repo) = Repo::find_by_id(pool, repo_id).await? else {
                return Ok(false);
            };
            if let Some(store) = RepositoryMemoryStore::read_only_configured_for_repository(
                pool, &repo.name, repo_id,
            )
            .await?
            {
                let state = store.state().await?;
                if state.maintenance_workspace_id == Some(workspace_id)
                    && (state.active_run_id.is_some()
                        || state.bootstrap.is_some()
                        || state.file_operation.is_some())
                {
                    return Ok(false);
                }
            }
            if owner.kind == OPENWIKI_SYNC {
                return Ok(owner.result.is_some());
            }
            let terminal: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM workflow_runs WHERE id=? AND workspace_id=? AND status IN ('succeeded','failed','canceled'))")
                .bind(owner.run_id).bind(workspace_id).fetch_one(pool).await?;
            Ok(terminal || owner.result.is_some())
        }
        _ => Ok(false),
    }
}

#[cfg(test)]
mod tests {
    use db::models::{
        session::{CreateSession, Session},
        workspace::CreateWorkspace,
        workspace_repo::CreateWorkspaceRepo,
    };
    use executors::{
        actions::{
            ExecutorAction, ExecutorActionType,
            script::{ScriptContext, ScriptRequest, ScriptRequestLanguage},
        },
        runtime::*,
    };

    use super::*;

    async fn fixture() -> (SqlitePool, Repo, Uuid) {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!("../db/migrations").run(&pool).await.unwrap();
        let repo = Repo::find_or_create(
            &pool,
            &PathBuf::from(format!("/nonexistent/workspace-usage-{}", Uuid::new_v4())),
            "Usage test",
        )
        .await
        .unwrap();
        let project = Uuid::new_v4();
        sqlx::query("INSERT INTO projects(id,name) VALUES(?,'Usage test')")
            .bind(project)
            .execute(&pool)
            .await
            .unwrap();
        (pool, repo, project)
    }
    async fn workspace(
        pool: &SqlitePool,
        repo: &Repo,
        owner: Option<&WorkspaceExecutionOwner>,
    ) -> Workspace {
        let ws = Workspace::create_with_owner(
            pool,
            &CreateWorkspace {
                branch: Uuid::new_v4().to_string(),
                name: Some("OpenWiki: misleading human name".into()),
            },
            Uuid::new_v4(),
            owner,
        )
        .await
        .unwrap();
        WorkspaceRepo::create_many(
            pool,
            ws.id,
            &[CreateWorkspaceRepo {
                repo_id: repo.id,
                target_branch: "main".into(),
            }],
        )
        .await
        .unwrap();
        ws
    }
    async fn integration(
        pool: &SqlitePool,
        repo: &Repo,
        project: Uuid,
        ws: Uuid,
        id: Uuid,
        session: Option<Uuid>,
        status: &str,
    ) {
        sqlx::query("INSERT INTO integration_runs(id,request_key,project_id,repository_id,storage_identity,target_ref,status,workspace_id,session_id,payload) VALUES(?,?,?,?,?,'refs/heads/test',?,?,?,?)")
            .bind(id).bind(id.to_string()).bind(project).bind(repo.id).bind(repo.path.to_string_lossy().as_ref()).bind(status).bind(ws).bind(session)
            .bind(Json(db::models::integration::IntegrationPayload::default())).execute(pool).await.unwrap();
    }
    fn request(ws: &Workspace, session_id: Uuid, correlation_id: Uuid) -> AgentRunRequestEnvelope {
        AgentRunRequestEnvelope {
            schema_version: 1,
            payload_version: 1,
            request_id: Uuid::new_v4(),
            idempotency_key: Uuid::new_v4().to_string(),
            session_id,
            agent_run_id: Uuid::new_v4(),
            turn_id: Uuid::new_v4(),
            correlation_id,
            intent: AgentRunIntent::Initial,
            runtime_profile_id: "CODEX".into(),
            provider_id: "codex".into(),
            workspace: WorkspaceReference {
                workspace_id: ws.id,
                mode: WorkspaceMode::IsolatedWorktree,
                path: "/nonexistent".into(),
            },
            input: CanonicalMessage {
                message_id: Uuid::new_v4(),
                role: AgentRuntimeMessageRole::User,
                content: "test".into(),
            },
            created_at: chrono::Utc::now(),
        }
    }

    async fn memory_fixture() -> (SqlitePool, Repo, tempfile::TempDir, RepositoryMemoryStore) {
        let (pool, mut repo, _) = fixture().await;
        // Only DB state is observed by these tests. No repository artifact
        // directory or user shared storage is provisioned.
        let temp = tempfile::tempdir().unwrap();
        repo.name = format!("usage-{}", repo.id);
        let store = RepositoryMemoryStore::with_artifacts(
            &pool,
            repo.id,
            utils::repository_memory::RepositoryMemoryStore::at_artifacts(temp.path()).unwrap(),
        );
        (pool, repo, temp, store)
    }

    #[tokio::test]
    async fn bootstrap_delegates_only_current_child_and_retains_usage_after_owner_release() {
        use utils::repository_memory::{OpenWikiBootstrapOwner, RepositoryMemoryState};
        let (pool, repo, _temp, store) = memory_fixture().await;
        let run_id = Uuid::new_v4();
        let ws = workspace(
            &pool,
            &repo,
            Some(&WorkspaceExecutionOwner::new(
                OPENWIKI_BOOTSTRAP,
                repo.id,
                Some(run_id),
            )),
        )
        .await;
        let mut state = RepositoryMemoryState {
            status: RepositoryWikiStatus::Initializing,
            maintenance_workspace_id: Some(ws.id),
            ..Default::default()
        };
        let mut previous = None;
        sqlx::query(
            "INSERT INTO workflows(id,name,graph_json,source) VALUES (?,'Bootstrap','{}','system')",
        )
        .bind(run_id)
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query("INSERT INTO orchestration_runs(id,request_id,idempotency_key,correlation_id,product_kind,source_definition_id,source_definition_version,plan_schema_version,plan_snapshot,status) VALUES (?,?,?,?,'workflow',?,'test',1,'{}','running')")
            .bind(run_id).bind(run_id).bind(run_id.to_string()).bind(run_id).bind(run_id).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO workflow_runs(id,workflow_id,repository_id,workspace_id,trigger_source,input_text,graph_snapshot,status,orchestration_run_id) VALUES (?,?,?,?,'openwiki_bootstrap','','{}','running',?)")
            .bind(run_id).bind(run_id).bind(repo.id).bind(ws.id).bind(run_id).execute(&pool).await.unwrap();
        for node in ["generate", "review", "refine", "re-review"] {
            let req = request(&ws, Uuid::new_v4(), run_id);
            state.maintenance_session_id = Some(req.session_id);
            state.bootstrap = Some(OpenWikiBootstrapOwner {
                workflow_run_id: run_id,
            });
            sqlx::query("UPDATE node_executions SET status='succeeded' WHERE run_id=?")
                .bind(run_id)
                .execute(&pool)
                .await
                .unwrap();
            sqlx::query("INSERT INTO sessions(id,workspace_id) VALUES (?,?)")
                .bind(req.session_id)
                .bind(ws.id)
                .execute(&pool)
                .await
                .unwrap();
            sqlx::query("INSERT INTO agent_runs(id,session_id,workspace_id,request_id,idempotency_key,correlation_id,schema_version,payload_version,runtime_profile_id,provider_id,workspace_mode,workspace_path,request_envelope,status) VALUES (?,?,?,?,?,?,1,1,'CODEX:default','codex','isolated_worktree',?,'{}','succeeded')")
                .bind(req.agent_run_id).bind(req.session_id).bind(ws.id).bind(req.request_id).bind(req.idempotency_key.clone()).bind(run_id).bind(&req.workspace.path).execute(&pool).await.unwrap();
            sqlx::query("INSERT INTO node_executions(id,run_id,node_id,node_type,iteration,status,session_id,agent_run_id) VALUES (?,?,?,'agent',0,'running',?,?)")
                .bind(Uuid::new_v4()).bind(run_id).bind(node).bind(req.session_id).bind(req.agent_run_id).execute(&pool).await.unwrap();
            let lock = store.try_lock().unwrap();
            store.save_state(&state).await.unwrap();
            drop(lock);
            validate_agent_owner(&pool, &ws, &req, false).await.unwrap();
            validate_agent_owner(&pool, &ws, &req, true).await.unwrap();
            let mut spoof = req.clone();
            spoof.agent_run_id = Uuid::new_v4();
            assert!(
                validate_agent_owner(&pool, &ws, &spoof, true)
                    .await
                    .is_err()
            );
            if matches!(node, "generate" | "refine") {
                let mut continued = req.clone();
                continued.intent = executors::runtime::AgentRunIntent::FollowUp;
                continued.agent_run_id = Uuid::new_v4();
                validate_agent_owner(&pool, &ws, &continued, true)
                    .await
                    .unwrap();
                continued.correlation_id = Uuid::new_v4();
                assert!(
                    validate_agent_owner(&pool, &ws, &continued, true)
                        .await
                        .is_err()
                );
            }
            if let Some(old) = &previous {
                assert!(validate_agent_owner(&pool, &ws, old, true).await.is_err());
            }
            previous = Some(req);
            assert!(!cleanup_allowed(&pool, ws.id).await.unwrap());
        }
        state.bootstrap = None;
        state.status = RepositoryWikiStatus::Current;
        let lock = store.try_lock().unwrap();
        store.save_state(&state).await.unwrap();
        drop(lock);
        assert!(
            validate_agent_owner(&pool, &ws, &previous.unwrap(), true)
                .await
                .is_err()
        );
        assert!(
            Workspace::find_by_id(&pool, ws.id)
                .await
                .unwrap()
                .unwrap()
                .is_execution_only()
        );
    }

    #[tokio::test]
    async fn sync_binds_before_launch_and_retained_no_op_result_prevents_reuse() {
        use db::models::workspace_usage::{
            WorkspaceExecutionResult, WorkspaceExecutionTerminalStatus,
        };
        use utils::repository_memory::RepositoryMemoryState;
        let (pool, repo, _temp, store) = memory_fixture().await;
        let mut ws = workspace(
            &pool,
            &repo,
            Some(&WorkspaceExecutionOwner::new(OPENWIKI_SYNC, repo.id, None)),
        )
        .await;
        let req = request(&ws, Uuid::new_v4(), Uuid::new_v4());
        let mut state = RepositoryMemoryState {
            status: RepositoryWikiStatus::Reconciling,
            maintenance_workspace_id: Some(ws.id),
            maintenance_session_id: Some(req.session_id),
            ..Default::default()
        };
        let lock = store.try_lock().unwrap();
        store.save_state(&state).await.unwrap();
        drop(lock);
        validate_agent_owner(&pool, &ws, &req, false).await.unwrap();
        assert!(validate_agent_owner(&pool, &ws, &req, true).await.is_err());
        Workspace::bind_execution_owner(&pool, ws.id, OPENWIKI_SYNC, req.agent_run_id)
            .await
            .unwrap();
        ws = Workspace::find_by_id(&pool, ws.id).await.unwrap().unwrap();
        state.active_run_id = Some(req.agent_run_id);
        let lock = store.try_lock().unwrap();
        store.save_state(&state).await.unwrap();
        drop(lock);
        validate_agent_owner(&pool, &ws, &req, true).await.unwrap();
        let mut spoof = req.clone();
        spoof.session_id = Uuid::new_v4();
        assert!(
            validate_agent_owner(&pool, &ws, &spoof, true)
                .await
                .is_err()
        );
        Workspace::save_execution_result(
            &pool,
            ws.id,
            OPENWIKI_SYNC,
            Some(req.agent_run_id),
            &WorkspaceExecutionResult {
                status: WorkspaceExecutionTerminalStatus::Succeeded,
                completed_at: chrono::Utc::now(),
                error: None,
                wiki_commit: None,
                no_op: true,
            },
        )
        .await
        .unwrap();
        ws = Workspace::find_by_id(&pool, ws.id).await.unwrap().unwrap();
        assert!(ws.is_execution_only());
        assert!(validate_agent_owner(&pool, &ws, &req, true).await.is_err());
    }

    #[tokio::test]
    async fn classification_backfill_includes_owners_recovered_after_migration() {
        let (pool, repo, project) = fixture().await;
        let internal = workspace(&pool, &repo, None).await;
        let status = Uuid::new_v4();
        let issue = Uuid::new_v4();
        sqlx::query("INSERT INTO local_project_statuses(id,project_id,name,color,sort_order) VALUES(?,?,'Todo','blue',0)").bind(status).bind(project).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO local_issues(id,project_id,issue_number,simple_id,status_id,title,sort_order) VALUES(?,?,1,'TEST-1',?,'ordinary title',0)").bind(issue).bind(project).bind(status).execute(&pool).await.unwrap();
        sqlx::query(
            "INSERT INTO local_workspace_links(workspace_id,project_id,issue_id) VALUES(?,?,?)",
        )
        .bind(internal.id)
        .bind(project)
        .bind(issue)
        .execute(&pool)
        .await
        .unwrap();
        integration(
            &pool,
            &repo,
            project,
            internal.id,
            Uuid::new_v4(),
            None,
            "succeeded",
        )
        .await;
        backfill(&pool).await.unwrap();
        backfill(&pool).await.unwrap();
        let labels: Vec<String> = sqlx::query_scalar("SELECT t.name FROM local_issue_tags it JOIN local_tags t ON t.id=it.tag_id WHERE it.issue_id=?")
            .bind(issue).fetch_all(&pool).await.unwrap();
        assert_eq!(labels, vec!["system"]);
    }

    #[tokio::test]
    async fn backfill_uses_evidence_not_names_and_does_not_reclassify_reserved_sources() {
        let (pool, repo, project) = fixture().await;
        let source = workspace(&pool, &repo, None).await;
        let internal = workspace(&pool, &repo, None).await;
        let run = Uuid::new_v4();
        integration(&pool, &repo, project, internal.id, run, None, "succeeded").await;
        sqlx::query("INSERT INTO integration_reservations VALUES('workspace',?,?)")
            .bind(db::models::integration::resource_key(source.id))
            .bind(run)
            .execute(&pool)
            .await
            .unwrap();
        let future = workspace(
            &pool,
            &repo,
            Some(&WorkspaceExecutionOwner::new("future", repo.id, None)),
        )
        .await;
        integration(
            &pool,
            &repo,
            project,
            future.id,
            Uuid::new_v4(),
            None,
            "failed",
        )
        .await;
        backfill(&pool).await.unwrap();
        backfill(&pool).await.unwrap();
        assert!(
            !Workspace::find_by_id(&pool, source.id)
                .await
                .unwrap()
                .unwrap()
                .is_execution_only()
        );
        let restored = Workspace::find_by_id(&pool, internal.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(restored.execution_owner.unwrap().run_id, Some(run));
        assert_eq!(
            Workspace::find_by_id(&pool, future.id)
                .await
                .unwrap()
                .unwrap()
                .execution_owner
                .unwrap()
                .kind,
            "future"
        );
        assert!(cleanup_allowed(&pool, internal.id).await.unwrap());
        assert!(!cleanup_allowed(&pool, future.id).await.unwrap());
    }

    #[tokio::test]
    async fn conflicting_historical_owners_are_inspection_only() {
        let (pool, repo, project) = fixture().await;
        let ws = workspace(&pool, &repo, None).await;
        integration(&pool, &repo, project, ws.id, Uuid::new_v4(), None, "failed").await;
        integration(
            &pool,
            &repo,
            project,
            ws.id,
            Uuid::new_v4(),
            None,
            "succeeded",
        )
        .await;
        backfill(&pool).await.unwrap();
        let ws = Workspace::find_by_id(&pool, ws.id).await.unwrap().unwrap();
        assert!(ws.is_execution_only());
        assert_eq!(
            ws.execution_owner.as_ref().unwrap().kind,
            "conflicting_evidence"
        );
        assert!(
            validate_agent_owner(
                &pool,
                &ws,
                &request(&ws, Uuid::new_v4(), Uuid::new_v4()),
                true
            )
            .await
            .is_err()
        );
    }

    #[tokio::test]
    async fn execution_only_integration_has_no_agent_dispatch_path() {
        let (pool, repo, project) = fixture().await;
        let id = Uuid::new_v4();
        let ws = workspace(
            &pool,
            &repo,
            Some(&WorkspaceExecutionOwner::new(
                INTEGRATION,
                repo.id,
                Some(id),
            )),
        )
        .await;
        let session = Session::create(
            &pool,
            &CreateSession {
                name: None,
                executor: None,
            },
            Uuid::new_v4(),
            ws.id,
        )
        .await
        .unwrap();
        integration(
            &pool,
            &repo,
            project,
            ws.id,
            id,
            Some(session.id),
            "integrating",
        )
        .await;
        let req = request(&ws, session.id, id);
        assert!(validate_agent_owner(&pool, &ws, &req, true).await.is_err());
        sqlx::query("INSERT INTO integration_reservations VALUES('workspace',?,?)")
            .bind(db::models::integration::resource_key(ws.id))
            .bind(id)
            .execute(&pool)
            .await
            .unwrap();
        assert!(validate_agent_owner(&pool, &ws, &req, true).await.is_err());
        let mut spoof = req.clone();
        spoof.session_id = Uuid::new_v4();
        assert!(
            validate_agent_owner(&pool, &ws, &spoof, true)
                .await
                .is_err()
        );
        spoof = req.clone();
        spoof.correlation_id = Uuid::new_v4();
        assert!(
            validate_agent_owner(&pool, &ws, &spoof, true)
                .await
                .is_err()
        );
        assert!(!cleanup_allowed(&pool, ws.id).await.unwrap());
        sqlx::query("UPDATE integration_runs SET status='succeeded' WHERE id=?")
            .bind(id)
            .execute(&pool)
            .await
            .unwrap();
        assert!(validate_agent_owner(&pool, &ws, &req, true).await.is_err());
    }

    #[tokio::test]
    async fn execution_only_scripts_are_rejected_and_passive_inspection_never_creates_files() {
        let (pool, repo, project) = fixture().await;
        let id = Uuid::new_v4();
        let mut ws = workspace(
            &pool,
            &repo,
            Some(&WorkspaceExecutionOwner::new(
                INTEGRATION,
                repo.id,
                Some(id),
            )),
        )
        .await;
        let session = Session::create(
            &pool,
            &CreateSession {
                name: None,
                executor: None,
            },
            Uuid::new_v4(),
            ws.id,
        )
        .await
        .unwrap();
        integration(
            &pool,
            &repo,
            project,
            ws.id,
            id,
            Some(session.id),
            "validating",
        )
        .await;
        sqlx::query("INSERT INTO integration_reservations VALUES('workspace',?,?)")
            .bind(db::models::integration::resource_key(ws.id))
            .bind(id)
            .execute(&pool)
            .await
            .unwrap();
        let action = ExecutorAction::new(
            ExecutorActionType::ScriptRequest(ScriptRequest {
                script: "test command".into(),
                language: ScriptRequestLanguage::Bash,
                context: ScriptContext::IntegrationValidation,
                working_dir: Some(format!("{}/.", repo.name)),
            }),
            None,
        );
        let reason =
            db::models::execution_process::ExecutionProcessRunReason::IntegrationValidation;
        assert!(
            validate_script_owner(&pool, ws.id, session.id, &action, &reason)
                .await
                .is_err()
        );
        let temp = tempfile::tempdir().unwrap();
        let missing = temp.path().join("missing");
        ws.container_ref = Some(missing.to_string_lossy().into_owned());
        assert!(inspection_root(&ws).is_err());
        assert!(!missing.exists());
        ws.container_ref = Some(temp.path().to_string_lossy().into_owned());
        assert_eq!(inspection_root(&ws).unwrap(), temp.path());
        ws.worktree_deleted = true;
        assert!(inspection_root(&ws).is_err());
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM execution_processes")
                .fetch_one(&pool)
                .await
                .unwrap(),
            0
        );
    }
    #[tokio::test]
    async fn completed_wiki_becomes_interactive_only_after_owner_cleanup() {
        use db::models::workspace_usage::{
            WorkspaceExecutionResult, WorkspaceExecutionTerminalStatus,
        };
        use utils::repository_memory::RepositoryMemoryState;
        let (pool, repo, _temp, store) = memory_fixture().await;
        let run_id = Uuid::new_v4();
        let ws = workspace(
            &pool,
            &repo,
            Some(&WorkspaceExecutionOwner::new(
                OPENWIKI_SYNC,
                repo.id,
                Some(run_id),
            )),
        )
        .await;
        Workspace::save_execution_result(
            &pool,
            ws.id,
            OPENWIKI_SYNC,
            Some(run_id),
            &WorkspaceExecutionResult {
                status: WorkspaceExecutionTerminalStatus::Failed,
                completed_at: chrono::Utc::now(),
                error: Some("failed handshake".into()),
                wiki_commit: None,
                no_op: false,
            },
        )
        .await
        .unwrap();
        let root = _temp.path().join("reused-source");
        std::fs::create_dir_all(&root).unwrap();
        let fixture_git = git::GitCli::new();
        fixture_git.git(&root, ["init"]).unwrap();
        fixture_git
            .git(&root, ["config", "user.name", "Fixture"])
            .unwrap();
        fixture_git
            .git(&root, ["config", "user.email", "fixture@example.invalid"])
            .unwrap();
        git::GitService::new()
            .initialize_repo_with_main_branch(&root)
            .unwrap();
        let mut state = RepositoryMemoryState {
            enabled: true,
            maintenance_workspace_id: Some(ws.id),
            active_run_id: Some(run_id),
            status: RepositoryWikiStatus::Error,
            ..Default::default()
        };
        store.save_state(&state).await.unwrap();
        assert!(
            crate::services::repository_memory::begin_coding_run(
                &pool,
                &repo,
                &ws,
                Uuid::new_v4(),
                &root,
                "main",
                true
            )
            .await
            .unwrap()
            .is_none()
        );
        release_completed(&pool).await.unwrap();
        assert!(
            Workspace::find_by_id(&pool, ws.id)
                .await
                .unwrap()
                .unwrap()
                .is_execution_only()
        );
        state.active_run_id = None;
        store.save_state(&state).await.unwrap();
        release_completed(&pool).await.unwrap();
        backfill(&pool).await.unwrap();
        let released = Workspace::find_by_id(&pool, ws.id).await.unwrap().unwrap();
        assert!(released.require_interactive().is_ok());
        let context = crate::services::repository_memory::begin_coding_run(
            &pool,
            &repo,
            &released,
            Uuid::new_v4(),
            &root,
            "main",
            true,
        )
        .await
        .unwrap()
        .unwrap();
        assert!(context.finalize_source);
        assert_eq!(
            released
                .execution_owner
                .unwrap()
                .0
                .result
                .unwrap()
                .error
                .as_deref(),
            Some("failed handshake")
        );
    }

    #[tokio::test]
    async fn blocked_integration_stays_reserved_but_completed_one_is_released() {
        let (pool, repo, project) = fixture().await;
        for (status, reserved) in [("blocked", true), ("succeeded", false), ("failed", false)] {
            let id = Uuid::new_v4();
            let ws = workspace(
                &pool,
                &repo,
                Some(&WorkspaceExecutionOwner::new(
                    INTEGRATION,
                    repo.id,
                    Some(id),
                )),
            )
            .await;
            integration(&pool, &repo, project, ws.id, id, None, status).await;
            // Historical metadata has no result field and a different JSON key order.
            sqlx::query("UPDATE workspaces SET execution_owner=json_object('repository_id',json_extract(execution_owner,'$.repository_id'),'run_id',json_extract(execution_owner,'$.run_id'),'kind','integration') WHERE id=?")
                .bind(ws.id).execute(&pool).await.unwrap();
            release_completed(&pool).await.unwrap();
            assert_eq!(
                Workspace::find_by_id(&pool, ws.id)
                    .await
                    .unwrap()
                    .unwrap()
                    .is_execution_only(),
                reserved
            );
        }
        let unknown = workspace(
            &pool,
            &repo,
            Some(&WorkspaceExecutionOwner::new("future", repo.id, None)),
        )
        .await;
        release_completed(&pool).await.unwrap();
        assert!(
            Workspace::find_by_id(&pool, unknown.id)
                .await
                .unwrap()
                .unwrap()
                .is_execution_only()
        );
    }

    #[tokio::test]
    async fn wiki_board_link_is_idempotent_and_never_guesses_between_boards() {
        let (pool, repo, project) = fixture().await;
        sqlx::query("INSERT INTO project_repos(id,project_id,repo_id) VALUES(?,?,?)")
            .bind(Uuid::new_v4())
            .bind(project)
            .bind(repo.id)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO local_project_statuses(id,project_id,name,color,sort_order) VALUES(?,?,'Todo','blue',0)")
            .bind(Uuid::new_v4()).bind(project).execute(&pool).await.unwrap();
        let ws = workspace(
            &pool,
            &repo,
            Some(&WorkspaceExecutionOwner::new(OPENWIKI_SYNC, repo.id, None)),
        )
        .await;
        ensure_wiki_board_link(&pool, &ws).await.unwrap();
        ensure_wiki_board_link(&pool, &ws).await.unwrap();
        assert_eq!(
            sqlx::query_scalar::<_, String>("SELECT simple_id FROM local_issues")
                .fetch_one(&pool)
                .await
                .unwrap(),
            "LOCAL-1"
        );
        sqlx::query("UPDATE projects SET key='WIKI' WHERE id=?")
            .bind(project)
            .execute(&pool)
            .await
            .unwrap();
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM local_issues")
                .fetch_one(&pool)
                .await
                .unwrap(),
            1
        );
        let labels: Vec<String> = sqlx::query_scalar("SELECT t.name FROM local_issue_tags it JOIN local_tags t ON t.id=it.tag_id JOIN local_workspace_links l ON l.issue_id=it.issue_id WHERE l.workspace_id=?")
            .bind(ws.id).fetch_all(&pool).await.unwrap();
        assert_eq!(labels, vec!["system"]);
        let priority: String = sqlx::query_scalar("SELECT priority FROM local_issues LIMIT 1")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(priority, "medium");
        sqlx::query("DELETE FROM project_repos WHERE repo_id=?")
            .bind(repo.id)
            .execute(&pool)
            .await
            .unwrap();
        let linked_only = workspace(
            &pool,
            &repo,
            Some(&WorkspaceExecutionOwner::new(OPENWIKI_SYNC, repo.id, None)),
        )
        .await;
        ensure_wiki_board_link(&pool, &linked_only).await.unwrap();
        assert_eq!(
            sqlx::query_scalar::<_, String>(
                "SELECT simple_id FROM local_issues WHERE issue_number=2"
            )
            .fetch_one(&pool)
            .await
            .unwrap(),
            "WIKI-2"
        );
        let classification: (String, String) = sqlx::query_as("SELECT t.name,i.priority FROM local_issues i JOIN local_issue_tags it ON it.issue_id=i.id JOIN local_tags t ON t.id=it.tag_id WHERE i.project_id=? AND i.issue_number=2").bind(project).fetch_one(&pool).await.unwrap();
        assert_eq!(classification, ("system".into(), "medium".into()));

        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM local_issues")
                .fetch_one(&pool)
                .await
                .unwrap(),
            2
        );
        let second = Uuid::new_v4();
        sqlx::query("INSERT INTO projects(id,name,key) VALUES(?,'Second','SECOND')")
            .bind(second)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO project_repos(id,project_id,repo_id) VALUES(?,?,?)")
            .bind(Uuid::new_v4())
            .bind(second)
            .bind(repo.id)
            .execute(&pool)
            .await
            .unwrap();
        let ambiguous = workspace(
            &pool,
            &repo,
            Some(&WorkspaceExecutionOwner::new(
                OPENWIKI_BOOTSTRAP,
                repo.id,
                None,
            )),
        )
        .await;
        ensure_wiki_board_link(&pool, &ambiguous).await.unwrap();
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM local_workspace_links")
                .fetch_one(&pool)
                .await
                .unwrap(),
            2
        );
        ensure_wiki_board_link(&pool, &ws).await.unwrap();
        assert_eq!(
            sqlx::query_scalar::<_, Uuid>(
                "SELECT project_id FROM local_workspace_links WHERE workspace_id=?"
            )
            .bind(ws.id)
            .fetch_one(&pool)
            .await
            .unwrap(),
            project
        );
    }

    #[tokio::test]
    async fn wiki_board_link_does_not_reuse_deleted_system_issue_number() {
        let (pool, repo, project) = fixture().await;
        sqlx::query("INSERT INTO project_repos(id,project_id,repo_id) VALUES(?,?,?)")
            .bind(Uuid::new_v4())
            .bind(project)
            .bind(repo.id)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO local_project_statuses(id,project_id,name,color,sort_order) VALUES(?,?,'Ready','blue',0)")
            .bind(Uuid::new_v4()).bind(project).execute(&pool).await.unwrap();
        for expected in [1, 2] {
            let ws = workspace(
                &pool,
                &repo,
                Some(&WorkspaceExecutionOwner::new(OPENWIKI_SYNC, repo.id, None)),
            )
            .await;
            ensure_wiki_board_link(&pool, &ws).await.unwrap();
            let number: i32 = sqlx::query_scalar("SELECT issue_number FROM local_issues")
                .fetch_one(&pool)
                .await
                .unwrap();
            assert_eq!(number, expected);
            sqlx::query("DELETE FROM local_workspace_links WHERE workspace_id=?")
                .bind(ws.id)
                .execute(&pool)
                .await
                .unwrap();
            sqlx::query("DELETE FROM local_issues WHERE project_id=?")
                .bind(project)
                .execute(&pool)
                .await
                .unwrap();
        }
    }

    #[tokio::test]
    async fn wiki_board_link_rolls_back_keyed_card_when_system_attachment_fails() {
        let (pool, repo, project) = fixture().await;
        sqlx::query("UPDATE projects SET key='WIKI' WHERE id=?")
            .bind(project)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO project_repos(id,project_id,repo_id) VALUES(?,?,?)")
            .bind(Uuid::new_v4())
            .bind(project)
            .bind(repo.id)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO local_project_statuses(id,project_id,name,color,sort_order) VALUES(?,?,'Ready','blue',0)").bind(Uuid::new_v4()).bind(project).execute(&pool).await.unwrap();
        let ws = workspace(
            &pool,
            &repo,
            Some(&WorkspaceExecutionOwner::new(OPENWIKI_SYNC, repo.id, None)),
        )
        .await;
        sqlx::query("CREATE TRIGGER reject_system_relation BEFORE INSERT ON local_issue_tags BEGIN SELECT RAISE(ABORT,'system attachment failure'); END").execute(&pool).await.unwrap();
        assert!(ensure_wiki_board_link(&pool, &ws).await.is_err());
        for table in [
            "local_issues",
            "local_workspace_links",
            "local_issue_tags",
            "local_tags",
        ] {
            assert_eq!(
                sqlx::query_scalar::<_, i64>(&format!("SELECT count(*) FROM {table}"))
                    .fetch_one(&pool)
                    .await
                    .unwrap(),
                0,
                "rollback {table}"
            );
        }
        sqlx::query("DROP TRIGGER reject_system_relation")
            .execute(&pool)
            .await
            .unwrap();
        ensure_wiki_board_link(&pool, &ws).await.unwrap();
        let row: (String, String, String) = sqlx::query_as("SELECT i.simple_id,i.priority,t.name FROM local_issues i JOIN local_issue_tags it ON it.issue_id=i.id JOIN local_tags t ON t.id=it.tag_id").fetch_one(&pool).await.unwrap();
        assert_eq!(row, ("WIKI-1".into(), "medium".into(), "system".into()));
    }

    #[tokio::test]
    async fn wiki_instructions_require_the_current_live_owner() {
        use utils::repository_memory::RepositoryMemoryState;
        let (pool, repo, _temp, store) = memory_fixture().await;
        let ws = workspace(
            &pool,
            &repo,
            Some(&WorkspaceExecutionOwner::new(OPENWIKI_SYNC, repo.id, None)),
        )
        .await;
        let session = Session::create(
            &pool,
            &CreateSession {
                executor: Some("CODEX".into()),
                name: None,
            },
            Uuid::new_v4(),
            ws.id,
        )
        .await
        .unwrap();
        let req = request(&ws, session.id, Uuid::new_v4());
        Workspace::bind_execution_owner(&pool, ws.id, OPENWIKI_SYNC, req.agent_run_id)
            .await
            .unwrap();
        let ws = Workspace::find_by_id(&pool, ws.id).await.unwrap().unwrap();
        let state = RepositoryMemoryState {
            status: RepositoryWikiStatus::Reconciling,
            maintenance_workspace_id: Some(ws.id),
            maintenance_session_id: Some(session.id),
            active_run_id: Some(req.agent_run_id),
            ..Default::default()
        };
        store.save_state(&state).await.unwrap();
        sqlx::query("INSERT INTO agent_runs(id,session_id,workspace_id,request_id,idempotency_key,correlation_id,schema_version,payload_version,runtime_profile_id,provider_id,workspace_mode,workspace_path,status,request_envelope) VALUES(?,?,?,?,?,?,1,1,'CODEX','codex','isolated_worktree','/tmp','running',?)")
            .bind(req.agent_run_id).bind(session.id).bind(ws.id).bind(req.request_id).bind(&req.idempotency_key).bind(req.correlation_id).bind(Json(&req)).execute(&pool).await.unwrap();
        assert!(allows_wiki_instruction(&pool, &ws, &req).await.unwrap());
        let mut stale = req.clone();
        stale.agent_run_id = Uuid::new_v4();
        assert!(!allows_wiki_instruction(&pool, &ws, &stale).await.unwrap());
        release_completed(&pool).await.unwrap();
        assert!(
            Workspace::find_by_id(&pool, ws.id)
                .await
                .unwrap()
                .unwrap()
                .is_execution_only()
        );
        for status in ["cancelling", "failed", "succeeded"] {
            sqlx::query("UPDATE agent_runs SET status=? WHERE id=?")
                .bind(status)
                .bind(req.agent_run_id)
                .execute(&pool)
                .await
                .unwrap();
            assert!(!allows_wiki_instruction(&pool, &ws, &req).await.unwrap());
        }
    }
}
