use axum::{
    Extension, Json, Router,
    extract::State,
    http::{StatusCode, header},
    routing::post,
};
use deployment::Deployment;
use git::repository_read::QueryRequest;
use utils::response::ApiResponse;

use crate::DeploymentImpl;

pub(super) fn router(d: &DeploymentImpl) -> Router<DeploymentImpl> {
    pool_router(d.db().pool.clone())
}
fn pool_router<S: Clone + Send + Sync + 'static>(pool: sqlx::SqlitePool) -> Router<S> {
    Router::new()
        .route("/repository/read", post(read))
        .with_state(pool)
}
async fn read(
    State(pool): State<sqlx::SqlitePool>,
    Extension(actor): Extension<server_access::Principal>,
    Json(request): Json<QueryRequest>,
) -> Result<
    (
        [(header::HeaderName, &'static str); 1],
        Json<ApiResponse<serde_json::Value>>,
    ),
    (StatusCode, String),
> {
    if !actor.scopes.contains("lvk:read") {
        return Err((StatusCode::FORBIDDEN, "Read permission required".into()));
    }
    let result = match crate::repository_read::query(&pool, request).await {
        Ok(value) => ApiResponse::success(value),
        Err(error) => ApiResponse::error(&format!("Repository read failed: {error}")),
    };
    Ok(([(header::CACHE_CONTROL, "no-store")], Json(result)))
}

#[cfg(test)]
mod tests {
    use axum::{
        body::{Body, to_bytes},
        http::Request,
    };
    use serde_json::{Value, json};
    use tower::ServiceExt;
    use uuid::Uuid;

    use super::*;
    #[tokio::test]
    async fn repository_http_auth_and_project_boundary_use_shared_service() {
        let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
        let temp = tempfile::tempdir().unwrap();
        let access = server_access::test_support::access(
            pool.clone(),
            temp.path().to_owned(),
            json!({"keys":[]}),
        )
        .await;
        sqlx::query("CREATE TABLE projects(id BLOB PRIMARY KEY)")
            .execute(&pool)
            .await
            .unwrap();
        let cookie = server_access::test_support::web_session(&access).await;
        let project = Uuid::new_v4();
        let repository = Uuid::new_v4();
        let csrf: String = sqlx::query_scalar("SELECT csrf FROM access_sessions LIMIT 1")
            .fetch_one(&pool)
            .await
            .unwrap();
        let input = json!({"project_id":project,"repository_id":repository,"target":{"mode":"commit","ref":"main"},"operation":{"kind":"list"}});
        let app = access.layer(Router::new().nest("/api", pool_router(pool.clone())));
        let req = |authenticated| {
            let mut r = Request::builder()
                .method("POST")
                .uri("/api/repository/read")
                .header("host", "203.0.113.7:8443")
                .header("content-type", "application/json")
                .header("origin", "https://203.0.113.7:8443");
            if authenticated {
                r = r.header("cookie", &cookie).header("x-lvk-csrf", &csrf);
            }
            r.body(Body::from(input.to_string())).unwrap()
        };
        assert_eq!(
            app.clone().oneshot(req(false)).await.unwrap().status(),
            StatusCode::UNAUTHORIZED
        );
        let response = app.oneshot(req(true)).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
        let value: Value =
            serde_json::from_slice(&to_bytes(response.into_body(), 8192).await.unwrap()).unwrap();
        assert_eq!(value["success"], false);
        assert!(
            value["message"]
                .as_str()
                .unwrap()
                .contains("Project does not exist")
        );
    }
}
