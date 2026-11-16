use axum::{
    Extension, Json,
    extract::{Path, State},
};
use deployment::Deployment;
use serde_json::{Value, json};
use utils::response::ApiResponse;
use uuid::Uuid;

pub use crate::repository_push::{
    RepositoryPushPreview, RepositoryPushReceipt, RepositoryPushRequest, RepositoryPushResult,
    RepositoryPushSelection, RepositoryPushVerification,
};
use crate::{DeploymentImpl, error::ApiError};
fn failure(error: anyhow::Error) -> ApiError {
    ApiError::BadRequest(format!(
        "Repository publication unavailable ({}); verify selection and the original receipt before proceeding",
        git::repository_push::error_code(&error)
    ))
}
pub async fn preview(
    State(d): State<DeploymentImpl>,
    Extension(actor): Extension<server_access::Principal>,
    Path(id): Path<Uuid>,
    Json(body): Json<RepositoryPushSelection>,
) -> Result<Json<ApiResponse<RepositoryPushPreview>>, ApiError> {
    crate::repository_push::authorize(&actor, true)
        .map_err(|_| ApiError::Forbidden("Read and write permissions required".into()))?;
    Ok(Json(ApiResponse::success(
        crate::repository_push::preview(&d.db().pool, id, body)
            .await
            .map_err(failure)?,
    )))
}
pub async fn push(
    State(d): State<DeploymentImpl>,
    Extension(actor): Extension<server_access::Principal>,
    Extension(lease): Extension<server_access::Lease>,
    Path(id): Path<Uuid>,
    Json(body): Json<RepositoryPushRequest>,
) -> Result<Json<ApiResponse<RepositoryPushReceipt, Value>>, ApiError> {
    if crate::repository_push::authorize(&actor, true).is_err() {
        return Ok(Json(ApiResponse::error_with_data(
            json!({"not_admitted":true,"outcome":"permission_denied"}),
        )));
    }
    match crate::repository_push::push(&d.db().pool, &actor, lease, id, body).await {
        Ok(receipt) => Ok(Json(ApiResponse::success(receipt))),
        Err(error) => {
            if let Some(rejection) = error.downcast_ref::<crate::repository_push::NotAdmitted>() {
                return Ok(Json(ApiResponse::error_with_data(
                    json!({"not_admitted":true,"outcome":rejection.0}),
                )));
            }
            Err(ApiError::BadRequest("Repository push outcome is unknown; retain the original request ID and verify its receipt and remote branch before any new publication".into()))
        }
    }
}
pub async fn receipt(
    State(d): State<DeploymentImpl>,
    Extension(actor): Extension<server_access::Principal>,
    Path((id, request)): Path<(Uuid, Uuid)>,
) -> Result<Json<ApiResponse<RepositoryPushReceipt>>, ApiError> {
    Ok(Json(ApiResponse::success(
        crate::repository_push::receipt(&d.db().pool, &actor, id, request)
            .await
            .map_err(failure)?,
    )))
}
pub async fn verify(
    State(d): State<DeploymentImpl>,
    Extension(actor): Extension<server_access::Principal>,
    Path((id, request)): Path<(Uuid, Uuid)>,
) -> Result<Json<ApiResponse<RepositoryPushVerification>>, ApiError> {
    Ok(Json(ApiResponse::success(
        crate::repository_push::verify(&d.db().pool, &actor, id, request)
            .await
            .map_err(failure)?,
    )))
}
