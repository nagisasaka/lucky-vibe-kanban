use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqlitePool};
use ts_rs::TS;
use uuid::Uuid;

use super::repo::Repo;

#[derive(Debug, Clone, FromRow, Serialize, Deserialize, TS)]
pub struct WorkspaceRepo {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub repo_id: Uuid,
    pub target_branch: String,
    pub worktree_name: Option<String>,
    #[ts(type = "Date")]
    pub created_at: DateTime<Utc>,
    #[ts(type = "Date")]
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, TS)]
pub struct CreateWorkspaceRepo {
    pub repo_id: Uuid,
    pub target_branch: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct RepoWithTargetBranch {
    #[serde(flatten)]
    pub repo: Repo,
    pub target_branch: String,
}

/// Repo info with copy_files configuration.
#[derive(Debug, Clone)]
pub struct RepoWithCopyFiles {
    pub id: Uuid,
    pub path: PathBuf,
    pub name: String,
    pub copy_files: Option<String>,
    pub worktree_name: Option<String>,
}

impl RepoWithCopyFiles {
    pub fn worktree_dir(&self, root: impl AsRef<std::path::Path>) -> PathBuf {
        root.as_ref()
            .join(self.worktree_name.as_deref().unwrap_or(&self.name))
    }
}

impl WorkspaceRepo {
    pub async fn find_repo_for_workspace(
        pool: &SqlitePool,
        workspace_id: Uuid,
        repo_id: Uuid,
    ) -> Result<Option<Repo>, sqlx::Error> {
        Ok(Self::find_repos_for_workspace(pool, workspace_id)
            .await?
            .into_iter()
            .find(|r| r.id == repo_id))
    }

    pub async fn create_many(
        pool: &SqlitePool,
        workspace_id: Uuid,
        repos: &[CreateWorkspaceRepo],
    ) -> Result<Vec<Self>, sqlx::Error> {
        if repos.is_empty() {
            return Ok(Vec::new());
        }

        let mut tx = pool.begin_with("BEGIN IMMEDIATE").await?;
        let result = Self::create_many_in_transaction(&mut tx, workspace_id, repos).await?;
        tx.commit().await?;
        Ok(result)
    }

    /// Share allocation with callers whose Workspace creation is already atomic.
    /// The caller must hold the SQLite writer lock before reading placements.
    pub(crate) async fn create_many_in_transaction(
        connection: &mut sqlx::SqliteConnection,
        workspace_id: Uuid,
        repos: &[CreateWorkspaceRepo],
    ) -> Result<Vec<Self>, sqlx::Error> {
        let occupied: Vec<String> = sqlx::query_scalar("SELECT COALESCE(wr.worktree_name, r.name) FROM workspace_repos wr JOIN repos r ON r.id = wr.repo_id WHERE wr.workspace_id = ?")
            .bind(workspace_id).fetch_all(&mut *connection).await?;
        let mut requested = Vec::with_capacity(repos.len());
        for repo in repos {
            let name: String = sqlx::query_scalar("SELECT name FROM repos WHERE id = ?")
                .bind(repo.repo_id)
                .fetch_one(&mut *connection)
                .await?;
            requested.push((repo.repo_id.to_string(), name));
        }
        let placements = utils::path::allocate_workspace_repo_names(&requested, &occupied);
        let mut results = Vec::with_capacity(repos.len());

        for repo in repos {
            let id = Uuid::new_v4();
            let worktree_name = &placements[&repo.repo_id.to_string()];
            let workspace_repo = sqlx::query_as!(
                WorkspaceRepo,
                r#"INSERT INTO workspace_repos (id, workspace_id, repo_id, target_branch, worktree_name)
                   VALUES ($1, $2, $3, $4, $5)
                   RETURNING id as "id!: Uuid",
                             workspace_id as "workspace_id!: Uuid",
                             repo_id as "repo_id!: Uuid",
                             target_branch,
                             worktree_name,
                             created_at as "created_at!: DateTime<Utc>",
                             updated_at as "updated_at!: DateTime<Utc>""#,
                id,
                workspace_id,
                repo.repo_id,
                repo.target_branch,
                worktree_name
            )
            .fetch_one(&mut *connection)
            .await?;
            results.push(workspace_repo);
        }

        Ok(results)
    }

    pub async fn find_by_workspace_id(
        pool: &SqlitePool,
        workspace_id: Uuid,
    ) -> Result<Vec<Self>, sqlx::Error> {
        sqlx::query_as!(
            WorkspaceRepo,
            r#"SELECT id as "id!: Uuid",
                      workspace_id as "workspace_id!: Uuid",
                      repo_id as "repo_id!: Uuid",
                      target_branch,
                      worktree_name,
                      created_at as "created_at!: DateTime<Utc>",
                      updated_at as "updated_at!: DateTime<Utc>"
               FROM workspace_repos
               WHERE workspace_id = $1"#,
            workspace_id
        )
        .fetch_all(pool)
        .await
    }

    pub async fn find_repos_for_workspace(
        pool: &SqlitePool,
        workspace_id: Uuid,
    ) -> Result<Vec<Repo>, sqlx::Error> {
        sqlx::query_as!(
            Repo,
            r#"SELECT r.id as "id!: Uuid",
                      r.path,
                      r.name,
                      r.display_name,
                      wr.worktree_name,
                      r.setup_script,
                      r.cleanup_script,
                      r.archive_script,
                      r.copy_files,
                      r.parallel_setup_script as "parallel_setup_script!: bool",
                      r.dev_server_script,
                      r.default_target_branch,
                      r.default_working_dir,
                      r.created_at as "created_at!: DateTime<Utc>",
                      r.updated_at as "updated_at!: DateTime<Utc>"
               FROM repos r
               JOIN workspace_repos wr ON r.id = wr.repo_id
               WHERE wr.workspace_id = $1
               ORDER BY r.display_name ASC"#,
            workspace_id
        )
        .fetch_all(pool)
        .await
    }

    pub async fn find_repos_with_target_branch_for_workspace(
        pool: &SqlitePool,
        workspace_id: Uuid,
    ) -> Result<Vec<RepoWithTargetBranch>, sqlx::Error> {
        let rows = sqlx::query!(
            r#"SELECT r.id as "id!: Uuid",
                      r.path,
                      r.name,
                      r.display_name,
                      wr.worktree_name,
                      r.setup_script,
                      r.cleanup_script,
                      r.archive_script,
                      r.copy_files,
                      r.parallel_setup_script as "parallel_setup_script!: bool",
                      r.dev_server_script,
                      r.default_target_branch,
                      r.default_working_dir,
                      r.created_at as "created_at!: DateTime<Utc>",
                      r.updated_at as "updated_at!: DateTime<Utc>",
                      wr.target_branch
               FROM repos r
               JOIN workspace_repos wr ON r.id = wr.repo_id
               WHERE wr.workspace_id = $1
               ORDER BY r.display_name ASC"#,
            workspace_id
        )
        .fetch_all(pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|row| RepoWithTargetBranch {
                repo: Repo {
                    id: row.id,
                    path: PathBuf::from(row.path),
                    name: row.name,
                    display_name: row.display_name,
                    worktree_name: row.worktree_name,
                    setup_script: row.setup_script,
                    cleanup_script: row.cleanup_script,
                    archive_script: row.archive_script,
                    copy_files: row.copy_files,
                    parallel_setup_script: row.parallel_setup_script,
                    dev_server_script: row.dev_server_script,
                    default_target_branch: row.default_target_branch,
                    default_working_dir: row.default_working_dir,
                    created_at: row.created_at,
                    updated_at: row.updated_at,
                },
                target_branch: row.target_branch,
            })
            .collect())
    }

    pub async fn find_by_workspace_and_repo_id(
        pool: &SqlitePool,
        workspace_id: Uuid,
        repo_id: Uuid,
    ) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as!(
            WorkspaceRepo,
            r#"SELECT id as "id!: Uuid",
                      workspace_id as "workspace_id!: Uuid",
                      repo_id as "repo_id!: Uuid",
                      target_branch,
                      worktree_name,
                      created_at as "created_at!: DateTime<Utc>",
                      updated_at as "updated_at!: DateTime<Utc>"
               FROM workspace_repos
               WHERE workspace_id = $1 AND repo_id = $2"#,
            workspace_id,
            repo_id
        )
        .fetch_optional(pool)
        .await
    }

    pub async fn update_target_branch(
        pool: &SqlitePool,
        workspace_id: Uuid,
        repo_id: Uuid,
        new_target_branch: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query!(
            "UPDATE workspace_repos SET target_branch = $1, updated_at = datetime('now') WHERE workspace_id = $2 AND repo_id = $3",
            new_target_branch,
            workspace_id,
            repo_id
        )
        .execute(pool)
        .await?;
        Ok(())
    }

    pub async fn update_target_branch_for_children_of_workspace(
        pool: &SqlitePool,
        parent_workspace_id: Uuid,
        old_branch: &str,
        new_branch: &str,
    ) -> Result<u64, sqlx::Error> {
        let result = sqlx::query!(
            r#"UPDATE workspace_repos
               SET target_branch = $1, updated_at = datetime('now')
               WHERE target_branch = $2
                 AND workspace_id IN (
                     SELECT w.id FROM workspaces w
                     JOIN tasks t ON w.task_id = t.id
                     WHERE t.parent_workspace_id = $3
                 )"#,
            new_branch,
            old_branch,
            parent_workspace_id
        )
        .execute(pool)
        .await?;
        Ok(result.rows_affected())
    }

    /// Find repos for a workspace with their copy_files configuration.
    pub async fn find_repos_with_copy_files(
        pool: &SqlitePool,
        workspace_id: Uuid,
    ) -> Result<Vec<RepoWithCopyFiles>, sqlx::Error> {
        let rows = sqlx::query!(
            r#"SELECT r.id as "id!: Uuid", r.path, r.name, r.copy_files, wr.worktree_name
               FROM repos r
               JOIN workspace_repos wr ON r.id = wr.repo_id
               WHERE wr.workspace_id = $1"#,
            workspace_id
        )
        .fetch_all(pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|row| RepoWithCopyFiles {
                id: row.id,
                path: PathBuf::from(row.path),
                name: row.name,
                copy_files: row.copy_files,
                worktree_name: row.worktree_name,
            })
            .collect())
    }
}

#[cfg(test)]
mod placement_tests {
    use super::*;
    use crate::models::workspace::{CreateWorkspace, Workspace};

    #[tokio::test]
    async fn migration_freezes_legacy_names_without_reconstructing_workspace() {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        crate::run_migrations(&pool).await.unwrap();
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("existing");
        std::fs::create_dir_all(&source).unwrap();
        let repo = Repo::find_or_create(&pool, &source, "Existing")
            .await
            .unwrap();
        let id = Uuid::new_v4();
        let before = Workspace::create(
            &pool,
            &CreateWorkspace {
                branch: "old-branch".into(),
                name: Some("Old work".into()),
            },
            id,
        )
        .await
        .unwrap();
        WorkspaceRepo::create_many(
            &pool,
            id,
            &[CreateWorkspaceRepo {
                repo_id: repo.id,
                target_branch: "old-target".into(),
            }],
        )
        .await
        .unwrap();
        sqlx::query("ALTER TABLE workspace_repos DROP COLUMN worktree_name")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::raw_sql(include_str!(
            "../../migrations/20261108140000_workspace_repo_placement.sql"
        ))
        .execute(&pool)
        .await
        .unwrap();
        Repo::update_name(&pool, repo.id, "new-name", "New display")
            .await
            .unwrap();
        let placed = WorkspaceRepo::find_repo_for_workspace(&pool, id, repo.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(placed.worktree_name(), "existing");
        let after = Workspace::find_by_id(&pool, id).await.unwrap().unwrap();
        assert_eq!(before.branch, after.branch);
        assert_eq!(before.created_at, after.created_at);
        assert_eq!(before.name, after.name);
        assert_eq!(
            WorkspaceRepo::find_by_workspace_and_repo_id(&pool, id, repo.id)
                .await
                .unwrap()
                .unwrap()
                .target_branch,
            "old-target"
        );
        pool.close().await;
    }

    #[tokio::test]
    async fn placements_survive_reload_additions_and_repository_rename() {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        crate::run_migrations(&pool).await.unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let mut repos = Vec::new();
        for parent in ["one", "two", "three"] {
            let path = tmp.path().join(parent).join("same");
            std::fs::create_dir_all(&path).unwrap();
            repos.push(Repo::find_or_create(&pool, &path, "same").await.unwrap());
        }
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();
        // Use explicit values rather than relying on query ordering.
        Workspace::create(
            &pool,
            &CreateWorkspace {
                branch: "a".into(),
                name: None,
            },
            a,
        )
        .await
        .unwrap();
        Workspace::create(
            &pool,
            &CreateWorkspace {
                branch: "b".into(),
                name: None,
            },
            b,
        )
        .await
        .unwrap();
        let inputs: Vec<_> = repos[..2]
            .iter()
            .map(|r| CreateWorkspaceRepo {
                repo_id: r.id,
                target_branch: "main".into(),
            })
            .collect();
        WorkspaceRepo::create_many(&pool, a, &inputs).await.unwrap();
        let mut reversed = inputs.clone();
        reversed.reverse();
        WorkspaceRepo::create_many(&pool, b, &reversed)
            .await
            .unwrap();
        let placements = |rows: Vec<Repo>| {
            rows.into_iter()
                .map(|r| (r.id, r.worktree_name().to_owned()))
                .collect::<std::collections::BTreeMap<_, _>>()
        };
        let original = placements(
            WorkspaceRepo::find_repos_for_workspace(&pool, a)
                .await
                .unwrap(),
        );
        assert_eq!(
            original,
            placements(
                WorkspaceRepo::find_repos_for_workspace(&pool, b)
                    .await
                    .unwrap()
            )
        );
        assert_ne!(original[&repos[0].id], original[&repos[1].id]);
        WorkspaceRepo::create_many(
            &pool,
            a,
            &[CreateWorkspaceRepo {
                repo_id: repos[2].id,
                target_branch: "other".into(),
            }],
        )
        .await
        .unwrap();
        Repo::update_name(&pool, repos[0].id, "renamed", "New display")
            .await
            .unwrap();
        for (id, placement) in original {
            let reloaded = WorkspaceRepo::find_repo_for_workspace(&pool, a, id)
                .await
                .unwrap()
                .unwrap();
            assert_eq!(reloaded.worktree_name(), placement);
        }
        let copy = WorkspaceRepo::find_repos_with_copy_files(&pool, a)
            .await
            .unwrap();
        let branch = WorkspaceRepo::find_repos_with_target_branch_for_workspace(&pool, a)
            .await
            .unwrap();
        for r in branch {
            assert_eq!(
                copy.iter()
                    .find(|c| c.id == r.repo.id)
                    .unwrap()
                    .worktree_dir(tmp.path()),
                r.repo.worktree_dir(tmp.path())
            );
        }
        assert_eq!(
            Repo::find_by_id(&pool, repos[0].id)
                .await
                .unwrap()
                .unwrap()
                .worktree_name,
            None
        );
        pool.close().await;
    }
}
