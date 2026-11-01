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

// New creation holds this guard through initial run/setup admission, or the
// workflow run/ready bind. Existing Workspace continuation does not take it.
// Archive order: MUTATIONS -> Issue -> sorted WS -> DB.
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
    let mut physical_effects_possible = false;
    for (ws, repos) in &workspaces {
        // After physical mutation starts, a database error can no longer prove
        // rollback of filesystem effects. Persist that distinction in receipt.
        match archive::archive_checked_observed(&mut tx,ws,repos,&mut physical_effects_possible).await {
            Ok(error) => outcomes.push(json!({"workspace_id":ws.id,"archived":true,"worktree_deleted":ws.can_delete_container_path() && error.is_none(),"deletion_error":error})),
            Err(error) if physical_effects_possible => {
                outcomes.push(json!({"workspace_id":ws.id,"physical_deletion_may_have_occurred":ws.can_delete_container_path()}));
                return Ok(uncertain(error, outcomes));
            }
            Err(error) => return Err(error),
        }
    }
    if let Err(error) = tx.commit().await {
        if !physical_effects_possible && db::models::todo::commit_definitely_rejected(&error) {
            return Err(error.into());
        }
        return Ok(uncertain(error, outcomes));
    }
    Ok(json!({"issue_id":issue,"archived":true,"workspaces":outcomes}))
}

fn uncertain(_error: impl std::fmt::Display, mut physical_results: Vec<Value>) -> Value {
    for result in &mut physical_results {
        if let Some(object) = result.as_object_mut() {
            object.remove("archived");
        }
    }
    let diagnostic = utils::mcp_error::Diagnostic::new(
        utils::mcp_error::Category::OutcomeUnknown,
        utils::mcp_error::Outcome::Unknown,
    );
    json!({"outcome":"uncertain","message":"結果不明。対象の変更が完了した可能性があります。自動再実行せず対象の状態とreceiptを確認してください。","error":diagnostic.value(),"physical_results":physical_results})
}

pub(crate) async fn unarchive(
    pool: &SqlitePool,
    project: Uuid,
    issue: Uuid,
) -> Result<Value, ApiError> {
    let mut tx = pool.begin_with("BEGIN IMMEDIATE").await?;
    let result = sqlx::query("UPDATE local_issues SET archived=0,updated_at=datetime('now','subsec') WHERE id=? AND project_id=?")
        .bind(issue).bind(project).execute(&mut *tx).await?;
    if result.rows_affected() != 1 {
        return Err(failure("Issue not found in Project"));
    }
    if let Err(error) = tx.commit().await {
        if db::models::todo::commit_definitely_rejected(&error) {
            return Err(error.into());
        }
        return Ok(uncertain(error, vec![]));
    }
    Ok(json!({"issue_id":issue,"archived":false}))
}
pub(crate) fn outcome(result: Result<Value, ApiError>) -> (&'static str, Value) {
    match result {
        Ok(value) if value.get("outcome").and_then(Value::as_str) == Some("uncertain") => {
            ("uncertain", value)
        }
        Ok(value) => ("succeeded", value),
        Err(error) => {
            let diagnostic = archive::archive_rejection(&error);
            (
                "failed",
                json!({"message":diagnostic.message,"error":diagnostic.value()}),
            )
        }
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

/// One best-effort attempt at the existing owner-release boundary. This is an
/// internal authority, independent of the human confirmation-token protocol.
pub(crate) async fn archive_generated(deployment: &DeploymentImpl, released: Workspace) {
    let pool = &deployment.db().pool;
    let result = archive_generated_for_pool(pool, &released, || async {
        Ok(deployment.container().scripts_settled(released.id).await?)
    })
    .await;
    if let Err(error) = result {
        tracing::warn!(workspace_id=%released.id, %error, "Generated archive deferred; product success retained");
    }
}

async fn archive_generated_for_pool<F, Fut>(
    pool: &SqlitePool,
    released: &Workspace,
    settled: F,
) -> Result<(), ApiError>
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = Result<bool, ApiError>>,
{
    let Some((issue, project, kind, repo, run_id, revision)) = sqlx::query_as::<_, (Uuid, Uuid, String, Uuid, Option<String>, i64)>(
        "SELECT issue_id,project_id,owner_kind,repository_id,owner_run_id,eligibility_revision FROM generated_issue_workspaces WHERE workspace_id=? AND attempt_result IS NULL"
    ).bind(released.id).fetch_optional(pool).await? else { return Ok(()); };
    let _admission = services::services::integration_admission::MUTATIONS
        .lock()
        .await;
    let _issue = lock_issue(issue).await;
    let _workspace = services::services::workspace_lifecycle::lock(released.id).await;
    // Persist before touching the filesystem. Interrupted or uncertain outcomes
    // are deliberately never retried by a duplicate completion event.
    let claimed = sqlx::query("UPDATE generated_issue_workspaces SET attempt_result='attempt started; outcome may be uncertain' WHERE workspace_id=? AND attempt_result IS NULL AND eligibility_revision=?")
        .bind(released.id).bind(revision).execute(pool).await?;
    if claimed.rows_affected() == 0 {
        return Ok(());
    }
    let result = async {
        if !settled().await? { return Err(failure("Workspace scripts still active")); }
        // BEGIN IMMEDIATE excludes concurrent DB writers until all final
        // eligibility/owner/link checks and physical mutation have finished.
        let repos = WorkspaceRepo::find_repos_for_workspace(pool, released.id).await?;
        let mut tx = pool.begin_with("BEGIN IMMEDIATE").await?;
        let eligible: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM generated_issue_workspaces WHERE workspace_id=? AND eligibility_revision=? AND attempt_result='attempt started; outcome may be uncertain')")
            .bind(released.id).bind(revision).fetch_one(&mut *tx).await?;
        if !eligible { return Err(failure("Generated archive eligibility invalidated after claim")); }
        let ws = sqlx::query_as::<_, Workspace>("SELECT * FROM workspaces WHERE id=?")
            .bind(released.id).fetch_optional(&mut *tx).await?.ok_or_else(|| failure("Workspace missing"))?;
        if ws.archived || ws.is_execution_only() || ws.execution_owner != released.execution_owner
            || ws.container_ref != released.container_ref || ws.branch != released.branch {
            return Err(failure("Released Workspace changed"));
        }
        let owner = ws.execution_owner.as_ref().ok_or_else(|| failure("Owner missing"))?;
        if owner.kind != kind || owner.repository_id != Some(repo) || owner.run_id.is_none() || owner.run_id.map(|id| id.to_string()) != run_id {
            return Err(failure("Generated owner mismatch"));
        }
        let current_repos: Vec<(Uuid, String, String)> = sqlx::query_as("SELECT r.id,r.path,r.name FROM repos r JOIN workspace_repos wr ON wr.repo_id=r.id WHERE wr.workspace_id=?")
            .bind(ws.id).fetch_all(&mut *tx).await?;
        if repos.len() != 1 || repos[0].id != repo || current_repos.len() != 1
            || current_repos[0].0 != repo || current_repos[0].1.as_str() != repos[0].path.to_string_lossy().as_ref()
            || current_repos[0].2 != repos[0].name { return Err(failure("Generated repository mismatch")); }
        let linked: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM local_workspace_links l JOIN local_issues i ON i.id=l.issue_id AND i.project_id=l.project_id WHERE l.workspace_id=? AND l.issue_id=? AND l.project_id=? AND i.archived=0)")
            .bind(ws.id).bind(issue).bind(project).fetch_one(&mut *tx).await?;
        if !linked { return Err(failure("Generated Issue link changed")); }
        use db::models::workspace_usage::{INTEGRATION, OPENWIKI_BOOTSTRAP, OPENWIKI_SYNC, WorkspaceExecutionTerminalStatus};
        let successful: bool = match kind.as_str() {
            INTEGRATION => sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM integration_runs WHERE id=? AND workspace_id=? AND repository_id=? AND project_id=? AND status='succeeded' AND json_extract(payload,'$.published')=1 AND json_extract(payload,'$.card_id')=?)")
                .bind(owner.run_id).bind(ws.id).bind(repo).bind(project).bind(issue.to_string()).fetch_one(&mut *tx).await?,
            OPENWIKI_SYNC => {
                let run_matches: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM agent_runs WHERE id=? AND workspace_id=? AND status='succeeded')")
                    .bind(owner.run_id).bind(ws.id).fetch_one(&mut *tx).await?;
                run_matches && owner.result.as_ref().is_some_and(|r| r.status == WorkspaceExecutionTerminalStatus::Succeeded)
            }
            OPENWIKI_BOOTSTRAP => sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM workflow_runs WHERE id=? AND workspace_id=? AND repository_id=? AND trigger_source='openwiki_bootstrap' AND status='succeeded')")
                .bind(owner.run_id).bind(ws.id).bind(repo).fetch_one(&mut *tx).await?,
            _ => false,
        };
        if !successful { return Err(failure("Generated product has no matching success")); }
        let cancelled: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM local_issues i JOIN local_project_statuses s ON s.id=i.status_id WHERE i.id=? AND lower(s.name) IN ('cancelled','canceled'))")
            .bind(issue).fetch_one(&mut *tx).await?;
        if cancelled { return Err(failure("Issue was cancelled by operator; automatic archive refused")); }
        let status_pending: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM generated_issue_status t JOIN local_issues i ON i.id=? JOIN local_project_statuses s ON s.id=i.status_id WHERE t.workspace_id=? AND (t.projected_status IS NOT 'done' OR lower(s.name)!='done'))")
            .bind(issue).bind(ws.id).fetch_one(&mut *tx).await?;
        if status_pending { return Err(failure("Generated Issue has not reached Done; automatic archive refused")); }
        archive::check_idle(&mut tx, ws.id).await?;
        archive_processes::check(ws.container_ref.as_deref()).await?;
        let deletion_error = archive::archive_checked(&mut tx, &ws, &repos).await?;
        // Other linked Workspaces are never archived by this completion event.
        let issue_archived = sqlx::query("UPDATE local_issues SET archived=1,updated_at=datetime('now','subsec') WHERE id=? AND project_id=? AND NOT EXISTS(SELECT 1 FROM local_workspace_links l JOIN workspaces w ON w.id=l.workspace_id WHERE l.issue_id=? AND w.archived=0)")
            .bind(issue).bind(project).bind(issue).execute(&mut *tx).await?.rows_affected() == 1;
        tx.commit().await?;
        Ok::<_, ApiError>(json!({"workspace_id":ws.id,"archived":true,"worktree_deleted":ws.can_delete_container_path() && deletion_error.is_none(),"issue_id":issue,"issue_archived":issue_archived,"deletion_error":deletion_error}))
    }.await;
    let detail = match &result {
        Ok(value) => value.to_string(),
        // These two domain facts are static operator decisions, not arbitrary
        // ApiError prose. Keep them visible without forwarding error chains.
        Err(ApiError::Conflict(message))
            if matches!(
                message.as_str(),
                "Issue was cancelled by operator; automatic archive refused"
                    | "Generated Issue has not reached Done; automatic archive refused"
            ) =>
        {
            message.clone()
        }
        Err(_) => "archive refused or uncertain; inspect target state".to_owned(),
    };
    match &result {
        Ok(value) if value["deletion_error"].is_null() => {
            tracing::info!(workspace_id=%released.id, issue_id=%issue, result=%detail, "Generated archive attempt finished")
        }
        _ => {
            tracing::warn!(workspace_id=%released.id, issue_id=%issue, result=%detail, "Generated archive refused, uncertain or deletion incomplete")
        }
    }
    sqlx::query("UPDATE generated_issue_workspaces SET attempt_result=? WHERE workspace_id=?")
        .bind(detail)
        .bind(released.id)
        .execute(pool)
        .await?;
    result.map(|_| ())
}

#[cfg(test)]
mod tests {
    use db::models::workspace::CreateWorkspace;

    use super::*;
    use crate::routes::workspaces::archive::tests::fixture;

    async fn issue(pool: &SqlitePool, status_index: usize) -> (Uuid, Uuid, Uuid) {
        let project = Uuid::new_v4();
        sqlx::query("INSERT INTO projects(id,name,key) SELECT ?,'Issue archive fixture','ARC'||(count(*)+1) FROM projects")
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
    async fn generated_fixture() -> (tempfile::TempDir, SqlitePool, Workspace, Uuid, Uuid, Uuid) {
        let (root, pool, ws, repo, _) = fixture().await;
        let (project, id, _) = issue(&pool, 0).await;
        link(&pool, ws.id, project, id).await;
        let run = Uuid::new_v4();
        let owner = db::models::workspace_usage::WorkspaceExecutionOwner::new(
            "integration",
            repo.id,
            Some(run),
        );
        sqlx::query("UPDATE workspaces SET execution_owner=? WHERE id=?")
            .bind(sqlx::types::Json(&owner))
            .bind(ws.id)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO integration_runs(id,request_key,project_id,repository_id,storage_identity,target_ref,status,workspace_id,payload) VALUES(?,?,?,?,?,'refs/heads/main','succeeded',?,?)")
            .bind(run).bind(run.to_string()).bind(project).bind(repo.id).bind(run.to_string()).bind(ws.id)
            .bind(serde_json::to_string(&db::models::integration::IntegrationPayload { published: true, card_id: Some(id), ..Default::default() }).unwrap()).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO generated_issue_workspaces(workspace_id,issue_id,project_id,owner_kind,repository_id,owner_run_id) VALUES(?,?,?,'integration',?,?)")
            .bind(ws.id).bind(id).bind(project).bind(repo.id).bind(run.to_string()).execute(&pool).await.unwrap();
        let status: Uuid = sqlx::query_scalar("SELECT i.status_id FROM local_issues i JOIN local_project_statuses s ON s.id=i.status_id WHERE i.id=? AND s.name='Done'")
            .bind(id).fetch_one(&pool).await.unwrap();
        let released = Workspace::find_by_id(&pool, ws.id).await.unwrap().unwrap();
        (root, pool, released, project, id, status)
    }

    #[tokio::test]
    async fn generated_archive_preserves_operator_cancelled_or_reopened_issue() {
        for name in ["Cancelled", "In Review"] {
            let (_root, pool, ws, project, id, _) = generated_fixture().await;
            let status: Uuid = sqlx::query_scalar(
                "SELECT id FROM local_project_statuses WHERE project_id=? AND name=?",
            )
            .bind(project)
            .bind(name)
            .fetch_one(&pool)
            .await
            .unwrap();
            sqlx::query("UPDATE local_issues SET status_id=? WHERE id=?")
                .bind(status)
                .bind(id)
                .execute(&pool)
                .await
                .unwrap();
            assert!(
                archive_generated_for_pool(&pool, &ws, || async { Ok(true) })
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
            let result: String = sqlx::query_scalar(
                "SELECT attempt_result FROM generated_issue_workspaces WHERE workspace_id=?",
            )
            .bind(ws.id)
            .fetch_one(&pool)
            .await
            .unwrap();
            assert!(result.contains(if name == "Cancelled" {
                "cancelled by operator"
            } else {
                "has not reached Done"
            }));
        }
    }

    #[tokio::test]
    async fn generated_archive_runs_only_on_owner_release_transition() {
        let (_root, pool, ws, _, id, status) = generated_fixture().await;
        sqlx::query("UPDATE workspaces SET usage='execution_only' WHERE id=?")
            .bind(ws.id)
            .execute(&pool)
            .await
            .unwrap();
        let released = services::services::workspace_usage::release_completed(&pool)
            .await
            .unwrap();
        assert_eq!(released.len(), 1);
        archive_generated_for_pool(&pool, &released[0], || async { Ok(true) })
            .await
            .unwrap();
        assert_issue(&pool, id, status, true).await;
        assert!(
            services::services::workspace_usage::release_completed(&pool)
                .await
                .unwrap()
                .is_empty()
        );
    }

    #[tokio::test]
    async fn generated_mechanical_success_replay_and_manual_unarchive() {
        let (_root, pool, ws, project, id, status) = generated_fixture().await;
        archive_generated_for_pool(&pool, &ws, || async { Ok(true) })
            .await
            .unwrap();
        assert_issue(&pool, id, status, true).await;
        let archived = Workspace::find_by_id(&pool, ws.id).await.unwrap().unwrap();
        assert!(archived.archived && archived.worktree_deleted);
        // Mechanical publication has no AgentRun; downstream Wiki is irrelevant.
        unarchive(&pool, project, id).await.unwrap();
        Workspace::set_archived(&pool, ws.id, false).await.unwrap();
        archive_generated_for_pool(&pool, &ws, || async {
            panic!("replay must not attempt cleanup")
        })
        .await
        .unwrap();
        assert_issue(&pool, id, status, false).await;
        assert!(
            !Workspace::find_by_id(&pool, ws.id)
                .await
                .unwrap()
                .unwrap()
                .archived
        );
    }

    #[tokio::test]
    async fn generated_bootstrap_archive_requires_matching_successful_workflow() {
        for successful in [false, true] {
            let (_root, pool, ws, _, id, status) = generated_fixture().await;
            sqlx::query("DELETE FROM integration_runs WHERE workspace_id=?")
                .bind(ws.id)
                .execute(&pool)
                .await
                .unwrap();
            let mut owner = ws.execution_owner.clone().unwrap();
            owner.kind = "openwiki_bootstrap".into();
            sqlx::query("UPDATE generated_issue_workspaces SET owner_kind='openwiki_bootstrap'")
                .execute(&pool)
                .await
                .unwrap();
            sqlx::query("UPDATE workspaces SET execution_owner=? WHERE id=?")
                .bind(sqlx::types::Json(&owner))
                .bind(ws.id)
                .execute(&pool)
                .await
                .unwrap();
            let workflow = Uuid::new_v4();
            sqlx::query("INSERT INTO workflows(id,name,source,graph_json) VALUES(?,'fixture','system','{}')")
                .bind(workflow).execute(&pool).await.unwrap();
            sqlx::query("INSERT INTO workflow_runs(id,workflow_id,repository_id,workspace_id,trigger_source,input_text,status) VALUES(?,?,?,?,'openwiki_bootstrap','',?)")
                .bind(owner.run_id).bind(workflow).bind(owner.repository_id).bind(ws.id)
                .bind(if successful { "succeeded" } else { "failed" }).execute(&pool).await.unwrap();
            let released = Workspace::find_by_id(&pool, ws.id).await.unwrap().unwrap();
            let result = archive_generated_for_pool(&pool, &released, || async { Ok(true) }).await;
            assert_eq!(result.is_ok(), successful);
            assert_issue(&pool, id, status, successful).await;
        }
    }

    #[tokio::test]
    async fn generated_wiki_success_and_noop_require_matching_run() {
        for no_op in [false, true] {
            let (_root, pool, ws, _, id, status) = generated_fixture().await;
            // The shared setup creates a mechanical Integration. A Sync
            // fixture must not retain that unrelated dispatch authority.
            sqlx::query("DELETE FROM integration_runs WHERE workspace_id=?")
                .bind(ws.id)
                .execute(&pool)
                .await
                .unwrap();
            let mut owner = ws.execution_owner.clone().unwrap();
            owner.kind = "openwiki_sync".into();
            owner.result = Some(db::models::workspace_usage::WorkspaceExecutionResult {
                status: db::models::workspace_usage::WorkspaceExecutionTerminalStatus::Succeeded,
                completed_at: chrono::Utc::now(),
                error: None,
                wiki_commit: if no_op {
                    None
                } else {
                    Some("published-wiki-commit".into())
                },
                no_op,
            });
            let session: Uuid = sqlx::query_scalar("SELECT id FROM sessions WHERE workspace_id=?")
                .bind(ws.id)
                .fetch_one(&pool)
                .await
                .unwrap();
            let run = owner.run_id.unwrap();
            sqlx::query("INSERT INTO agent_runs(id,session_id,workspace_id,request_id,idempotency_key,correlation_id,schema_version,payload_version,runtime_profile_id,provider_id,workspace_mode,workspace_path,status,request_envelope) VALUES(?,?,?,?,?,?,1,1,'fixture','codex','isolated_worktree','/fixture','succeeded','{}')")
                .bind(run).bind(session).bind(ws.id).bind(Uuid::new_v4()).bind(run.to_string()).bind(run).execute(&pool).await.unwrap();
            sqlx::query("UPDATE workspaces SET execution_owner=? WHERE id=?")
                .bind(sqlx::types::Json(&owner))
                .bind(ws.id)
                .execute(&pool)
                .await
                .unwrap();
            sqlx::query("UPDATE generated_issue_workspaces SET owner_kind='openwiki_sync'")
                .execute(&pool)
                .await
                .unwrap();
            let released = Workspace::find_by_id(&pool, ws.id).await.unwrap().unwrap();
            archive_generated_for_pool(&pool, &released, || async { Ok(true) })
                .await
                .unwrap();
            assert_issue(&pool, id, status, true).await;
        }
    }

    #[tokio::test]
    async fn generated_archive_refuses_busy_owner_mismatch_and_aba() {
        for case in [
            "busy",
            "db_busy",
            "owner",
            "run",
            "repository",
            "aba",
            "manual",
            "ordinary",
            "unpublished",
        ] {
            let (_root, pool, ws, project, id, status) = generated_fixture().await;
            match case {
                "db_busy" => {
                    sqlx::query("INSERT INTO execution_processes(id,session_id,run_reason,status) SELECT ?,id,'devserver','running' FROM sessions WHERE workspace_id=?")
                        .bind(Uuid::new_v4()).bind(ws.id).execute(&pool).await.unwrap();
                }
                "owner" => {
                    sqlx::query("UPDATE workspaces SET execution_owner=json_set(execution_owner,'$.kind','openwiki_sync') WHERE id=?").bind(ws.id).execute(&pool).await.unwrap();
                }
                "run" => {
                    sqlx::query("UPDATE generated_issue_workspaces SET owner_run_id=?")
                        .bind(Uuid::new_v4().to_string())
                        .execute(&pool)
                        .await
                        .unwrap();
                }
                "repository" => {
                    sqlx::query("UPDATE generated_issue_workspaces SET repository_id=?")
                        .bind(Uuid::new_v4())
                        .execute(&pool)
                        .await
                        .unwrap();
                }
                "aba" => {
                    sqlx::query("DELETE FROM local_workspace_links WHERE workspace_id=?")
                        .bind(ws.id)
                        .execute(&pool)
                        .await
                        .unwrap();
                    link(&pool, ws.id, project, id).await;
                }
                "manual" => {
                    sqlx::query("UPDATE local_issues SET archived=1 WHERE id=?")
                        .bind(id)
                        .execute(&pool)
                        .await
                        .unwrap();
                    unarchive(&pool, project, id).await.unwrap();
                }
                "ordinary" => {
                    sqlx::query("DELETE FROM generated_issue_workspaces")
                        .execute(&pool)
                        .await
                        .unwrap();
                }
                "unpublished" => {
                    sqlx::query(
                        "UPDATE integration_runs SET payload=json_set(payload,'$.published',0)",
                    )
                    .execute(&pool)
                    .await
                    .unwrap();
                }
                _ => {}
            }
            let _ = archive_generated_for_pool(&pool, &ws, || async { Ok(case != "busy") }).await;
            assert_issue(&pool, id, status, false).await;
            assert!(
                !Workspace::find_by_id(&pool, ws.id)
                    .await
                    .unwrap()
                    .unwrap()
                    .archived,
                "{case}"
            );
            assert!(std::path::Path::new(ws.container_ref.as_ref().unwrap()).exists());
        }
    }

    #[tokio::test]
    async fn generated_archive_invalidates_intervention_after_claim_before_deletion() {
        for intervention in [
            "unarchive",
            "archive_unarchive",
            "link_aba",
            "owner_aba",
            "path_aba",
        ] {
            let (_root, pool, ws, project, id, status) = generated_fixture().await;
            let result = archive_generated_for_pool(&pool, &ws, || async {
                let started: String = sqlx::query_scalar(
                    "SELECT attempt_result FROM generated_issue_workspaces WHERE workspace_id=?",
                )
                .bind(ws.id)
                .fetch_one(&pool)
                .await
                .unwrap();
                assert_eq!(started, "attempt started; outcome may be uncertain");
                match intervention {
                    "unarchive" => {
                        unarchive(&pool, project, id).await.unwrap();
                    }
                    "archive_unarchive" => {
                        sqlx::query("UPDATE local_issues SET archived=1 WHERE id=?")
                            .bind(id)
                            .execute(&pool)
                            .await
                            .unwrap();
                        unarchive(&pool, project, id).await.unwrap();
                    }
                    "link_aba" => {
                        sqlx::query("DELETE FROM local_workspace_links WHERE workspace_id=?")
                            .bind(ws.id)
                            .execute(&pool)
                            .await
                            .unwrap();
                        link(&pool, ws.id, project, id).await;
                    }
                    "owner_aba" => {
                        sqlx::query("UPDATE workspaces SET execution_owner=NULL WHERE id=?")
                            .bind(ws.id)
                            .execute(&pool)
                            .await
                            .unwrap();
                        sqlx::query("UPDATE workspaces SET execution_owner=? WHERE id=?")
                            .bind(ws.execution_owner.clone())
                            .bind(ws.id)
                            .execute(&pool)
                            .await
                            .unwrap();
                    }
                    "path_aba" => {
                        sqlx::query("UPDATE workspaces SET container_ref=NULL WHERE id=?")
                            .bind(ws.id)
                            .execute(&pool)
                            .await
                            .unwrap();
                        sqlx::query("UPDATE workspaces SET container_ref=? WHERE id=?")
                            .bind(&ws.container_ref)
                            .bind(ws.id)
                            .execute(&pool)
                            .await
                            .unwrap();
                    }
                    _ => unreachable!(),
                }
                Ok(true)
            })
            .await;
            assert!(result.is_err(), "{intervention}");
            assert_issue(&pool, id, status, false).await;
            assert!(
                !Workspace::find_by_id(&pool, ws.id)
                    .await
                    .unwrap()
                    .unwrap()
                    .archived
            );
            assert!(std::path::Path::new(ws.container_ref.as_ref().unwrap()).exists());
            archive_generated_for_pool(&pool, &ws, || async {
                panic!("invalidated attempt must not replay")
            })
            .await
            .unwrap();
        }
    }

    #[tokio::test]
    async fn generated_archive_preserves_other_workspace_and_issue_status() {
        let (_root, pool, ws, project, id, status) = generated_fixture().await;
        let other = Workspace::create(
            &pool,
            &CreateWorkspace {
                branch: "ordinary".into(),
                name: None,
            },
            Uuid::new_v4(),
        )
        .await
        .unwrap();
        link(&pool, other.id, project, id).await;
        archive_generated_for_pool(&pool, &ws, || async { Ok(true) })
            .await
            .unwrap();
        assert_issue(&pool, id, status, false).await;
        assert!(
            Workspace::find_by_id(&pool, ws.id)
                .await
                .unwrap()
                .unwrap()
                .archived
        );
        assert!(
            !Workspace::find_by_id(&pool, other.id)
                .await
                .unwrap()
                .unwrap()
                .archived
        );
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
    #[test]
    fn archive_public_errors_keep_physical_facts_without_source_text() {
        let marker = "SELECT secret FROM /private/database";
        let value = uncertain(
            marker,
            vec![json!({"workspace_id":Uuid::new_v4(),"archived":true,"worktree_deleted":true})],
        );
        assert_eq!(value["error"]["outcome"], "unknown");
        assert_eq!(value["physical_results"][0]["worktree_deleted"], true);
        assert!(value["physical_results"][0].get("archived").is_none());
        assert!(!value.to_string().contains(marker));
        let (status, value) = outcome(Err(ApiError::Conflict(marker.into())));
        assert_eq!(status, "failed");
        assert_eq!(value["error"]["category"], "conflict");
        assert!(!value.to_string().contains(marker));
    }
}
