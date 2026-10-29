use std::path::PathBuf;

use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use tower::ServiceExt;

use super::*;

async fn request(
    app: &Router,
    method: &str,
    cookie: &str,
    csrf: &str,
    body: Value,
) -> (StatusCode, String) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(method)
                .uri("/api/runtime-access-settings")
                .header("Host", "203.0.113.7:8443")
                .header("Cookie", cookie)
                .header("x-lvk-csrf", csrf)
                .header("Content-Type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    (
        response.status(),
        String::from_utf8(
            to_bytes(response.into_body(), 100000)
                .await
                .unwrap()
                .to_vec(),
        )
        .unwrap(),
    )
}

#[tokio::test]
async fn settings_browser_authority_defaults_save_reload_and_reset() {
    let (mut remote, auth, provider) = tests::fixture().await;
    let tmp = tempfile::tempdir().unwrap();
    let pool = remote.pool.clone();
    for migration in [
        include_str!("../../../db/migrations/20261020200000_runtime_access_settings.sql"),
        include_str!("../../../db/migrations/20261013000000_remote_mcp_operations.sql"),
    ] {
        sqlx::raw_sql(migration).execute(&pool).await.unwrap();
    }
    sqlx::query("CREATE TABLE workspaces(id BLOB,container_ref TEXT)")
        .execute(&pool)
        .await
        .unwrap();
    let access =
        server_access::test_support::access(pool.clone(), tmp.path().into(), json!({"keys":[]}))
            .await;
    sqlx::query("UPDATE access_members SET role='admin'")
        .execute(&pool)
        .await
        .unwrap();
    let cookie = server_access::test_support::web_session(&access).await;
    let csrf: String = sqlx::query_scalar("SELECT csrf FROM access_sessions LIMIT 1")
        .fetch_one(&pool)
        .await
        .unwrap();
    let app = access.layer(environment_settings::router(pool.clone()));
    assert_eq!(
        request(&app, "GET", "", "", json!(null)).await.0,
        StatusCode::UNAUTHORIZED
    );
    let defaults = json!({"files_enabled":true,"execution_enabled":true,"file_roots":["/"]});
    let read = request(&app, "GET", &cookie, &csrf, json!(null)).await;
    assert_eq!(read.0, StatusCode::OK);
    assert_eq!(
        serde_json::from_str::<Value>(&read.1).unwrap()["settings"],
        defaults
    );
    let mut save = json!({"revision":0,"settings":defaults});
    assert_eq!(
        request(&app, "PUT", &cookie, "wrong", save.clone()).await.0,
        StatusCode::FORBIDDEN
    );
    sqlx::query("UPDATE access_members SET role='member'")
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        request(&app, "PUT", &cookie, &csrf, save.clone()).await.0,
        StatusCode::FORBIDDEN
    );
    sqlx::query("UPDATE access_members SET role='admin'")
        .execute(&pool)
        .await
        .unwrap();
    let internal = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/api/runtime-access-settings")
                .header("Host", "127.0.0.1")
                .header(
                    "x-lvk-internal-token",
                    server_access::test_support::internal_token(&access),
                )
                .header("Content-Type", "application/json")
                .body(Body::from(save.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(internal.status(), StatusCode::FORBIDDEN);
    for roots in [
        json!([]),
        json!([""]),
        json!(["relative"]),
        json!([tmp.path().join("../outside")]),
        json!([tmp.path().join("missing")]),
    ] {
        let mut bad = save.clone();
        bad["settings"]["file_roots"] = roots;
        assert_eq!(
            request(&app, "PUT", &cookie, &csrf, bad).await.0,
            StatusCode::BAD_REQUEST
        );
    }
    assert_eq!(
        request(&app, "PUT", &cookie, &csrf, save.clone()).await.0,
        StatusCode::OK
    );
    assert_eq!(
        request(&app, "PUT", &cookie, &csrf, save.clone()).await.0,
        StatusCode::CONFLICT
    );
    save["revision"] = json!(1);
    save["settings"]["file_roots"] = json!([tmp.path(), "/"]);
    assert_eq!(
        request(&app, "PUT", &cookie, &csrf, save.clone()).await.0,
        StatusCode::OK
    );
    let reloaded = environment_settings::load(&pool).await.unwrap();
    assert_eq!(reloaded.2.file_roots.len(), 2);
    remote.environment = Some(Arc::new(environment::Environment::managed(pool.clone())));
    let http = environment_tests::serve(remote, auth).await;
    let cap = http.value("get_runtime_access", json!({})).await;
    assert_eq!(cap["settings"]["file_roots"].as_array().unwrap().len(), 2);
    for files in [false, true] {
        for execution in [false, true] {
            let rev = environment_settings::load(&pool).await.unwrap().0;
            let a = json!({"revision":rev,"settings":{"files_enabled":files,"execution_enabled":execution,"file_roots":["/"]}});
            assert_eq!(
                request(&app, "PUT", &cookie, &csrf, a).await.0,
                StatusCode::OK
            );
            let cap = http.value("get_runtime_access", json!({})).await;
            assert_eq!(cap["settings"]["files_enabled"], files);
            assert_eq!(cap["settings"]["execution_enabled"], execution);
        }
    }
    let gone = tmp.path().join("gone");
    std::fs::create_dir(&gone).unwrap();
    let rev = environment_settings::load(&pool).await.unwrap().0;
    let enabled = json!({"revision":rev,"settings":{"files_enabled":true,"execution_enabled":true,"file_roots":[gone]}});
    assert_eq!(
        request(&app, "PUT", &cookie, &csrf, enabled).await.0,
        StatusCode::OK
    );
    std::fs::remove_dir(&gone).unwrap();
    let off = json!({"revision":rev+1,"settings":{"files_enabled":false,"execution_enabled":true,"file_roots":[gone]}});
    assert_eq!(
        request(&app, "PUT", &cookie, &csrf, off).await.0,
        StatusCode::OK
    );
    // Upgrade discards old settings once. New saves survive subsequent migration runs.
    sqlx::query("UPDATE runtime_access_settings SET settings=?")
        .bind(r#"{"enabled":false,"grants":[]}"#)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::raw_sql(include_str!(
        "../../../db/migrations/20261029010000_runtime_access_redesign.sql"
    ))
    .execute(&pool)
    .await
    .unwrap();
    assert_eq!(
        environment_settings::load(&pool).await.unwrap().2,
        environment::RuntimeAccessSettings::default()
    );
    sqlx::query("INSERT INTO runtime_access_settings VALUES(1,1,'malformed')")
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        request(&app, "GET", &cookie, &csrf, json!(null)).await.0,
        StatusCode::SERVICE_UNAVAILABLE
    );
    provider.abort();
}

#[test]
fn settings_strict_shape_and_canonical_roots() {
    let mut s = environment::RuntimeAccessSettings::default();
    s.validate().unwrap();
    assert_eq!(s.file_roots, vec![PathBuf::from("/")]);
    assert!(
        serde_json::from_value::<environment::RuntimeAccessSettings>(
            json!({"enabled":false,"grants":[]})
        )
        .is_err()
    );
    let dir = tempfile::tempdir().unwrap();
    s.file_roots = vec![dir.path().into(), dir.path().join(".")];
    s.validate().unwrap();
    assert_eq!(s.file_roots.len(), 1);
}

#[tokio::test]
async fn runtime_redesign_migration_resets_once_and_preserves_new_saves() {
    let dir = tempfile::tempdir().unwrap();
    let migrations = dir.path().join("migrations");
    std::fs::create_dir(&migrations).unwrap();
    std::fs::write(
        migrations.join("20261020200000_runtime_access_settings.sql"),
        include_str!("../../../db/migrations/20261020200000_runtime_access_settings.sql"),
    )
    .unwrap();
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::migrate::Migrator::new(migrations.as_path())
        .await
        .unwrap()
        .run(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO runtime_access_settings VALUES(1,9,?)")
        .bind(r#"{"enabled":false,"grants":[]}"#)
        .execute(&pool)
        .await
        .unwrap();
    std::fs::write(
        migrations.join("20261029010000_runtime_access_redesign.sql"),
        include_str!("../../../db/migrations/20261029010000_runtime_access_redesign.sql"),
    )
    .unwrap();
    let migrator = sqlx::migrate::Migrator::new(migrations.as_path())
        .await
        .unwrap();
    migrator.run(&pool).await.unwrap();
    assert_eq!(
        environment_settings::load(&pool).await.unwrap().2,
        environment::RuntimeAccessSettings::default()
    );
    let new = environment::RuntimeAccessSettings {
        files_enabled: false,
        execution_enabled: false,
        ..Default::default()
    };
    sqlx::query("INSERT INTO runtime_access_settings VALUES(1,1,?)")
        .bind(serde_json::to_string(&new).unwrap())
        .execute(&pool)
        .await
        .unwrap();
    migrator.run(&pool).await.unwrap();
    assert_eq!(environment_settings::load(&pool).await.unwrap().2, new);
}
