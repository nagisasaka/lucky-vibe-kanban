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
        assert_eq!(body["data"]["disks"].as_array().unwrap().len(), 1);
        assert_eq!(body["data"]["scope"]["statistics"], "system-visible");
        // A session alone cannot use a write method (CSRF boundary or 405).
        let status = app.oneshot(request("POST", true)).await.unwrap().status();
        assert!(status == StatusCode::FORBIDDEN || status == StatusCode::METHOD_NOT_ALLOWED);
    }
}

pub(super) fn environment_router(d: &crate::DeploymentImpl) -> Router<crate::DeploymentImpl> {
    use deployment::Deployment;
    Router::new()
        .route("/environment-info", get(environment))
        .with_state(d.db().pool.clone())
        .merge(display_name_router(crate::host_display_name::path().ok()))
}

async fn running_sessions(pool: &sqlx::SqlitePool) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar("SELECT COUNT(DISTINCT r.session_id) FROM agent_runs r JOIN agent_run_state s ON s.agent_run_id=r.id JOIN agent_process_registry p ON p.run_attempt_id=s.last_run_attempt_id WHERE s.status='running' AND s.projection_status='current' AND p.registry_status='running'")
        .fetch_one(pool).await
}

async fn environment(
    axum::extract::State(pool): axum::extract::State<sqlx::SqlitePool>,
) -> impl IntoResponse {
    let sessions = running_sessions(&pool).await.ok();
    let hostname = crate::host_display_name::hostname().await;
    let display_name = match crate::host_display_name::path() {
        Ok(path) => crate::host_display_name::load(&path)
            .await
            .ok()
            .and_then(|v| v.host_display_name),
        Err(_) => None,
    };
    // Neither /proc/uptime nor PID 1 start time proves container creation time.
    let container = tokio::fs::try_exists("/.dockerenv").await.unwrap_or(false);
    (
        [(header::CACHE_CONTROL, "no-store")],
        Json(ApiResponse::<serde_json::Value>::success(
            serde_json::json!({
                "hostname":hostname,"host_display_name":display_name,"container":container,"uptime_seconds":null,
                "running_sessions":sessions,"observed_at":chrono::Utc::now()
            }),
        )),
    )
}

#[cfg(test)]
mod environment_tests {
    use super::*;
    #[tokio::test]
    async fn counts_only_running_confirmed_sessions_across_workspaces() {
        let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
        for ddl in [
            "CREATE TABLE agent_runs(id INTEGER, session_id INTEGER, workspace_id INTEGER)",
            "CREATE TABLE agent_run_state(agent_run_id INTEGER, status TEXT, projection_status TEXT, last_run_attempt_id INTEGER)",
            "CREATE TABLE agent_process_registry(run_attempt_id INTEGER, registry_status TEXT)",
        ] {
            sqlx::query(ddl).execute(&pool).await.unwrap();
        }
        let cases = [
            (1, 1, "running", "current", "running"),
            (2, 2, "running", "current", "running"),
            (3, 1, "running", "current", "running"), // same Session counted once
            (4, 4, "pending", "current", "reserved"),
            (5, 5, "starting", "current", "running"),
            (6, 6, "awaiting_input", "current", "running"),
            (7, 7, "awaiting_approval", "current", "running"),
            (8, 8, "succeeded", "current", "running"),
            (9, 9, "running", "projection_degraded", "running"),
            (10, 10, "running", "current", "exited"),
            (11, 11, "cancelled", "current", "running"),
        ];
        for (id, session, status, projection, process) in cases {
            sqlx::query("INSERT INTO agent_runs VALUES(?,?,?)")
                .bind(id)
                .bind(session)
                .bind(id)
                .execute(&pool)
                .await
                .unwrap();
            sqlx::query("INSERT INTO agent_run_state VALUES(?,?,?,?)")
                .bind(id)
                .bind(status)
                .bind(projection)
                .bind(id)
                .execute(&pool)
                .await
                .unwrap();
            sqlx::query("INSERT INTO agent_process_registry VALUES(?,?)")
                .bind(id)
                .bind(process)
                .execute(&pool)
                .await
                .unwrap();
        }
        assert_eq!(running_sessions(&pool).await.unwrap(), 2);
        sqlx::query("UPDATE agent_run_state SET last_run_attempt_id=999 WHERE agent_run_id=2")
            .execute(&pool)
            .await
            .unwrap();
        assert_eq!(running_sessions(&pool).await.unwrap(), 1);
        sqlx::query("DROP TABLE agent_run_state")
            .execute(&pool)
            .await
            .unwrap();
        assert!(running_sessions(&pool).await.is_err()); // failure never becomes zero
    }
}

fn display_name_router<S: Clone + Send + Sync + 'static>(
    path: Option<std::path::PathBuf>,
) -> Router<S> {
    Router::new()
        .route(
            "/host-display-name",
            get(get_display_name).put(put_display_name),
        )
        .with_state(path)
}
async fn get_display_name(
    axum::extract::State(path): axum::extract::State<Option<std::path::PathBuf>>,
) -> impl IntoResponse {
    let result = match path {
        Some(path) => crate::host_display_name::load(&path).await,
        None => Err(std::io::Error::other("Instance settings path unavailable")),
    };
    display_name_response(result)
}

async fn put_display_name(
    axum::extract::State(path): axum::extract::State<Option<std::path::PathBuf>>,
    Json(value): Json<crate::host_display_name::HostDisplayName>,
) -> impl IntoResponse {
    let value = match crate::host_display_name::normalize(value.host_display_name) {
        Ok(value) => value,
        Err(message) => {
            return (
                StatusCode::BAD_REQUEST,
                [(header::CACHE_CONTROL, "no-store")],
                Json(ApiResponse::error(message)),
            );
        }
    };
    let result = match path {
        Some(path) => crate::host_display_name::save(&path, &value)
            .await
            .map(|_| value),
        None => Err(std::io::Error::other("Instance settings path unavailable")),
    };
    display_name_response(result)
}

type DisplayNameResponse = (
    StatusCode,
    [(header::HeaderName, &'static str); 1],
    Json<ApiResponse<crate::host_display_name::HostDisplayName>>,
);
fn display_name_response(
    result: std::io::Result<crate::host_display_name::HostDisplayName>,
) -> DisplayNameResponse {
    let (status, body) = match result {
        Ok(value) => (StatusCode::OK, ApiResponse::success(value)),
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            ApiResponse::error("Host display name unavailable"),
        ),
    };
    (status, [(header::CACHE_CONTROL, "no-store")], Json(body))
}

#[cfg(test)]
mod host_display_name_tests {
    use axum::{
        body::{Body, to_bytes},
        http::Request,
    };
    use serde_json::{Value, json};
    use tower::ServiceExt;

    use super::*;
    #[tokio::test]
    async fn host_display_name_http_auth_validation_and_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
        let access = server_access::test_support::access(
            pool.clone(),
            dir.path().to_owned(),
            json!({"keys":[]}),
        )
        .await;
        let cookie = server_access::test_support::web_session(&access).await;
        let csrf: String = sqlx::query_scalar("SELECT csrf FROM access_sessions LIMIT 1")
            .fetch_one(&pool)
            .await
            .unwrap();
        let path = dir.path().join("host-display-name.json");
        let app = access.layer(Router::new().nest("/api", display_name_router(Some(path.clone()))));
        let request = |method: &str, value: Value, auth: bool| {
            let mut r = Request::builder()
                .method(method)
                .uri("/api/host-display-name")
                .header("host", "203.0.113.7:8443")
                .header("content-type", "application/json")
                .header("origin", "https://203.0.113.7:8443");
            if auth {
                r = r.header("cookie", &cookie).header("x-lvk-csrf", &csrf);
            }
            r.body(Body::from(value.to_string())).unwrap()
        };
        assert_eq!(
            app.clone()
                .oneshot(request("PUT", json!({"host_display_name":"No"}), false))
                .await
                .unwrap()
                .status(),
            StatusCode::UNAUTHORIZED
        );
        assert!(!path.exists());
        let response = app
            .clone()
            .oneshot(request(
                "PUT",
                json!({"host_display_name":" Rabbit "}),
                true,
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let response = app
            .clone()
            .oneshot(request("GET", Value::Null, true))
            .await
            .unwrap();
        assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
        let body: Value =
            serde_json::from_slice(&to_bytes(response.into_body(), 16384).await.unwrap()).unwrap();
        assert_eq!(body["data"]["host_display_name"], "Rabbit");
        assert_eq!(
            app.clone()
                .oneshot(request(
                    "PUT",
                    json!({"host_display_name":"x".repeat(81)}),
                    true
                ))
                .await
                .unwrap()
                .status(),
            StatusCode::BAD_REQUEST
        );
        assert_eq!(
            crate::host_display_name::load(&path)
                .await
                .unwrap()
                .host_display_name
                .as_deref(),
            Some("Rabbit")
        );
        assert_eq!(
            app.oneshot(request("PUT", json!({"host_display_name":" "}), true))
                .await
                .unwrap()
                .status(),
            StatusCode::OK
        );
        assert!(
            crate::host_display_name::load(&path)
                .await
                .unwrap()
                .host_display_name
                .is_none()
        );
    }
}
