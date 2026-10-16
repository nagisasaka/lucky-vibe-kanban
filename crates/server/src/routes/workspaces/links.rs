use axum::{
    Extension, Json, Router,
    extract::{Path as AxumPath, State},
    middleware::from_fn_with_state,
    response::Json as ResponseJson,
    routing::{delete, post},
};
use db::models::workspace::Workspace;
use deployment::Deployment;
use serde::Deserialize;
use utils::response::ApiResponse;
use uuid::Uuid;

use crate::{DeploymentImpl, error::ApiError, middleware::load_workspace_middleware};

#[derive(Debug, Deserialize)]
pub struct LinkWorkspaceRequest {
    pub project_id: Uuid,
    pub issue_id: Uuid,
}

async fn upsert_local_workspace_link(
    pool: &sqlx::SqlitePool,
    workspace_id: Uuid,
    project_id: Uuid,
    issue_id: Uuid,
) -> Result<(), ApiError> {
    sqlx::query(
        r#"
        INSERT INTO local_workspace_links
            (workspace_id, project_id, issue_id)
        VALUES (?, ?, ?)
        ON CONFLICT(workspace_id) DO UPDATE SET
            project_id = excluded.project_id,
            issue_id = excluded.issue_id,
            updated_at = datetime('now', 'subsec')
        "#,
    )
    .bind(workspace_id)
    .bind(project_id)
    .bind(issue_id)
    .execute(pool)
    .await?;

    Ok(())
}

async fn delete_local_workspace_link(
    pool: &sqlx::SqlitePool,
    workspace_id: Uuid,
) -> Result<(), ApiError> {
    sqlx::query("DELETE FROM local_workspace_links WHERE workspace_id = ?")
        .bind(workspace_id)
        .execute(pool)
        .await?;

    Ok(())
}

pub(crate) async fn link_workspace_to_issue(
    deployment: &DeploymentImpl,
    workspace: &Workspace,
    project_id: Uuid,
    issue_id: Uuid,
) -> Result<(), ApiError> {
    workspace.require_interactive()?;
    upsert_local_workspace_link(&deployment.db().pool, workspace.id, project_id, issue_id).await?;

    Ok(())
}

pub async fn link_workspace(
    Extension(workspace): Extension<Workspace>,
    State(deployment): State<DeploymentImpl>,
    Json(payload): Json<LinkWorkspaceRequest>,
) -> Result<ResponseJson<ApiResponse<()>>, ApiError> {
    link_workspace_to_issue(
        &deployment,
        &workspace,
        payload.project_id,
        payload.issue_id,
    )
    .await?;

    Ok(ResponseJson(ApiResponse::success(())))
}

pub async fn unlink_workspace(
    AxumPath(workspace_id): AxumPath<uuid::Uuid>,
    State(deployment): State<DeploymentImpl>,
) -> Result<ResponseJson<ApiResponse<()>>, ApiError> {
    // Unlike POST, this idempotent route also accepts already-deleted records.
    // Check an existing workspace before removing its local link.
    if let Some(workspace) = Workspace::find_by_id(&deployment.db().pool, workspace_id).await? {
        workspace.require_interactive()?;
        db::models::integration::guard_workspace(&deployment.db().pool, workspace_id).await?;
    }

    delete_local_workspace_link(&deployment.db().pool, workspace_id).await?;

    Ok(ResponseJson(ApiResponse::success(())))
}

pub fn router(deployment: &DeploymentImpl) -> Router<DeploymentImpl> {
    let post_router = Router::new()
        .route("/", post(link_workspace))
        .layer(from_fn_with_state(
            deployment.clone(),
            load_workspace_middleware,
        ));

    let delete_router = Router::new().route("/", delete(unlink_workspace));

    post_router.merge(delete_router)
}

#[cfg(test)]
mod tests {
    use sqlx::{Row, SqlitePool, sqlite::SqlitePoolOptions};
    use uuid::Uuid;

    use super::{delete_local_workspace_link, upsert_local_workspace_link};

    async fn setup_pool() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("connect sqlite");

        sqlx::query(
            r#"
            CREATE TABLE local_workspace_links (
                workspace_id BLOB PRIMARY KEY,
                project_id BLOB NOT NULL,
                issue_id BLOB NOT NULL,
                created_at TEXT NOT NULL DEFAULT (datetime('now', 'subsec')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now', 'subsec'))
            )
            "#,
        )
        .execute(&pool)
        .await
        .expect("create local_workspace_links");

        pool
    }

    #[tokio::test]
    async fn upsert_local_workspace_link_inserts_and_updates() {
        let pool = setup_pool().await;
        let workspace_id = Uuid::new_v4();
        let project_id = Uuid::new_v4();
        let first_issue_id = Uuid::new_v4();
        let second_issue_id = Uuid::new_v4();

        upsert_local_workspace_link(&pool, workspace_id, project_id, first_issue_id)
            .await
            .expect("insert link");
        upsert_local_workspace_link(&pool, workspace_id, project_id, second_issue_id)
            .await
            .expect("update link");

        let row = sqlx::query(
            "SELECT project_id, issue_id FROM local_workspace_links WHERE workspace_id = ?",
        )
        .bind(workspace_id)
        .fetch_one(&pool)
        .await
        .expect("fetch link");

        let linked_project_id: Uuid = row.try_get("project_id").expect("project_id");
        let linked_issue_id: Uuid = row.try_get("issue_id").expect("issue_id");

        assert_eq!(linked_project_id, project_id);
        assert_eq!(linked_issue_id, second_issue_id);
    }

    #[tokio::test]
    async fn delete_local_workspace_link_removes_link() {
        let pool = setup_pool().await;
        let workspace_id = Uuid::new_v4();

        upsert_local_workspace_link(&pool, workspace_id, Uuid::new_v4(), Uuid::new_v4())
            .await
            .expect("insert link");
        delete_local_workspace_link(&pool, workspace_id)
            .await
            .expect("delete link");

        let count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM local_workspace_links WHERE workspace_id = ?")
                .bind(workspace_id)
                .fetch_one(&pool)
                .await
                .expect("count links");

        assert_eq!(count, 0);
    }
}
