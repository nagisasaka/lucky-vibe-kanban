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
async fn settings_browser_admin_save_reload_revoke_and_no_ai_escalation() {
    let (mut remote, auth, provider) = tests::fixture().await;
    let tmp = tempfile::tempdir().unwrap();
    let pool = remote.pool.clone();
    let access =
        server_access::test_support::access(pool.clone(), tmp.path().into(), json!({"keys":[]}))
            .await;
    sqlx::raw_sql(include_str!(
        "../../../db/migrations/20261020200000_runtime_access_settings.sql"
    ))
    .execute(&pool)
    .await
    .unwrap();
    sqlx::raw_sql(include_str!(
        "../../../db/migrations/20261013000000_remote_mcp_operations.sql"
    ))
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("CREATE TABLE workspaces(id BLOB,container_ref TEXT)")
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
    sqlx::query("UPDATE access_members SET role='member'")
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        request(&app, "GET", &cookie, &csrf, json!(null)).await.0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        request(
            &app,
            "PUT",
            &cookie,
            &csrf,
            json!({"revision":0,"settings":{"enabled":false,"grants":[]}})
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    sqlx::query("UPDATE access_members SET role='admin'")
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        request(&app, "PUT", &cookie, "wrong", json!({})).await.0,
        StatusCode::FORBIDDEN
    );
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
                .body(Body::from(
                    r#"{"revision":0,"settings":{"enabled":true,"grants":[]}}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(internal.status(), StatusCode::FORBIDDEN);
    let bearer = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/runtime-access-settings")
                .header("Host", "203.0.113.7:8443")
                .header("Authorization", "Bearer runtime-token-is-not-a-web-session")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(bearer.status(), StatusCode::UNAUTHORIZED);
    let (status, body) = request(&app, "GET", &cookie, &csrf, json!(null)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        serde_json::from_str::<Value>(&body).unwrap()["settings"]["enabled"],
        false
    );
    remote.environment = Some(Arc::new(environment::Environment::managed(pool.clone())));
    let http = environment_tests::serve(remote, auth).await;
    assert_eq!(
        http.call("runtime", "get_runtime_access", json!({})).await["isError"],
        true
    );
    std::fs::write(tmp.path().join("log"), "test log").unwrap();
    let grant = environment_tests::grant(tmp.path(), PathBuf::from("/bin/sleep"));
    let input = json!({"revision":0,"settings":{"enabled":true,"grants":[grant]}});
    for (field, value) in [
        ("root", json!("/")),
        ("root", json!("../outside")),
        ("root", json!(tmp.path().join("missing"))),
        ("subjects", json!([""])),
        ("project_id", json!(Uuid::nil())),
        ("executables", json!(["relative"])),
    ] {
        let mut bad = input.clone();
        bad["settings"]["grants"][0][field] = value;
        assert_eq!(
            request(&app, "PUT", &cookie, &csrf, bad).await.0,
            StatusCode::BAD_REQUEST
        );
    }
    let (status, body) = request(&app, "PUT", &cookie, &csrf, input.clone()).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        request(&app, "PUT", &cookie, &csrf, input).await.0,
        StatusCode::CONFLICT
    );
    let regions = http.value("get_runtime_access", json!({})).await;
    assert_eq!(regions["regions"][0]["display_name"], "Example API");
    http.value(
        "read_runtime_file",
        json!({"region":"example-api","path":"log"}),
    )
    .await;
    // A fresh settings router reads the same durable state without changing the process registry.
    let reopened = access.layer(environment_settings::router(pool.clone()));
    let (_, reread) = request(&reopened, "GET", &cookie, &csrf, json!(null)).await;
    assert_eq!(
        serde_json::from_str::<Value>(&reread).unwrap()["revision"],
        1
    );
    let run_request = Uuid::new_v4();
    http.value("run_process", json!({"region":"example-api","request_id":run_request,"purpose":"Verify revocation without forced termination","executable":"/bin/sleep","args":["30"],"cwd":".","lifetime_seconds":40})).await;
    let receipt = http.settle(run_request).await;
    assert_eq!(receipt["status"], "succeeded", "{receipt}");
    let identity = receipt["result"]["identity"].as_str().unwrap();
    let _cleanup = environment_process::TestProcessCleanup::new(identity).unwrap();
    let mut removed_subject: Value = serde_json::from_str(&body).unwrap();
    removed_subject.as_object_mut().unwrap().remove("source");
    removed_subject.as_object_mut().unwrap().remove("projects");
    removed_subject
        .as_object_mut()
        .unwrap()
        .remove("legacy_error");
    removed_subject["settings"]["grants"][0]["subjects"] = json!(["another-subject"]);
    assert_eq!(
        request(&app, "PUT", &cookie, &csrf, removed_subject)
            .await
            .0,
        StatusCode::OK
    );
    assert_eq!(http.call("runtime","stop_process",json!({"region":"example-api","identity":identity,"request_id":Uuid::new_v4(),"purpose":"Must be denied"})).await["isError"],true);
    assert_eq!(http.call("runtime","read_process_output",json!({"region":"example-api","process_id":receipt["result"]["run_id"],"stream":"stdout"})).await["isError"],true);
    assert!(
        std::path::Path::new(&format!(
            "/proc/{}",
            receipt["result"]["pid"].as_u64().unwrap()
        ))
        .exists()
    );
    let revoke = json!({"revision":2,"settings":{"enabled":false,"grants":[]}});
    assert_eq!(
        request(&app, "PUT", &cookie, &csrf, revoke).await.0,
        StatusCode::OK
    );
    assert_eq!(
        http.call(
            "runtime",
            "read_runtime_file",
            json!({"region":"example-api","path":"log"})
        )
        .await["isError"],
        true
    );
    assert_eq!(environment_settings::load(&pool).await.unwrap().1, "saved");
    assert!(!tools().iter().any(|t| t.name.contains("settings")));
    // Disabling an unchanged saved grant must work even after its directory disappears.
    let disappearing = tmp.path().join("disappearing");
    std::fs::create_dir(&disappearing).unwrap();
    let mut input = json!({"revision":3,"settings":{"enabled":true,"grants":[environment_tests::grant(&disappearing,PathBuf::from("/bin/sleep"))]}});
    let (status, saved) = request(&app, "PUT", &cookie, &csrf, input.clone()).await;
    assert_eq!(status, StatusCode::OK);
    input["settings"] = serde_json::from_str::<Value>(&saved).unwrap()["settings"].clone();
    std::fs::remove_dir(&disappearing).unwrap();
    input["revision"] = json!(4);
    input["settings"]["enabled"] = json!(false);
    assert_eq!(
        request(&app, "PUT", &cookie, &csrf, input).await.0,
        StatusCode::OK
    );
    provider.abort();
}
use std::path::PathBuf;

#[tokio::test]
async fn settings_persist_and_saved_disable_supersedes_legacy() {
    let tmp = tempfile::tempdir().unwrap();
    let db = tmp.path().join("settings.sqlite");
    let connect = || {
        sqlx::sqlite::SqliteConnectOptions::new()
            .filename(&db)
            .create_if_missing(true)
    };
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(connect())
        .await
        .unwrap();
    sqlx::raw_sql(include_str!(
        "../../../db/migrations/20261020200000_runtime_access_settings.sql"
    ))
    .execute(&pool)
    .await
    .unwrap();
    let file = tmp.path().join("legacy.json");
    let legacy = json!({"enabled":true,"grants":[environment_tests::grant(tmp.path(),PathBuf::from("/bin/sh"))]});
    std::fs::write(&file, legacy.to_string()).unwrap();
    let loaded = environment_settings::load_with_legacy(&pool, Some(file.clone()))
        .await
        .unwrap();
    assert_eq!(loaded.1, "legacy_file");
    assert!(loaded.2.enabled);
    sqlx::query("INSERT INTO runtime_access_settings VALUES(1,1,?)")
        .bind(r#"{"enabled":false,"grants":[]}"#)
        .execute(&pool)
        .await
        .unwrap();
    pool.close().await;
    std::fs::write(
        &file,
        "invalid legacy JSON must never override saved revocation",
    )
    .unwrap();
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .connect_with(connect())
        .await
        .unwrap();
    let loaded = environment_settings::load_with_legacy(&pool, Some(file))
        .await
        .unwrap();
    assert_eq!(loaded.0, 1);
    assert_eq!(loaded.1, "saved");
    assert!(!loaded.2.enabled);
    pool.close().await;
}

#[tokio::test]
async fn invalid_legacy_recovery_is_explicit_and_storage_errors_fail_closed() {
    let (mut remote, auth, provider) = tests::fixture().await;
    let tmp = tempfile::tempdir().unwrap();
    let pool = remote.pool.clone();
    let access =
        server_access::test_support::access(pool.clone(), tmp.path().into(), json!({"keys":[]}))
            .await;
    sqlx::raw_sql(include_str!(
        "../../../db/migrations/20261020200000_runtime_access_settings.sql"
    ))
    .execute(&pool)
    .await
    .unwrap();
    let cookie = server_access::test_support::web_session(&access).await;
    let csrf: String = sqlx::query_scalar("SELECT csrf FROM access_sessions LIMIT 1")
        .fetch_one(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE access_members SET role='admin'")
        .execute(&pool)
        .await
        .unwrap();
    let app = access.layer(environment_settings::router(pool.clone()));
    remote.environment = Some(Arc::new(environment::Environment::managed(pool.clone())));
    let http = environment_tests::serve(remote, auth).await;
    let file = tmp.path().join("legacy.json");
    for case in ["malformed", "missing_file", "missing_root"] {
        match case {
            "malformed" => std::fs::write(&file, "{").unwrap(),
            "missing_file" => std::fs::remove_file(&file).unwrap(),
            _ => {
                let grant =
                    environment_tests::grant(&tmp.path().join("missing"), PathBuf::from("/bin/sh"));
                std::fs::write(&file, json!({"enabled":true,"grants":[grant]}).to_string())
                    .unwrap();
            }
        }
        let view = environment_settings::read_view(&pool, Some(file.clone()))
            .await
            .unwrap();
        assert_eq!(view.source, "invalid_legacy");
        assert_eq!(view.revision, 0);
        assert!(!view.settings.enabled);
        assert!(view.settings.grants.is_empty());
        let error = serde_json::to_value(view.legacy_error.unwrap()).unwrap();
        assert_eq!(
            error["kind"],
            match case {
                "malformed" => "invalid_json",
                "missing_file" => "unreadable",
                _ => "invalid_grant",
            }
        );
        // Runtime's loader fails closed; merely opening the settings view writes nothing.
        assert!(
            environment_settings::load_with_legacy(&pool, Some(file.clone()))
                .await
                .is_err()
        );
        let count: i64 = sqlx::query_scalar("SELECT count(*) FROM runtime_access_settings")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 0);
        assert_eq!(
            request(
                &app,
                "PUT",
                &cookie,
                &csrf,
                json!({"revision":0,"settings":{"enabled":false,"grants":[]}})
            )
            .await
            .0,
            StatusCode::OK
        );
        let view = environment_settings::read_view(&pool, Some(file.clone()))
            .await
            .unwrap();
        assert_eq!(view.source, "saved");
        assert!(view.legacy_error.is_none());
        assert_eq!(view.revision, 1);
        let loaded = environment_settings::load_with_legacy(&pool, Some(file.clone()))
            .await
            .unwrap();
        assert!(!loaded.2.enabled);
        assert!(loaded.2.grants.is_empty());
        assert_eq!(loaded.1, "saved");
        assert_eq!(
            http.call("runtime", "get_runtime_access", json!({})).await["isError"],
            true
        );
        // Reset only this isolated test database for the next independent case.
        sqlx::query("DELETE FROM runtime_access_settings")
            .execute(&pool)
            .await
            .unwrap();
    }
    sqlx::query("INSERT INTO runtime_access_settings VALUES(1,1,'broken saved JSON')")
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        environment_settings::read_view(&pool, Some(file.clone()))
            .await
            .err()
            .unwrap()
            .0,
        StatusCode::SERVICE_UNAVAILABLE
    );
    sqlx::query("DROP TABLE runtime_access_settings")
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        environment_settings::read_view(&pool, Some(file))
            .await
            .err()
            .unwrap()
            .0,
        StatusCode::SERVICE_UNAVAILABLE
    );
    assert_eq!(
        request(&app, "GET", &cookie, &csrf, json!(null)).await.0,
        StatusCode::SERVICE_UNAVAILABLE
    );
    provider.abort();
}

#[tokio::test]
async fn runtime_and_local19_migrations_have_unique_sqlx_versions_fresh_and_upgrade() {
    // Reference copy only; LOCAL-19 is not added to the production migrator.
    let other = include_str!("test_fixtures/local19_7a8ca336_kanban_lifecycle.sql");
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../db/migrations");
    for scenario in ["fresh", "upgrade_from_local19", "upgrade_from_local11"] {
        let tmp = tempfile::tempdir().unwrap();
        let migrations = tmp.path().join("migrations");
        std::fs::create_dir(&migrations).unwrap();
        let runtime_name = "20261020200000_runtime_access_settings.sql";
        let other_name = "20261020190000_kanban_lifecycle.sql";
        let push_name = "20261018180000_web_push.sql";
        let push = include_str!("test_fixtures/local19_7a8ca336_web_push.sql");
        for entry in std::fs::read_dir(&root).unwrap() {
            let path = entry.unwrap().path();
            if path.file_name().unwrap() != runtime_name || scenario != "upgrade_from_local19" {
                std::fs::copy(&path, migrations.join(path.file_name().unwrap())).unwrap();
            }
        }
        if scenario != "upgrade_from_local11" {
            std::fs::write(migrations.join(other_name), other).unwrap();
            std::fs::write(migrations.join(push_name), push).unwrap();
        }
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                sqlx::sqlite::SqliteConnectOptions::new()
                    .filename(tmp.path().join("db.sqlite"))
                    .create_if_missing(true),
            )
            .await
            .unwrap();
        sqlx::migrate::Migrator::new(migrations.as_path())
            .await
            .unwrap()
            .run(&pool)
            .await
            .unwrap();
        std::fs::copy(root.join(runtime_name), migrations.join(runtime_name)).unwrap();
        std::fs::write(migrations.join(other_name), other).unwrap();
        std::fs::write(migrations.join(push_name), push).unwrap();
        let migrator = sqlx::migrate::Migrator::new(migrations.as_path())
            .await
            .unwrap();
        let versions: std::collections::HashSet<_> = migrator.iter().map(|m| m.version).collect();
        assert_eq!(
            versions.len(),
            migrator.iter().count(),
            "SQLx identifies migrations by numeric prefix, not suffix"
        );
        migrator.run(&pool).await.unwrap();
        migrator.run(&pool).await.unwrap();
        let applied: i64 = sqlx::query_scalar("SELECT count(*) FROM _sqlx_migrations WHERE version IN (20261020190000,20261020200000) AND success=1").fetch_one(&pool).await.unwrap();
        assert_eq!(applied, 2, "{scenario}");
        sqlx::query("SELECT agent_status_owner FROM local_issues LIMIT 1")
            .fetch_optional(&pool)
            .await
            .unwrap();
        sqlx::query("SELECT revision FROM runtime_access_settings")
            .fetch_optional(&pool)
            .await
            .unwrap();
        pool.close().await;
    }
}
