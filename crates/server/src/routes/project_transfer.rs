//! Authenticated, bounded uploads/downloads for additive Project transfer.
//! Preview and import inspect the exact uploaded bytes, never a caller path.
use std::collections::BTreeMap;

use axum::{
    Extension, Json, Router,
    body::Bytes,
    extract::{DefaultBodyLimit, Path, State},
    http::{HeaderMap, HeaderValue, header},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use base64::{Engine, engine::general_purpose::STANDARD};
use db::project_transfer::{self as transfer, TransferError};
use deployment::Deployment;
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::SqlitePool;
use utils::response::ApiResponse;
use uuid::Uuid;

use crate::{DeploymentImpl, error::ApiError};

const FILE_LIMIT: usize = 32 * 1024 * 1024;
const BASE64_LIMIT: usize = FILE_LIMIT.div_ceil(3) * 4;

pub(super) fn router(d: &DeploymentImpl) -> Router<DeploymentImpl> {
    pool_router(d.db().pool.clone())
}
fn pool_router<S: Clone + Send + Sync + 'static>(pool: SqlitePool) -> Router<S> {
    Router::new()
        .route("/projects/{project_id}/transfer/export", post(export))
        .route("/projects/{project_id}/transfer/history", get(history))
        .route("/projects/{project_id}/transfer/report", get(report))
        .route(
            "/project-transfer/preview",
            post(preview).layer(DefaultBodyLimit::max(FILE_LIMIT)),
        )
        .route(
            "/project-transfer/import",
            post(import).layer(DefaultBodyLimit::max(BASE64_LIMIT + 64 * 1024)),
        )
        .with_state(pool)
}
fn require(actor: &server_access::Principal, scope: &str) -> Result<(), ApiError> {
    if !actor.scopes.contains(scope) {
        return Err(ApiError::Forbidden(
            "Project transfer permission required".into(),
        ));
    }
    Ok(())
}
async fn require_project(pool: &SqlitePool, id: Uuid) -> Result<(), ApiError> {
    if !super::local_remote::project_exists(pool, id).await? {
        // Keep existing local Project error semantics; do not change API fallback.
        return Err(ApiError::BadRequest("Project not found".into()));
    }
    Ok(())
}
fn failure(error: TransferError) -> ApiError {
    match error {
        TransferError::Invalid(message)
            if message.contains("already imported") || message.contains("key already exists") =>
        {
            ApiError::Conflict(message)
        }
        TransferError::Invalid(message) => ApiError::BadRequest(message),
        // Never return SQLite paths, source values or raw deserialization errors.
        _ => ApiError::BadRequest("Project transfer could not be validated or completed".into()),
    }
}
fn json_response<T: serde::Serialize>(data: T) -> Response {
    (
        [(header::CACHE_CONTROL, "no-store")],
        Json(ApiResponse::<T>::success(data)),
    )
        .into_response()
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Export {
    #[serde(default)]
    include_history: bool,
}
async fn export(
    State(pool): State<SqlitePool>,
    Extension(actor): Extension<server_access::Principal>,
    Path(project): Path<Uuid>,
    Json(input): Json<Export>,
) -> Result<Response, ApiError> {
    require(&actor, "lvk:read")?;
    require_project(&pool, project).await?;
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("project.sqlite");
    transfer::export_project_with_options(&pool, project, &path, input.include_history)
        .await
        .map_err(failure)?;
    let bytes = tokio::fs::read(&path).await?;
    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/vnd.sqlite3"),
    );
    headers.insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_static("attachment; filename=\"lvk-project.sqlite\""),
    );
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    Ok((headers, bytes).into_response())
}
async fn upload(bytes: &[u8]) -> Result<tempfile::NamedTempFile, ApiError> {
    if bytes.is_empty() || bytes.len() > FILE_LIMIT {
        return Err(ApiError::BadRequest(
            "Select a SQLite package up to 32 MiB".into(),
        ));
    }
    let file = tempfile::NamedTempFile::new()?;
    tokio::fs::write(file.path(), bytes).await?;
    Ok(file)
}
async fn preview(
    Extension(actor): Extension<server_access::Principal>,
    bytes: Bytes,
) -> Result<Response, ApiError> {
    require(&actor, "lvk:read")?;
    let file = upload(&bytes).await?;
    let data = transfer::inspect_package(file.path())
        .await
        .map_err(failure)?;
    let mut data = serde_json::to_value(data)
        .map_err(|_| ApiError::BadRequest("Unable to inspect Project package".into()))?;
    data["sha256"] = json!(format!("{:x}", Sha256::digest(&bytes)));
    Ok(json_response(data))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Import {
    data_base64: String,
    expected_sha256: String,
    key: String,
    repo_map: BTreeMap<Uuid, Uuid>,
}
async fn import(
    State(pool): State<SqlitePool>,
    Extension(actor): Extension<server_access::Principal>,
    Json(input): Json<Import>,
) -> Result<Response, ApiError> {
    require(&actor, "lvk:write")?;
    let bytes = bound_bytes(&input)?;
    let file = upload(&bytes).await?;
    let result = transfer::import_project(
        &pool,
        file.path(),
        &input.key,
        Uuid::from_u128(2),
        &input.repo_map,
    )
    .await
    .map_err(failure)?;
    Ok(json_response(result))
}
fn bound_bytes(input: &Import) -> Result<Vec<u8>, ApiError> {
    if input.data_base64.len() > BASE64_LIMIT
        || input.expected_sha256.len() != 64
        || !input
            .expected_sha256
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(ApiError::BadRequest(
            "Invalid package size or preview fingerprint".into(),
        ));
    }
    let bytes = STANDARD
        .decode(&input.data_base64)
        .map_err(|_| ApiError::BadRequest("Invalid package encoding".into()))?;
    if bytes.is_empty()
        || bytes.len() > FILE_LIMIT
        || format!("{:x}", Sha256::digest(&bytes)) != input.expected_sha256
    {
        return Err(ApiError::Conflict(
            "Package bytes changed since preview; inspect the selected file again".into(),
        ));
    }
    Ok(bytes)
}
async fn history(
    State(pool): State<SqlitePool>,
    Extension(actor): Extension<server_access::Principal>,
    Path(project): Path<Uuid>,
) -> Result<Response, ApiError> {
    require(&actor, "lvk:read")?;
    require_project(&pool, project).await?;
    Ok(json_response(
        transfer::list_imported_history(&pool, project)
            .await
            .map_err(failure)?,
    ))
}
async fn report(
    State(pool): State<SqlitePool>,
    Extension(actor): Extension<server_access::Principal>,
    Path(project): Path<Uuid>,
) -> Result<Response, ApiError> {
    require(&actor, "lvk:read")?;
    require_project(&pool, project).await?;
    let report: Option<String> =
        sqlx::query_scalar("SELECT report FROM project_transfer_receipts WHERE project_id=?")
            .bind(project)
            .fetch_optional(&pool)
            .await?;
    let data: Option<Value> = report
        .map(|r| serde_json::from_str(&r))
        .transpose()
        .map_err(|_| ApiError::BadRequest("Imported Project report is unavailable".into()))?;
    Ok(json_response(data))
}

#[cfg(test)]
mod tests;
