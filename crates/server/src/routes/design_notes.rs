use axum::{
    Json, Router,
    extract::{Path, Query, State},
    routing::get,
};
use db::models::design_note::{
    DesignNote, DesignNoteAddition, DesignNoteDetail, DesignNoteResult, DesignNoteWrite,
};
use deployment::Deployment;
use serde::Deserialize;
use utils::response::ApiResponse;
use uuid::Uuid;

use crate::{DeploymentImpl, error::ApiError};
#[derive(Default, Deserialize)]
struct Page {
    #[serde(default)]
    offset: u32,
}
pub fn router() -> Router<DeploymentImpl> {
    Router::new()
        .route(
            "/projects/{project}/design-notes/mcp-read/{tool}",
            get(mcp_read),
        )
        .route("/projects/{project}/design-notes", get(list).post(write))
        .route("/projects/{project}/design-notes/{id}", get(read))
        .route(
            "/projects/{project}/design-notes/{id}/history",
            get(history),
        )
        .route(
            "/projects/{project}/design-notes/{id}/additions/{addition}/history",
            get(addition_history),
        )
}
async fn list(
    State(d): State<DeploymentImpl>,
    Path(p): Path<Uuid>,
    Query(q): Query<Page>,
) -> Result<Json<ApiResponse<Vec<DesignNote>>>, ApiError> {
    Ok(Json(ApiResponse::success(
        DesignNote::list(&d.db().pool, p, q.offset).await?,
    )))
}
async fn read(
    State(d): State<DeploymentImpl>,
    Path((p, id)): Path<(Uuid, Uuid)>,
) -> Result<Json<ApiResponse<DesignNoteDetail>>, ApiError> {
    Ok(Json(ApiResponse::success(
        DesignNote::detail(&d.db().pool, p, id).await?,
    )))
}
async fn history(
    State(d): State<DeploymentImpl>,
    Path((p, id)): Path<(Uuid, Uuid)>,
    Query(q): Query<Page>,
) -> Result<Json<ApiResponse<Vec<DesignNote>>>, ApiError> {
    Ok(Json(ApiResponse::success(
        DesignNote::history(&d.db().pool, p, id, q.offset).await?,
    )))
}
async fn addition_history(
    State(d): State<DeploymentImpl>,
    Path((p, id, a)): Path<(Uuid, Uuid, Uuid)>,
    Query(q): Query<Page>,
) -> Result<Json<ApiResponse<Vec<DesignNoteAddition>>>, ApiError> {
    Ok(Json(ApiResponse::success(
        DesignNote::addition_history(&d.db().pool, p, id, a, q.offset).await?,
    )))
}
async fn write(
    State(d): State<DeploymentImpl>,
    Path(p): Path<Uuid>,
    Json(input): Json<DesignNoteWrite>,
) -> Result<Json<ApiResponse<DesignNoteResult>>, ApiError> {
    let channel = server_access::ACTOR
        .try_with(|p| p.channel)
        .unwrap_or("local");
    let actor = serde_json::json!([
        channel,
        server_access::actor_issuer(),
        server_access::actor_id()
    ])
    .to_string();
    Ok(Json(ApiResponse::success(
        DesignNote::write(&d.db().pool, p, &actor, input).await?,
    )))
}

// Same canonical database, separate bounded wire projection for both MCP transports.
async fn mcp_read(
    State(d): State<DeploymentImpl>,
    Path((project, tool)): Path<(Uuid, String)>,
    Query(q): Query<db::models::design_note_read::DesignNoteRead>,
) -> Result<Json<ApiResponse<serde_json::Value>>, ApiError> {
    if q.project_id != project {
        return Err(ApiError::DesignNote(
            db::models::design_note::DesignNoteError::NotFound,
        ));
    }
    Ok(Json(ApiResponse::success(
        db::models::design_note_read::read(&d.db().pool, &tool, q).await?,
    )))
}
