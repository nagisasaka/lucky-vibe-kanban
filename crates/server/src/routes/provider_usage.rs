use axum::{Extension, Json, Router, extract::State, http::StatusCode, routing::post};
use deployment::Deployment;
use utils::response::ApiResponse;

use crate::{DeploymentImpl, provider_usage};

pub fn router() -> Router<DeploymentImpl> {
    Router::new().route("/provider-usage/read", post(read))
}
async fn read(
    State(deployment): State<DeploymentImpl>,
    Extension(actor): Extension<server_access::Principal>,
    Json(request): Json<provider_usage::Request>,
) -> Result<Json<ApiResponse<serde_json::Value>>, (StatusCode, &'static str)> {
    if !provider_usage::allows_server_context(
        actor.admin,
        actor.channel,
        actor.scopes.contains("lvk:read"),
    ) {
        return Err((
            StatusCode::FORBIDDEN,
            "Authorized server account read access required",
        ));
    }
    provider_usage::read(&deployment.db().pool, request, true)
        .await
        .map(|value| Json(ApiResponse::success(value)))
        .map_err(|_| {
            (
                StatusCode::BAD_REQUEST,
                "Invalid or unavailable provider/account/session context",
            )
        })
}
