use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use serde_json::{Value, json};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use tower::ServiceExt;

use super::*;

async fn database(path: &std::path::Path) -> SqlitePool {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            SqliteConnectOptions::new()
                .filename(path)
                .create_if_missing(true)
                .foreign_keys(true),
        )
        .await
        .unwrap();
    db::run_migrations(&pool).await.unwrap();
    pool
}
async fn fixture(pool: &SqlitePool) -> Uuid {
    let project = Uuid::new_v4();
    let status = Uuid::new_v4();
    sqlx::query("INSERT INTO projects(id,name,key) VALUES(?,'source project','SRC')")
        .bind(project)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO local_project_statuses(id,project_id,name,color,sort_order) VALUES(?,?,'Done','blue',1)").bind(status).bind(project).execute(pool).await.unwrap();
    sqlx::query("INSERT INTO local_issues(id,project_id,issue_number,simple_id,status_id,title,description,sort_order) VALUES(?,?,1,'SRC-1',?,'source issue','unaltered body',1)").bind(Uuid::new_v4()).bind(project).bind(status).execute(pool).await.unwrap();
    project
}
struct Client {
    app: Router,
    cookie: String,
    csrf: String,
    _directory: tempfile::TempDir,
    auth: SqlitePool,
}
impl Client {
    async fn new(pool: SqlitePool) -> Self {
        let directory = tempfile::tempdir().unwrap();
        let auth = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        let access = server_access::test_support::access(
            auth.clone(),
            directory.path().to_owned(),
            json!({"keys":[]}),
        )
        .await;
        let cookie = server_access::test_support::web_session(&access).await;
        let csrf = sqlx::query_scalar("SELECT csrf FROM access_sessions LIMIT 1")
            .fetch_one(&auth)
            .await
            .unwrap();
        let app = access.layer(Router::new().nest("/api", pool_router(pool)));
        Self {
            app,
            cookie,
            csrf,
            _directory: directory,
            auth,
        }
    }
    async fn call(
        &self,
        method: &str,
        path: &str,
        body: Vec<u8>,
        kind: &str,
        authenticated: bool,
        csrf: bool,
    ) -> Response {
        let mut request = Request::builder()
            .method(method)
            .uri(path)
            .header("host", "203.0.113.7:8443")
            .header("origin", "https://203.0.113.7:8443")
            .header("content-type", kind);
        if authenticated {
            request = request.header("cookie", &self.cookie);
        }
        if csrf {
            request = request.header("x-lvk-csrf", &self.csrf);
        }
        self.app
            .clone()
            .oneshot(request.body(Body::from(body)).unwrap())
            .await
            .unwrap()
    }
}
async fn value(response: Response) -> Value {
    serde_json::from_slice(
        &to_bytes(response.into_body(), 48 * 1024 * 1024)
            .await
            .unwrap(),
    )
    .unwrap()
}
#[tokio::test]
async fn project_transfer_http_preserves_auth_csrf_and_exact_preview_bytes() {
    let source_root = tempfile::tempdir().unwrap();
    let destination = tempfile::tempdir().unwrap();
    let source = database(&source_root.path().join("source.sqlite")).await;
    let project = fixture(&source).await;
    let source_client = Client::new(source.clone()).await;
    let export_url = format!("/api/projects/{project}/transfer/export");
    for (authenticated, csrf, expected) in [
        (false, false, StatusCode::UNAUTHORIZED),
        (true, false, StatusCode::FORBIDDEN),
    ] {
        assert_eq!(
            source_client
                .call(
                    "POST",
                    &export_url,
                    b"{}".to_vec(),
                    "application/json",
                    authenticated,
                    csrf
                )
                .await
                .status(),
            expected
        );
    }
    let exported = source_client
        .call(
            "POST",
            &export_url,
            b"{}".to_vec(),
            "application/json",
            true,
            true,
        )
        .await;
    assert_eq!(exported.status(), StatusCode::OK);
    assert_eq!(
        exported.headers()[header::CONTENT_TYPE],
        "application/vnd.sqlite3"
    );
    assert_eq!(exported.headers()[header::CACHE_CONTROL], "no-store");
    let bytes = to_bytes(exported.into_body(), FILE_LIMIT).await.unwrap();
    drop(source_client);
    source.close().await;
    source_root.close().unwrap();
    let path = destination.path().join("destination.sqlite");
    let dest = database(&path).await;
    let existing = fixture(&dest).await;
    let client = Client::new(dest.clone()).await;
    let preview = client
        .call(
            "POST",
            "/api/project-transfer/preview",
            bytes.to_vec(),
            "application/octet-stream",
            true,
            true,
        )
        .await;
    assert_eq!(preview.status(), StatusCode::OK);
    assert_eq!(preview.headers()[header::CACHE_CONTROL], "no-store");
    let preview = value(preview).await["data"].clone();
    assert_eq!(preview["sha256"], format!("{:x}", Sha256::digest(&bytes)));
    assert_eq!(preview["history_included"], false);
    let mut request = json!({"data_base64":STANDARD.encode(&bytes),"expected_sha256":"0".repeat(64),"key":"NEW","repo_map":{}});
    let wrong = client
        .call(
            "POST",
            "/api/project-transfer/import",
            request.to_string().into_bytes(),
            "application/json",
            true,
            true,
        )
        .await;
    assert_eq!(wrong.status(), StatusCode::CONFLICT);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM projects")
            .fetch_one(&dest)
            .await
            .unwrap(),
        1
    );
    request["expected_sha256"] = preview["sha256"].clone();
    let imported = client
        .call(
            "POST",
            "/api/project-transfer/import",
            request.to_string().into_bytes(),
            "application/json",
            true,
            true,
        )
        .await;
    assert_eq!(imported.status(), StatusCode::OK);
    let result = value(imported).await["data"].clone();
    let new_id = result["project_id"].as_str().unwrap();
    let report = client
        .call(
            "GET",
            &format!("/api/projects/{new_id}/transfer/report"),
            vec![],
            "application/json",
            true,
            false,
        )
        .await;
    assert_eq!(report.status(), StatusCode::OK);
    assert_eq!(value(report).await["data"]["project_id"], new_id);
    let history = client
        .call(
            "GET",
            &format!("/api/projects/{new_id}/transfer/history"),
            vec![],
            "application/json",
            true,
            false,
        )
        .await;
    assert_eq!(value(history).await["data"], json!([]));
    let retry = client
        .call(
            "POST",
            "/api/project-transfer/import",
            request.to_string().into_bytes(),
            "application/json",
            true,
            true,
        )
        .await;
    assert_eq!(retry.status(), StatusCode::CONFLICT);
    let unchanged: String = sqlx::query_scalar("SELECT name FROM projects WHERE id=?")
        .bind(existing)
        .fetch_one(&dest)
        .await
        .unwrap();
    assert_eq!(unchanged, "source project");
    let unknown = client
        .call(
            "GET",
            "/api/unknown-transfer-route",
            vec![],
            "application/json",
            true,
            false,
        )
        .await;
    assert_eq!(unknown.status(), StatusCode::NOT_FOUND);
    client.auth.close().await;
    drop(client);
    dest.close().await;
    let restarted = database(&path).await;
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM projects")
            .fetch_one(&restarted)
            .await
            .unwrap(),
        2
    );
    restarted.close().await;
}
#[test]
fn import_rejects_changed_bytes_even_if_semantically_equivalent() {
    let bytes = b"SQLite bytes as previewed";
    let input = Import {
        data_base64: STANDARD.encode(bytes),
        expected_sha256: format!("{:x}", Sha256::digest(bytes)),
        key: "NEW".into(),
        repo_map: BTreeMap::new(),
    };
    assert_eq!(bound_bytes(&input).unwrap(), bytes);
    let changed = Import {
        data_base64: STANDARD.encode(b"SQLite bytes as previewed "),
        ..input
    };
    assert!(bound_bytes(&changed).is_err());
}

#[tokio::test]
async fn completed_archive_http_roundtrip_remaps_links_and_survives_reopen() {
    let source_root = tempfile::tempdir().unwrap();
    let destination = tempfile::tempdir().unwrap();
    let source = database(&source_root.path().join("source.sqlite")).await;
    let project = fixture(&source).await;
    let issue: Uuid = sqlx::query_scalar("SELECT id FROM local_issues WHERE project_id=?")
        .bind(project)
        .fetch_one(&source)
        .await
        .unwrap();
    let repo = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO repos(id,path,name,display_name) VALUES(?,'/old-root/repo','repo','repo')",
    )
    .bind(repo)
    .execute(&source)
    .await
    .unwrap();
    sqlx::query("INSERT INTO project_repos(id,project_id,repo_id) VALUES(?,?,?)")
        .bind(Uuid::new_v4())
        .bind(project)
        .bind(repo)
        .execute(&source)
        .await
        .unwrap();
    let run = Uuid::new_v4();
    let body = "Saved complete answer ![missing](attachment://old-id) /old-root/data";
    // A prior imported display archive is re-exported through the real HTTP
    // endpoint. Original runtime-to-display projection has separate DB coverage.
    let archive = json!({"issue_id":issue,"workspace_id":Uuid::new_v4(),"session_id":Uuid::new_v4(),"run_id":run,"status":"succeeded","messages":[{"role":"user","content":"saved question"},{"role":"assistant","content":body}],"manifests":[{"id":run,"repo_id":repo,"semantics":{"goal":"portable goal","summary":"saved semantic Manifest"},"created_at":"2026-11-16T00:00:00Z","base_commit":"a".repeat(40),"source_commit":"b".repeat(40),"target_branch":"main","changed_paths":["src/app.rs"]}]});
    sqlx::query(
        "INSERT INTO project_transfer_history(project_id,issue_id,run_id,payload) VALUES(?,?,?,?)",
    )
    .bind(project)
    .bind(issue)
    .bind(run)
    .bind(archive.to_string())
    .execute(&source)
    .await
    .unwrap();
    let client = Client::new(source.clone()).await;
    let uri = format!("/api/projects/{project}/transfer/export");
    let opt_out = client
        .call("POST", &uri, b"{}".to_vec(), "application/json", true, true)
        .await;
    assert_eq!(opt_out.status(), StatusCode::OK);
    let opt_out = to_bytes(opt_out.into_body(), FILE_LIMIT).await.unwrap();
    assert!(!String::from_utf8_lossy(&opt_out).contains(body));
    let opt_in = client
        .call(
            "POST",
            &uri,
            br#"{"include_history":true}"#.to_vec(),
            "application/json",
            true,
            true,
        )
        .await;
    assert_eq!(opt_in.status(), StatusCode::OK);
    let bytes = to_bytes(opt_in.into_body(), FILE_LIMIT).await.unwrap();
    client.auth.close().await;
    drop(client);
    source.close().await;
    source_root.close().unwrap();
    let path = destination.path().join("destination.sqlite");
    let dest = database(&path).await;
    let target_repo = Uuid::new_v4();
    sqlx::query("INSERT INTO repos(id,path,name,display_name) VALUES(?,'/new-root/prepared','prepared','prepared')").bind(target_repo).execute(&dest).await.unwrap();
    let client = Client::new(dest.clone()).await;
    let preview = value(
        client
            .call(
                "POST",
                "/api/project-transfer/preview",
                bytes.to_vec(),
                "application/octet-stream",
                true,
                true,
            )
            .await,
    )
    .await["data"]
        .clone();
    assert_eq!(preview["history_count"], 1);
    assert_eq!(preview["manifest_count"], 1);
    let request = json!({"data_base64":STANDARD.encode(&bytes),"expected_sha256":preview["sha256"],"key":"PORT","repo_map":{repo.to_string():target_repo}});
    let imported = client
        .call(
            "POST",
            "/api/project-transfer/import",
            request.to_string().into_bytes(),
            "application/json",
            true,
            true,
        )
        .await;
    assert_eq!(imported.status(), StatusCode::OK);
    let project = value(imported).await["data"]["project_id"]
        .as_str()
        .unwrap()
        .to_owned();
    client.auth.close().await;
    drop(client);
    dest.close().await;
    let restarted = database(&path).await;
    let client = Client::new(restarted.clone()).await;
    let response = client
        .call(
            "GET",
            &format!("/api/projects/{project}/transfer/history"),
            vec![],
            "application/json",
            true,
            false,
        )
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    let history = value(response).await["data"].clone();
    assert_eq!(history[0]["messages"][1]["content"], body);
    assert_ne!(history[0]["issue_id"], issue.to_string());
    assert_ne!(history[0]["run_id"], run.to_string());
    assert_eq!(history[0]["manifests"][0]["id"], history[0]["run_id"]);
    assert_eq!(
        history[0]["manifests"][0]["repo_id"],
        target_repo.to_string()
    );
    assert_eq!(
        history[0]["manifests"][0]["semantics"]["summary"],
        "saved semantic Manifest"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM agent_runs")
            .fetch_one(&restarted)
            .await
            .unwrap(),
        0
    );
    client.auth.close().await;
    drop(client);
    restarted.close().await;
}
