//! Opt-in single-Issue deletion. Existing DELETE and MCP contracts stay unchanged.
//! DB changes commit atomically before awaited cleanup; receipts retain unknown outcomes.
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::LazyLock,
    time::{Duration, Instant},
};

use axum::{
    Json, Router,
    extract::{Path as RoutePath, State},
    routing::{get, post},
};
use db::models::{repo::Repo, workspace::Workspace};
use deployment::Deployment;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use services::services::container::ContainerService;
use sqlx::{Row, SqliteConnection, SqlitePool};
use tokio::sync::Mutex;
use utils::response::ApiResponse;
use uuid::Uuid;
use workspace_manager::{
    WorkspaceDeletionContext, WorkspaceManager, delete_workspace_record_in_transaction,
};

use super::issue_archive::{local_operations, lock_issue};
use crate::{
    DeploymentImpl,
    error::ApiError,
    remote_mcp::operations::Receipt,
    routes::workspaces::{archive, archive_processes},
};

#[derive(Clone, Debug, PartialEq, Serialize)]
struct Target {
    workspace_id: Uuid,
    name: Option<String>,
    branch: String,
    path: Option<String>,
    /// External folders are excluded entirely, including their Workspace history.
    excluded: bool,
    reason: Option<String>,
    #[serde(skip)]
    identity: Value,
    #[serde(skip)]
    repo_identity: Value,
    #[serde(skip)]
    physical_identity: Option<PathBuf>,
}
#[derive(Clone, Debug, PartialEq)]
struct Snapshot {
    revision: i64,
    issue_revision: i64,
    targets: Vec<Target>,
}
struct Confirmation {
    principal: String,
    project: Uuid,
    issue: Uuid,
    snapshot: Snapshot,
    expires: Instant,
}
static CONFIRMATIONS: LazyLock<Mutex<HashMap<Uuid, Confirmation>>> = LazyLock::new(Mutex::default);
#[derive(Deserialize, Serialize)]
struct Prepare {
    project_id: Uuid,
}
#[derive(Deserialize, Serialize)]
struct Confirm {
    project_id: Uuid,
    confirmation_token: Uuid,
    request_id: Uuid,
}
fn conflict(message: impl ToString) -> ApiError {
    ApiError::Conflict(message.to_string())
}
fn principal() -> String {
    format!(
        "issue-delete-local:{}:{}",
        server_access::actor_issuer(),
        server_access::actor_id()
    )
}

async fn repos(conn: &mut SqliteConnection, id: Uuid) -> Result<Vec<Repo>, ApiError> {
    Ok(sqlx::query("SELECT r.*,wr.worktree_name FROM repos r JOIN workspace_repos wr ON wr.repo_id=r.id WHERE wr.workspace_id=? ORDER BY r.id")
        .bind(id).try_map(|row: sqlx::sqlite::SqliteRow| Ok(Repo {
            path: PathBuf::from(row.try_get::<String, _>("path")?),
            id: row.try_get("id")?,
            name: row.try_get("name")?,
            display_name: row.try_get("display_name")?,
            worktree_name: row.try_get("worktree_name")?,
            setup_script: row.try_get("setup_script")?,
            cleanup_script: row.try_get("cleanup_script")?,
            archive_script: row.try_get("archive_script")?,
            copy_files: row.try_get("copy_files")?,
            parallel_setup_script: row.try_get("parallel_setup_script")?,
            dev_server_script: row.try_get("dev_server_script")?,
            default_target_branch: row.try_get("default_target_branch")?,
            default_working_dir: row.try_get("default_working_dir")?,
            created_at: row.try_get("created_at")?,
            updated_at: row.try_get("updated_at")?,
        })).fetch_all(conn).await?)
}

/// Resolve missing archived paths through their real parent; never follow a target symlink.
fn physical_path(path: &Path) -> std::io::Result<PathBuf> {
    match std::fs::symlink_metadata(path) {
        Ok(m) if m.file_type().is_symlink() => {
            Err(std::io::Error::other("Symlink target is not owned"))
        }
        Ok(_) => path.canonicalize(),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(path
            .parent()
            .ok_or_else(|| std::io::Error::other("Missing parent"))?
            .canonicalize()?
            .join(
                path.file_name()
                    .ok_or_else(|| std::io::Error::other("Missing name"))?,
            )),
        Err(e) => Err(e),
    }
}
fn overlaps(a: &Path, b: &Path) -> bool {
    a.starts_with(b) || b.starts_with(a)
}
fn owned_worktree(repo: &Repo, child: &Path, branch: &str) -> bool {
    fn check(repo: &Repo, child: &Path, branch: &str) -> Option<bool> {
        let source = git2::Repository::open(&repo.path).ok()?;
        let tree = git2::Repository::open(child).ok()?;
        Some(
            tree.is_worktree()
                && tree.commondir().canonicalize().ok()?
                    == source.commondir().canonicalize().ok()?
                && !tree.path().join("locked").exists()
                && tree.head().ok()?.shorthand() == Some(branch),
        )
    }
    check(repo, child, branch).unwrap_or(false)
}
fn shared_root() -> std::io::Result<PathBuf> {
    let path = utils::path::get_vibe_kanban_temp_dir_read_only()?.join("shared");
    Ok(path.canonicalize().unwrap_or(path))
}
fn owned_path(ws: &Workspace, repos: &[Repo], base: &Path) -> Option<PathBuf> {
    let path = Path::new(ws.container_ref.as_deref()?);
    let physical = physical_path(path).ok()?;
    let base = base.canonicalize().ok()?;
    if (physical.exists() && !physical.is_dir()) || overlaps(&physical, &shared_root().ok()?) {
        return None;
    }
    // Only one immediate, UUID-labelled managed container; never its parent/shared store.
    if physical.parent()? != base
        || !path
            .file_name()?
            .to_str()?
            .starts_with(&format!("{}-", utils::text::short_uuid(&ws.id)))
    {
        return None;
    }
    for repo in repos {
        let source = repo.path.canonicalize().ok()?;
        if overlaps(&physical, &source) {
            return None;
        }
        let child = repo.worktree_dir(&physical);
        if child.parent()? != physical || child.file_name()?.to_str()? == ".lvk" {
            return None;
        }
        if let Ok(meta) = std::fs::symlink_metadata(&child)
            && (meta.file_type().is_symlink()
                || child.canonicalize().ok()?.parent()? != physical
                || !owned_worktree(repo, &child, &ws.branch))
        {
            return None;
        }
    }
    Some(physical)
}

async fn snapshot(
    conn: &mut SqliteConnection,
    project: Uuid,
    issue: Uuid,
    base: &Path,
) -> Result<Snapshot, ApiError> {
    let row = sqlx::query("SELECT archive_link_revision,integration_revision FROM local_issues WHERE id=? AND project_id=?")
        .bind(issue).bind(project).fetch_optional(&mut *conn).await?.ok_or_else(|| conflict("Issue not found in Project"))?;
    let ids: Vec<Uuid> = sqlx::query_scalar(
        "SELECT workspace_id FROM local_workspace_links WHERE issue_id=? ORDER BY workspace_id",
    )
    .bind(issue)
    .fetch_all(&mut *conn)
    .await?;
    let mut targets = Vec::new();
    for id in ids {
        let ws = sqlx::query_as::<_, Workspace>("SELECT * FROM workspaces WHERE id=?")
            .bind(id)
            .fetch_one(&mut *conn)
            .await?;
        let repos = repos(conn, id).await?;
        let excluded = ws.is_direct_folder() || !ws.can_delete_container_path();
        let mut reason = if excluded {
            Some("external".to_owned())
        } else {
            None
        };
        if !excluded
            && (ws.is_execution_only()
                || ws.execution_owner.is_some()
                || ws.pinned
                || ws.arena_group_id.is_some())
        {
            reason = Some("protected".into());
        }
        let shared: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM local_workspace_links WHERE workspace_id=?1 AND (issue_id!=?2 OR project_id!=?3)) OR EXISTS(SELECT 1 FROM workflow_runs WHERE workspace_id=?1 AND issue_id IS NOT ?2) OR EXISTS(SELECT 1 FROM workflow_attempts WHERE workspace_id=?1 AND issue_id IS NOT ?2)")
            .bind(id).bind(issue).bind(project).fetch_one(&mut *conn).await?;
        if !excluded && (shared || ws.task_id.is_some_and(|task| task != issue)) {
            reason = Some("shared".into());
        }
        let protected_refs: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM integration_runs WHERE workspace_id=?1 OR session_id IN (SELECT id FROM sessions WHERE workspace_id=?1)) OR EXISTS(SELECT 1 FROM orchestration_agent_run_links l JOIN agent_runs r ON r.id=l.agent_run_id WHERE r.workspace_id=?1) OR EXISTS(SELECT 1 FROM arena_groups WHERE promoted_workspace_id=?1 OR implementation_workspace_id=?1) OR EXISTS(SELECT 1 FROM arena_events WHERE source_workspace_id=?1 OR target_workspace_id=?1 OR synthesis_workspace_id=?1)")
            .bind(id).fetch_one(&mut *conn).await?;
        if !excluded && protected_refs {
            reason = Some("protected".into());
        }
        let physical = if excluded {
            None
        } else {
            owned_path(&ws, &repos, base)
        };
        if !excluded && physical.is_none() {
            reason = Some("ownership".into());
        }
        if let Some(path) = &physical {
            let sources: Vec<String> = sqlx::query_scalar("SELECT path FROM repos")
                .fetch_all(&mut *conn)
                .await?;
            if sources.iter().any(|source| {
                Path::new(source)
                    .canonicalize()
                    .is_ok_and(|source| overlaps(path, &source))
            }) {
                reason = Some("ownership".into());
            }
            let others: Vec<String> = sqlx::query_scalar(
                "SELECT container_ref FROM workspaces WHERE id!=? AND container_ref IS NOT NULL",
            )
            .bind(id)
            .fetch_all(&mut *conn)
            .await?;
            if others.iter().any(|other| {
                Path::new(other)
                    .canonicalize()
                    .or_else(|_| physical_path(Path::new(other)))
                    .is_ok_and(|other| overlaps(path, &other))
            }) {
                reason = Some("shared".into());
            }
        }
        if !excluded && reason.is_none() {
            match archive::check_idle(conn, id).await {
                Ok(()) => {}
                Err(ApiError::Conflict(_)) => reason = Some("busy".into()),
                Err(error) => return Err(error),
            }
        }
        targets.push(Target {
            workspace_id: id, name: ws.name.clone(), branch: ws.branch.clone(), path: ws.container_ref.clone(), excluded, reason,
            identity: json!({"kind":ws.workspace_kind,"ownership":ws.container_ownership,"usage":ws.usage,"owner":ws.execution_owner,"archived":ws.archived,"deleted":ws.worktree_deleted,"pinned":ws.pinned,"arena":ws.arena_group_id,"task":ws.task_id}),
            repo_identity: json!(repos.iter().map(|r| json!({"id":r.id,"path":r.path,"name":r.name,"placement":r.worktree_name})).collect::<Vec<_>>()), physical_identity: physical,
        });
    }
    Ok(Snapshot {
        revision: row.try_get("archive_link_revision")?,
        issue_revision: row.try_get("integration_revision")?,
        targets,
    })
}

async fn prepare_for_pool(
    pool: &SqlitePool,
    project: Uuid,
    issue: Uuid,
    actor: String,
    base: &Path,
) -> Result<Value, ApiError> {
    let mut tx = pool.begin().await?;
    let snapshot = snapshot(&mut tx, project, issue, base).await?;
    tx.rollback().await?;
    let token = Uuid::new_v4();
    let response = json!({"confirmation_token":token,"workspaces":snapshot.targets,"expires_at":chrono::Utc::now()+chrono::Duration::minutes(10)});
    let mut tokens = CONFIRMATIONS.lock().await;
    tokens.retain(|_, c| c.expires > Instant::now());
    tokens.insert(
        token,
        Confirmation {
            principal: actor,
            project,
            issue,
            snapshot,
            expires: Instant::now() + Duration::from_secs(600),
        },
    );
    Ok(response)
}

async fn delete_for_pool<F, Fut, C, CFut>(
    pool: &SqlitePool,
    issue: Uuid,
    request: &Confirm,
    actor: &str,
    base: &Path,
    settled: F,
    cleanup: C,
) -> Result<Value, ApiError>
where
    F: Fn(Uuid) -> Fut,
    Fut: std::future::Future<Output = Result<bool, ApiError>>,
    C: Fn(WorkspaceDeletionContext) -> CFut,
    CFut: std::future::Future<Output = Vec<String>>,
{
    let _admission = services::services::integration_admission::MUTATIONS
        .lock()
        .await;
    let _issue = lock_issue(issue).await;
    let confirmation = {
        let mut tokens = CONFIRMATIONS.lock().await;
        if !tokens.get(&request.confirmation_token).is_some_and(|c| {
            c.principal == actor
                && c.project == request.project_id
                && c.issue == issue
                && c.expires > Instant::now()
        }) {
            return Err(conflict(
                "Confirmation expired or caller/target changed; confirm again",
            ));
        }
        tokens
            .remove(&request.confirmation_token)
            .expect("validated token")
    };
    let mut guards = Vec::new();
    for target in &confirmation.snapshot.targets {
        if !target.excluded {
            guards.push(services::services::workspace_lifecycle::lock(target.workspace_id).await);
            if !settled(target.workspace_id).await? {
                return Err(conflict(format!(
                    "Workspace {}: scripts are not settled",
                    target.workspace_id
                )));
            }
        }
    }
    let mut tx = pool.begin_with("BEGIN IMMEDIATE").await?;
    let current = snapshot(&mut tx, request.project_id, issue, base).await?;
    if current != confirmation.snapshot {
        return Err(conflict(
            "Workspace targets changed; refresh and confirm again",
        ));
    }
    let mut contexts = Vec::new();
    for target in &current.targets {
        if target.excluded {
            continue;
        }
        if let Some(reason) = &target.reason {
            return Err(conflict(format!(
                "Workspace {}: {reason}",
                target.workspace_id
            )));
        }
        archive::check_idle(&mut tx, target.workspace_id).await?;
        archive_processes::check(target.path.as_deref()).await?;
        let id = target.workspace_id;
        contexts.push(WorkspaceDeletionContext {
            workspace_id: id,
            branch_name: target.branch.clone(),
            workspace_dir: target.physical_identity.clone(),
            repositories: repos(&mut tx, id).await?,
            repo_paths: Vec::new(),
            session_ids: sqlx::query_scalar(
                "SELECT id FROM sessions WHERE workspace_id=? ORDER BY id",
            )
            .bind(id)
            .fetch_all(&mut *tx)
            .await?,
        });
    }
    // The existing lifecycle primitive and every FK/trigger run before any filesystem change.
    // A failure on the last Workspace rolls back the entire batch, including the Issue.
    for context in &contexts {
        delete_workspace_record_in_transaction(&mut tx, context.workspace_id)
            .await
            .map_err(conflict)?;
    }
    sqlx::query("DELETE FROM local_issues WHERE id=? AND project_id=?")
        .bind(issue)
        .bind(request.project_id)
        .execute(&mut *tx)
        .await?;
    if let Err(error) = tx.commit().await {
        return Ok(
            json!({"outcome":"uncertain","issue_deleted":null,"message":error.to_string(),"workspaces":[]}),
        );
    }
    let mut outcomes = Vec::new();
    for context in contexts {
        let id = context.workspace_id;
        // A new registration must not acquire this path after DB deletion. Recheck
        // under a write barrier and retain it through physical cleanup.
        let errors = match pool.begin_with("BEGIN IMMEDIATE").await {
            Ok(mut barrier) => {
                let validation = async {
                    let path = context.workspace_dir.as_deref().ok_or_else(|| conflict("Missing owned path"))?;
                    if physical_path(path)? != path || overlaps(path, &shared_root()?) { return Err(conflict("Physical target changed")); }
                    let paths: Vec<String> = sqlx::query_scalar("SELECT container_ref FROM workspaces WHERE container_ref IS NOT NULL UNION SELECT path FROM repos")
                        .fetch_all(&mut *barrier).await?;
                    if paths.iter().any(|other| Path::new(other).canonicalize().or_else(|_| physical_path(Path::new(other))).is_ok_and(|other| overlaps(path, &other))) {
                        return Err(conflict("Physical target is now referenced elsewhere"));
                    }
                    for repo in &context.repositories {
                        let child = repo.worktree_dir(path);
                        if child.parent() != Some(path) || std::fs::symlink_metadata(&child).is_ok_and(|m| m.file_type().is_symlink() || !owned_worktree(repo, &child, &context.branch_name)) { return Err(conflict("Worktree ownership changed")); }
                    }
                    Ok::<_,ApiError>(())
                }.await;
                let errors = match validation {
                    Ok(()) => cleanup(context).await,
                    Err(e) => vec![e.to_string()],
                };
                // This barrier performs no DB writes. Rollback only releases the lock.
                let _ = barrier.rollback().await;
                errors
            }
            Err(error) => vec![error.to_string()],
        };
        outcomes.push(json!({"workspace_id":id,"db_deleted":true,"cleanup_completed":errors.is_empty(),"cleanup_errors":errors}));
    }
    Ok(
        json!({"issue_deleted":true,"workspaces":outcomes,"excluded":current.targets.iter().filter(|t|t.excluded).collect::<Vec<_>>()}),
    )
}

async fn prepare(
    State(d): State<DeploymentImpl>,
    RoutePath(issue): RoutePath<Uuid>,
    Json(request): Json<Prepare>,
) -> Result<Json<ApiResponse<Value>>, ApiError> {
    let base = worktree_manager::WorktreeManager::get_worktree_base_dir_read_only()?;
    Ok(Json(ApiResponse::success(
        prepare_for_pool(&d.db().pool, request.project_id, issue, principal(), &base).await?,
    )))
}
async fn confirm(
    State(d): State<DeploymentImpl>,
    RoutePath(issue): RoutePath<Uuid>,
    Json(request): Json<Confirm>,
) -> Result<Json<ApiResponse<Receipt>>, ApiError> {
    let store = local_operations(&d.db().pool);
    let actor = principal();
    // Retain the confirmed identifiers/paths for operator recovery even when
    // a crash happens after DB commit and before cleanup results are settled.
    let targets = {
        let tokens = CONFIRMATIONS.lock().await;
        tokens
            .get(&request.confirmation_token)
            .filter(|c| c.principal == actor && c.project == request.project_id && c.issue == issue)
            .map(|c| json!({"issue_id":issue,"workspaces":c.snapshot.targets}))
            .unwrap_or_else(|| json!({"issue_id":issue}))
    };
    let receipt = store
        .submit_and_wait(
            actor.clone(),
            request.request_id,
            request.project_id,
            "confirm_issue_delete".into(),
            json!({"issue_id":issue,"request":request}),
            targets,
            move |_| async move {
                let result = async {
                    let base =
                        worktree_manager::WorktreeManager::get_worktree_base_dir_read_only()?;
                    delete_for_pool(
                        &d.db().pool,
                        issue,
                        &request,
                        &actor,
                        &base,
                        |id| {
                            let d = d.clone();
                            async move { Ok(d.container().scripts_settled(id).await?) }
                        },
                        |context| async move {
                            WorkspaceManager::cleanup_deleted_workspace(
                                &context,
                                &utils::assets::asset_dir(),
                            )
                            .await
                        },
                    )
                    .await
                }
                .await;
                match result {
                    Ok(value) if value["outcome"] == "uncertain" => ("uncertain", value),
                    Ok(value) => ("succeeded", value),
                    Err(error) => ("failed", json!({"message":error.to_string()})),
                }
            },
        )
        .await
        .map_err(conflict)?;
    Ok(Json(ApiResponse::success(receipt)))
}
async fn receipt(
    State(d): State<DeploymentImpl>,
    RoutePath(id): RoutePath<Uuid>,
) -> Result<Json<ApiResponse<Receipt>>, ApiError> {
    Ok(Json(ApiResponse::success(
        local_operations(&d.db().pool)
            .by_request(&principal(), id)
            .await
            .map_err(conflict)?,
    )))
}
pub(crate) fn router() -> Router<DeploymentImpl> {
    Router::new()
        .route("/issues/{issue_id}/delete/prepare", post(prepare))
        .route("/issues/{issue_id}/delete/confirm", post(confirm))
        .route("/issue-delete-operations/{request_id}", get(receipt))
}

#[cfg(test)]
mod tests {
    use db::models::{session::Session, workspace::CreateWorkspace};

    use super::*;

    struct Fixture {
        root: tempfile::TempDir,
        pool: SqlitePool,
        ws: Workspace,
        repo: Repo,
        session: Session,
        project: Uuid,
        issue: Uuid,
    }
    async fn fixture() -> Fixture {
        let (root, pool, ws, repo, session) =
            crate::routes::workspaces::archive::tests::fixture().await;
        let path = root
            .path()
            .join(format!("{}-fixture", utils::text::short_uuid(&ws.id)));
        std::fs::rename(ws.container_ref.as_ref().unwrap(), &path).unwrap();
        git::GitCli::new()
            .git(
                &repo.path,
                [
                    "worktree",
                    "repair",
                    repo.worktree_dir(&path).to_str().unwrap(),
                ],
            )
            .unwrap();
        Workspace::update_container_ref(&pool, ws.id, path.to_str().unwrap())
            .await
            .unwrap();
        let project = Uuid::new_v4();
        let issue = Uuid::new_v4();
        sqlx::query("INSERT INTO projects(id,name,key) VALUES (?,'Delete fixture','DEL')")
            .bind(project)
            .execute(&pool)
            .await
            .unwrap();
        let status = super::super::ensure_default_statuses(&pool, project)
            .await
            .unwrap()[0]
            .id;
        sqlx::query("INSERT INTO local_issues(id,project_id,issue_number,simple_id,status_id,title,sort_order) VALUES(?,?,1,'DEL-1',?,'Delete fixture',0)").bind(issue).bind(project).bind(status).execute(&pool).await.unwrap();
        sqlx::query(
            "INSERT INTO local_workspace_links(workspace_id,issue_id,project_id) VALUES(?,?,?)",
        )
        .bind(ws.id)
        .bind(issue)
        .bind(project)
        .execute(&pool)
        .await
        .unwrap();
        let ws = Workspace::find_by_id(&pool, ws.id).await.unwrap().unwrap();
        Fixture {
            root,
            pool,
            ws,
            repo,
            session,
            project,
            issue,
        }
    }
    async fn prepare(f: &Fixture) -> Confirm {
        let p = prepare_for_pool(&f.pool, f.project, f.issue, "fixture".into(), f.root.path())
            .await
            .unwrap();
        Confirm {
            project_id: f.project,
            confirmation_token: serde_json::from_value(p["confirmation_token"].clone()).unwrap(),
            request_id: Uuid::new_v4(),
        }
    }
    async fn run(f: &Fixture, request: &Confirm) -> Result<Value, ApiError> {
        delete_for_pool(
            &f.pool,
            f.issue,
            request,
            "fixture",
            f.root.path(),
            |_| async { Ok(true) },
            |context| async move {
                WorkspaceManager::cleanup_deleted_workspace(&context, f.root.path()).await
            },
        )
        .await
    }
    async fn kept(f: &Fixture) {
        assert!(
            Workspace::find_by_id(&f.pool, f.ws.id)
                .await
                .unwrap()
                .is_some()
        );
        assert!(Path::new(f.ws.container_ref.as_ref().unwrap()).exists());
        assert!(
            sqlx::query_scalar::<_, bool>("SELECT EXISTS(SELECT 1 FROM local_issues WHERE id=?)")
                .bind(f.issue)
                .fetch_one(&f.pool)
                .await
                .unwrap()
        );
    }

    #[tokio::test]
    async fn issue_delete_on_removes_history_files_keeps_branch_source_shared_and_child() {
        let f = fixture().await;
        let tree = Path::new(f.ws.container_ref.as_ref().unwrap());
        std::fs::write(f.repo.worktree_dir(tree).join("untracked"), "fixture").unwrap();
        let persistent = f.root.path().join("persistent");
        std::fs::create_dir(&persistent).unwrap();
        std::fs::write(persistent.join("keep"), "retained").unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(&persistent, tree.join("shared")).unwrap();
        let logs =
            utils::execution_logs::process_logs_session_dir_in_root(f.root.path(), f.session.id);
        let audit = utils::native_audit::session_dir_in_root(f.root.path(), f.session.id);
        for path in [&logs, &audit] {
            std::fs::create_dir_all(path).unwrap();
            std::fs::write(path.join("fixture"), "fixture").unwrap();
        }
        let child = Uuid::new_v4();
        sqlx::query("INSERT INTO local_issues(id,project_id,issue_number,simple_id,status_id,title,sort_order,parent_issue_id) SELECT ?,project_id,2,'DEL-2',status_id,'Child',1,id FROM local_issues WHERE id=?").bind(child).bind(f.issue).execute(&f.pool).await.unwrap();
        let request = prepare(&f).await;
        let result = run(&f, &request).await.unwrap();
        assert_eq!(result["issue_deleted"], true);
        assert_eq!(result["workspaces"][0]["cleanup_completed"], true);
        assert!(
            Workspace::find_by_id(&f.pool, f.ws.id)
                .await
                .unwrap()
                .is_none()
        );
        assert!(
            Session::find_by_id(&f.pool, f.session.id)
                .await
                .unwrap()
                .is_none()
        );
        assert!(!tree.exists() && !logs.exists() && !audit.exists());
        assert!(f.repo.path.exists() && persistent.join("keep").exists());
        git::GitCli::new()
            .git(
                &f.repo.path,
                [
                    "show-ref",
                    "--verify",
                    &format!("refs/heads/{}", f.ws.branch),
                ],
            )
            .unwrap();
        let parent: Option<Uuid> =
            sqlx::query_scalar("SELECT parent_issue_id FROM local_issues WHERE id=?")
                .bind(child)
                .fetch_one(&f.pool)
                .await
                .unwrap();
        assert!(parent.is_none());
        assert!(run(&f, &request).await.is_err());
    }

    #[tokio::test]
    async fn issue_delete_external_is_explicitly_excluded_with_history_and_files_kept() {
        let f = fixture().await;
        sqlx::query("UPDATE workspaces SET container_ownership='external',workspace_kind='direct_folder' WHERE id=?").bind(f.ws.id).execute(&f.pool).await.unwrap();
        let request = prepare(&f).await;
        let result = run(&f, &request).await.unwrap();
        assert_eq!(result["excluded"][0]["reason"], "external");
        assert!(result["workspaces"].as_array().unwrap().is_empty());
        assert!(
            Workspace::find_by_id(&f.pool, f.ws.id)
                .await
                .unwrap()
                .is_some()
        );
        assert!(
            Session::find_by_id(&f.pool, f.session.id)
                .await
                .unwrap()
                .is_some()
        );
        assert!(Path::new(f.ws.container_ref.as_ref().unwrap()).exists());
    }

    #[tokio::test]
    async fn issue_delete_busy_protected_shared_and_unknown_ownership_refuse_before_any_deletion() {
        for case in [
            "busy",
            "pinned",
            "owner",
            "usage",
            "outside",
            "shared",
            "integration",
        ] {
            let f = fixture().await;
            match case {
                "busy" => {
                    sqlx::query("INSERT INTO execution_processes(id,session_id,run_reason,status) VALUES(?,?,'devserver','running')").bind(Uuid::new_v4()).bind(f.session.id).execute(&f.pool).await.unwrap();
                }
                "pinned" => {
                    sqlx::query("UPDATE workspaces SET pinned=1 WHERE id=?")
                        .bind(f.ws.id)
                        .execute(&f.pool)
                        .await
                        .unwrap();
                }
                "owner" => {
                    sqlx::query("UPDATE workspaces SET execution_owner=? WHERE id=?")
                        .bind(r#"{"kind":"unknown","run_id":null,"repository_id":null}"#)
                        .bind(f.ws.id)
                        .execute(&f.pool)
                        .await
                        .unwrap();
                }
                "usage" => {
                    sqlx::query("UPDATE workspaces SET usage='execution_only' WHERE id=?")
                        .bind(f.ws.id)
                        .execute(&f.pool)
                        .await
                        .unwrap();
                }
                "outside" => {
                    sqlx::query("UPDATE workspaces SET container_ref=? WHERE id=?")
                        .bind(f.repo.path.to_str().unwrap())
                        .bind(f.ws.id)
                        .execute(&f.pool)
                        .await
                        .unwrap();
                }
                "shared" => {
                    let other = Workspace::create(
                        &f.pool,
                        &CreateWorkspace {
                            branch: "other".into(),
                            name: None,
                        },
                        Uuid::new_v4(),
                    )
                    .await
                    .unwrap();
                    Workspace::update_container_ref(
                        &f.pool,
                        other.id,
                        f.ws.container_ref.as_ref().unwrap(),
                    )
                    .await
                    .unwrap();
                }
                "integration" => {
                    let run = Uuid::new_v4();
                    sqlx::query("INSERT INTO integration_runs(id,request_key,project_id,repository_id,storage_identity,target_ref,status,payload) VALUES(?,?,?,?,'fixture','main','preparing','{}')")
                        .bind(run).bind(run.to_string()).bind(f.project).bind(f.repo.id).execute(&f.pool).await.unwrap();
                    sqlx::query("INSERT INTO integration_reservations VALUES('workspace',?,?)")
                        .bind(db::models::integration::resource_key(f.ws.id))
                        .bind(run)
                        .execute(&f.pool)
                        .await
                        .unwrap();
                }
                _ => unreachable!(),
            }
            let request = prepare(&f).await;
            assert!(run(&f, &request).await.is_err(), "{case}");
            kept(&f).await;
        }
    }

    #[tokio::test]
    async fn issue_delete_target_changes_and_scope_require_reconfirmation() {
        for case in [
            "link_aba",
            "branch",
            "pinned",
            "principal",
            "project",
            "expired",
            "path",
            "repo",
        ] {
            let f = fixture().await;
            let mut request = prepare(&f).await;
            match case {
                "link_aba" => {
                    sqlx::query("DELETE FROM local_workspace_links WHERE workspace_id=?")
                        .bind(f.ws.id)
                        .execute(&f.pool)
                        .await
                        .unwrap();
                    sqlx::query("INSERT INTO local_workspace_links(workspace_id,project_id,issue_id) VALUES(?,?,?)").bind(f.ws.id).bind(f.project).bind(f.issue).execute(&f.pool).await.unwrap();
                }
                "branch" => {
                    sqlx::query("UPDATE workspaces SET branch='changed' WHERE id=?")
                        .bind(f.ws.id)
                        .execute(&f.pool)
                        .await
                        .unwrap();
                }
                "pinned" => {
                    sqlx::query("UPDATE workspaces SET pinned=1 WHERE id=?")
                        .bind(f.ws.id)
                        .execute(&f.pool)
                        .await
                        .unwrap();
                }
                "path" => {
                    let changed = f
                        .root
                        .path()
                        .join(format!("{}-changed", utils::text::short_uuid(&f.ws.id)));
                    std::fs::create_dir(&changed).unwrap();
                    Workspace::update_container_ref(&f.pool, f.ws.id, changed.to_str().unwrap())
                        .await
                        .unwrap();
                }
                "repo" => {
                    sqlx::query(
                        "UPDATE workspace_repos SET worktree_name='changed' WHERE workspace_id=?",
                    )
                    .bind(f.ws.id)
                    .execute(&f.pool)
                    .await
                    .unwrap();
                }
                "principal" => {
                    CONFIRMATIONS
                        .lock()
                        .await
                        .get_mut(&request.confirmation_token)
                        .unwrap()
                        .principal = "other".into();
                }
                "project" => request.project_id = Uuid::new_v4(),
                "expired" => {
                    CONFIRMATIONS
                        .lock()
                        .await
                        .get_mut(&request.confirmation_token)
                        .unwrap()
                        .expires = Instant::now() - Duration::from_secs(1)
                }
                _ => unreachable!(),
            }
            assert!(run(&f, &request).await.is_err(), "{case}");
            kept(&f).await;
        }
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn issue_delete_rejects_symlink_swap_after_confirmation() {
        let f = fixture().await;
        let request = prepare(&f).await;
        let original = Path::new(f.ws.container_ref.as_ref().unwrap());
        let moved = f.root.path().join("saved-worktree");
        std::fs::rename(original, &moved).unwrap();
        std::os::unix::fs::symlink(&f.repo.path, original).unwrap();
        assert!(run(&f, &request).await.is_err());
        kept(&f).await;
        assert!(f.repo.path.join(".git").is_dir() && moved.exists());
    }

    #[tokio::test]
    async fn issue_delete_late_db_failure_rolls_back_all_without_cleanup() {
        let f = fixture().await;
        let request = prepare(&f).await;
        sqlx::query("CREATE TRIGGER fixture_delete_failure BEFORE DELETE ON local_issues BEGIN SELECT RAISE(ABORT,'fixture refusal'); END").execute(&f.pool).await.unwrap();
        assert!(run(&f, &request).await.is_err());
        kept(&f).await;
        assert!(
            Session::find_by_id(&f.pool, f.session.id)
                .await
                .unwrap()
                .is_some()
        );
    }

    #[tokio::test]
    async fn issue_delete_last_workspace_failure_rolls_back_previous_workspace() {
        let f = fixture().await;
        let second = Workspace::create(
            &f.pool,
            &CreateWorkspace {
                branch: "second".into(),
                name: None,
            },
            Uuid::from_u128(u128::MAX),
        )
        .await
        .unwrap();
        let path = f.root.path().join("ffff-second");
        std::fs::create_dir(&path).unwrap();
        Workspace::update_container_ref(&f.pool, second.id, path.to_str().unwrap())
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO local_workspace_links(workspace_id,project_id,issue_id) VALUES(?,?,?)",
        )
        .bind(second.id)
        .bind(f.project)
        .bind(f.issue)
        .execute(&f.pool)
        .await
        .unwrap();
        let request = prepare(&f).await;
        sqlx::query("CREATE TRIGGER fixture_last_refusal BEFORE DELETE ON workspaces WHEN OLD.id=x'ffffffffffffffffffffffffffffffff' BEGIN SELECT RAISE(ABORT,'last workspace protected'); END").execute(&f.pool).await.unwrap();
        assert!(run(&f, &request).await.is_err());
        kept(&f).await;
        assert!(
            Workspace::find_by_id(&f.pool, second.id)
                .await
                .unwrap()
                .is_some()
                && path.exists()
        );
    }

    #[tokio::test]
    async fn issue_delete_already_archived_missing_tree_still_deletes_history() {
        let f = fixture().await;
        WorkspaceManager::cleanup_workspace_strict(
            Path::new(f.ws.container_ref.as_ref().unwrap()),
            std::slice::from_ref(&f.repo),
        )
        .await
        .unwrap();
        sqlx::query("UPDATE workspaces SET archived=1,worktree_deleted=1 WHERE id=?")
            .bind(f.ws.id)
            .execute(&f.pool)
            .await
            .unwrap();
        let request = prepare(&f).await;
        let result = run(&f, &request).await.unwrap();
        assert_eq!(result["workspaces"][0]["cleanup_completed"], true);
        assert!(
            Session::find_by_id(&f.pool, f.session.id)
                .await
                .unwrap()
                .is_none()
        );
        assert!(f.repo.path.exists());
    }

    #[tokio::test]
    async fn issue_delete_cleanup_failure_and_same_request_receipt_do_not_repeat() {
        let f = fixture().await;
        let request = prepare(&f).await;
        let id = request.request_id;
        let store = local_operations(&f.pool);
        let input = json!({"issue":f.issue,"request":request});
        let input2 = input.clone();
        let pool = f.pool.clone();
        let issue = f.issue;
        let base = f.root.path().to_owned();
        let project = f.project;
        let result = store
            .submit_and_wait(
                "fixture".into(),
                id,
                project,
                "delete_fixture".into(),
                input,
                json!({}),
                move |_| async move {
                    let v = delete_for_pool(
                        &pool,
                        issue,
                        &request,
                        "fixture",
                        &base,
                        |_| async { Ok(true) },
                        |_| async { vec!["injected cleanup failure".into()] },
                    )
                    .await
                    .unwrap();
                    ("succeeded", v)
                },
            )
            .await
            .unwrap();
        assert_eq!(result.status, "succeeded");
        let value: Value = serde_json::from_str(result.result.as_ref().unwrap()).unwrap();
        assert_eq!(value["issue_deleted"], true);
        assert_eq!(value["workspaces"][0]["cleanup_completed"], false);
        assert!(Path::new(f.ws.container_ref.as_ref().unwrap()).exists());
        let repeated = store
            .submit_and_wait(
                "fixture".into(),
                id,
                project,
                "delete_fixture".into(),
                input2,
                json!({}),
                |_| async { panic!("must not replay deletion") },
            )
            .await
            .unwrap();
        assert_eq!(result.result, repeated.result);
    }
}
