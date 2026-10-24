//! Manual archive protocol shared by the UI and stdio MCP. Preparing never
//! changes Workspace state. A token authorizes one attempt for one exact target.
use std::{
    collections::HashMap,
    sync::LazyLock,
    time::{Duration, Instant},
};

use axum::{Extension, Json, extract::State};
use db::models::{workspace::Workspace, workspace_repo::WorkspaceRepo};
use deployment::Deployment;
use serde::{Deserialize, Serialize};
use services::services::container::ContainerService;
use sqlx::{SqliteConnection, SqlitePool};
use tokio::sync::Mutex;
use utils::{repository_memory::RepositoryMemoryStore, response::ApiResponse};
use uuid::Uuid;
use workspace_manager::WorkspaceManager;

use crate::{DeploymentImpl, error::ApiError};

pub const CONFIRMATION: &str = "ワークツリーが削除されます";
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ConfirmationScope {
    Local,
    Remote { principal: String, project_id: Uuid },
}
struct Confirmation {
    scope: ConfirmationScope,
    workspace_id: Uuid,
    container_ref: Option<String>,
    branch: String,
    expires: Instant,
}
static CONFIRMATIONS: LazyLock<Mutex<HashMap<Uuid, Confirmation>>> = LazyLock::new(Mutex::default);

#[derive(Serialize)]
pub struct PreparedArchive {
    pub(crate) confirmation_token: Uuid,
    pub(crate) expires_at: chrono::DateTime<chrono::Utc>,
    message: &'static str,
}
#[derive(Deserialize)]
pub struct ConfirmArchive {
    confirmation_token: Uuid,
}
#[derive(Serialize)]
pub struct ArchiveResult {
    pub(crate) workspace: Workspace,
    pub(crate) deletion_error: Option<String>,
}

// The same predicate runs before presenting confirmation and under the final
// admission barrier. Persisted unfinished host work also counts as busy after
// its AgentRun ends; usage alone never determines whether work is active.
async fn check_idle(conn: &mut SqliteConnection, id: Uuid) -> Result<(), ApiError> {
    check_idle_with_memory(
        conn,
        id,
        RepositoryMemoryStore::read_only_configured_for_repository,
    )
    .await
}

async fn check_idle_with_memory(
    conn: &mut SqliteConnection,
    id: Uuid,
    memory: impl FnOnce(&str, Uuid) -> std::io::Result<Option<RepositoryMemoryStore>>,
) -> Result<(), ApiError> {
    if crate::routes::openwiki::preparation::is_active(id) {
        return Err(ApiError::Conflict(
            "使用中のWorkspaceはarchiveできません。".into(),
        ));
    }
    let busy: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM agent_runs WHERE workspace_id=?1 AND status NOT IN ('succeeded','failed','cancelled','crashed','audit_failed'))
        OR EXISTS(SELECT 1 FROM agent_process_registry p JOIN agent_run_attempts a ON a.id=p.run_attempt_id JOIN agent_runs r ON r.id=a.agent_run_id WHERE r.workspace_id=?1 AND p.registry_status IN ('reserved','spawned','running','unreachable'))
        OR EXISTS(SELECT 1 FROM execution_processes p JOIN sessions s ON s.id=p.session_id WHERE s.workspace_id=?1 AND p.status='running')
        OR EXISTS(SELECT 1 FROM integration_reservations WHERE resource_kind='workspace' AND resource_key=?2)
        OR EXISTS(SELECT 1 FROM resource_operations WHERE workspace_id=?1 AND status IN ('queued','blocked','launching','running','recovery_required'))
        OR EXISTS(SELECT 1 FROM resource_mediations WHERE workspace_id=?1 AND status IN ('pending','preparing','running'))
        OR EXISTS(SELECT 1 FROM workflow_runs w WHERE w.status NOT IN ('succeeded','failed','canceled') AND (
            w.workspace_id=?1
            OR EXISTS(SELECT 1 FROM workflow_attempts a WHERE a.id=w.attempt_id AND a.workspace_id=?1)
            OR EXISTS(SELECT 1 FROM node_executions n JOIN agent_runs r ON r.id=n.agent_run_id WHERE n.run_id=w.id AND r.workspace_id=?1)))
        OR EXISTS(SELECT 1 FROM integration_runs WHERE workspace_id=?1 AND status NOT IN ('succeeded','failed','cancelled'))"
    ).bind(id).bind(db::models::integration::resource_key(id)).fetch_one(&mut *conn).await?;
    if busy {
        return Err(ApiError::Conflict(
            "使用中のWorkspaceはarchiveできません。".into(),
        ));
    }
    // Read the current owner through the same connection, including inside the
    // final SQLite transaction. Do not use cleanup_allowed: its usage/unknown-
    // owner policy applies to automatic release, not manual archive permission.
    let owner: Option<sqlx::types::Json<db::models::workspace_usage::WorkspaceExecutionOwner>> =
        sqlx::query_scalar("SELECT execution_owner FROM workspaces WHERE id=?")
            .bind(id)
            .fetch_one(&mut *conn)
            .await?;
    if let Some(owner) = owner
        && matches!(
            owner.kind.as_str(),
            db::models::workspace_usage::OPENWIKI_SYNC
                | db::models::workspace_usage::OPENWIKI_BOOTSTRAP
        )
        && let Some(repo_id) = owner.repository_id
        && let Some(name) = sqlx::query_scalar::<_, String>("SELECT name FROM repos WHERE id=?")
            .bind(repo_id)
            .fetch_optional(&mut *conn)
            .await?
        && let Some(store) = memory(&name, repo_id)?
    {
        let state = store.state()?;
        if state.maintenance_workspace_id == Some(id)
            && (state.active_run_id.is_some() || state.bootstrap.is_some())
        {
            return Err(ApiError::Conflict(
                "使用中のWorkspaceはarchiveできません。".into(),
            ));
        }
    }
    Ok(())
}

pub async fn prepare(
    Extension(workspace): Extension<Workspace>,
    State(deployment): State<DeploymentImpl>,
) -> Result<Json<ApiResponse<PreparedArchive>>, ApiError> {
    let _lifecycle = services::services::workspace_lifecycle::lock(workspace.id).await;
    if !deployment.container().scripts_settled(workspace.id).await? {
        return Err(ApiError::Conflict(
            "使用中のWorkspaceはarchiveできません。".into(),
        ));
    }
    prepare_for_pool(&deployment.db().pool, &workspace)
        .await
        .map(|v| Json(ApiResponse::success(v)))
}

async fn prepare_for_pool(
    pool: &SqlitePool,
    workspace: &Workspace,
) -> Result<PreparedArchive, ApiError> {
    prepare_scoped(pool, workspace, ConfirmationScope::Local).await
}

pub(crate) async fn prepare_scoped(
    pool: &SqlitePool,
    workspace: &Workspace,
    scope: ConfirmationScope,
) -> Result<PreparedArchive, ApiError> {
    let mut connection = pool.acquire().await?;
    check_project(&mut connection, workspace.id, &scope).await?;
    check_idle(&mut connection, workspace.id).await?;
    drop(connection);
    if workspace.archived {
        return Err(ApiError::Conflict("Workspace is already archived".into()));
    }
    super::archive_processes::check(workspace.container_ref.as_deref()).await?;
    let token = Uuid::new_v4();
    let mut confirmations = CONFIRMATIONS.lock().await;
    confirmations.retain(|_, value| value.expires > Instant::now());
    confirmations.insert(
        token,
        Confirmation {
            scope,
            workspace_id: workspace.id,
            container_ref: workspace.container_ref.clone(),
            branch: workspace.branch.clone(),
            expires: Instant::now() + Duration::from_secs(600),
        },
    );
    Ok(PreparedArchive {
        confirmation_token: token,
        expires_at: chrono::Utc::now() + chrono::Duration::seconds(600),
        message: CONFIRMATION,
    })
}

pub async fn confirm(
    Extension(workspace): Extension<Workspace>,
    State(deployment): State<DeploymentImpl>,
    Json(request): Json<ConfirmArchive>,
) -> Result<Json<ApiResponse<ArchiveResult>>, ApiError> {
    // Finish an admitted deletion even if the browser disconnects.
    let pool = deployment.db().pool.clone();
    let result = tokio::spawn(async move {
        let _admission = services::services::integration_admission::MUTATIONS
            .lock()
            .await;
        archive_for_pool(&pool, workspace.id, request.confirmation_token, || async {
            Ok(deployment.container().scripts_settled(workspace.id).await?)
        })
        .await
    })
    .await
    .map_err(|error| ApiError::Io(std::io::Error::other(error.to_string())))??;
    Ok(Json(ApiResponse::success(result)))
}

async fn archive_for_pool<F, Fut>(
    pool: &SqlitePool,
    id: Uuid,
    token: Uuid,
    settled: F,
) -> Result<ArchiveResult, ApiError>
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = Result<bool, ApiError>>,
{
    archive_scoped(pool, id, token, ConfirmationScope::Local, settled).await
}

pub(crate) async fn archive_scoped<F, Fut>(
    pool: &SqlitePool,
    id: Uuid,
    token: Uuid,
    scope: ConfirmationScope,
    settled: F,
) -> Result<ArchiveResult, ApiError>
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = Result<bool, ApiError>>,
{
    let _lifecycle = services::services::workspace_lifecycle::lock(id).await;
    let confirmation = {
        let mut confirmations = CONFIRMATIONS.lock().await;
        if !confirmations.get(&token).is_some_and(|value| {
            value.workspace_id == id && value.scope == scope && value.expires > Instant::now()
        }) {
            return Err(ApiError::Conflict(
                "Archive confirmation expired, used or belongs to another caller/target".into(),
            ));
        }
        confirmations
            .remove(&token)
            .expect("validated confirmation")
    };
    let workspace = Workspace::find_by_id(pool, id)
        .await?
        .ok_or_else(|| ApiError::BadRequest("Workspace missing".into()))?;
    if workspace.archived
        || workspace.container_ref != confirmation.container_ref
        || workspace.branch != confirmation.branch
    {
        return Err(ApiError::Conflict(
            "Workspace changed; confirm again".into(),
        ));
    }
    if !settled().await? {
        return Err(ApiError::Conflict(
            "使用中のWorkspaceはarchiveできません。".into(),
        ));
    }
    let repos = WorkspaceRepo::find_repos_for_workspace(pool, id).await?;
    let mut tx = pool.begin_with("BEGIN IMMEDIATE").await?;
    check_project(&mut tx, id, &scope).await?;
    check_idle(&mut tx, id).await?;
    super::archive_processes::check(workspace.container_ref.as_deref()).await?;
    sqlx::query("UPDATE workspaces SET archived=1, updated_at=datetime('now','subsec') WHERE id=?")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    let deletion = if workspace.can_delete_container_path() {
        if let Some(path) = workspace.container_ref.as_deref() {
            WorkspaceManager::cleanup_workspace_strict(std::path::Path::new(path), &repos).await
        } else {
            Ok(())
        }
    } else {
        Ok(())
    };
    let deletion_error = deletion.err().map(|error| {
        format!("ワークツリーの物理削除に失敗しました（{error}）。手動で削除してください。")
    });
    if workspace.can_delete_container_path() {
        sqlx::query("UPDATE workspaces SET worktree_deleted=? WHERE id=?")
            .bind(deletion_error.is_none())
            .bind(id)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    Ok(ArchiveResult {
        workspace: Workspace::find_by_id(pool, id)
            .await?
            .ok_or_else(|| ApiError::BadRequest("Workspace missing".into()))?,
        deletion_error,
    })
}

async fn check_project(
    conn: &mut SqliteConnection,
    id: Uuid,
    scope: &ConfirmationScope,
) -> Result<(), ApiError> {
    if let ConfirmationScope::Remote { project_id, .. } = scope {
        let belongs: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM local_workspace_links l JOIN local_issues i ON i.id=l.issue_id JOIN projects p ON p.id=l.project_id WHERE l.workspace_id=? AND l.project_id=? AND i.project_id=?)")
            .bind(id).bind(project_id).bind(project_id).fetch_one(conn).await?;
        if !belongs {
            return Err(ApiError::Conflict(
                "Workspace does not exist in this Project".into(),
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
pub(crate) async fn expire_for_test(token: Uuid) {
    CONFIRMATIONS.lock().await.get_mut(&token).unwrap().expires =
        Instant::now() - Duration::from_secs(1);
}

#[cfg(test)]
pub(crate) mod tests {
    use std::path::Path;

    use db::models::{
        repo::Repo,
        session::{CreateSession, Session},
        workspace::CreateWorkspace,
        workspace_repo::CreateWorkspaceRepo,
    };

    use super::*;

    fn git(path: &Path, args: &[&str]) {
        let output = std::process::Command::new("git")
            .arg("-C")
            .arg(path)
            .args(args)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    pub(crate) async fn fixture() -> (tempfile::TempDir, SqlitePool, Workspace, Repo, Session) {
        let root = tempfile::tempdir().unwrap();
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!("../db/migrations").run(&pool).await.unwrap();
        let source = root.path().join("repo");
        std::fs::create_dir(&source).unwrap();
        git(&source, &["init", "-b", "main"]);
        git(
            &source,
            &[
                "-c",
                "user.name=Fixture",
                "-c",
                "user.email=fixture@example.invalid",
                "commit",
                "--allow-empty",
                "-m",
                "initial",
            ],
        );
        let repo = Repo::find_or_create(&pool, &source, "Archive fixture")
            .await
            .unwrap();
        let workspace = Workspace::create(
            &pool,
            &CreateWorkspace {
                branch: "archive-fixture".into(),
                name: Some("fixture".into()),
            },
            Uuid::new_v4(),
        )
        .await
        .unwrap();
        WorkspaceRepo::create_many(
            &pool,
            workspace.id,
            &[CreateWorkspaceRepo {
                repo_id: repo.id,
                target_branch: "main".into(),
            }],
        )
        .await
        .unwrap();
        let worktree = root.path().join("workspace");
        std::fs::create_dir(&worktree).unwrap();
        git(
            &source,
            &[
                "worktree",
                "add",
                "-b",
                "archive-fixture",
                worktree.join(&repo.name).to_str().unwrap(),
            ],
        );
        Workspace::update_container_ref(&pool, workspace.id, worktree.to_str().unwrap())
            .await
            .unwrap();
        let session = Session::create(
            &pool,
            &CreateSession {
                executor: None,
                name: Some("retained conversation".into()),
            },
            Uuid::new_v4(),
            workspace.id,
        )
        .await
        .unwrap();
        let workspace = Workspace::find_by_id(&pool, workspace.id)
            .await
            .unwrap()
            .unwrap();
        (root, pool, workspace, repo, session)
    }

    async fn busy(pool: &SqlitePool, session: &Session, reason: &str) {
        sqlx::query("INSERT INTO execution_processes(id,session_id,run_reason,status) VALUES(?,?,?,'running')")
            .bind(Uuid::new_v4()).bind(session.id).bind(reason).execute(pool).await.unwrap();
    }

    #[tokio::test]
    #[cfg(target_os = "linux")]
    async fn manual_archive_unregistered_process_blocks_preflight_and_confirmation_race() {
        struct FixtureChild(std::process::Child);
        impl Drop for FixtureChild {
            fn drop(&mut self) {
                let _ = self.0.kill();
                let _ = self.0.wait();
            }
        }
        let (_root, pool, ws, _repo, _session) = fixture().await;
        let prepared = prepare_for_pool(&pool, &ws).await.unwrap();
        let child = FixtureChild(
            std::process::Command::new("/bin/sleep")
                .arg("60")
                .current_dir(ws.container_ref.as_ref().unwrap())
                .spawn()
                .unwrap(),
        );
        assert!(prepare_for_pool(&pool, &ws).await.is_err());
        assert!(
            archive_for_pool(&pool, ws.id, prepared.confirmation_token, || async {
                Ok(true)
            })
            .await
            .is_err()
        );
        assert!(
            !Workspace::find_by_id(&pool, ws.id)
                .await
                .unwrap()
                .unwrap()
                .archived
        );
        assert!(Path::new(ws.container_ref.as_ref().unwrap()).exists());
        drop(child);
        let prepared = prepare_for_pool(&pool, &ws).await.unwrap();
        let result = archive_for_pool(&pool, ws.id, prepared.confirmation_token, || async {
            Ok(true)
        })
        .await
        .unwrap();
        assert!(result.workspace.archived && result.workspace.worktree_deleted);
    }

    #[tokio::test]
    async fn manual_archive_rejects_host_preparation_wait_and_prevents_late_start() {
        let (_root, pool, ws, _repo, _session) = fixture().await;
        let prepared = prepare_for_pool(&pool, &ws).await.unwrap();
        // Simulate an installer lock wait: the actual host preparation guard is
        // held, but no Agent/process/Workflow or durable run exists yet.
        let preparation = crate::routes::openwiki::preparation::begin(&pool, ws.id)
            .await
            .unwrap();
        let refusal = tokio::time::timeout(Duration::from_secs(1), prepare_for_pool(&pool, &ws))
            .await
            .unwrap();
        assert!(matches!(refusal, Err(ApiError::Conflict(_))));
        assert!(matches!(
            archive_for_pool(&pool, ws.id, prepared.confirmation_token, || async {
                Ok(true)
            })
            .await,
            Err(ApiError::Conflict(_))
        ));
        assert!(
            !Workspace::find_by_id(&pool, ws.id)
                .await
                .unwrap()
                .unwrap()
                .archived
        );
        assert!(Path::new(ws.container_ref.as_ref().unwrap()).exists());
        drop(preparation);
        assert!(!crate::routes::openwiki::preparation::is_active(ws.id));
        // Cancellation drops the future's guard, just as normal completion or
        // a preparation error does; no permanent usage-based refusal remains.
        let started = std::sync::Arc::new(tokio::sync::Notify::new());
        let task = tokio::spawn({
            let pool = pool.clone();
            let started = started.clone();
            async move {
                let _preparation = crate::routes::openwiki::preparation::begin(&pool, ws.id)
                    .await
                    .unwrap();
                started.notify_one();
                std::future::pending::<()>().await;
            }
        });
        started.notified().await;
        assert!(prepare_for_pool(&pool, &ws).await.is_err());
        task.abort();
        assert!(task.await.unwrap_err().is_cancelled());
        let prepared = prepare_for_pool(&pool, &ws).await.unwrap();
        let result = archive_for_pool(&pool, ws.id, prepared.confirmation_token, || async {
            Ok(true)
        })
        .await
        .unwrap();
        assert!(result.workspace.archived && result.workspace.worktree_deleted);
        // Archive wins admission first: host preparation cannot resume it.
        assert!(
            crate::routes::openwiki::preparation::begin(&pool, ws.id)
                .await
                .is_err()
        );
        assert!(!Path::new(ws.container_ref.as_ref().unwrap()).exists());
    }

    #[tokio::test]
    async fn manual_archive_rechecks_unfinished_host_maintenance_without_active_agent() {
        use utils::repository_memory::{
            OpenWikiBootstrapOwner, OpenWikiBootstrapPhase, RepositoryMemoryState,
        };

        for kind in ["openwiki_sync", "openwiki_bootstrap"] {
            let (root, pool, ws, repo, _session) = fixture().await;
            let persistent = root.path().join("isolated-memory");
            std::fs::create_dir(&persistent).unwrap();
            let store = RepositoryMemoryStore::at_persistent(&persistent).unwrap();
            let owner = db::models::workspace_usage::WorkspaceExecutionOwner::new(
                kind,
                repo.id,
                Some(Uuid::new_v4()),
            );
            sqlx::query(
                "UPDATE workspaces SET usage='execution_only',execution_owner=? WHERE id=?",
            )
            .bind(sqlx::types::Json(owner))
            .bind(ws.id)
            .execute(&pool)
            .await
            .unwrap();
            let mut state = RepositoryMemoryState::default();
            let save = |state: &RepositoryMemoryState| {
                let _lock = store.try_lock().unwrap();
                store.save_state(state).unwrap();
            };
            // Inject only the read-only store location. The real DB+memory idle
            // predicate is shared by prepare and the final deletion transaction.
            let resolve = |name: &str, id: Uuid| {
                assert_eq!(name, repo.name);
                assert_eq!(id, repo.id);
                RepositoryMemoryStore::read_only_at_persistent(&persistent)
            };
            save(&state);
            check_idle_with_memory(&mut pool.acquire().await.unwrap(), ws.id, resolve)
                .await
                .unwrap();
            state.maintenance_workspace_id = Some(ws.id);
            if kind == "openwiki_sync" {
                state.active_run_id = Some(Uuid::new_v4());
            } else {
                state.bootstrap = Some(OpenWikiBootstrapOwner {
                    workflow_run_id: Uuid::new_v4(),
                    server_instance_id: Uuid::new_v4(),
                    phase: OpenWikiBootstrapPhase::Publishing,
                    child: None,
                    review_fingerprint: None,
                });
            }
            save(&state);
            // No live Agent/process exists. Host finalization alone must reject
            // both a fresh prepare and a recheck after an earlier idle result.
            assert!(matches!(
                check_idle_with_memory(&mut pool.acquire().await.unwrap(), ws.id, resolve).await,
                Err(ApiError::Conflict(_))
            ));
            let mut tx = pool.begin_with("BEGIN IMMEDIATE").await.unwrap();
            assert!(matches!(
                check_idle_with_memory(&mut tx, ws.id, resolve).await,
                Err(ApiError::Conflict(_))
            ));
            tx.rollback().await.unwrap();
            assert!(
                !Workspace::find_by_id(&pool, ws.id)
                    .await
                    .unwrap()
                    .unwrap()
                    .archived
            );
            assert!(Path::new(ws.container_ref.as_ref().unwrap()).exists());
            // An unrelated maintenance workspace does not block this one.
            state.maintenance_workspace_id = Some(Uuid::new_v4());
            save(&state);
            check_idle_with_memory(&mut pool.acquire().await.unwrap(), ws.id, resolve)
                .await
                .unwrap();
            // Historical attribution and execution-only usage are not busy.
            state.maintenance_workspace_id = Some(ws.id);
            state.active_run_id = None;
            state.bootstrap = None;
            save(&state);
            check_idle_with_memory(&mut pool.acquire().await.unwrap(), ws.id, resolve)
                .await
                .unwrap();
            // A damaged known ownership record cannot authorize deletion.
            std::fs::write(persistent.join("knowledge/state.json"), "invalid").unwrap();
            assert!(matches!(
                check_idle_with_memory(&mut pool.acquire().await.unwrap(), ws.id, resolve).await,
                Err(ApiError::Io(_))
            ));
        }
    }

    #[tokio::test]
    async fn manual_archive_completed_execution_only_before_and_after_release() {
        for usage in ["execution_only", "interactive"] {
            let (_root, pool, ws, _repo, session) = fixture().await;
            // A completed preparation result, with no active Integration run.
            // Use the real release function for the after-release variant.
            let owner = serde_json::json!({
                "kind": "integration", "run_id": null, "repository_id": null,
                "result": { "status": "succeeded", "completed_at": chrono::Utc::now(),
                    "error": null, "wiki_commit": null, "no_op": false }
            });
            sqlx::query(
                "UPDATE workspaces SET usage='execution_only',execution_owner=? WHERE id=?",
            )
            .bind(owner.to_string())
            .bind(ws.id)
            .execute(&pool)
            .await
            .unwrap();
            let ws = Workspace::find_by_id(&pool, ws.id).await.unwrap().unwrap();
            busy(&pool, &session, "devserver").await;
            assert!(prepare_for_pool(&pool, &ws).await.is_err());
            sqlx::query("UPDATE execution_processes SET status='completed'")
                .execute(&pool)
                .await
                .unwrap();
            if usage == "interactive" {
                services::services::workspace_usage::release_completed(&pool)
                    .await
                    .unwrap();
            }
            let ws = Workspace::find_by_id(&pool, ws.id).await.unwrap().unwrap();
            assert_eq!(ws.is_execution_only(), usage == "execution_only");
            let prepared = prepare_for_pool(&pool, &ws).await.unwrap();
            let result = archive_for_pool(&pool, ws.id, prepared.confirmation_token, || async {
                Ok(true)
            })
            .await
            .unwrap();
            assert!(result.workspace.archived && result.workspace.worktree_deleted);
            assert_eq!(
                result.workspace.usage, ws.usage,
                "archive must not grant reuse permissions"
            );
        }
    }

    #[tokio::test]
    async fn manual_archive_success_cancel_single_confirmation_and_retained_history_branch() {
        let (_root, pool, ws, repo, session) = fixture().await;
        let tree = Path::new(ws.container_ref.as_ref().unwrap()).join(&repo.name);
        std::fs::write(tree.join("retained.txt"), "committed work\n").unwrap();
        git(&tree, &["add", "."]);
        git(
            &tree,
            &[
                "-c",
                "user.name=Fixture",
                "-c",
                "user.email=fixture@example.invalid",
                "commit",
                "-m",
                "retained work",
            ],
        );
        let prepared = prepare_for_pool(&pool, &ws).await.unwrap();
        assert_eq!(prepared.message, "ワークツリーが削除されます");
        // Preparing/cancelling has no data or filesystem effect.
        assert!(
            !Workspace::find_by_id(&pool, ws.id)
                .await
                .unwrap()
                .unwrap()
                .archived
        );
        assert!(Path::new(ws.container_ref.as_ref().unwrap()).exists());
        let result = archive_for_pool(&pool, ws.id, prepared.confirmation_token, || async {
            Ok(true)
        })
        .await
        .unwrap();
        assert!(result.workspace.archived && result.workspace.worktree_deleted);
        assert!(result.deletion_error.is_none());
        assert!(!Path::new(ws.container_ref.as_ref().unwrap()).exists());
        assert!(
            Session::find_by_id(&pool, session.id)
                .await
                .unwrap()
                .is_some()
        );
        git(
            &repo.path,
            &["show-ref", "--verify", "refs/heads/archive-fixture"],
        );
        let git_service = git::GitService::new();
        let base = git_service
            .get_base_commit(&repo.path, &ws.branch, "main")
            .unwrap();
        let diffs = git_service
            .get_branch_diffs(&repo.path, &base, &ws.branch)
            .unwrap();
        assert_eq!(diffs.len(), 1);
        assert_eq!(diffs[0].new_content.as_deref(), Some("committed work\n"));

        assert!(
            archive_for_pool(&pool, ws.id, prepared.confirmation_token, || async {
                Ok(true)
            })
            .await
            .is_err()
        );
        assert!(services::services::workspace_usage::inspection_root(&result.workspace).is_err());
        assert!(!Path::new(ws.container_ref.as_ref().unwrap()).exists());
        WorkspaceManager::ensure_workspace_exists(
            Path::new(ws.container_ref.as_ref().unwrap()),
            &[workspace_manager::RepoWorkspaceInput::new(
                repo,
                "main".into(),
            )],
            &ws.branch,
        )
        .await
        .unwrap();
        assert!(Path::new(ws.container_ref.as_ref().unwrap()).is_dir());
    }

    #[tokio::test]
    async fn manual_archive_rejects_busy_before_confirmation_and_after_confirmation() {
        for reason in ["devserver", "setupscript"] {
            let (_root, pool, ws, _repo, session) = fixture().await;
            let prepared = prepare_for_pool(&pool, &ws).await.unwrap();
            busy(&pool, &session, reason).await;
            assert!(matches!(
                prepare_for_pool(&pool, &ws).await,
                Err(ApiError::Conflict(_))
            ));
            assert!(matches!(
                archive_for_pool(&pool, ws.id, prepared.confirmation_token, || async {
                    Ok(true)
                })
                .await,
                Err(ApiError::Conflict(_))
            ));
            assert!(
                !Workspace::find_by_id(&pool, ws.id)
                    .await
                    .unwrap()
                    .unwrap()
                    .archived
            );
            assert!(Path::new(ws.container_ref.as_ref().unwrap()).exists());
        }
    }

    #[tokio::test]
    async fn manual_archive_failure_commits_archived_but_not_deleted() {
        let (root, pool, ws, _repo, _session) = fixture().await;
        // A file as container root yields a real OS NotADirectory failure, even
        // when tests run as root. Only this test-owned path is ever targeted.
        let invalid = root.path().join("not-a-directory");
        std::fs::write(&invalid, "retain").unwrap();
        Workspace::update_container_ref(&pool, ws.id, invalid.to_str().unwrap())
            .await
            .unwrap();
        // A stale successful-deletion flag must not hide this new failure.
        sqlx::query("UPDATE workspaces SET worktree_deleted=1 WHERE id=?")
            .bind(ws.id)
            .execute(&pool)
            .await
            .unwrap();
        let ws = Workspace::find_by_id(&pool, ws.id).await.unwrap().unwrap();
        let prepared = prepare_for_pool(&pool, &ws).await.unwrap();
        let result = archive_for_pool(&pool, ws.id, prepared.confirmation_token, || async {
            Ok(true)
        })
        .await
        .unwrap();
        assert!(result.workspace.archived);
        assert!(!result.workspace.worktree_deleted);
        let message = result.deletion_error.unwrap();
        assert!(message.starts_with("ワークツリーの物理削除に失敗しました（"));
        assert!(message.ends_with("）。手動で削除してください。"));
        assert!(message.contains("directory"), "{message}");
        assert!(invalid.exists());
    }

    #[tokio::test]
    async fn manual_archive_token_is_bound_to_target_and_existing_archived_is_retained() {
        let (_root, pool, ws, _repo, _session) = fixture().await;
        let prepared = prepare_for_pool(&pool, &ws).await.unwrap();
        assert!(
            archive_for_pool(
                &pool,
                Uuid::new_v4(),
                prepared.confirmation_token,
                || async { Ok(true) }
            )
            .await
            .is_err()
        );
        Workspace::set_archived(&pool, ws.id, true).await.unwrap();
        let archived = Workspace::find_by_id(&pool, ws.id).await.unwrap().unwrap();
        assert!(prepare_for_pool(&pool, &archived).await.is_err());
        assert!(Path::new(ws.container_ref.as_ref().unwrap()).exists());
        assert!(!archived.worktree_deleted);
    }
    #[tokio::test]
    async fn manual_archive_rejects_agent_and_integration_and_unsettled_script() {
        let (_root, pool, ws, repo, session) = fixture().await;
        let prepared = prepare_for_pool(&pool, &ws).await.unwrap();
        assert!(
            archive_for_pool(&pool, ws.id, prepared.confirmation_token, || async {
                Ok(false)
            })
            .await
            .is_err()
        );
        assert!(Path::new(ws.container_ref.as_ref().unwrap()).exists());
        let run = Uuid::new_v4();
        sqlx::query("INSERT INTO agent_runs(id,session_id,workspace_id,request_id,idempotency_key,correlation_id,schema_version,payload_version,runtime_profile_id,provider_id,workspace_mode,workspace_path,status,request_envelope) VALUES(?,?,?,?,?,?,1,1,'CODEX','codex','isolated_worktree',?,'awaiting_input','{}')")
            .bind(run).bind(session.id).bind(ws.id).bind(Uuid::new_v4()).bind(run.to_string()).bind(Uuid::new_v4()).bind(ws.container_ref.as_deref()).execute(&pool).await.unwrap();
        assert!(prepare_for_pool(&pool, &ws).await.is_err());
        sqlx::query("UPDATE agent_runs SET status='succeeded' WHERE id=?")
            .bind(run)
            .execute(&pool)
            .await
            .unwrap();
        let prepared = prepare_for_pool(&pool, &ws).await.unwrap();
        let project = Uuid::new_v4();
        let integration = Uuid::new_v4();
        sqlx::query("INSERT INTO projects(id,name) VALUES(?,'fixture')")
            .bind(project)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO integration_runs(id,request_key,project_id,repository_id,storage_identity,target_ref,status,payload) VALUES(?,?,?,?,'fixture','main','preparing','{}')")
            .bind(integration).bind(integration.to_string()).bind(project).bind(repo.id).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO integration_reservations VALUES('workspace',?,?)")
            .bind(db::models::integration::resource_key(ws.id))
            .bind(integration)
            .execute(&pool)
            .await
            .unwrap();
        assert!(prepare_for_pool(&pool, &ws).await.is_err());
        assert!(
            archive_for_pool(&pool, ws.id, prepared.confirmation_token, || async {
                Ok(true)
            })
            .await
            .is_err()
        );
        assert!(
            !Workspace::find_by_id(&pool, ws.id)
                .await
                .unwrap()
                .unwrap()
                .archived
        );
        assert!(Path::new(ws.container_ref.as_ref().unwrap()).exists());
    }

    #[tokio::test]
    async fn manual_archive_execution_only_integration_without_reservation_is_busy_until_terminal()
    {
        let (_root, pool, ws, repo, _session) = fixture().await;
        sqlx::query("UPDATE workspaces SET usage='execution_only' WHERE id=?")
            .bind(ws.id)
            .execute(&pool)
            .await
            .unwrap();
        let project = Uuid::new_v4();
        let integration = Uuid::new_v4();
        sqlx::query("INSERT INTO projects(id,name) VALUES(?,'fixture')")
            .bind(project)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO integration_runs(id,request_key,project_id,repository_id,workspace_id,storage_identity,target_ref,status,payload) VALUES(?,?,?,?,?,'fixture','main','preparing','{}')")
            .bind(integration).bind(integration.to_string()).bind(project).bind(repo.id).bind(ws.id).execute(&pool).await.unwrap();
        let ws = Workspace::find_by_id(&pool, ws.id).await.unwrap().unwrap();
        assert!(prepare_for_pool(&pool, &ws).await.is_err());
        sqlx::query("UPDATE integration_runs SET status='succeeded' WHERE id=?")
            .bind(integration)
            .execute(&pool)
            .await
            .unwrap();
        let prepared = prepare_for_pool(&pool, &ws).await.unwrap();
        let result = archive_for_pool(&pool, ws.id, prepared.confirmation_token, || async {
            Ok(true)
        })
        .await
        .unwrap();
        assert!(result.workspace.archived && result.workspace.worktree_deleted);
        assert!(result.workspace.is_execution_only());
    }

    #[tokio::test]
    async fn manual_archive_rejects_active_resource_mediation_without_agent() {
        let (_root, pool, ws, _repo, _session) = fixture().await;
        let operation = Uuid::new_v4();
        sqlx::query("INSERT INTO resource_operations(id,workspace_id,session_id,spec,status) VALUES(?,?,?,'{}','succeeded')")
            .bind(operation).bind(Uuid::new_v4()).bind(Uuid::new_v4()).execute(&pool).await.unwrap();
        let mediation = Uuid::new_v4();
        sqlx::query("INSERT INTO resource_mediations(id,trigger_operation_id,snapshot,status,workspace_id) VALUES(?,?,'{}','preparing',?)")
            .bind(mediation).bind(operation).bind(ws.id).execute(&pool).await.unwrap();
        assert!(prepare_for_pool(&pool, &ws).await.is_err());
        sqlx::query("UPDATE resource_mediations SET status='applied' WHERE id=?")
            .bind(mediation)
            .execute(&pool)
            .await
            .unwrap();
        assert!(prepare_for_pool(&pool, &ws).await.is_ok());
    }

    #[tokio::test]
    async fn manual_archive_waiting_for_admission_rechecks_usage() {
        let (_root, pool, ws, _repo, session) = fixture().await;
        let prepared = prepare_for_pool(&pool, &ws).await.unwrap();
        let guard = services::services::workspace_lifecycle::lock(ws.id).await;
        let attempt = tokio::spawn({
            let pool = pool.clone();
            async move {
                archive_for_pool(&pool, ws.id, prepared.confirmation_token, || async {
                    Ok(true)
                })
                .await
            }
        });
        busy(&pool, &session, "devserver").await;
        drop(guard);
        assert!(attempt.await.unwrap().is_err());
        assert!(Path::new(ws.container_ref.as_ref().unwrap()).exists());
    }

    #[tokio::test]
    async fn manual_archive_rejects_workflow_between_agent_nodes() {
        let (_root, pool, ws, repo, _session) = fixture().await;
        let prepared = prepare_for_pool(&pool, &ws).await.unwrap();
        let workflow = Uuid::new_v4();
        let run = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO workflows(id,source,name,graph_json) VALUES(?,'system','fixture','{}')",
        )
        .bind(workflow)
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query("INSERT INTO workflow_runs(id,workflow_id,repository_id,workspace_id,input_text,status) VALUES(?,?,?,?,'fixture','awaiting_human')")
            .bind(run).bind(workflow).bind(repo.id).bind(ws.id).execute(&pool).await.unwrap();
        assert!(prepare_for_pool(&pool, &ws).await.is_err());
        assert!(
            archive_for_pool(&pool, ws.id, prepared.confirmation_token, || async {
                Ok(true)
            })
            .await
            .is_err()
        );
        assert!(Path::new(ws.container_ref.as_ref().unwrap()).exists());
        sqlx::query("UPDATE workflow_runs SET status='succeeded' WHERE id=?")
            .bind(run)
            .execute(&pool)
            .await
            .unwrap();
        assert!(prepare_for_pool(&pool, &ws).await.is_ok());
    }
}
