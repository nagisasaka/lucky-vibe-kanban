//! On-demand repository shared storage operations; no resource-monitor polling.
use axum::{
    Json,
    extract::{Path, State},
};
use deployment::Deployment;
use utils::{
    response::ApiResponse,
    shared_storage::{SharedStorageKind, SharedStorageUsage},
};
use uuid::Uuid;

use crate::{DeploymentImpl, error::ApiError};

// Bound recursive IO and serialize manual deletions against our own measurements.
static IO_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

// This is a point-in-time check, not a lock against later launches or external tools.
// The settings UI asks the user to stop all consumers before confirming deletion.
async fn repository_busy(pool: &sqlx::SqlitePool, repo_id: Uuid) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar::<_, bool>(r#"
        SELECT EXISTS (
            SELECT 1 FROM workspace_repos wr
            JOIN agent_runs ar ON ar.workspace_id = wr.workspace_id
            LEFT JOIN agent_run_state state ON state.agent_run_id = ar.id
            WHERE wr.repo_id = ? AND (
                state.agent_run_id IS NULL OR state.status IN ('pending','starting','running','awaiting_input','awaiting_approval','cancelling')
                OR EXISTS (SELECT 1 FROM agent_run_attempts a
                    JOIN agent_process_registry p ON p.run_attempt_id = a.id
                    WHERE a.agent_run_id = ar.id AND p.registry_status IN ('reserved','spawned','running','unreachable'))
            )
        ) OR EXISTS (
            SELECT 1 FROM workspace_repos wr
            JOIN sessions s ON s.workspace_id = wr.workspace_id
            JOIN execution_processes ep ON ep.session_id = s.id
            WHERE wr.repo_id = ? AND ep.status = 'running'
        )
    "#).bind(repo_id).bind(repo_id).fetch_one(pool).await
}

pub async fn get(
    State(d): State<DeploymentImpl>,
    Path(repo_id): Path<Uuid>,
) -> Result<Json<ApiResponse<SharedStorageUsage>>, ApiError> {
    let repo = d.repo().get_by_id(&d.db().pool, repo_id).await?;
    let root = utils::path::shared_resources_dir(&repo.name, repo.id);
    let guard = IO_LOCK.lock().await;
    let usage = tokio::task::spawn_blocking(move || {
        let _guard = guard;
        utils::shared_storage::usage(&root)
    })
    .await
    .map_err(|e| ApiError::BadRequest(e.to_string()))?;
    Ok(Json(ApiResponse::success(usage)))
}

pub async fn clear(
    State(d): State<DeploymentImpl>,
    Path((repo_id, kind)): Path<(Uuid, SharedStorageKind)>,
) -> Result<Json<ApiResponse<SharedStorageUsage>>, ApiError> {
    let repo = d.repo().get_by_id(&d.db().pool, repo_id).await?;
    let guard = IO_LOCK.lock().await;
    if repository_busy(&d.db().pool, repo_id).await? {
        return Err(ApiError::Conflict("Repository has active or unsettled Runs / processes. Stop them before clearing shared data.".into()));
    }
    let root = utils::path::shared_resources_dir(&repo.name, repo.id);
    let usage = tokio::task::spawn_blocking(move || {
        let _guard = guard;
        utils::shared_storage::clear(&root, kind)?;
        Ok::<_, std::io::Error>(utils::shared_storage::usage(&root))
    })
    .await
    .map_err(|e| ApiError::BadRequest(e.to_string()))??;
    Ok(Json(ApiResponse::success(usage)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn shared_storage_busy_checks_all_workspaces_and_unsettled_processes() {
        let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
        // Minimal isolated schema for the activity query: no live LVK DB is opened.
        sqlx::raw_sql(
            "CREATE TABLE workspace_repos (workspace_id BLOB, repo_id BLOB);
            CREATE TABLE agent_runs (id BLOB, workspace_id BLOB);
            CREATE TABLE agent_run_state (agent_run_id BLOB, status TEXT);
            CREATE TABLE agent_run_attempts (id BLOB, agent_run_id BLOB);
            CREATE TABLE agent_process_registry (run_attempt_id BLOB, registry_status TEXT);
            CREATE TABLE sessions (id BLOB, workspace_id BLOB);
            CREATE TABLE execution_processes (session_id BLOB, status TEXT);",
        )
        .execute(&pool)
        .await
        .unwrap();
        let repo = Uuid::new_v4();
        let other = Uuid::new_v4();
        let workspace = Uuid::new_v4();
        let run = Uuid::new_v4();
        sqlx::query("INSERT INTO workspace_repos VALUES (?, ?)")
            .bind(workspace)
            .bind(repo)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO agent_runs VALUES (?, ?)")
            .bind(run)
            .bind(workspace)
            .execute(&pool)
            .await
            .unwrap();
        for status in [
            "pending",
            "starting",
            "running",
            "awaiting_input",
            "awaiting_approval",
            "cancelling",
            "succeeded",
        ] {
            sqlx::query("DELETE FROM agent_run_state")
                .execute(&pool)
                .await
                .unwrap();
            sqlx::query("INSERT INTO agent_run_state VALUES (?, ?)")
                .bind(run)
                .bind(status)
                .execute(&pool)
                .await
                .unwrap();
            assert_eq!(
                repository_busy(&pool, repo).await.unwrap(),
                status != "succeeded"
            );
            assert!(!repository_busy(&pool, other).await.unwrap());
        }
        sqlx::query("INSERT INTO agent_run_attempts VALUES (?, ?)")
            .bind(run)
            .bind(run)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO agent_process_registry VALUES (?, 'unreachable')")
            .bind(run)
            .execute(&pool)
            .await
            .unwrap();
        assert!(repository_busy(&pool, repo).await.unwrap());
        sqlx::query("DELETE FROM agent_process_registry")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO sessions VALUES (?, ?)")
            .bind(run)
            .bind(workspace)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO execution_processes VALUES (?, 'running')")
            .bind(run)
            .execute(&pool)
            .await
            .unwrap();
        assert!(repository_busy(&pool, repo).await.unwrap());
    }
}

#[cfg(all(test, target_os = "linux"))]
#[path = "shared_storage_http_tests.rs"]
mod http_tests;
