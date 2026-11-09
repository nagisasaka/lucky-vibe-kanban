//! Shared read service for HTTP/stdio and remote MCP. Only registered roots.
use std::path::PathBuf;

use anyhow::{Result, ensure};
use db::models::{repo::Repo, workspace::Workspace, workspace_repo::WorkspaceRepo};
use git::repository_read::{Request, Target};
use serde_json::{Value, json};
use sqlx::SqlitePool;

async fn root(pool: &SqlitePool, request: &Request) -> Result<PathBuf> {
    let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM projects WHERE id=?)")
        .bind(request.project_id)
        .fetch_one(pool)
        .await?;
    ensure!(exists, "Project does not exist");
    ensure!(
        crate::routes::local_remote::project_associated_repository_ids(pool, request.project_id)
            .await?
            .contains(&request.repository_id),
        "Repository does not belong to Project"
    );
    let repo = Repo::find_by_id(pool, request.repository_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Repository not found"))?;
    let path = match &request.target {
        Target::Commit { .. } => repo.path,
        Target::Workspace { workspace_id } => {
            let linked: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM local_workspace_links l JOIN local_issues i ON i.id=l.issue_id AND i.project_id=l.project_id WHERE l.workspace_id=? AND l.project_id=?)")
                .bind(workspace_id).bind(request.project_id).fetch_one(pool).await?;
            ensure!(linked, "Workspace does not belong to Project");
            ensure!(
                WorkspaceRepo::find_by_workspace_and_repo_id(
                    pool,
                    *workspace_id,
                    request.repository_id
                )
                .await?
                .is_some(),
                "Repository does not belong to Workspace"
            );
            let repo =
                WorkspaceRepo::find_repo_for_workspace(pool, *workspace_id, request.repository_id)
                    .await?
                    .ok_or_else(|| anyhow::anyhow!("Repository not found in Workspace"))?;
            let workspace = Workspace::find_by_id(pool, *workspace_id)
                .await?
                .ok_or_else(|| anyhow::anyhow!("Workspace not found"))?;
            ensure!(!workspace.worktree_deleted, "Workspace checkout deleted");
            if workspace.is_direct_folder() {
                repo.path
            } else {
                PathBuf::from(
                    workspace
                        .container_ref
                        .ok_or_else(|| anyhow::anyhow!("Workspace checkout unavailable"))?,
                )
                .join(repo.worktree_name())
            }
        }
    };
    let path = path.canonicalize()?;
    ensure!(path.is_dir(), "Repository unavailable");
    Ok(path)
}
pub(crate) async fn read(pool: &SqlitePool, request: Request) -> Result<Value> {
    let before = root(pool, &request).await?;
    let owned_root = before.clone();
    let owned_request = request.clone();
    let mut value = tokio::task::spawn_blocking(move || {
        git::repository_read::read(&owned_root, &owned_request)
    })
    .await??;
    ensure!(
        root(pool, &request).await? == before,
        "Repository registration changed during read"
    );
    value["project_id"] = json!(request.project_id);
    value["repository_id"] = json!(request.repository_id);
    value["observed_at"] = json!(chrono::Utc::now());
    Ok(value)
}
