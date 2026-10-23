use axum::{
    Json, Router,
    extract::{Path, Query, State},
    routing::{get, post},
};
use db::models::todo::{
    CreateTodo, Todo, TodoCreationSource, TodoError, TodoPage, TodoQuery, UpdateTodo,
};
use deployment::Deployment;
use serde::Deserialize;
use utils::response::ApiResponse;
use uuid::Uuid;

use crate::{DeploymentImpl, error::ApiError};

pub fn router() -> Router<DeploymentImpl> {
    Router::new()
        .route(
            "/workspaces/{workspace_id}/todo-project",
            get(workspace_project),
        )
        .route("/projects/{project_id}/todos", get(list).post(create))
        .route("/internal/projects/{project_id}/todos", post(create_mcp))
        .route(
            "/projects/{project_id}/todos/{todo_id}",
            get(read).patch(update).delete(delete),
        )
}
async fn list(
    State(d): State<DeploymentImpl>,
    Path(project): Path<Uuid>,
    Query(query): Query<TodoQuery>,
) -> Result<Json<ApiResponse<TodoPage>>, ApiError> {
    Ok(Json(ApiResponse::success(
        Todo::list(&d.db().pool, project, query).await?,
    )))
}
async fn read(
    State(d): State<DeploymentImpl>,
    Path((project, id)): Path<(Uuid, Uuid)>,
) -> Result<Json<ApiResponse<Todo>>, ApiError> {
    Ok(Json(ApiResponse::success(
        Todo::get(&d.db().pool, project, id).await?,
    )))
}
async fn create(
    State(d): State<DeploymentImpl>,
    Path(project): Path<Uuid>,
    Json(input): Json<CreateTodo>,
) -> Result<Json<ApiResponse<Todo>>, ApiError> {
    Ok(Json(ApiResponse::success(
        Todo::create_from(&d.db().pool, project, input, web_creation_source()).await?,
    )))
}
// ACTOR is populated by access middleware after credential verification.
// Internal callers using the generic REST API are not assumed to be the UI.
fn web_creation_source() -> TodoCreationSource {
    match server_access::ACTOR.try_with(|p| p.channel).ok() {
        Some("local" | "basic" | "web") => TodoCreationSource::Ui,
        _ => TodoCreationSource::Unknown,
    }
}

async fn create_mcp(
    State(d): State<DeploymentImpl>,
    Path(project): Path<Uuid>,
    Json(input): Json<CreateTodo>,
) -> Result<Json<ApiResponse<Todo>>, ApiError> {
    if server_access::ACTOR.try_with(|p| p.channel).ok() != Some("internal") {
        return Err(ApiError::Forbidden(
            "Internal MCP authentication required".into(),
        ));
    }
    Ok(Json(ApiResponse::success(
        Todo::create_from(&d.db().pool, project, input, TodoCreationSource::Mcp).await?,
    )))
}

async fn update(
    State(d): State<DeploymentImpl>,
    Path((project, id)): Path<(Uuid, Uuid)>,
    Json(input): Json<UpdateTodo>,
) -> Result<Json<ApiResponse<Todo>>, ApiError> {
    Ok(Json(ApiResponse::success(
        Todo::update(&d.db().pool, project, id, input).await?,
    )))
}
#[derive(Deserialize)]
struct DeleteQuery {
    expected_revision: i32,
}
async fn delete(
    State(d): State<DeploymentImpl>,
    Path((project, id)): Path<(Uuid, Uuid)>,
    Query(query): Query<DeleteQuery>,
) -> Result<Json<ApiResponse<()>>, ApiError> {
    Todo::delete(&d.db().pool, project, id, query.expected_revision).await?;
    Ok(Json(ApiResponse::success(())))
}

async fn workspace_project(
    State(d): State<DeploymentImpl>,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<Option<Uuid>>>, ApiError> {
    let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM workspaces WHERE id=?)")
        .bind(id)
        .fetch_one(&d.db().pool)
        .await?;
    if !exists {
        return Err(ApiError::Todo(TodoError::NotFound));
    }
    let project =
        sqlx::query_scalar("SELECT project_id FROM local_workspace_links WHERE workspace_id=?")
            .bind(id)
            .fetch_optional(&d.db().pool)
            .await?;
    Ok(Json(ApiResponse::success(project)))
}
