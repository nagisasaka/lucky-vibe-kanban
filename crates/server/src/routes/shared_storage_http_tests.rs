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
        owner_cleanup_regression(&d, &app, &repo, &root, &other).await?;
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

async fn owner_cleanup_regression(
    d: &DeploymentImpl,
    app: &axum::Router,
    repo: &Repo,
    root: &std::path::Path,
    other_source: &std::path::Path,
) -> anyhow::Result<()> {
    use db::models::{
        workspace::Workspace,
        workspace_repo::{CreateWorkspaceRepo, WorkspaceRepo},
        workspace_usage::{
            OPENWIKI_BOOTSTRAP, OPENWIKI_SYNC, WorkspaceExecutionOwner, WorkspaceExecutionResult,
            WorkspaceExecutionTerminalStatus,
        },
    };
    use uuid::Uuid;

    let pool = &d.db().pool;
    let other = Repo::find_or_create(pool, other_source, "Unrelated fixture").await?;
    let other_root = utils::path::shared_resources_dir(&other.name, other.id);
    for kind in ["cache", "tmp"] {
        fs::create_dir_all(other_root.join(kind))?;
    }
    let delete = |repo_id: Uuid, kind: &str| {
        Request::builder()
            .method("DELETE")
            .uri(format!("/api/repos/{repo_id}/shared-storage/{kind}"))
            .header("host", "localhost")
            .header("x-lvk-csrf", "local")
            .body(Body::empty())
            .unwrap()
    };
    let projects = [Uuid::new_v4(), Uuid::new_v4()];
    for (index, project) in projects.iter().enumerate() {
        sqlx::query("INSERT INTO projects(id,name,key) VALUES(?,'Owner fixture',?)")
            .bind(project)
            .bind(format!("OWNER{index}"))
            .execute(pool)
            .await?;
        sqlx::query("INSERT INTO local_project_statuses(id,project_id,name,color,sort_order) VALUES(?,?,'Todo','blue',0)")
            .bind(Uuid::new_v4()).bind(project).execute(pool).await?;
    }
    for (index, kind) in [OPENWIKI_SYNC, OPENWIKI_BOOTSTRAP].into_iter().enumerate() {
        for status in [
            WorkspaceExecutionTerminalStatus::Succeeded,
            WorkspaceExecutionTerminalStatus::Failed,
            WorkspaceExecutionTerminalStatus::Cancelled,
        ] {
            let owner = WorkspaceExecutionOwner::new(kind, repo.id, None);
            let workspace = crate::routes::workspaces::create::create_workspace_record_with_owner(
                d,
                Some("No-AgentRun maintenance fixture".into()),
                Some(&owner),
            )
            .await?;
            services::services::workspace_usage::ensure_wiki_board_link_for_project(
                pool,
                &workspace,
                Some(projects[index]),
            )
            .await?;
            ensure!(
                sqlx::query_scalar::<_, Uuid>(
                    "SELECT project_id FROM local_workspace_links WHERE workspace_id=?"
                )
                .bind(workspace.id)
                .fetch_one(pool)
                .await?
                    == projects[index]
            );
            // Cover both the preparation gap before membership creation and the
            // bound file-operation owner after membership, without any AgentRun.
            for linked in [false, true] {
                if linked {
                    WorkspaceRepo::create_many(
                        pool,
                        workspace.id,
                        &[CreateWorkspaceRepo {
                            repo_id: repo.id,
                            target_branch: "main".into(),
                        }],
                    )
                    .await?;
                    Workspace::bind_execution_owner(pool, workspace.id, kind, workspace.id).await?;
                }
                for directory in ["cache", "tmp"] {
                    fs::write(root.join(directory).join("data"), b"12345")?;
                    let response = app.clone().oneshot(delete(repo.id, directory)).await?;
                    ensure!(
                        response.status() == StatusCode::CONFLICT,
                        "{kind} linked={linked} must block {directory} cleanup"
                    );
                    ensure!(fs::read(root.join(directory).join("data"))? == b"12345");
                    fs::write(other_root.join(directory).join("data"), b"other")?;
                    ensure!(
                        app.clone()
                            .oneshot(delete(other.id, directory))
                            .await?
                            .status()
                            == StatusCode::OK
                    );
                    ensure!(!other_root.join(directory).join("data").exists());
                }
            }
            // Membership can still identify the target if an older owner lacks its repo ID.
            sqlx::query("UPDATE workspaces SET execution_owner=json_remove(execution_owner,'$.repository_id') WHERE id=?")
                .bind(workspace.id).execute(pool).await?;
            for directory in ["cache", "tmp"] {
                ensure!(
                    app.clone()
                        .oneshot(delete(repo.id, directory))
                        .await?
                        .status()
                        == StatusCode::CONFLICT
                );
                ensure!(root.join(directory).join("data").exists());
            }
            sqlx::query("UPDATE workspaces SET execution_owner=json_set(execution_owner,'$.repository_id',?) WHERE id=?")
                .bind(repo.id.to_string()).bind(workspace.id).execute(pool).await?;
            ensure!(
                sqlx::query_scalar::<_, i64>(
                    "SELECT count(*) FROM agent_runs WHERE workspace_id=?"
                )
                .bind(workspace.id)
                .fetch_one(pool)
                .await?
                    == 0
            );
            Workspace::save_execution_result(
                pool,
                workspace.id,
                kind,
                Some(workspace.id),
                &WorkspaceExecutionResult {
                    status,
                    completed_at: chrono::Utc::now(),
                    error: None,
                    wiki_commit: None,
                    no_op: true,
                },
            )
            .await?;
            // A still-live preparation future must also fence cleanup even if its
            // terminal result was already saved; dropping the existing guard settles it.
            let preparation =
                crate::routes::openwiki::preparation::begin(pool, workspace.id).await?;
            for directory in ["cache", "tmp"] {
                ensure!(
                    app.clone()
                        .oneshot(delete(repo.id, directory))
                        .await?
                        .status()
                        == StatusCode::CONFLICT
                );
                ensure!(root.join(directory).join("data").exists());
            }
            drop(preparation);
            // Preserve historical execution_only owners: completion, not disappearance
            // of the usage label or workspace, must allow manual cleanup again.
            for directory in ["cache", "tmp"] {
                ensure!(
                    app.clone()
                        .oneshot(delete(repo.id, directory))
                        .await?
                        .status()
                        == StatusCode::OK
                );
                ensure!(!root.join(directory).join("data").exists());
            }
        }
    }
    for kind in ["cache", "tmp"] {
        fs::write(root.join(kind).join("data"), b"12345")?;
    }
    println!(
        "No-AgentRun sync/bootstrap owners: before/after membership, both Projects, cache/tmp 409 + preserved contents, all terminal results permit cleanup, unrelated repo unaffected"
    );
    Ok(())
}
