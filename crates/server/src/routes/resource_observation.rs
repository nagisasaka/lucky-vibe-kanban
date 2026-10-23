use axum::{
    Json, Router,
    http::{StatusCode, header},
    response::IntoResponse,
    routing::get,
};
use utils::response::ApiResponse;

pub(super) fn router<S: Clone + Send + Sync + 'static>() -> Router<S> {
    Router::new().route("/resource-observation", get(observe))
}

async fn observe() -> impl IntoResponse {
    let (status, body) = match crate::resource_observation::observation().await {
        Ok(value) => (
            StatusCode::OK,
            ApiResponse::<serde_json::Value>::success(value),
        ),
        Err(_) => (
            StatusCode::SERVICE_UNAVAILABLE,
            ApiResponse::error("Resource observation unavailable; retry later"),
        ),
    };
    (status, [(header::CACHE_CONTROL, "no-store")], Json(body))
}

#[cfg(test)]
mod tests {
    use axum::{
        body::{Body, to_bytes},
        http::Request,
    };
    use serde_json::{Value, json};
    use tower::ServiceExt;

    use super::*;

    #[tokio::test]
    async fn resource_observation_web_requires_session_and_is_read_only() {
        let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
        let dir = tempfile::tempdir().unwrap();
        let access =
            server_access::test_support::access(pool, dir.path().to_owned(), json!({"keys":[]}))
                .await;
        let cookie = server_access::test_support::web_session(&access).await;
        let app = access.layer(Router::new().nest("/api", router()));
        let request = |method, authenticated| {
            let mut builder = Request::builder()
                .method(method)
                .uri("/api/resource-observation")
                .header("host", "203.0.113.7:8443");
            if authenticated {
                builder = builder.header("cookie", &cookie);
            }
            builder.body(Body::empty()).unwrap()
        };
        assert_eq!(
            app.clone()
                .oneshot(request("GET", false))
                .await
                .unwrap()
                .status(),
            StatusCode::UNAUTHORIZED
        );
        let response = app.clone().oneshot(request("GET", true)).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
        let body: Value =
            serde_json::from_slice(&to_bytes(response.into_body(), 16384).await.unwrap()).unwrap();
        assert_eq!(body["success"], true);
        assert_eq!(body["data"]["disks"].as_array().unwrap().len(), 2);
        assert_eq!(body["data"]["scope"]["statistics"], "system-visible");
        // A session alone cannot use a write method (CSRF boundary or 405).
        let status = app.oneshot(request("POST", true)).await.unwrap().status();
        assert!(status == StatusCode::FORBIDDEN || status == StatusCode::METHOD_NOT_ALLOWED);
    }
}
