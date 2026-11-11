//! Shared read service for HTTP/stdio and remote MCP. Only registered roots.
use std::path::PathBuf;

use anyhow::{Result, ensure};
use db::models::{repo::Repo, workspace::Workspace, workspace_repo::WorkspaceRepo};
use git::repository_read::{QueryRequest, Request, Target};
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
pub(crate) async fn query(pool: &SqlitePool, request: QueryRequest) -> Result<Value> {
    match request {
        QueryRequest::Project(request) => read(pool, request).await,
        QueryRequest::Bundled(request) => {
            let source = crate::bundled_source::current()?;
            let info = source.clone();
            let mut result =
                tokio::task::spawn_blocking(move || read_bundled(&source, request)).await??;
            result["source"] = serde_json::to_value(info)?;
            result["observed_at"] = json!(chrono::Utc::now());
            Ok(result)
        }
    }
}

fn read_bundled(
    source: &crate::bundled_source::SourceInfo,
    request: git::repository_read::BundledRequest,
) -> Result<Value> {
    let target = request.target.unwrap_or_else(|| Target::Commit {
        r#ref: source.commit.clone(),
    });
    ensure!(
        matches!(target, Target::Commit { .. }),
        "Bundled LVK source supports commit mode only"
    );
    // A distinct, stable resource identity binds continuation cursors. It is
    // never used for DB lookup or to extend any Project's repository access.
    let internal = Request {
        project_id: uuid::Uuid::nil(),
        repository_id: uuid::Uuid::nil(),
        target,
        operation: request.operation,
        limit: request.limit,
        cursor: request.cursor,
        expected_revision: request.expected_revision,
    };
    git::repository_read::read(&source.path, &internal)
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

#[cfg(test)]
mod bundled_tests {
    use git::repository_read::BundledRequest;

    use super::*;

    #[test]
    fn bundled_object_reads_share_budgets_history_and_ignore_dirty_worktree() {
        let temp = tempfile::tempdir().unwrap();
        let repo = git2::Repository::init(temp.path()).unwrap();
        let sig = git2::Signature::now("Author", "author@example.invalid").unwrap();
        let commit = |text: &str, message: &str, parent: Option<git2::Oid>| {
            std::fs::write(temp.path().join("guide.md"), text).unwrap();
            let mut index = repo.index().unwrap();
            index.add_path(std::path::Path::new("guide.md")).unwrap();
            let tree = repo.find_tree(index.write_tree().unwrap()).unwrap();
            let parents: Vec<_> = parent
                .map(|id| repo.find_commit(id).unwrap())
                .into_iter()
                .collect();
            let refs: Vec<_> = parents.iter().collect();
            repo.commit(Some("HEAD"), &sig, &sig, message, &tree, &refs)
                .unwrap()
        };
        let first = commit("Original help\n", "Explain the original problem", None);
        let head = commit("Current help\n", "Update general help", Some(first));
        let source = crate::bundled_source::SourceInfo {
            path: temp.path().to_owned(),
            build_revision: head.to_string(),
            commit: head.to_string(),
            history_scope: "fixture ancestors",
        };
        std::fs::write(temp.path().join("guide.md"), "dirty content\n").unwrap();
        let read = |input: Value| {
            let request: BundledRequest = serde_json::from_value(input).unwrap();
            read_bundled(&source, request)
        };
        let current = read(json!({"source":"lvk","operation":{"kind":"read","path":"guide.md","start_line":1,"end_line":1}})).unwrap();
        assert_eq!(current["items"][0]["lines"][0], "Current help");
        assert_eq!(current["source_head"], head.to_string());
        let old = read(json!({"source":"lvk","target":{"mode":"commit","ref":first.to_string()},"operation":{"kind":"read","path":"guide.md","start_line":1,"end_line":1}})).unwrap();
        assert_eq!(old["items"][0]["lines"][0], "Original help");
        let list = read(json!({"source":"lvk","operation":{"kind":"list"}})).unwrap();
        assert!(list.to_string().contains("guide.md"));
        let grep = read(json!({"source":"lvk","operation":{"kind":"grep","query":"Current","syntax":"literal","case_sensitive":true,"output":"content"}})).unwrap();
        assert!(grep.to_string().contains("Current help"));
        let log = read(json!({"source":"lvk","operation":{"kind":"log","base":null}})).unwrap();
        assert!(log.to_string().contains("Explain the original problem"));
        let diff = read(json!({"source":"lvk","target":{"mode":"commit","ref":head.to_string()},"operation":{"kind":"diff","comparison":"direct","base":null,"end":null,"parent":null,"path":"guide.md"}})).unwrap();
        assert!(diff.to_string().contains("Original help"));
        assert!(diff.to_string().contains("Current help"));
        assert!(read(json!({"source":"lvk","target":{"mode":"workspace","workspace_id":uuid::Uuid::new_v4()},"operation":{"kind":"list"}})).is_err());
        assert!(read(json!({"source":"lvk","operation":{"kind":"read","path":"../outside","start_line":1,"end_line":1}})).is_err());
    }

    #[test]
    fn bundled_selector_cannot_become_an_arbitrary_path_or_project_bypass() {
        for input in [
            json!({"source":"/repos/private","operation":{"kind":"list"}}),
            json!({"source":"lvk","path":"/repos/private","operation":{"kind":"list"}}),
            json!({"source":"lvk","project_id":uuid::Uuid::new_v4(),"operation":{"kind":"list"}}),
            json!({"source":"lvk","repository_id":uuid::Uuid::new_v4(),"operation":{"kind":"list"}}),
        ] {
            assert!(serde_json::from_value::<QueryRequest>(input).is_err());
        }
    }
}
