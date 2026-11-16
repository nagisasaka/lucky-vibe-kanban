use db::models::integration::{IntegrationPayload, IntegrationSelection, IntegrationSource};

use super::*;

async fn fixture() -> anyhow::Result<(tempfile::TempDir, sqlx::SqlitePool, IntegrationRun)> {
    let dir = tempfile::tempdir()?;
    let root = dir.path();
    for args in [
        vec!["init", "-b", "candidate"],
        vec!["config", "user.name", "Fixture"],
        vec!["config", "user.email", "fixture@example.invalid"],
    ] {
        support::git(root, &args)?;
    }
    std::fs::write(root.join("code"), "adopted code")?;
    std::fs::write(root.join(".gitignore"), "docs/changes/\n")?;
    support::git(root, &["add", "."])?;
    support::git(root, &["commit", "-m", "source"])?;
    let head = support::git(root, &["rev-parse", "HEAD"])?;
    // Task-owned SQLite receipt fixture: no running service, provider, or live DB.
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await?;
    sqlx::query("CREATE TABLE integration_runs(id BLOB PRIMARY KEY,status TEXT,workspace_id BLOB,session_id BLOB,agent_run_id BLOB,payload TEXT,error TEXT,updated_at TEXT)").execute(&pool).await?;
    let issue_id = Uuid::new_v4();
    let snapshot = IntegrationIssueSnapshot {
        issue_id,
        simple_id: "LOCAL-399".into(),
        title: "Intent: \"keep\"\nthis".into(),
        version_id: Uuid::new_v4(),
        version_revision: 7,
        body: "## 人間の意図\n原文を残す。\n\n進行メモも原文。\n".into(),
        frozen_at: "2026-11-16T23:59:59+00:00".into(),
    };
    let run = IntegrationRun {
        id: Uuid::new_v4(),
        request_key: "archive-fixture".into(),
        project_id: Uuid::new_v4(),
        repository_id: Uuid::new_v4(),
        storage_identity: root.display().to_string(),
        target_ref: "refs/heads/main".into(),
        status: "integrating".into(),
        workspace_id: None,
        session_id: None,
        agent_run_id: None,
        payload: sqlx::types::Json(IntegrationPayload {
            issue_archive_enabled: true,
            observed_target: head.clone(),
            sources: vec![IntegrationSource {
                selection: IntegrationSelection {
                    card_id: issue_id,
                    workspace_id: Uuid::new_v4(),
                    expected_commit: head.clone(),
                },
                branch: "source".into(),
                commit: head,
                title: snapshot.title.clone(),
                description: Some("compatibility field is not the archive source".into()),
                issue_snapshot: Some(snapshot),
                retry_context: Some("DO NOT ARCHIVE: retry context".into()),
                status_id: Uuid::new_v4(),
                requirements_revision: 1,
                event_ids: vec![],
                related_workspaces: vec![],
                done_result: None,
            }],
            ..Default::default()
        }),
        cancel_requested: false,
        error: None,
        created_at: String::new(),
        updated_at: String::new(),
    };
    sqlx::query("INSERT INTO integration_runs(id) VALUES(?)")
        .bind(run.id)
        .execute(&pool)
        .await?;
    run.save(&pool).await?;
    Ok((dir, pool, run))
}

#[tokio::test]
async fn archive_exact_body_retry_and_git_receipt_recovery() -> anyhow::Result<()> {
    let (dir, pool, mut run) = fixture().await?;
    let root = dir.path();
    let git = git::GitService::new();
    let parent = run.payload.observed_target.clone();
    let request = Uuid::new_v4();
    assert!(verify(&run, root, &parent).is_err()); // publication is blocked until prepared
    let commit = prepare(&pool, &git, &mut run, root, request, &parent).await?;
    let (path, content) = document(run.payload.sources[0].issue_snapshot.as_ref().unwrap())?;
    assert!(path.starts_with("docs/changes/2026-11-16-LOCAL-399-"));
    assert_eq!(std::fs::read(root.join(&path))?, content.as_bytes());
    assert!(!content.contains("DO NOT ARCHIVE"));
    assert!(!content.contains("compatibility field"));
    assert!(content.ends_with(&run.payload.sources[0].issue_snapshot.as_ref().unwrap().body));
    assert_eq!(support::git(root, &["rev-list", "--count", "HEAD"])?, "2");
    let log = support::git(root, &["log", "-1", "--format=%B"])?;
    assert!(log.contains("LVK-Issue: LOCAL-399"));
    assert!(log.contains(&format!(
        "LVK-Issue-UUID: {}",
        run.payload.sources[0].selection.card_id
    )));
    // Later Issue edits/settings are absent from this serialized adopted payload.
    let saved = serde_json::to_string(&run)?;
    run = serde_json::from_str(&saved)?;
    assert_eq!(
        prepare(&pool, &git, &mut run, root, request, &parent).await?,
        commit
    );
    // Git succeeded but the DB receipt did not. A follow-up Run may use a new
    // request ID; it still recovers only the saved parent/tree/message.
    run.payload.archive_publication.as_mut().unwrap().commit = None;
    assert_eq!(
        prepare(&pool, &git, &mut run, root, Uuid::new_v4(), &parent).await?,
        commit
    );
    assert_eq!(support::git(root, &["rev-list", "--count", "HEAD"])?, "2");
    verify(&run, root, &commit)?;
    assert!(support::git(root, &["status", "--porcelain"])?.is_empty());
    // A later commit must not silently edit the archive before publication.
    std::fs::write(root.join(&path), "changed")?;
    support::git(root, &["add", "-f", "--", &path])?;
    support::git(root, &["commit", "-m", "tampered archive"])?;
    assert!(verify(&run, root, &support::git(root, &["rev-parse", "HEAD"])?).is_err());
    Ok(())
}

#[tokio::test]
async fn archive_off_wrong_selection_collision_and_other_repo() -> anyhow::Result<()> {
    let (dir, pool, mut run) = fixture().await?;
    let root = dir.path();
    let git = git::GitService::new();
    let parent = run.payload.observed_target.clone();
    run.payload.issue_archive_enabled = false;
    assert!(
        prepare(&pool, &git, &mut run, root, Uuid::new_v4(), &parent)
            .await
            .is_err()
    );
    verify(&run, root, &parent)?;
    assert!(!root.join("docs").exists());
    run.payload.issue_archive_enabled = true;
    let original_id = run.payload.sources[0].selection.card_id;
    run.payload.sources[0].selection.card_id = Uuid::new_v4();
    assert!(
        prepare(&pool, &git, &mut run, root, Uuid::new_v4(), &parent)
            .await
            .is_err()
    );
    run.payload.sources[0].selection.card_id = original_id;
    let other = tempfile::tempdir()?;
    std::fs::write(other.path().join("keep"), "other repository")?;
    let (path, _) = document(run.payload.sources[0].issue_snapshot.as_ref().unwrap())?;
    std::fs::create_dir_all(root.join("docs/changes"))?;
    std::fs::write(root.join(&path), "existing content")?;
    assert!(
        prepare(&pool, &git, &mut run, root, Uuid::new_v4(), &parent)
            .await
            .is_err()
    );
    assert_eq!(
        std::fs::read_to_string(root.join(&path))?,
        "existing content"
    );
    assert_eq!(support::git(root, &["rev-parse", "HEAD"])?, parent);
    assert!(verify(&run, root, &parent).is_err());
    assert!(!other.path().join("docs").exists());
    assert_eq!(
        std::fs::read_to_string(other.path().join("keep"))?,
        "other repository"
    );
    Ok(())
}

#[cfg(unix)]
#[tokio::test]
async fn archive_refuses_symlink_and_unrelated_index() -> anyhow::Result<()> {
    for symlink in [false, true] {
        let (dir, pool, mut run) = fixture().await?;
        let root = dir.path();
        let outside = tempfile::tempdir()?;
        if symlink {
            std::os::unix::fs::symlink(outside.path(), root.join("docs"))?;
        } else {
            std::fs::write(root.join("unrelated"), "not archive")?;
            support::git(root, &["add", "unrelated"])?;
        }
        let parent = run.payload.observed_target.clone();
        assert!(
            prepare(
                &pool,
                &git::GitService::new(),
                &mut run,
                root,
                Uuid::new_v4(),
                &parent
            )
            .await
            .is_err()
        );
        assert_eq!(std::fs::read_dir(outside.path())?.count(), 0);
        assert_eq!(support::git(root, &["rev-parse", "HEAD"])?, parent);
    }
    Ok(())
}

#[tokio::test]
async fn archive_multiple_issues_once_and_reject_later_second_archive() -> anyhow::Result<()> {
    let (dir, pool, mut run) = fixture().await?;
    let root = dir.path();
    let mut second = run.payload.sources[0].clone();
    second.selection.card_id = Uuid::new_v4();
    let snapshot = second.issue_snapshot.as_mut().unwrap();
    snapshot.issue_id = second.selection.card_id;
    snapshot.simple_id = "LOCAL-400".into();
    // The UTC day, not the local offset or the retry wall clock, names the file.
    snapshot.frozen_at = "2026-11-17T00:30:00+01:00".into();
    run.payload.sources.push(second);
    let parent = run.payload.observed_target.clone();
    let commit = prepare(
        &pool,
        &git::GitService::new(),
        &mut run,
        root,
        Uuid::new_v4(),
        &parent,
    )
    .await?;
    let files = documents(&run)?;
    assert_eq!(files.len(), 2);
    assert!(
        files
            .keys()
            .all(|p| p.starts_with("docs/changes/2026-11-16-"))
    );
    let trailers = support::git(root, &["log", "-1", "--format=%B"])?;
    assert!(trailers.contains("LVK-Issue: LOCAL-399"));
    assert!(trailers.contains("LVK-Issue: LOCAL-400"));
    assert_eq!(support::git(root, &["rev-list", "--count", "HEAD"])?, "2");
    // A new Integration/day cannot create a second archive for the same Issue.
    run.payload.archive_publication = None;
    for source in &mut run.payload.sources {
        source.issue_snapshot.as_mut().unwrap().frozen_at = "2026-11-18T00:00:00Z".into();
    }
    assert!(
        prepare(
            &pool,
            &git::GitService::new(),
            &mut run,
            root,
            Uuid::new_v4(),
            &commit
        )
        .await
        .is_err()
    );
    assert_eq!(std::fs::read_dir(root.join("docs/changes"))?.count(), 2);
    assert_eq!(support::git(root, &["rev-parse", "HEAD"])?, commit);
    Ok(())
}

#[tokio::test]
async fn archive_multiple_issues_and_one_archive_per_issue() -> anyhow::Result<()> {
    let (dir, pool, mut run) = fixture().await?;
    let mut second = run.payload.sources[0].clone();
    second.selection.card_id = Uuid::new_v4();
    second.issue_snapshot.as_mut().unwrap().issue_id = second.selection.card_id;
    // Same instance-local display number, different UUID: no filename collision.
    run.payload.sources.push(second);
    let parent = run.payload.observed_target.clone();
    let commit = prepare(
        &pool,
        &git::GitService::new(),
        &mut run,
        dir.path(),
        Uuid::new_v4(),
        &parent,
    )
    .await?;
    assert_eq!(
        std::fs::read_dir(dir.path().join("docs/changes"))?.count(),
        2
    );
    let log = support::git(dir.path(), &["log", "-1", "--format=%B"])?;
    assert_eq!(log.matches("LVK-Issue: LOCAL-399").count(), 2);
    assert_eq!(log.matches("LVK-Issue-UUID:").count(), 2);
    // A separate admission of the same Issue on a later day is not a retry.
    run.payload.archive_publication = None;
    for source in &mut run.payload.sources {
        source.issue_snapshot.as_mut().unwrap().frozen_at = "2026-11-17T00:00:01Z".into();
    }
    assert!(
        prepare(
            &pool,
            &git::GitService::new(),
            &mut run,
            dir.path(),
            Uuid::new_v4(),
            &commit
        )
        .await
        .is_err()
    );
    assert_eq!(
        std::fs::read_dir(dir.path().join("docs/changes"))?.count(),
        2
    );
    assert_eq!(support::git(dir.path(), &["rev-parse", "HEAD"])?, commit);
    Ok(())
}

#[cfg(unix)]
#[tokio::test]
async fn archive_commit_failure_retries_only_saved_tree() -> anyhow::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let (dir, pool, mut run) = fixture().await?;
    let root = dir.path();
    let hook = root.join(".git/hooks/pre-commit");
    std::fs::write(&hook, "#!/bin/sh\nexit 1\n")?;
    std::fs::set_permissions(&hook, std::fs::Permissions::from_mode(0o755))?;
    let parent = run.payload.observed_target.clone();
    let request = Uuid::new_v4();
    assert!(
        prepare(
            &pool,
            &git::GitService::new(),
            &mut run,
            root,
            request,
            &parent
        )
        .await
        .is_err()
    );
    assert!(
        run.payload
            .archive_publication
            .as_ref()
            .unwrap()
            .tree
            .is_some()
    );
    assert!(
        run.payload
            .archive_publication
            .as_ref()
            .unwrap()
            .commit
            .is_none()
    );
    assert!(verify(&run, root, &parent).is_err());
    std::fs::remove_file(hook)?;
    let commit = prepare(
        &pool,
        &git::GitService::new(),
        &mut run,
        root,
        request,
        &parent,
    )
    .await?;
    verify(&run, root, &commit)?;
    assert_eq!(support::git(root, &["rev-list", "--count", "HEAD"])?, "2");
    Ok(())
}

#[test]
fn archive_date_uses_frozen_utc_day() {
    let snapshot = IntegrationIssueSnapshot {
        issue_id: Uuid::new_v4(),
        simple_id: "LOCAL-399".into(),
        title: "UTC day".into(),
        version_id: Uuid::new_v4(),
        version_revision: 0,
        body: String::new(),
        frozen_at: "2026-11-17T00:30:00+01:00".into(),
    };
    assert!(
        document(&snapshot)
            .unwrap()
            .0
            .starts_with("docs/changes/2026-11-16-")
    );
    // Rendering has no current-time or settings input, including after midnight.
    assert_eq!(
        document(&snapshot).unwrap(),
        document(&serde_json::from_str(&serde_json::to_string(&snapshot).unwrap()).unwrap())
            .unwrap()
    );
}

#[tokio::test]
async fn archive_body_mentions_do_not_claim_another_issue_identity() -> anyhow::Result<()> {
    let (dir, pool, mut run) = fixture().await?;
    let first = run.payload.sources[0].selection.card_id;
    let second = Uuid::new_v4();
    run.payload.sources[0].issue_snapshot.as_mut().unwrap().body =
        format!("Example only:\nissue_id: {second}\n");
    let parent = run.payload.observed_target.clone();
    let commit = prepare(
        &pool,
        &git::GitService::new(),
        &mut run,
        dir.path(),
        Uuid::new_v4(),
        &parent,
    )
    .await?;
    run.payload.archive_publication = None;
    run.payload.sources[0].selection.card_id = second;
    let snapshot = run.payload.sources[0].issue_snapshot.as_mut().unwrap();
    snapshot.issue_id = second;
    snapshot.body = format!("References first Issue {first}");
    let second_commit = prepare(
        &pool,
        &git::GitService::new(),
        &mut run,
        dir.path(),
        Uuid::new_v4(),
        &commit,
    )
    .await?;
    assert_ne!(second_commit, commit);
    assert_eq!(
        std::fs::read_dir(dir.path().join("docs/changes"))?.count(),
        2
    );
    Ok(())
}
