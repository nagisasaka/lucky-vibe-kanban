use db::models::{repo::Repo, workspace::CreateWorkspace, workspace_repo::CreateWorkspaceRepo};

use super::*;

#[tokio::test]
async fn summary_diff_unknown_and_multi_repo_failure() {
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::migrate!("../db/migrations").run(&pool).await.unwrap();
    let temp = tempfile::TempDir::new().unwrap();
    let git = GitService::new();
    let mut workspace = Workspace::create(
        &pool,
        &CreateWorkspace {
            branch: "feature".into(),
            name: Some("summary fixture".into()),
        },
        Uuid::new_v4(),
    )
    .await
    .unwrap();
    assert!(compute_diff_stats(&pool, &git, &workspace).await.is_none());
    workspace.container_ref = Some(temp.path().to_string_lossy().into());
    for name in ["a", "b"] {
        let path = temp.path().join(name);
        std::fs::create_dir_all(&path).unwrap();
        let fixture_git = git::GitCli::new();
        fixture_git.git(&path, ["init"]).unwrap();
        fixture_git
            .git(&path, ["config", "user.name", "Fixture"])
            .unwrap();
        fixture_git
            .git(&path, ["config", "user.email", "fixture@example.invalid"])
            .unwrap();
        git.initialize_repo_with_main_branch(&path).unwrap();
        let cli = git::GitCli::new();
        cli.git(&path, ["config", "user.name", "Fixture"]).unwrap();
        cli.git(&path, ["config", "user.email", "fixture@example.invalid"])
            .unwrap();
        std::fs::write(path.join("base.txt"), "base\n").unwrap();
        git.commit(&path, "base").unwrap();
        cli.git(&path, ["checkout", "-b", "feature"]).unwrap();
        let repo = Repo::find_or_create(&pool, &path, name).await.unwrap();
        WorkspaceRepo::create_many(
            &pool,
            workspace.id,
            &[CreateWorkspaceRepo {
                repo_id: repo.id,
                target_branch: "main".into(),
            }],
        )
        .await
        .unwrap();
    }
    let zero = compute_diff_stats(&pool, &git, &workspace).await.unwrap();
    assert_eq!(
        (zero.files_changed, zero.lines_added, zero.lines_removed),
        (0, Some(0), Some(0))
    );
    std::fs::write(temp.path().join("a/new.txt"), "new\n").unwrap();
    let text = compute_diff_stats(&pool, &git, &workspace).await.unwrap();
    assert_eq!(
        (text.files_changed, text.lines_added, text.lines_removed),
        (1, Some(1), Some(0))
    );
    std::fs::write(temp.path().join("b/new.bin"), b"binary\0").unwrap();
    let binary = compute_diff_stats(&pool, &git, &workspace).await.unwrap();
    assert_eq!(
        (
            binary.files_changed,
            binary.lines_added,
            binary.lines_removed
        ),
        (2, None, None)
    );
    // One failed repository must invalidate even a nonzero successful subtotal.
    std::fs::remove_dir_all(temp.path().join("b/.git")).unwrap();
    assert!(compute_diff_stats(&pool, &git, &workspace).await.is_none());
    std::fs::remove_dir_all(temp.path().join("a/.git")).unwrap();
    assert!(compute_diff_stats(&pool, &git, &workspace).await.is_none());
    pool.close().await;
}
