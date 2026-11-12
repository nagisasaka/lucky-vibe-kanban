//! The authoritative Issue body lives in a named version. All body writes hold
//! the SQLite writer across validation, version mutation and compatibility copy.
use axum::{
    Router,
    extract::{Json, Path, State},
    routing::{get, post},
};
use deployment::Deployment;
use serde::{Deserialize, Serialize};
use sqlx::{SqliteConnection, SqlitePool};
use uuid::Uuid;

use super::IssueMutationError;
use crate::{DeploymentImpl, error::ApiError};

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub(crate) struct IssueVersion {
    pub id: Uuid,
    pub issue_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub revision: i64,
}
#[derive(Debug, Serialize)]
pub(crate) struct VersionState {
    pub versions: Vec<IssueVersion>,
    pub current_version_id: Uuid,
    pub current_revision: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_version_id: Option<Uuid>,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CreateVersion {
    pub name: String,
    pub source_version_id: Option<Uuid>,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct EditVersion {
    pub expected_revision: i64,
    pub name: Option<String>,
    #[serde(default, deserialize_with = "api_types::some_if_present")]
    pub description: Option<Option<String>>,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SelectVersion {
    pub version_id: Uuid,
    pub expected_current_revision: i64,
    pub expected_version_revision: i64,
}

fn conflict() -> ApiError {
    ApiError::Conflict("Issue version changed; keep your draft and reload before retrying".into())
}
fn validate_name(name: &str) -> Result<(), ApiError> {
    if name.trim().is_empty() || name.chars().count() > 200 {
        return Err(ApiError::BadRequest(
            "Version name must contain 1–200 characters".into(),
        ));
    }
    Ok(())
}
async fn state_on(conn: &mut SqliteConnection, issue: Uuid) -> Result<VersionState, ApiError> {
    let (current_version_id, current_revision): (Uuid, i64) = sqlx::query_as(
        "SELECT current_version_id,current_version_revision FROM local_issues WHERE id=?",
    )
    .bind(issue)
    .fetch_optional(&mut *conn)
    .await?
    .ok_or_else(|| ApiError::BadRequest("Issue not found".into()))?;
    let versions = sqlx::query_as("SELECT id,issue_id,name,description,revision FROM issue_versions WHERE issue_id=? ORDER BY rowid")
        .bind(issue).fetch_all(&mut *conn).await?;
    Ok(VersionState {
        versions,
        current_version_id,
        current_revision,
        created_version_id: None,
    })
}
pub(crate) async fn list(pool: &SqlitePool, issue: Uuid) -> Result<VersionState, ApiError> {
    // One snapshot keeps pointer and version revisions coherent.
    let mut tx = pool.begin().await?;
    let state = state_on(&mut tx, issue).await?;
    tx.commit().await?;
    Ok(state)
}
pub(crate) async fn create(
    pool: &SqlitePool,
    issue: Uuid,
    request: CreateVersion,
) -> Result<VersionState, ApiError> {
    create_outcome(pool, issue, request)
        .await
        .map_err(IssueMutationError::into_api_error)
}
pub(crate) async fn create_outcome(
    pool: &SqlitePool,
    issue: Uuid,
    request: CreateVersion,
) -> Result<VersionState, IssueMutationError> {
    validate_name(&request.name)?;
    let mut tx = pool.begin_with("BEGIN IMMEDIATE").await?;
    let state = state_on(&mut tx, issue).await?;
    let description = match request.source_version_id {
        Some(id) => state
            .versions
            .iter()
            .find(|v| v.id == id)
            .ok_or_else(|| ApiError::BadRequest("Source version not found in this Issue".into()))?
            .description
            .clone(),
        None => None,
    };
    let created_version_id = Uuid::new_v4();
    sqlx::query("INSERT INTO issue_versions(id,issue_id,name,description) VALUES(?,?,?,?)")
        .bind(created_version_id)
        .bind(issue)
        .bind(request.name)
        .bind(description)
        .execute(&mut *tx)
        .await?;
    let mut result = state_on(&mut tx, issue).await?;
    result.created_version_id = Some(created_version_id);
    tx.commit().await.map_err(super::issue_commit_failure)?;
    Ok(result)
}
/// Called only after the compatibility operation checks the observed Issue's
/// updated_at in the same IMMEDIATE transaction. Switching versions always
/// advances that token, including switch-away-and-back.
pub(crate) async fn save_current_on(
    conn: &mut SqliteConnection,
    issue: Uuid,
    description: Option<String>,
) -> Result<(), ApiError> {
    let result = sqlx::query("UPDATE issue_versions SET description=?,revision=revision+1 WHERE issue_id=? AND id=(SELECT current_version_id FROM local_issues WHERE id=?)")
        .bind(description).bind(issue).bind(issue).execute(conn).await?;
    if result.rows_affected() != 1 {
        return Err(conflict());
    }
    Ok(())
}
async fn copy_current_on(
    conn: &mut SqliteConnection,
    issue: Uuid,
    description: Option<String>,
) -> Result<(), ApiError> {
    // Existing Integration revision and reservation triggers run here. Any
    // rejection rolls back the earlier authoritative version write as well.
    sqlx::query("UPDATE local_issues SET description=?,updated_at=strftime('%Y-%m-%d %H:%M:%f',MAX(julianday('now'),julianday(updated_at)+0.001/86400.0)) WHERE id=?")
        .bind(description).bind(issue).execute(&mut *conn).await?;
    db::models::issue_order::promote(conn, issue).await?;
    Ok(())
}
pub(crate) async fn edit(
    pool: &SqlitePool,
    issue: Uuid,
    version: Uuid,
    request: EditVersion,
) -> Result<VersionState, ApiError> {
    edit_outcome(pool, issue, version, request)
        .await
        .map_err(IssueMutationError::into_api_error)
}
pub(crate) async fn edit_outcome(
    pool: &SqlitePool,
    issue: Uuid,
    version: Uuid,
    request: EditVersion,
) -> Result<VersionState, IssueMutationError> {
    if let Some(name) = &request.name {
        validate_name(name)?;
    }
    let mut tx = pool.begin_with("BEGIN IMMEDIATE").await?;
    let state = state_on(&mut tx, issue).await?;
    let old = state
        .versions
        .iter()
        .find(|v| v.id == version)
        .ok_or_else(|| ApiError::BadRequest("Version not found in this Issue".into()))?;
    if old.revision != request.expected_revision {
        return Err(conflict().into());
    }
    let body_changed = request.description.is_some();
    let description = request
        .description
        .unwrap_or_else(|| old.description.clone());
    let updated = sqlx::query("UPDATE issue_versions SET name=?,description=?,revision=revision+1 WHERE id=? AND issue_id=? AND revision=?")
        .bind(request.name.unwrap_or_else(||old.name.clone())).bind(&description).bind(version).bind(issue).bind(request.expected_revision).execute(&mut *tx).await?;
    if updated.rows_affected() != 1 {
        return Err(conflict().into());
    }
    if body_changed && version == state.current_version_id {
        copy_current_on(&mut tx, issue, description).await?;
    }
    let result = state_on(&mut tx, issue).await?;
    tx.commit().await.map_err(super::issue_commit_failure)?;
    Ok(result)
}
pub(crate) async fn select(
    pool: &SqlitePool,
    issue: Uuid,
    request: SelectVersion,
) -> Result<VersionState, ApiError> {
    select_outcome(pool, issue, request)
        .await
        .map_err(IssueMutationError::into_api_error)
}
pub(crate) async fn select_outcome(
    pool: &SqlitePool,
    issue: Uuid,
    request: SelectVersion,
) -> Result<VersionState, IssueMutationError> {
    let mut tx = pool.begin_with("BEGIN IMMEDIATE").await?;
    let state = state_on(&mut tx, issue).await?;
    let version = state
        .versions
        .iter()
        .find(|v| v.id == request.version_id)
        .ok_or_else(|| ApiError::BadRequest("Version not found in this Issue".into()))?;
    if state.current_revision != request.expected_current_revision
        || version.revision != request.expected_version_revision
    {
        return Err(conflict().into());
    }
    if state.current_version_id != request.version_id {
        sqlx::query("UPDATE local_issues SET current_version_id=?,current_version_revision=current_version_revision+1 WHERE id=? AND current_version_revision=?")
            .bind(request.version_id).bind(issue).bind(request.expected_current_revision).execute(&mut *tx).await?;
        copy_current_on(&mut tx, issue, version.description.clone()).await?;
    }
    let result = state_on(&mut tx, issue).await?;
    tx.commit().await.map_err(super::issue_commit_failure)?;
    Ok(result)
}
pub(super) fn router() -> Router<DeploymentImpl> {
    Router::new()
        .route(
            "/v1/issues/{issue_id}/versions",
            get(get_versions).post(create_version),
        )
        .route(
            "/v1/issues/{issue_id}/versions/current",
            post(select_version),
        )
        .route(
            "/v1/issues/{issue_id}/versions/{version_id}",
            get(get_version).patch(edit_version),
        )
}
async fn get_versions(
    State(d): State<DeploymentImpl>,
    Path(issue): Path<Uuid>,
) -> Result<Json<VersionState>, ApiError> {
    Ok(Json(list(&d.db().pool, issue).await?))
}
async fn create_version(
    State(d): State<DeploymentImpl>,
    Path(issue): Path<Uuid>,
    Json(request): Json<CreateVersion>,
) -> Result<Json<VersionState>, ApiError> {
    Ok(Json(create(&d.db().pool, issue, request).await?))
}
async fn edit_version(
    State(d): State<DeploymentImpl>,
    Path((issue, version)): Path<(Uuid, Uuid)>,
    Json(request): Json<EditVersion>,
) -> Result<Json<VersionState>, ApiError> {
    Ok(Json(edit(&d.db().pool, issue, version, request).await?))
}
async fn select_version(
    State(d): State<DeploymentImpl>,
    Path(issue): Path<Uuid>,
    Json(request): Json<SelectVersion>,
) -> Result<Json<VersionState>, ApiError> {
    Ok(Json(select(&d.db().pool, issue, request).await?))
}

async fn get_version(
    State(d): State<DeploymentImpl>,
    Path((issue, version)): Path<(Uuid, Uuid)>,
) -> Result<Json<IssueVersion>, ApiError> {
    let version = sqlx::query_as("SELECT id,issue_id,name,description,revision FROM issue_versions WHERE issue_id=? AND id=?")
        .bind(issue).bind(version).fetch_optional(&d.db().pool).await?
        .ok_or_else(|| ApiError::BadRequest("Version not found in this Issue".into()))?;
    Ok(Json(version))
}
