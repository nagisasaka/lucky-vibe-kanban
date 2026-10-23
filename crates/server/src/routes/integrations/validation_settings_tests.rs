//! Opt-in real repository settings HTTP boundary and existing script executor.
//! Must run alone with fresh process-wide instance paths; no live DB/repository.
use std::{path::PathBuf, sync::Arc, time::Duration};

use anyhow::{Context, ensure};
use db::models::repo::Repo;
use deployment::Deployment;
use executors::{
    actions::{
        Executable,
        script::{ScriptContext, ScriptRequest, ScriptRequestLanguage},
    },
    approvals::NoopExecutorApprovalService,
    env::{ExecutionEnv, RepoContext},
};
use serde_json::{Value, json};

use super::mechanical;
use crate::DeploymentImpl;

#[tokio::test]
#[ignore = "requires a fresh local130-validation-* LVK_INSTANCE_DIR; run alone"]
async fn isolated_validation_settings_http_and_executor() -> anyhow::Result<()> {
    let instance = PathBuf::from(std::env::var("LVK_INSTANCE_DIR")?);
    ensure!(instance.is_absolute());
    ensure!(
        instance
            .file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("local130-validation-")
    );
    ensure!(
        !instance.join("data/db.v2.sqlite").exists(),
        "Use fresh isolated state"
    );
    ensure!(std::env::var("LVK_ACCESS_DEPLOYMENT").as_deref() == Ok("local"));
    ensure!(std::env::var("LVK_AUTH_MODE").as_deref() == Ok("local"));
    let shutdown = tokio_util::sync::CancellationToken::new();
    let deployment = DeploymentImpl::new(shutdown.clone()).await?;
    ensure!(utils::assets::asset_dir().starts_with(&instance));
    let root = instance.join("fixture-repository");
    std::fs::create_dir(&root)?;
    let git = |args: &[&str]| mechanical::git(&root, args);
    git(&["init", "-b", "main"])?;
    git(&["config", "user.name", "Validation fixture"])?;
    git(&["config", "user.email", "fixture@example.test"])?;
    std::fs::write(root.join("feature"), "fixture\n")?;
    git(&["add", "."])?;
    git(&["commit", "-m", "base"])?;
    let base = git(&["rev-parse", "HEAD"])?;
    let repo = Repo::find_or_create(&deployment.db().pool, &root, "fixture").await?;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let address = listener.local_addr()?;
    let access =
        server_access::Access::from_env(deployment.db().pool.clone(), utils::assets::asset_dir())?;
    // Use the production repository route registration and access/CSRF layer.
    let router = access.layer(
        axum::Router::new()
            .nest("/api", crate::routes::repo::router())
            .with_state(deployment.clone()),
    );
    let stop = shutdown.clone();
    let server = tokio::spawn(async move {
        axum::serve(listener, router)
            .with_graceful_shutdown(async move { stop.cancelled().await })
            .await
    });
    let result = async {
        let client = reqwest::Client::builder().timeout(Duration::from_secs(10)).build()?;
        let url = format!("http://{address}/api/repos/{}/integration-validation", repo.id);
        let initial: Value = client.get(&url).send().await?.error_for_status()?.json().await?;
        ensure!(initial["success"] == true && initial["data"]["content"].is_null());
        ensure!(initial["data"]["branch"] == "refs/heads/main");
        let revision = initial["data"]["revision"].as_str().context("revision")?;
        let marker = instance.join("executed-marker");
        let command = "test -f feature &&\nprintf 'fixture executed\\n' > \"$LVK_VALIDATION_MARKER\"";
        let content = serde_json::to_string(&json!({"commands": [command]}))?;
        let request = json!({"expected_revision": revision, "content": content});
        ensure!(client.put(&url).json(&request).send().await?.status() == reqwest::StatusCode::FORBIDDEN);
        let invalid = json!({"expected_revision": revision, "content": "{\"commands\":[]}"});
        ensure!(client.put(&url).header("x-lvk-csrf", "local").json(&invalid).send().await?.status() == reqwest::StatusCode::BAD_REQUEST);
        ensure!(!root.join(".lvk/integration-validation.json").exists());
        let saved: Value = client.put(&url).header("x-lvk-csrf", "local").json(&request).send().await?.error_for_status()?.json().await?;
        ensure!(saved["success"] == true && saved["data"]["content"] == content);
        ensure!(!marker.exists(), "Saving must not execute commands");
        ensure!(git(&["rev-parse", "HEAD"])? == base);
        ensure!(git(&["diff", "--cached", "--name-only"])?.is_empty());
        let readback: Value = client.get(&url).send().await?.error_for_status()?.json().await?;
        ensure!(saved == readback);
        ensure!(client.put(&url).header("x-lvk-csrf", "local").json(&request).send().await?.status() == reqwest::StatusCode::CONFLICT);
        ensure!(mechanical::validation_plan(&root, &base, false).is_err(), "Uncommitted settings must not affect admission");
        let revision = saved["data"]["revision"].as_str().unwrap();
        {
            let guard = deployment.git().try_publication_guard(&root)?;
            let busy = json!({"expected_revision": revision, "content": "{\"commands\":[\"echo replacement\"]}"});
            ensure!(client.put(&url).header("x-lvk-csrf", "local").json(&busy).send().await?.status() == reqwest::StatusCode::CONFLICT);
            drop(guard);
        }
        ensure!(std::fs::read_to_string(root.join(".lvk/integration-validation.json"))? == content);
        #[cfg(unix)]
        if unsafe { libc::geteuid() } != 0 {
            use std::os::unix::fs::PermissionsExt;
            let directory = root.join(".lvk");
            let permissions = std::fs::metadata(&directory)?.permissions();
            std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o500))?;
            let failing = json!({"expected_revision": revision, "content": "{\"commands\":[\"echo replacement\"]}"});
            let response = client.put(&url).header("x-lvk-csrf", "local").json(&failing).send().await;
            std::fs::set_permissions(&directory, permissions)?;
            ensure!(response?.status() == reqwest::StatusCode::INTERNAL_SERVER_ERROR);
            ensure!(std::fs::read_to_string(root.join(".lvk/integration-validation.json"))? == content);
        }
        git(&["add", ".lvk/integration-validation.json"])?;
        git(&["commit", "-m", "validation settings"])?;
        let frozen = git(&["rev-parse", "HEAD"])?;
        let plan = mechanical::validation_plan(&root, &frozen, false)?;
        ensure!(plan.len() == 1 && plan[0].command == command && plan[0].required);
        ensure!(plan[0].cwd == "." && plan[0].evidence.starts_with(&frozen));
        // A later working-copy edit must not alter this run's frozen plan.
        std::fs::write(root.join(".lvk/integration-validation.json"), "invalid working copy")?;
        ensure!(mechanical::validation_plan(&root, &frozen, false)?[0].command == command);
        ensure!(mechanical::validation_plan(&root, "invalid", true)?.is_empty());
        std::fs::write(root.join(".lvk/integration-validation.json"), &content)?;
        let mut env = ExecutionEnv::new(RepoContext::default(), false, String::new());
        env.insert("LVK_VALIDATION_MARKER", marker.to_string_lossy());
        let action = ScriptRequest {
            script: plan[0].command.clone(),
            language: ScriptRequestLanguage::Bash,
            context: ScriptContext::IntegrationValidation,
            working_dir: Some(plan[0].cwd.clone()),
        };
        let mut spawned = action.spawn(&root, Arc::new(NoopExecutorApprovalService), &env).await?;
        let status = match tokio::time::timeout(Duration::from_secs(10), spawned.child.wait()).await {
            Ok(status) => status?,
            Err(error) => {
                spawned.child.kill().await?;
                spawned.child.wait().await?;
                return Err(error.into());
            }
        };
        ensure!(status.success());
        ensure!(std::fs::read_to_string(&marker)? == "fixture executed\n");
        ensure!(git(&["rev-parse", "HEAD"])? == frozen && git(&["status", "--porcelain"])?.is_empty());
        println!("HTTP GET/PUT/CSRF/400/409/readback/non-execution and frozen-target -> ScriptRequest execution passed; fixture {frozen}");
        anyhow::Ok(())
    }.await;
    shutdown.cancel();
    server.await??;
    result
}
