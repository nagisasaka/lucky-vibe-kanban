//! Real recovery entry with isolated Git/SQLite/filesystem state. No model calls.
use utils::repository_memory::{ChangeManifest, MemoryIntegration, SemanticChanges};

use super::*;

fn git(root: &FsPath, args: &[&str]) -> anyhow::Result<String> {
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()?;
    anyhow::ensure!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(String::from_utf8(output.stdout)?.trim().into())
}

#[tokio::test]
#[ignore = "requires fresh lvk-openwiki-admission-* LVK_INSTANCE_DIR and its bin before OpenWiki on PATH; run alone"]
async fn recovery_never_admits_for_publication_pending_backlog_or_direct_main() -> anyhow::Result<()>
{
    let instance = PathBuf::from(std::env::var("LVK_INSTANCE_DIR")?);
    anyhow::ensure!(instance.is_absolute());
    anyhow::ensure!(
        instance
            .file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("lvk-openwiki-admission-")
    );
    anyhow::ensure!(!instance.join("data/db.v2.sqlite").exists());
    // If admission regresses, fail before OpenWiki setup or any paid agent.
    std::fs::create_dir_all(instance.join("bin"))?;
    let trap = instance.join("bin/openwiki");
    std::fs::write(
        &trap,
        "#!/bin/sh\ntouch \"$LVK_INSTANCE_DIR/unexpected-openwiki\"\nexit 71\n",
    )?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&trap, std::fs::Permissions::from_mode(0o700))?;
    }
    let resolved = std::process::Command::new("sh")
        .args(["-c", "command -v openwiki"])
        .output()?;
    anyhow::ensure!(resolved.status.success());
    anyhow::ensure!(PathBuf::from(String::from_utf8(resolved.stdout)?.trim()) == trap);
    let shutdown = tokio_util::sync::CancellationToken::new();
    let deployment = DeploymentImpl::new(shutdown.clone()).await?;
    let result = exercise(&deployment, &instance).await;
    shutdown.cancel();
    result
}

async fn exercise(deployment: &DeploymentImpl, instance: &FsPath) -> anyhow::Result<()> {
    anyhow::ensure!(utils::assets::asset_dir().starts_with(instance));
    for scenario in ["publication", "new-pending", "old-backlog", "direct-main"] {
        let root = instance.join(scenario);
        std::fs::create_dir(&root)?;
        git(&root, &["init", "-b", "main"])?;
        git(&root, &["config", "user.name", "Fixture"])?;
        git(&root, &["config", "user.email", "fixture@example.invalid"])?;
        std::fs::create_dir(root.join("openwiki"))?;
        std::fs::write(root.join("openwiki/quickstart.md"), "# Existing Wiki\n")?;
        git(&root, &["add", "."])?;
        git(&root, &["commit", "-m", "baseline"])?;
        let base = git(&root, &["rev-parse", "HEAD"])?;
        std::fs::write(root.join("source"), scenario)?;
        git(&root, &["add", "."])?;
        git(&root, &["commit", "-m", "source change"])?;
        let source = git(&root, &["rev-parse", "HEAD"])?;
        let repo = Repo::find_or_create(&deployment.db().pool, &root, scenario).await?;
        workspace_manager::shared_resources::ensure_repository_memory(&repo)?;
        let store = RepositoryMemoryStore::for_repository(&repo.name, repo.id)?;
        let state = RepositoryMemoryState {
            status: RepositoryWikiStatus::Current,
            source_commit: Some(if scenario == "publication" {
                source.clone()
            } else {
                base.clone()
            }),
            wiki_commit: Some(if scenario == "publication" {
                source.clone()
            } else {
                base.clone()
            }),
            last_success: Some(chrono::Utc::now()),
            ..RepositoryMemoryState::enabled_defaults(Some("main".into()))
        };
        store.save_state(&state)?;
        let mut event_ids = vec![];
        if scenario != "direct-main" {
            // Publication acknowledges its own event, leaving an unrelated
            // older backlog intact, as auto-merge's semantic outbox does.
            for index in 0..if scenario == "publication" { 2 } else { 1 } {
                let event = ChangeManifest {
                    version: 1,
                    event_id: Uuid::new_v4(),
                    repository_id: repo.id,
                    workspace_id: Uuid::new_v4(),
                    task_id: None,
                    created_at: chrono::Utc::now()
                        - chrono::Duration::days(if scenario == "new-pending" { 0 } else { 90 }),
                    base_commit: base.clone(),
                    source_commit: source.clone(),
                    target_branch: Some("main".into()),
                    changed_paths: vec!["source".into()],
                    semantics: SemanticChanges {
                        summary: scenario.into(),
                        ..Default::default()
                    },
                };
                store.publish_event(&event)?;
                store.save_integration(&MemoryIntegration {
                    id: Uuid::new_v4(),
                    event_ids: vec![event.event_id],
                    target_branch: "main".into(),
                    before_commit: base.clone(),
                    integrated_commit: Some(source.clone()),
                    source_error: None,
                })?;
                if scenario == "publication" && index == 0 {
                    store.acknowledge(&WikiReconciliationReceipt {
                        event_id: event.event_id,
                        reconciled_at: chrono::Utc::now(),
                        target_commit: source.clone(),
                        wiki_commit: Some(source.clone()),
                        result: ReconciliationResult::Updated,
                        error: None,
                    })?;
                } else {
                    event_ids.push(event.event_id);
                }
            }
        }
        let saved = serde_json::to_value(store.state()?)?;
        // Each call reopens persisted state, as startup and periodic recovery do.
        for _ in 0..3 {
            recover_repository(deployment, &repo).await?;
            assert_eq!(serde_json::to_value(store.state()?)?, saved, "{scenario}");
            assert_eq!(pending_events(&store, "main")?.len(), event_ids.len());
            for id in &event_ids {
                assert!(
                    store.receipt(*id)?.is_none(),
                    "backlog must not be force-acknowledged"
                );
            }
            for table in ["workspaces", "sessions", "agent_runs", "workflow_runs"] {
                let count: i64 = sqlx::query_scalar(&format!("SELECT COUNT(*) FROM {table}"))
                    .fetch_one(&deployment.db().pool)
                    .await?;
                assert_eq!(count, 0, "{scenario}: no new {table}");
            }
        }
        assert!(!instance.join("unexpected-openwiki").exists());
    }
    Ok(())
}
