//! Opt-in production router test, with a fresh task-owned instance and fixture data only.
use std::{fs, path::PathBuf};

use anyhow::ensure;
use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use db::models::repo::Repo;
use deployment::Deployment;
use serde_json::Value;
use tower::ServiceExt;

use crate::DeploymentImpl;

#[tokio::test]
#[ignore = "run alone with a fresh lvk-348-http-* LVK_INSTANCE_DIR and local auth"]
async fn isolated_shared_storage_http() -> anyhow::Result<()> {
    let instance = PathBuf::from(std::env::var("LVK_INSTANCE_DIR")?);
    ensure!(
        instance.is_absolute()
            && instance
                .file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("lvk-348-http-")
    );
    ensure!(!instance.join("data/db.v2.sqlite").exists());
    ensure!(std::env::var("LVK_ACCESS_DEPLOYMENT").as_deref() == Ok("local"));
    ensure!(std::env::var("LVK_AUTH_MODE").as_deref() == Ok("local"));
    ensure!(
        std::env::var("LVK_PUBLIC_URL")
            .unwrap_or_default()
            .is_empty(),
        "Use isolated local HTTP configuration"
    );
    let shutdown = tokio_util::sync::CancellationToken::new();
    let d = DeploymentImpl::new(shutdown.clone()).await?;
    let result = async {
        let source = instance.join("source");
        fs::create_dir(&source)?;
        fs::write(source.join("source.rs"), "keep source")?;
        let repo = Repo::find_or_create(&d.db().pool, &source, "fixture").await?;
        let root = utils::path::shared_resources_dir(&repo.name, repo.id);
        ensure!(root.starts_with(&instance));
        for kind in ["cache", "tmp", "persistent"] {
            fs::create_dir_all(root.join(kind))?;
            fs::write(root.join(kind).join("data"), b"12345")?;
        }
        let other = instance.join("other-repository");
        fs::create_dir(&other)?;
        fs::write(other.join("keep"), b"preserved")?;
        std::os::unix::fs::symlink(&other, root.join("cache/escape"))?;
        let link = source.join(".lvk/shared/cache");
        fs::create_dir_all(link.parent().unwrap())?;
        std::os::unix::fs::symlink(root.join("cache"), &link)?;
        let access = server_access::Access::from_env(d.db().pool.clone(), utils::assets::asset_dir())?;
        let app = access.layer(axum::Router::new().nest("/api", crate::routes::repo::router()).with_state(d.clone()));
        let request = |method: &str, suffix: &str, csrf: bool| {
            let mut req = Request::builder().method(method).uri(format!("/api/repos/{}/shared-storage{suffix}", repo.id)).header("host", "localhost");
            if csrf { req = req.header("x-lvk-csrf", "local"); }
            req.body(Body::empty()).unwrap()
        };
        let response = app.clone().oneshot(request("GET", "", false)).await?;
        ensure!(response.status() == StatusCode::OK);
        let body: Value = serde_json::from_slice(&to_bytes(response.into_body(), 65536).await?)?;
        ensure!(body["data"]["tmp"]["bytes"] == 5);
        ensure!(app.clone().oneshot(request("DELETE", "/cache", false)).await?.status() == StatusCode::FORBIDDEN);
        ensure!(root.join("cache/data").exists());
        for invalid in ["persistent", "source", "%2e%2e%2fpersistent"] {
            let response = app.clone().oneshot(request("DELETE", &format!("/{invalid}"), true)).await?;
            ensure!(response.status().is_client_error());
        }
        let response = app.clone().oneshot(request("DELETE", "/cache", true)).await?;
        ensure!(response.status() == StatusCode::OK);
        let body: Value = serde_json::from_slice(&to_bytes(response.into_body(), 65536).await?)?;
        ensure!(body["data"]["cache"]["bytes"] == 0);
        ensure!(body["data"]["tmp"]["bytes"] == 5);
        ensure!(link.is_dir() && fs::symlink_metadata(&link)?.file_type().is_symlink());
        ensure!(root.join("persistent/data").exists() && source.join("source.rs").exists() && other.join("keep").exists());
        // Replaced fixed target is an error, never deletion of its external target.
        fs::remove_dir(root.join("cache"))?;
        std::os::unix::fs::symlink(&other, root.join("cache"))?;
        ensure!(app.clone().oneshot(request("DELETE", "/cache", true)).await?.status().is_server_error());
        let response = app.oneshot(request("GET", "", false)).await?;
        let body: Value = serde_json::from_slice(&to_bytes(response.into_body(), 65536).await?)?;
        ensure!(body["data"]["cache"]["bytes"].is_null() && body["data"]["cache"]["error"].is_string());
        ensure!(other.join("keep").exists());
        println!("Production shared-storage routes: CSRF, fixed kinds, preserved links/source/persistent/other repo, refreshed sizes and replaced-target failure passed");
        anyhow::Ok(())
    }.await;
    shutdown.cancel();
    result
}
