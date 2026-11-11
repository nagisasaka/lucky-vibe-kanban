//! Source establishment for fixed user presets, separate from Wiki validation.
use std::path::PathBuf;

use db::models::{
    repository_state::RepositoryMemoryStore, workspace::Workspace, workspace_repo::WorkspaceRepo,
};
use deployment::Deployment;
use serde_json::{Value, json};
use sqlx::SqlitePool;
use uuid::Uuid;
use workflow::{WorkflowGraph, presets::contract};

use crate::{DeploymentImpl, error::ApiError};

pub async fn source_snapshot(pool: &SqlitePool, workspace_id: Uuid) -> Result<Value, ApiError> {
    let workspace = Workspace::find_by_id(pool, workspace_id)
        .await?
        .ok_or_else(|| ApiError::BadRequest("Workspace missing".into()))?;
    let root = PathBuf::from(
        workspace
            .container_ref
            .clone()
            .ok_or_else(|| ApiError::BadRequest("Workspace is not prepared".into()))?,
    );
    let repos = WorkspaceRepo::find_repos_for_workspace(pool, workspace_id).await?;
    if workspace.is_direct_folder() || repos.is_empty() {
        return Err(ApiError::BadRequest(
            "Preset source identity currently requires a Git Workspace".into(),
        ));
    }
    let mut sources = Vec::new();
    for repo in repos {
        let path = root.join(repo.worktree_name());
        let head = git::GitService::new()
            .get_head_info(&path)
            .map_err(|e| ApiError::BadRequest(e.to_string()))?;
        let output = tokio::process::Command::new("git")
            .arg("-C")
            .arg(&path)
            .args([
                "--no-optional-locks",
                "-c",
                "core.fsmonitor=false",
                "-c",
                "core.untrackedCache=false",
                "status",
                "--porcelain=v1",
                "--untracked-files=normal",
            ])
            .output()
            .await
            .map_err(|e| ApiError::BadRequest(e.to_string()))?;
        if !output.status.success() || !output.stdout.is_empty() {
            return Err(ApiError::Conflict(format!(
                "Preset handoff requires established clean source: {}",
                repo.name
            )));
        }
        sources.push(json!({"repository_id":repo.id,"sha":head.oid,"path":path}));
    }
    sources.sort_by_key(|source| {
        source["repository_id"]
            .as_str()
            .unwrap_or_default()
            .to_owned()
    });
    Ok(json!(sources))
}

#[derive(Debug, thiserror::Error)]
#[error("Coding completion is still publishing source")]
pub struct CompletionPending;

pub async fn establish(
    deployment: &DeploymentImpl,
    graph: &WorkflowGraph,
    workspace_id: Uuid,
    node_id: &str,
    agent_run_id: Uuid,
    report: &str,
) -> anyhow::Result<String> {
    if contract(graph).is_none() {
        return Ok(report.into());
    }
    let pool = &deployment.db().pool;
    let node = graph
        .nodes
        .iter()
        .find(|n| n.id == node_id)
        .ok_or_else(|| ApiError::BadRequest("Preset node missing".into()))?;
    let pinned: Option<String> = sqlx::query_scalar(
        "SELECT source_base FROM node_executions WHERE agent_run_id=? AND node_id=? ORDER BY iteration DESC LIMIT 1",
    )
    .bind(agent_run_id)
    .bind(node_id)
    .fetch_optional(pool)
    .await?
    .flatten();
    let mut manifests = Vec::new();
    if node.data.source_read_only == Some(false) {
        // Reuse normal coding completion, including its bounded publication lock.
        for repo in WorkspaceRepo::find_repos_for_workspace(pool, workspace_id).await? {
            if let Some(store) =
                RepositoryMemoryStore::existing_for_repository(pool, &repo.name, repo.id)
                    .await
                    .map_err(|e| ApiError::BadRequest(e.to_string()))?
            {
                let context = store
                    .run(agent_run_id)
                    .map_err(|e| ApiError::BadRequest(e.to_string()))?
                    .ok_or_else(|| {
                        ApiError::BadRequest(
                            "Implementation has no coding completion context".into(),
                        )
                    })?;
                let manifest_path = store
                    .draft_path(agent_run_id)
                    .parent()
                    .and_then(|p| p.parent())
                    .map(|p| p.join("events").join(format!("{agent_run_id}.json")));
                let include = deployment.config().read().await.manifest_in_commit_body;
                let completion = tokio::task::spawn_blocking(move || {
                    tokio::runtime::Handle::current().block_on(
                        services::services::repository_memory::complete_coding_run(
                            &store, &context, include,
                        ),
                    )
                })
                .await?;
                if let Ok(Some(event)) = &completion {
                    manifests.push(json!({"event_id":event.event_id,"source_commit":event.source_commit,"path":manifest_path}));
                }
                if let Err(error) = completion {
                    if error
                        .downcast_ref::<std::io::Error>()
                        .is_some_and(|e| e.kind() == std::io::ErrorKind::WouldBlock)
                    {
                        return Err(CompletionPending.into());
                    }
                    return Err(error);
                }
            }
        }
    }
    if report.trim().is_empty() {
        return Err(ApiError::BadRequest("Preset phase returned no report".into()).into());
    }
    let sources = source_snapshot(pool, workspace_id).await?;
    if node.data.source_read_only == Some(true)
        && pinned
            .as_deref()
            .is_none_or(|v| serde_json::from_str::<Value>(v).ok() != Some(sources.clone()))
    {
        return Err(
            ApiError::Conflict("Protected source changed during reporting phase".into()).into(),
        );
    }
    sqlx::query("UPDATE node_executions SET source_snapshot=? WHERE agent_run_id=?")
        .bind(sources.to_string())
        .bind(agent_run_id)
        .execute(pool)
        .await?;
    Ok(format!(
        "{report}\n\n## Established source handoff\n{}",
        json!({"agent_run_id":agent_run_id,"sources":sources,"phase_base":pinned.and_then(|v|serde_json::from_str::<Value>(&v).ok()),"change_manifests":manifests})
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn git(root: &std::path::Path, args: &[&str]) -> String {
        let output = std::process::Command::new("git")
            .arg("-C")
            .arg(root)
            .args(args)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8_lossy(&output.stdout).trim().into()
    }
    #[tokio::test]
    async fn post_commit_source_identity_and_independent_sessions() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("repo");
        std::fs::create_dir(&root).unwrap();
        git(&root, &["init", "--initial-branch=main"]);
        git(&root, &["config", "user.name", "Preset fixture"]);
        git(&root, &["config", "user.email", "fixture@example.invalid"]);
        std::fs::write(root.join("source.txt"), "base").unwrap();
        git(&root, &["add", "."]);
        git(&root, &["commit", "-m", "base"]);
        let base = git(&root, &["rev-parse", "HEAD"]);
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        db::run_migrations(&pool).await.unwrap();
        let workspace = Uuid::new_v4();
        let repo = Uuid::new_v4();
        sqlx::query("INSERT INTO repos(id,path,name,display_name) VALUES(?,?,'repo','fixture')")
            .bind(repo)
            .bind(root.to_string_lossy().as_ref())
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO workspaces(id,branch,container_ref) VALUES(?,'main',?)")
            .bind(workspace)
            .bind(temp.path().to_string_lossy().as_ref())
            .execute(&pool)
            .await
            .unwrap();
        WorkspaceRepo::create_many(
            &pool,
            workspace,
            &[db::models::workspace_repo::CreateWorkspaceRepo {
                repo_id: repo,
                target_branch: "main".into(),
            }],
        )
        .await
        .unwrap();
        sqlx::query("UPDATE workspace_repos SET worktree_name='repo' WHERE workspace_id=?")
            .bind(workspace)
            .execute(&pool)
            .await
            .unwrap();
        let first = source_snapshot(&pool, workspace).await.unwrap();
        assert_eq!(first[0]["sha"], base);
        std::fs::write(root.join("source.txt"), "implementation").unwrap();
        assert!(source_snapshot(&pool, workspace).await.is_err());
        git(&root, &["add", "."]);
        git(&root, &["commit", "-m", "implementation"]);
        let completed = source_snapshot(&pool, workspace).await.unwrap();
        assert_ne!(completed[0]["sha"], base);
        assert_eq!(completed[0]["sha"], git(&root, &["rev-parse", "HEAD"]));
        let request =
            serde_json::from_value(serde_json::json!({"preset":"implement_review"})).unwrap();
        let mut graph = workflow::presets::compile(request).unwrap();
        crate::routes::workflows::ensure_agent_node_sessions(&pool, workspace, &mut graph)
            .await
            .unwrap();
        let sessions = graph
            .nodes
            .iter()
            .filter_map(|n| n.data.session_id.clone())
            .collect::<std::collections::HashSet<_>>();
        assert_eq!(sessions.len(), 2);
        for session in sessions {
            let bound: Uuid = sqlx::query_scalar("SELECT workspace_id FROM sessions WHERE id=?")
                .bind(Uuid::parse_str(&session).unwrap())
                .fetch_one(&pool)
                .await
                .unwrap();
            assert_eq!(bound, workspace);
        }
    }
}
