//! One confirmation for an Issue and its exact linked Workspace set. No status
//! writes; no background cleanup. Receipts prevent replay of unknown outcomes.
use std::{
    collections::HashMap,
    sync::LazyLock,
    time::{Duration, Instant},
};

use axum::{
    Json, Router,
    extract::{Path, State},
    routing::{get, post},
};
use db::models::{workspace::Workspace, workspace_repo::WorkspaceRepo};
use deployment::Deployment;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use services::services::container::ContainerService;
use sqlx::{Row, SqliteConnection, SqlitePool};
use tokio::sync::Mutex;
use utils::response::ApiResponse;
use uuid::Uuid;

use crate::{
    DeploymentImpl,
    error::ApiError,
    remote_mcp::operations::{Operations, Receipt},
    routes::workspaces::{
        archive::{self, ConfirmationScope},
        archive_processes,
    },
};

// Creation holds this guard only until the initial link is persisted, before
// container/Agent admission. Archive order: MUTATIONS -> Issue -> sorted WS -> DB.
static ISSUE_LOCKS: LazyLock<std::sync::Mutex<HashMap<Uuid, std::sync::Weak<Mutex<()>>>>> =
    LazyLock::new(std::sync::Mutex::default);
pub(crate) async fn lock_issue(issue: Uuid) -> tokio::sync::OwnedMutexGuard<()> {
    let mutex = {
        let mut locks = ISSUE_LOCKS.lock().unwrap();
        locks.retain(|_, lock| lock.strong_count() > 0);
        let mutex = locks
            .get(&issue)
            .and_then(std::sync::Weak::upgrade)
            .unwrap_or_else(|| std::sync::Arc::new(Mutex::new(())));
        locks.insert(issue, std::sync::Arc::downgrade(&mutex));
        mutex
    };
    mutex.lock_owned().await
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Target {
    pub workspace_id: Uuid,
    pub path: Option<String>,
    pub branch: String,
    pub archived: bool,
    pub worktree_deleted: bool,
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct Snapshot {
    revision: i64,
    targets: Vec<Target>,
}
struct Confirmation {
    scope: ConfirmationScope,
    project_id: Uuid,
    issue_id: Uuid,
    archive_workspaces: bool,
    snapshot: Snapshot,
    expires: Instant,
}
static CONFIRMATIONS: LazyLock<Mutex<HashMap<Uuid, Confirmation>>> = LazyLock::new(Mutex::default);
#[derive(Deserialize, Serialize)]
pub(crate) struct Prepare {
    pub project_id: Uuid,
    pub archive_workspaces: bool,
}
#[derive(Deserialize, Serialize)]
pub(crate) struct Confirm {
    pub project_id: Uuid,
    pub confirmation_token: Uuid,
    pub request_id: Uuid,
}
#[derive(Deserialize, Serialize)]
pub(crate) struct Unarchive {
    pub project_id: Uuid,
    pub request_id: Uuid,
}

pub(crate) fn local_operations(pool: &SqlitePool) -> Operations {
    Operations {
        pool: pool.clone(),
        process_instance: *crate::remote_mcp::PROCESS_INSTANCE.get_or_init(Uuid::new_v4),
    }
}
fn local_principal() -> String {
    // Same authenticated actor namespace on every local request; remote tokens
    // use their connection-bound namespace and cannot cross this boundary.
    format!(
        "issue-archive-local:{}:{}",
        server_access::actor_issuer(),
        server_access::actor_id()
    )
}
fn failure(error: impl std::fmt::Display) -> ApiError {
    ApiError::Conflict(error.to_string())
}

async fn snapshot(
    conn: &mut SqliteConnection,
    project: Uuid,
    issue: Uuid,
    expected_archived: bool,
) -> Result<Snapshot, ApiError> {
    let row = sqlx::query(
        "SELECT archived,archive_link_revision FROM local_issues WHERE id=? AND project_id=?",
    )
    .bind(issue)
    .bind(project)
    .fetch_optional(&mut *conn)
    .await?
    .ok_or_else(|| ApiError::BadRequest("Issue not found in Project".into()))?;
    if row.try_get::<bool, _>("archived")? != expected_archived {
        return Err(failure(
            "Issue archive state changed; refresh and confirm again",
        ));
    }
    let rows = sqlx::query("SELECT w.id,w.container_ref,w.branch,w.archived,w.worktree_deleted FROM local_workspace_links l JOIN workspaces w ON w.id=l.workspace_id WHERE l.issue_id=? AND l.project_id=? ORDER BY w.id")
        .bind(issue).bind(project).fetch_all(&mut *conn).await?;
    let targets = rows
        .iter()
        .map(|r| {
            Ok(Target {
                workspace_id: r.try_get("id")?,
                path: r.try_get("container_ref")?,
                branch: r.try_get("branch")?,
                archived: r.try_get("archived")?,
                worktree_deleted: r.try_get("worktree_deleted")?,
            })
        })
        .collect::<Result<Vec<_>, sqlx::Error>>()?;
    Ok(Snapshot {
        revision: row.try_get("archive_link_revision")?,
        targets,
    })
}

pub(crate) async fn require_unarchived(
    pool: &SqlitePool,
    project: Uuid,
    issue: Uuid,
) -> Result<(), ApiError> {
    let archived: bool =
        sqlx::query_scalar("SELECT archived FROM local_issues WHERE id=? AND project_id=?")
            .bind(issue)
            .bind(project)
            .fetch_optional(pool)
            .await?
            .ok_or_else(|| ApiError::BadRequest("Issue not found in Project".into()))?;
    if archived {
        return Err(failure("アーカイブ解除すると利用できます"));
    }
    Ok(())
}

pub(crate) async fn prepare_scoped<F, Fut>(
    pool: &SqlitePool,
    issue: Uuid,
    request: Prepare,
    scope: ConfirmationScope,
    settled: F,
) -> Result<Value, ApiError>
where
    F: Fn(Uuid) -> Fut,
    Fut: std::future::Future<Output = Result<bool, ApiError>>,
{
    let before = snapshot(
        &mut *pool.acquire().await?,
        request.project_id,
        issue,
        false,
    )
    .await?;
    let mut guards = Vec::new();
    if request.archive_workspaces {
        for target in before.targets.iter().filter(|t| !t.archived) {
            guards.push(services::services::workspace_lifecycle::lock(target.workspace_id).await);
            if !settled(target.workspace_id).await? {
                return Err(failure(
                    "使用中のWorkspaceはarchiveできません。チェックを外すとIssueのみアーカイブできます。",
                ));
            }
        }
    }
    let mut tx = pool.begin_with("BEGIN IMMEDIATE").await?;
    if snapshot(&mut tx, request.project_id, issue, false).await? != before {
        return Err(failure("Workspace links changed; confirm again"));
    }
    if request.archive_workspaces {
        for target in before.targets.iter().filter(|t| !t.archived) {
            archive::check_idle(&mut tx, target.workspace_id).await?;
            archive_processes::check(target.path.as_deref()).await?;
        }
    }
    tx.rollback().await?;
    let token = Uuid::new_v4();
    let response = json!({"issue_id":issue,"project_id":request.project_id,"archive_workspaces":request.archive_workspaces,
        "workspaces":before.targets,"confirmation_token":token,"expires_at":chrono::Utc::now()+chrono::Duration::minutes(10),
        "message": if request.archive_workspaces { "Issueと未アーカイブの配下Workspaceをアーカイブします。対象のワークツリーを物理削除します。本文・履歴・Git branchは保持します。" } else { "Issueのみアーカイブします。Workspace・プロセス・ファイルは変更しません。" }});
    let mut tokens = CONFIRMATIONS.lock().await;
    tokens.retain(|_, c| c.expires > Instant::now());
    tokens.insert(
        token,
        Confirmation {
            scope,
            project_id: request.project_id,
            issue_id: issue,
            archive_workspaces: request.archive_workspaces,
            snapshot: before,
            expires: Instant::now() + Duration::from_secs(600),
        },
    );
    Ok(response)
}

pub(crate) async fn archive_scoped<F, Fut>(
    pool: &SqlitePool,
    issue: Uuid,
    request: &Confirm,
    scope: ConfirmationScope,
    settled: F,
) -> Result<Value, ApiError>
where
    F: Fn(Uuid) -> Fut,
    Fut: std::future::Future<Output = Result<bool, ApiError>>,
{
    let _admission = services::services::integration_admission::MUTATIONS
        .lock()
        .await;
    let _issue = lock_issue(issue).await;
    let confirmation = {
        let mut tokens = CONFIRMATIONS.lock().await;
        if !tokens.get(&request.confirmation_token).is_some_and(|c| {
            c.scope == scope
                && c.issue_id == issue
                && c.project_id == request.project_id
                && c.expires > Instant::now()
        }) {
            return Err(failure(
                "Confirmation expired, used or belongs to another caller/target; confirm again",
            ));
        }
        tokens
            .remove(&request.confirmation_token)
            .expect("validated token")
    };
    let mut guards = Vec::new();
    let mut workspaces = Vec::new();
    if confirmation.archive_workspaces {
        for target in confirmation.snapshot.targets.iter().filter(|t| !t.archived) {
            guards.push(services::services::workspace_lifecycle::lock(target.workspace_id).await);
            if !settled(target.workspace_id).await? {
                return Err(failure("使用中のWorkspaceはarchiveできません。"));
            }
            let ws = Workspace::find_by_id(pool, target.workspace_id)
                .await?
                .ok_or_else(|| failure("Workspace changed; confirm again"))?;
            let repos = WorkspaceRepo::find_repos_for_workspace(pool, ws.id).await?;
            workspaces.push((ws, repos));
        }
    }
    let mut tx = pool.begin_with("BEGIN IMMEDIATE").await?;
    if snapshot(&mut tx, request.project_id, issue, false).await? != confirmation.snapshot {
        return Err(failure("Workspace links or targets changed; confirm again"));
    }
    // Complete all checks before the first logical or physical mutation.
    for (ws, _) in &workspaces {
        archive::check_idle(&mut tx, ws.id).await?;
        archive_processes::check(ws.container_ref.as_deref()).await?;
    }
    sqlx::query("UPDATE local_issues SET archived=1,updated_at=datetime('now','subsec') WHERE id=? AND project_id=?")
        .bind(issue).bind(request.project_id).execute(&mut *tx).await?;
    let mut outcomes = Vec::new();
    for (ws, repos) in &workspaces {
        // After physical mutation starts, a database error can no longer prove
        // rollback of filesystem effects. Persist that distinction in receipt.
        match archive::archive_checked(&mut tx,ws,repos).await {
            Ok(error) => outcomes.push(json!({"workspace_id":ws.id,"archived":true,"worktree_deleted":ws.can_delete_container_path() && error.is_none(),"deletion_error":error})),
            Err(error) => return Ok(uncertain(error, outcomes)),
        }
    }
    if let Err(error) = tx.commit().await {
        return Ok(uncertain(error, outcomes));
    }
    Ok(json!({"issue_id":issue,"archived":true,"workspaces":outcomes}))
}

fn uncertain(error: impl std::fmt::Display, mut physical_results: Vec<Value>) -> Value {
    for result in &mut physical_results {
        if let Some(object) = result.as_object_mut() {
            object.remove("archived");
        }
    }
    json!({"outcome":"uncertain","message":format!("結果不明。一部の物理削除が完了した可能性があります。論理アーカイブの確定も不明です。自動再実行せず対象とreceiptを確認してください: {error}"),"physical_results":physical_results})
}

pub(crate) async fn unarchive(
    pool: &SqlitePool,
    project: Uuid,
    issue: Uuid,
) -> Result<Value, ApiError> {
    let result = sqlx::query("UPDATE local_issues SET archived=0,updated_at=datetime('now','subsec') WHERE id=? AND project_id=?")
        .bind(issue).bind(project).execute(pool).await?;
    if result.rows_affected() != 1 {
        return Err(failure("Issue not found in Project"));
    }
    Ok(json!({"issue_id":issue,"archived":false}))
}
pub(crate) fn outcome(result: Result<Value, ApiError>) -> (&'static str, Value) {
    match result {
        Ok(value) if value.get("outcome").and_then(Value::as_str) == Some("uncertain") => {
            ("uncertain", value)
        }
        Ok(value) => ("succeeded", value),
        Err(error) => ("failed", json!({"message":error.to_string()})),
    }
}

async fn prepare(
    State(d): State<DeploymentImpl>,
    Path(issue): Path<Uuid>,
    Json(request): Json<Prepare>,
) -> Result<Json<ApiResponse<Value>>, ApiError> {
    Ok(Json(ApiResponse::success(
        prepare_scoped(
            &d.db().pool,
            issue,
            request,
            ConfirmationScope::Local,
            |id| {
                let d = d.clone();
                async move { Ok(d.container().scripts_settled(id).await?) }
            },
        )
        .await?,
    )))
}
async fn confirm(
    State(d): State<DeploymentImpl>,
    Path(issue): Path<Uuid>,
    Json(request): Json<Confirm>,
) -> Result<Json<ApiResponse<Receipt>>, ApiError> {
    let store = local_operations(&d.db().pool);
    let input = json!({"issue_id":issue,"request":request});
    let receipt = store
        .submit_and_wait(
            local_principal(),
            request.request_id,
            request.project_id,
            "confirm_issue_archive".into(),
            input,
            json!({"issue_id":issue}),
            move |_| async move {
                outcome(
                    archive_scoped(
                        &d.db().pool,
                        issue,
                        &request,
                        ConfirmationScope::Local,
                        |id| {
                            let d = d.clone();
                            async move { Ok(d.container().scripts_settled(id).await?) }
                        },
                    )
                    .await,
                )
            },
        )
        .await
        .map_err(failure)?;
    Ok(Json(ApiResponse::success(receipt)))
}
async fn restore(
    State(d): State<DeploymentImpl>,
    Path(issue): Path<Uuid>,
    Json(request): Json<Unarchive>,
) -> Result<Json<ApiResponse<Receipt>>, ApiError> {
    let store = local_operations(&d.db().pool);
    let input = json!({"issue_id":issue,"project_id":request.project_id});
    let receipt =
        store
            .submit_and_wait(
                local_principal(),
                request.request_id,
                request.project_id,
                "unarchive_issue".into(),
                input,
                json!({"issue_id":issue}),
                move |_| async move {
                    outcome(unarchive(&d.db().pool, request.project_id, issue).await)
                },
            )
            .await
            .map_err(failure)?;
    Ok(Json(ApiResponse::success(receipt)))
}
async fn receipt(
    State(d): State<DeploymentImpl>,
    Path(request): Path<Uuid>,
) -> Result<Json<ApiResponse<Receipt>>, ApiError> {
    Ok(Json(ApiResponse::success(
        local_operations(&d.db().pool)
            .by_request(&local_principal(), request)
            .await
            .map_err(failure)?,
    )))
}
pub(crate) fn router() -> Router<DeploymentImpl> {
    Router::new()
        .route("/issues/{issue_id}/archive/prepare", post(prepare))
        .route("/issues/{issue_id}/archive/confirm", post(confirm))
        .route("/issues/{issue_id}/unarchive", post(restore))
        .route("/issue-archive-operations/{request_id}", get(receipt))
}

#[cfg(test)]
mod tests {
    use db::models::workspace::CreateWorkspace;

    use super::*;
    use crate::routes::workspaces::archive::tests::fixture;

    async fn issue(pool: &SqlitePool, status_index: usize) -> (Uuid, Uuid, Uuid) {
        let project = Uuid::new_v4();
        sqlx::query("INSERT INTO projects(id,name) VALUES(?,'Issue archive fixture')")
            .bind(project)
            .execute(pool)
            .await
            .unwrap();
        let statuses = super::super::ensure_default_statuses(pool, project)
            .await
            .unwrap();
        let id = Uuid::new_v4();
        let status = statuses[status_index].id;
        sqlx::query("INSERT INTO local_issues(id,project_id,issue_number,simple_id,status_id,title,sort_order) VALUES(?,?,1,'LOCAL-1',?,'archive fixture',0)").bind(id).bind(project).bind(status).execute(pool).await.unwrap();
        (project, id, status)
    }
    async fn link(pool: &SqlitePool, ws: Uuid, project: Uuid, id: Uuid) {
        sqlx::query(
            "INSERT INTO local_workspace_links(workspace_id,project_id,issue_id) VALUES(?,?,?)",
        )
        .bind(ws)
        .bind(project)
        .bind(id)
        .execute(pool)
        .await
        .unwrap();
    }
    async fn prepare(pool: &SqlitePool, project: Uuid, id: Uuid, children: bool) -> Confirm {
        let prepared = prepare_scoped(
            pool,
            id,
            Prepare {
                project_id: project,
                archive_workspaces: children,
            },
            ConfirmationScope::Local,
            |_| async { Ok(true) },
        )
        .await
        .unwrap();
        Confirm {
            project_id: project,
            confirmation_token: serde_json::from_value(prepared["confirmation_token"].clone())
                .unwrap(),
            request_id: Uuid::new_v4(),
        }
    }
    async fn assert_issue(pool: &SqlitePool, id: Uuid, status: Uuid, archived: bool) {
        let value = super::super::get_local_issue(pool, id).await.unwrap();
        assert_eq!(value.status_id, status);
        assert_eq!(value.archived, archived);
    }
    #[tokio::test]
    async fn issue_archive_zero_targets_all_statuses_preserve_status_and_unarchive_only_issue() {
        let (_root, pool, ws, _repo, _session) = fixture().await;
        for index in [0, 3, 4] {
            let (project, id, status) = issue(&pool, index).await;
            assert_issue(&pool, id, status, false).await;
            let request = prepare(&pool, project, id, true).await;
            let result = archive_scoped(&pool, id, &request, ConfirmationScope::Local, |_| async {
                Ok(true)
            })
            .await
            .unwrap();
            assert_eq!(result["workspaces"], json!([]));
            assert_issue(&pool, id, status, true).await;
            unarchive(&pool, project, id).await.unwrap();
            assert_issue(&pool, id, status, false).await;
        }
        assert!(
            !Workspace::find_by_id(&pool, ws.id)
                .await
                .unwrap()
                .unwrap()
                .archived
        );
        assert!(std::path::Path::new(ws.container_ref.as_ref().unwrap()).exists());
    }
    #[tokio::test]
    async fn issue_archive_checkbox_off_allows_busy_workspace_and_retains_everything() {
        let (_root, pool, ws, _repo, session) = fixture().await;
        let (project, id, status) = issue(&pool, 0).await;
        link(&pool, ws.id, project, id).await;
        sqlx::query("INSERT INTO execution_processes(id,session_id,run_reason,status) VALUES(?,?,'devserver','running')").bind(Uuid::new_v4()).bind(session.id).execute(&pool).await.unwrap();
        assert!(
            prepare_scoped(
                &pool,
                id,
                Prepare {
                    project_id: project,
                    archive_workspaces: true
                },
                ConfirmationScope::Local,
                |_| async { Ok(true) }
            )
            .await
            .is_err()
        );
        let request = prepare(&pool, project, id, false).await;
        archive_scoped(&pool, id, &request, ConfirmationScope::Local, |_| async {
            panic!("OFF must not inspect scripts")
        })
        .await
        .unwrap();
        assert_issue(&pool, id, status, true).await;
        assert!(
            !Workspace::find_by_id(&pool, ws.id)
                .await
                .unwrap()
                .unwrap()
                .archived
        );
        assert!(std::path::Path::new(ws.container_ref.as_ref().unwrap()).exists());
        // Later workflow transitions remain writable without unarchiving Issue.
        let statuses = super::super::ensure_default_statuses(&pool, project)
            .await
            .unwrap();
        sqlx::query("UPDATE local_issues SET status_id=? WHERE id=?")
            .bind(statuses[1].id)
            .bind(id)
            .execute(&pool)
            .await
            .unwrap();
        assert_issue(&pool, id, statuses[1].id, true).await;
    }
    #[tokio::test]
    async fn issue_archive_multiple_targets_busy_one_rejects_all_before_mutation() {
        let (_root, pool, ws, _repo, _session) = fixture().await;
        let (project, id, status) = issue(&pool, 3).await;
        link(&pool, ws.id, project, id).await;
        let second = Workspace::create(
            &pool,
            &CreateWorkspace {
                branch: "second".into(),
                name: None,
            },
            Uuid::from_u128(u128::MAX),
        )
        .await
        .unwrap();
        link(&pool, second.id, project, id).await;
        let request = prepare(&pool, project, id, true).await;
        assert!(
            archive_scoped(
                &pool,
                id,
                &request,
                ConfirmationScope::Local,
                |target| async move { Ok(target != second.id) }
            )
            .await
            .is_err()
        );
        assert_issue(&pool, id, status, false).await;
        assert!(
            !Workspace::find_by_id(&pool, ws.id)
                .await
                .unwrap()
                .unwrap()
                .archived
        );
        assert!(
            !Workspace::find_by_id(&pool, second.id)
                .await
                .unwrap()
                .unwrap()
                .archived
        );
        assert!(std::path::Path::new(ws.container_ref.as_ref().unwrap()).exists());
        let request = prepare(&pool, project, id, true).await;
        let result = archive_scoped(&pool, id, &request, ConfirmationScope::Local, |_| async {
            Ok(true)
        })
        .await
        .unwrap();
        assert_eq!(result["workspaces"].as_array().unwrap().len(), 2);
        assert_issue(&pool, id, status, true).await;
        unarchive(&pool, project, id).await.unwrap();
        assert!(
            Workspace::find_by_id(&pool, ws.id)
                .await
                .unwrap()
                .unwrap()
                .archived
        );
        assert!(!std::path::Path::new(ws.container_ref.as_ref().unwrap()).exists());
    }
    #[tokio::test]
    async fn issue_archive_link_aba_and_changed_path_require_confirmation_again() {
        let (_root, pool, ws, _repo, _session) = fixture().await;
        let (project, id, status) = issue(&pool, 0).await;
        link(&pool, ws.id, project, id).await;
        let request = prepare(&pool, project, id, true).await;
        sqlx::query("DELETE FROM local_workspace_links WHERE workspace_id=?")
            .bind(ws.id)
            .execute(&pool)
            .await
            .unwrap();
        link(&pool, ws.id, project, id).await;
        assert!(
            archive_scoped(&pool, id, &request, ConfirmationScope::Local, |_| async {
                Ok(true)
            })
            .await
            .is_err()
        );
        let request = prepare(&pool, project, id, true).await;
        sqlx::query("UPDATE workspaces SET branch='changed' WHERE id=?")
            .bind(ws.id)
            .execute(&pool)
            .await
            .unwrap();
        assert!(
            archive_scoped(&pool, id, &request, ConfirmationScope::Local, |_| async {
                Ok(true)
            })
            .await
            .is_err()
        );
        assert_issue(&pool, id, status, false).await;
        assert!(std::path::Path::new(ws.container_ref.as_ref().unwrap()).exists());
    }
    #[tokio::test]
    async fn issue_archive_normal_deletion_failure_commits_but_database_failure_after_cleanup_is_uncertain()
     {
        for crash in [false, true] {
            let (root, pool, ws, _repo, _session) = fixture().await;
            let (project, id, status) = issue(&pool, 0).await;
            link(&pool, ws.id, project, id).await;
            if crash {
                let second = Workspace::create(
                    &pool,
                    &CreateWorkspace {
                        branch: "second".into(),
                        name: None,
                    },
                    Uuid::from_u128(u128::MAX),
                )
                .await
                .unwrap();
                link(&pool, second.id, project, id).await;
                sqlx::query("CREATE TRIGGER injected_crash BEFORE UPDATE OF archived ON workspaces WHEN NEW.id=x'ffffffffffffffffffffffffffffffff' BEGIN SELECT RAISE(ABORT,'injected database failure'); END").execute(&pool).await.unwrap();
            } else {
                let invalid = root.path().join("not-directory");
                std::fs::write(&invalid, "retain").unwrap();
                Workspace::update_container_ref(&pool, ws.id, invalid.to_str().unwrap())
                    .await
                    .unwrap();
            }
            let request = prepare(&pool, project, id, true).await;
            let value = archive_scoped(&pool, id, &request, ConfirmationScope::Local, |_| async {
                Ok(true)
            })
            .await
            .unwrap();
            if crash {
                assert_eq!(value["outcome"], "uncertain");
                assert_issue(&pool, id, status, false).await;
                assert!(!std::path::Path::new(ws.container_ref.as_ref().unwrap()).exists());
            } else {
                assert_issue(&pool, id, status, true).await;
                assert!(
                    value["workspaces"][0]["deletion_error"]
                        .as_str()
                        .unwrap()
                        .contains("物理削除に失敗")
                );
                assert!(
                    !Workspace::find_by_id(&pool, ws.id)
                        .await
                        .unwrap()
                        .unwrap()
                        .worktree_deleted
                );
            }
        }
    }
    #[tokio::test]
    async fn issue_archive_refuses_new_links_preserves_existing_links_and_checks_token_scope() {
        let (_root, pool, ws, _repo, _session) = fixture().await;
        let (project, id, status) = issue(&pool, 0).await;
        link(&pool, ws.id, project, id).await;
        let request = prepare(&pool, project, id, false).await;
        assert!(
            archive_scoped(
                &pool,
                id,
                &request,
                ConfirmationScope::Remote {
                    principal: "foreign".into(),
                    project_id: project
                },
                |_| async { Ok(true) }
            )
            .await
            .is_err()
        );
        archive_scoped(&pool, id, &request, ConfirmationScope::Local, |_| async {
            Ok(true)
        })
        .await
        .unwrap();
        assert!(require_unarchived(&pool, project, id).await.is_err());
        let second = Workspace::create(
            &pool,
            &CreateWorkspace {
                branch: "second".into(),
                name: None,
            },
            Uuid::new_v4(),
        )
        .await
        .unwrap();
        assert!(
            sqlx::query(
                "INSERT INTO local_workspace_links(workspace_id,project_id,issue_id) VALUES(?,?,?)"
            )
            .bind(second.id)
            .bind(project)
            .bind(id)
            .execute(&pool)
            .await
            .is_err()
        );
        // A workflow reusing its existing link is allowed; no automatic unarchive.
        sqlx::query("INSERT INTO local_workspace_links(workspace_id,project_id,issue_id) VALUES(?,?,?) ON CONFLICT(workspace_id) DO UPDATE SET issue_id=excluded.issue_id").bind(ws.id).bind(project).bind(id).execute(&pool).await.unwrap();
        assert_issue(&pool, id, status, true).await;
        let row =
            sqlx::query("SELECT count(*) AS n FROM local_workspace_links WHERE workspace_id=?")
                .bind(ws.id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(row.get::<i64, _>("n"), 1);
    }
}
