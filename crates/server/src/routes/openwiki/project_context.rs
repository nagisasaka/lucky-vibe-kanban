//! Carry an explicit originating Project without changing repository-only admission.
use db::models::workspace::Workspace;
use sqlx::SqlitePool;
use uuid::Uuid;

pub(super) async fn validate(
    pool: &SqlitePool,
    repository_id: Uuid,
    project_id: Option<Uuid>,
) -> anyhow::Result<()> {
    if let Some(project_id) = project_id {
        anyhow::ensure!(
            crate::routes::local_remote::project_exists(pool, project_id).await?,
            "The selected Project no longer exists. Select an existing Project and retry."
        );
        anyhow::ensure!(
            crate::routes::local_remote::project_creation_repository_ids(pool, project_id)
                .await?
                .contains(&repository_id),
            "Repository does not belong to the selected Project. Check its configured working location or linked repositories, then retry."
        );
    }
    Ok(())
}

pub(super) async fn link(
    pool: &SqlitePool,
    workspace: &Workspace,
    project_id: Option<Uuid>,
) -> anyhow::Result<()> {
    let repository_id = workspace
        .execution_owner
        .as_ref()
        .and_then(|owner| owner.repository_id)
        .ok_or_else(|| anyhow::anyhow!("Wiki execution repository is missing"))?;
    // Recheck before linking as working-location defaults may have changed while
    // admission was waiting for the repository lock or OpenWiki version check.
    validate(pool, repository_id, project_id).await?;
    services::services::workspace_usage::ensure_wiki_board_link_for_project(
        pool, workspace, project_id,
    )
    .await
}

#[cfg(test)]
mod tests {
    use db::models::{
        repo::Repo,
        scratch::{
            CreateScratch, DraftWorkspaceRepo, ProjectRepoDefaultsData, Scratch, ScratchPayload,
        },
        session::{CreateSession, Session},
        workspace::CreateWorkspace,
        workspace_usage::{OPENWIKI_BOOTSTRAP, OPENWIKI_SYNC, WorkspaceExecutionOwner},
    };

    use super::*;

    #[tokio::test]
    async fn wiki_project_context_preserves_bootstrap_sync_and_repository_only_admission() {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!("../db/migrations").run(&pool).await.unwrap();
        let repo = Repo::find_or_create(
            &pool,
            std::path::Path::new("/nonexistent/wiki-context-fixture"),
            "Wiki fixture",
        )
        .await
        .unwrap();
        let selected = Uuid::new_v4();
        let other = Uuid::new_v4();
        let unrelated = Uuid::new_v4();
        for project in [selected, other, unrelated] {
            sqlx::query("INSERT INTO projects(id,name) VALUES(?,'Fixture')")
                .bind(project)
                .execute(&pool)
                .await
                .unwrap();
            sqlx::query("INSERT INTO local_project_statuses(id,project_id,name,color,sort_order) VALUES(?,?,'Todo','blue',0)")
                .bind(Uuid::new_v4()).bind(project).execute(&pool).await.unwrap();
        }
        // A newly configured Project need not have any Issue or project_repos row.
        Scratch::create(
            &pool,
            selected,
            &CreateScratch {
                payload: ScratchPayload::ProjectRepoDefaults(ProjectRepoDefaultsData {
                    repos: vec![DraftWorkspaceRepo {
                        repo_id: repo.id,
                        target_branch: "main".into(),
                    }],
                    directory_path: None,
                }),
            },
        )
        .await
        .unwrap();
        // A directory-mode default must not authorise stale Git repo entries.
        Scratch::create(
            &pool,
            unrelated,
            &CreateScratch {
                payload: ScratchPayload::ProjectRepoDefaults(ProjectRepoDefaultsData {
                    repos: vec![DraftWorkspaceRepo {
                        repo_id: repo.id,
                        target_branch: "main".into(),
                    }],
                    directory_path: Some("/nonexistent/directory-mode".into()),
                }),
            },
        )
        .await
        .unwrap();
        let deleted = Uuid::new_v4();
        Scratch::create(
            &pool,
            deleted,
            &CreateScratch {
                payload: ScratchPayload::ProjectRepoDefaults(ProjectRepoDefaultsData {
                    repos: vec![DraftWorkspaceRepo {
                        repo_id: repo.id,
                        target_branch: "main".into(),
                    }],
                    directory_path: None,
                }),
            },
        )
        .await
        .unwrap();
        assert!(
            validate(&pool, repo.id, Some(deleted))
                .await
                .unwrap_err()
                .to_string()
                .contains("no longer exists")
        );
        validate(&pool, repo.id, Some(selected)).await.unwrap();
        assert!(validate(&pool, repo.id, Some(unrelated)).await.is_err());
        validate(&pool, repo.id, None).await.unwrap();

        for kind in [OPENWIKI_BOOTSTRAP, OPENWIKI_SYNC] {
            let ws = Workspace::create_with_owner(
                &pool,
                &CreateWorkspace {
                    branch: Uuid::new_v4().to_string(),
                    name: Some("OpenWiki fixture".into()),
                },
                Uuid::new_v4(),
                Some(&WorkspaceExecutionOwner::new(kind, repo.id, None)),
            )
            .await
            .unwrap();
            // No explicit Project and no inferred Board remains a valid admission.
            link(&pool, &ws, None).await.unwrap();
            assert_eq!(
                sqlx::query_scalar::<_, i64>(
                    "SELECT COUNT(*) FROM local_workspace_links WHERE workspace_id=?"
                )
                .bind(ws.id)
                .fetch_one(&pool)
                .await
                .unwrap(),
                0
            );
            assert!(link(&pool, &ws, Some(unrelated)).await.is_err());
            link(&pool, &ws, Some(selected)).await.unwrap();
            link(&pool, &ws, Some(selected)).await.unwrap();
            assert_eq!(
                sqlx::query_scalar::<_, Uuid>(
                    "SELECT project_id FROM local_workspace_links WHERE workspace_id=?"
                )
                .bind(ws.id)
                .fetch_one(&pool)
                .await
                .unwrap(),
                selected
            );
            // Both preparation branches can continue to ordinary Session creation.
            Session::create(
                &pool,
                &CreateSession {
                    executor: Some("CODEX".into()),
                    name: None,
                },
                Uuid::new_v4(),
                ws.id,
            )
            .await
            .unwrap();
        }
        // A shared Repo must use the explicit Project, never an arbitrary candidate.
        for project in [selected, other] {
            sqlx::query("INSERT INTO project_repos(id,project_id,repo_id) VALUES(?,?,?)")
                .bind(Uuid::new_v4())
                .bind(project)
                .bind(repo.id)
                .execute(&pool)
                .await
                .unwrap();
        }
        for (project, expected) in [(None, None), (Some(other), Some(other))] {
            let ws = Workspace::create_with_owner(
                &pool,
                &CreateWorkspace {
                    branch: Uuid::new_v4().to_string(),
                    name: None,
                },
                Uuid::new_v4(),
                Some(&WorkspaceExecutionOwner::new(OPENWIKI_SYNC, repo.id, None)),
            )
            .await
            .unwrap();
            link(&pool, &ws, project).await.unwrap();
            assert_eq!(
                sqlx::query_scalar::<_, Uuid>(
                    "SELECT project_id FROM local_workspace_links WHERE workspace_id=?"
                )
                .bind(ws.id)
                .fetch_optional(&pool)
                .await
                .unwrap(),
                expected
            );
        }
        // Missing visible statuses retain the existing best-effort behaviour.
        sqlx::query("UPDATE local_project_statuses SET hidden=1 WHERE project_id=?")
            .bind(other)
            .execute(&pool)
            .await
            .unwrap();
        let ws = Workspace::create_with_owner(
            &pool,
            &CreateWorkspace {
                branch: Uuid::new_v4().to_string(),
                name: None,
            },
            Uuid::new_v4(),
            Some(&WorkspaceExecutionOwner::new(
                OPENWIKI_BOOTSTRAP,
                repo.id,
                None,
            )),
        )
        .await
        .unwrap();
        link(&pool, &ws, Some(other)).await.unwrap();
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM local_workspace_links WHERE workspace_id=?"
            )
            .bind(ws.id)
            .fetch_one(&pool)
            .await
            .unwrap(),
            0
        );
    }

    #[test]
    fn wiki_project_context_accepts_legacy_sync_bodies() {
        for body in ["{}", r#"{"from_commit":null}"#, r#"{"from_commit":"abcd"}"#] {
            let request: super::super::SyncRepositoryMemory = serde_json::from_str(body).unwrap();
            assert!(request.project_id.is_none());
        }
    }
}
