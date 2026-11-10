//! Conversation-owned Auto-merge admission, execution evidence and safe publication.
mod admission;
pub(crate) mod conversation;
mod runtime;
mod support;
pub(crate) use admission::AdmissionFailure;
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    routing::{get, post},
};
use db::models::integration::{IntegrationRun, IntegrationSelection};
use deployment::Deployment;
use executors::profile::ExecutorConfig;
pub(crate) use runtime::reserve_action;
pub use runtime::spawn_monitor;
use serde::Deserialize;
use ts_rs::TS;
use utils::response::ApiResponse;
use uuid::Uuid;

use crate::{DeploymentImpl, error::ApiError};

fn repository_checkout(
    container: &std::path::Path,
    direct_folder: bool,
    repository_path: &std::path::Path,
    repository_name: &str,
) -> std::path::PathBuf {
    // A direct folder is an execution directory, potentially a parent of
    // several repositories. Membership identifies the actual Git checkout.
    if direct_folder {
        repository_path.to_path_buf()
    } else {
        container.join(repository_name)
    }
}

#[derive(Debug, Clone, Deserialize, TS)]
pub struct CreateIntegrationRequest {
    // Stable per explicit UI submission; retries must preserve the selection.
    pub request_id: Uuid,
    pub project_id: Uuid,
    pub repository_id: Uuid,
    pub target_branch: String,
    pub selections: Vec<IntegrationSelection>,
    pub executor_config: Option<ExecutorConfig>,
    #[serde(default)]
    #[ts(optional)]
    pub skip_validation: Option<bool>,
    #[serde(default)]
    #[ts(optional)]
    pub instructions: Option<String>,
}

impl CreateIntegrationRequest {
    pub(crate) fn resolved_skip_validation(&self, server_default: bool) -> bool {
        self.skip_validation.unwrap_or(server_default)
    }
}

#[derive(Debug, Deserialize)]
struct ListQuery {
    project_id: Uuid,
}

fn error(error: impl std::fmt::Display) -> ApiError {
    ApiError::BadRequest(error.to_string())
}

pub(crate) async fn create(
    State(deployment): State<DeploymentImpl>,
    Json(request): Json<CreateIntegrationRequest>,
) -> Result<Json<ApiResponse<IntegrationRun>>, ApiError> {
    let run = admission::submit(&deployment, request)
        .await
        .map_err(error)?;
    Ok(Json(ApiResponse::success(run)))
}

pub(crate) async fn create_with_expected_target(
    deployment: &DeploymentImpl,
    request: CreateIntegrationRequest,
    expected_target: &str,
) -> Result<IntegrationRun, AdmissionFailure> {
    match admission::submit_with_expected_target(deployment, request.clone(), Some(expected_target))
        .await
    {
        Ok(run) => Ok(run),
        Err(error) => {
            admission::reconcile_failed_admission(
                &deployment.db().pool,
                &request,
                expected_target,
                &error,
            )
            .await
        }
    }
}

async fn list(
    State(deployment): State<DeploymentImpl>,
    Query(query): Query<ListQuery>,
) -> Result<Json<ApiResponse<Vec<IntegrationRun>>>, ApiError> {
    let runs: Vec<IntegrationRun> = sqlx::query_as(
        "SELECT * FROM integration_runs WHERE project_id=? ORDER BY created_at DESC LIMIT 100",
    )
    .bind(query.project_id)
    .fetch_all(&deployment.db().pool)
    .await?;
    Ok(Json(ApiResponse::success(runs)))
}

async fn repositories(
    State(deployment): State<DeploymentImpl>,
    Query(query): Query<ListQuery>,
) -> Result<Json<ApiResponse<Vec<db::models::repo::Repo>>>, ApiError> {
    let ids = project_repository_ids(&deployment.db().pool, query.project_id).await?;
    let repos = db::models::repo::Repo::find_by_ids(&deployment.db().pool, &ids).await?;
    Ok(Json(ApiResponse::success(repos)))
}

pub(crate) async fn project_repository_ids(
    pool: &sqlx::SqlitePool,
    project_id: Uuid,
) -> Result<Vec<Uuid>, sqlx::Error> {
    // Modern local Boards link repositories through adopted Workspaces; their
    // scratch working-location default is not a project_repos membership row.
    // Use the same current association for discovery and admission. Keep legacy
    // explicit bindings, without leaking repositories from another Board.
    sqlx::query_scalar(
        "SELECT r.id FROM repos r WHERE
         EXISTS(SELECT 1 FROM project_repos p WHERE p.repo_id=r.id AND p.project_id=?)
         OR EXISTS(SELECT 1 FROM workspace_repos wr
           JOIN local_workspace_links l ON l.workspace_id=wr.workspace_id
           JOIN local_issues i ON i.id=l.issue_id
           WHERE wr.repo_id=r.id AND i.project_id=?)
         ORDER BY r.display_name,r.id",
    )
    .bind(project_id)
    .bind(project_id)
    .fetch_all(pool)
    .await
}

async fn status(
    State(deployment): State<DeploymentImpl>,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<IntegrationRun>>, ApiError> {
    let run = IntegrationRun::find(&deployment.db().pool, id).await?;
    Ok(Json(ApiResponse::success(run)))
}

async fn inspect(
    State(deployment): State<DeploymentImpl>,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<serde_json::Value>>, ApiError> {
    let run = IntegrationRun::find(&deployment.db().pool, id).await?;
    Ok(Json(ApiResponse::success(
        conversation::inspection(&deployment.db().pool, &run)
            .await
            .map_err(error)?,
    )))
}

pub(crate) async fn cancel(
    State(deployment): State<DeploymentImpl>,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<IntegrationRun>>, ApiError> {
    // Publishing is past the cancellation boundary. Never attach Git undo to a
    // Card reopening or to a late duplicate cancel.
    Ok(Json(ApiResponse::success(
        IntegrationRun::request_cancel(&deployment.db().pool, id).await?,
    )))
}

async fn recover(
    State(deployment): State<DeploymentImpl>,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<IntegrationRun>>, ApiError> {
    let run = IntegrationRun::find(&deployment.db().pool, id).await?;
    let can_recover = matches!(run.status.as_str(), "post_processing" | "recovery_required")
        || (run.status == "succeeded"
            && run.payload.published
            && run
                .payload
                .wiki_result
                .as_deref()
                .is_some_and(|s| s.starts_with("Wiki receipt pending:")));
    if !can_recover {
        return Err(error(
            "Only publication reconciliation may be retried on this run; create a new explicit selection for another integration",
        ));
    }
    runtime::request_recovery(&deployment, run)
        .await
        .map_err(error)?;
    status(State(deployment), Path(id)).await
}

pub fn router() -> Router<DeploymentImpl> {
    Router::new()
        .route("/integrations/worker", post(conversation::worker))
        .route("/integrations", get(list).post(create))
        .route("/integrations/repositories", get(repositories))
        .route("/integrations/{id}", get(status))
        .route("/integrations/{id}/inspect", get(inspect))
        .route("/integrations/{id}/cancel", post(cancel))
        .route("/integrations/{id}/recover", post(recover))
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod validation_settings_tests;
