//! Read-only upgrade refusal, never a legacy reader/fallback for product work.
//! Old binaries must be stopped before changing lock domains. No files are moved.
use std::{
    fs,
    io::{self, Read},
    path::Path,
};

use sqlx::SqlitePool;
use utils::repository_memory::{
    IntegrationWikiSetup, RepositoryMemoryRun, SourcePublication, WikiSetupCheckpoint,
    WikiSetupPhase, reject_symlinks,
};
use uuid::Uuid;

fn pending(message: impl std::fmt::Display) -> io::Error {
    io::Error::other(format!(
        "Repository artifact cutover refused: {message}. Settle the operation with the old version and original paths offline; no legacy files were changed"
    ))
}

fn records<T: serde::de::DeserializeOwned>(root: &Path, kind: &str) -> io::Result<Vec<T>> {
    let path = root.join(kind);
    reject_symlinks(&path)?;
    let entries = match fs::read_dir(&path) {
        Ok(entries) => entries,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(vec![]),
        Err(e) => return Err(pending(e)),
    };
    entries
        .map(|entry| {
            let path = entry?.path();
            reject_symlinks(&path)?;
            let mut options = fs::OpenOptions::new();
            options.read(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
            }
            let file = options.open(&path)?;
            if !file.metadata()?.is_file() {
                return Err(pending("legacy artifact is not a regular file"));
            }
            let mut bytes = Vec::new();
            file.take(1024 * 1024 + 1).read_to_end(&mut bytes)?;
            if bytes.len() > 1024 * 1024 {
                return Err(pending("legacy artifact exceeds bound"));
            }
            serde_json::from_slice(&bytes).map_err(pending)
        })
        .collect()
}

pub(super) async fn verify_at(pool: &SqlitePool, repo: Uuid, root: &Path) -> io::Result<()> {
    // Do not enumerate events/: final Manifests have no filesystem compatibility.
    for run in records::<RepositoryMemoryRun>(root, "runs")? {
        if run.repository_id != repo {
            return Err(pending("legacy Run repository mismatch"));
        }
        if !run.finalize_source {
            continue;
        }
        let final_record = super::RepositoryMemoryStore::read_event(pool, repo, run.run_id).await?;
        if final_record
            .as_ref()
            .is_some_and(|event| event.workspace_id == run.workspace_id)
        {
            continue;
        }
        let completed = records::<Uuid>(root, "completed-runs")?;
        if completed.contains(&run.run_id) {
            continue;
        }
        return Err(pending(format!(
            "unfinished Run {} (fixed draft {})",
            run.run_id,
            run.draft_path.display()
        )));
    }
    for publication in records::<SourcePublication>(root, "source-publications")? {
        let final_record =
            super::RepositoryMemoryStore::read_event(pool, repo, publication.manifest.event_id)
                .await?;
        if final_record.is_none_or(|event| event.workspace_id != publication.manifest.workspace_id)
        {
            return Err(pending(format!(
                "unreconciled source publication {}",
                publication.manifest.event_id
            )));
        }
    }
    if let Some(state) =
        super::RepositoryMemoryStore::state_on(&mut *pool.acquire().await.map_err(pending)?, repo)
            .await?
        && (state.active_run_id.is_some()
            || state.bootstrap.is_some()
            || state.file_operation.is_some())
    {
        return Err(pending(
            "repository maintenance is unfinished; preserve its frozen paths",
        ));
    }
    for setup in records::<WikiSetupCheckpoint>(root, "wiki-setups")? {
        if setup.phase != WikiSetupPhase::Restored {
            return Err(pending(format!(
                "unfinished Wiki restoration {}",
                setup.workspace_id
            )));
        }
    }
    for setup in records::<IntegrationWikiSetup>(root, "integration-wiki-setups")? {
        if setup.repository_id != repo || setup.checkpoint.phase != WikiSetupPhase::Restored {
            return Err(pending(format!(
                "unfinished integration restoration {}",
                setup.integration_id
            )));
        }
    }
    let locks = root.join("locks");
    reject_symlinks(&locks)?;
    match fs::read_dir(locks) {
        Ok(entries) => {
            for entry in entries {
                let path = entry?.path();
                reject_symlinks(&path)?;
                let mut options = fs::OpenOptions::new();
                options.read(true).write(true);
                #[cfg(unix)]
                {
                    use std::os::unix::fs::OpenOptionsExt;
                    options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
                }
                let file = options.open(&path)?;
                if !file.metadata()?.is_file() {
                    return Err(pending("legacy lock is not a regular file"));
                }
                file.try_lock()
                    .map_err(|_| pending("old process holds a legacy lock"))?;
                file.unlock()?;
            }
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        Err(e) => return Err(pending(e)),
    }
    Ok(())
}

async fn verify_repository_at(pool: &SqlitePool, repo: Uuid, root: &Path) -> io::Result<()> {
    let state =
        super::RepositoryMemoryStore::state_on(&mut *pool.acquire().await.map_err(pending)?, repo)
            .await?;
    let Some(state) = state else {
        return Ok(());
    };
    match state.artifact_layout {
        Some(2) => return Ok(()),
        None | Some(1) => {}
        _ => return Err(pending("unsupported repository artifact layout")),
    }
    reject_symlinks(root)?;
    match fs::metadata(root) {
        Ok(metadata) if metadata.is_dir() => {}
        _ => {
            return Err(pending(
                "legacy recovery storage is unavailable; verify the original volume before acknowledging cutover",
            ));
        }
    }
    let writers: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM agent_runs r JOIN workspace_repos wr ON wr.workspace_id=r.workspace_id WHERE wr.repo_id=? AND (r.status NOT IN ('succeeded','failed','cancelled','crashed','audit_failed') OR EXISTS(SELECT 1 FROM agent_run_attempts a JOIN agent_process_registry p ON p.run_attempt_id=a.id WHERE a.agent_run_id=r.id AND p.registry_status IN ('reserved','spawned','running','unreachable')))) OR EXISTS(SELECT 1 FROM execution_processes ep JOIN sessions s ON s.id=ep.session_id JOIN workspace_repos wr ON wr.workspace_id=s.workspace_id WHERE wr.repo_id=? AND ep.status='running')")
        .bind(repo).bind(repo).fetch_one(pool).await.map_err(pending)?;
    if writers {
        return Err(pending(
            "old coding hosts or execution processes have not settled",
        ));
    }
    verify_at(pool, repo, root).await?;
    // Preserve every other payload field and all historical/import rows. This
    // small existing-state receipt ends legacy inspection, it does not import data.
    sqlx::query("UPDATE repository_wiki_state SET payload=json_set(payload,'$.artifact_layout',2) WHERE repository_id=?")
        .bind(repo).execute(pool).await.map_err(pending)?;
    Ok(())
}

pub async fn verify_retired_artifacts(pool: &SqlitePool) -> io::Result<()> {
    let repos: Vec<(Uuid, String)> = sqlx::query_as("SELECT id,name FROM repos ORDER BY id")
        .fetch_all(pool)
        .await
        .map_err(pending)?;
    for (id, name) in repos {
        verify_repository_at(
            pool,
            id,
            &utils::path::shared_resources_dir(&name, id).join("persistent/knowledge"),
        )
        .await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use utils::repository_memory::{ChangeManifest, RepositoryWikiStatus, SemanticChanges};

    use super::*;

    #[tokio::test]
    async fn cutover_refuses_old_fixed_paths_and_locks_without_changing_legacy_files() {
        let temp = tempfile::tempdir().unwrap();
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        crate::run_migrations(&pool).await.unwrap();
        let repo = Uuid::new_v4();
        let root = temp.path().join("persistent/knowledge");
        fs::create_dir_all(root.join("runs")).unwrap();
        fs::create_dir(root.join("events")).unwrap();
        fs::write(
            root.join("events/malformed.json"),
            b"obsolete, never parsed",
        )
        .unwrap();
        verify_at(&pool, repo, &root).await.unwrap();
        let id = Uuid::new_v4();
        let run = RepositoryMemoryRun {
            run_id: id,
            repository_id: repo,
            workspace_id: Uuid::new_v4(),
            task_id: None,
            created_at: chrono::Utc::now(),
            base_commit: "a".repeat(40),
            target_branch: "main".into(),
            repository_path: temp.path().join("old-worktree"),
            draft_path: root.join("drafts/fixed.json"),
            wiki_status: RepositoryWikiStatus::Current,
            finalize_source: true,
        };
        let bytes = serde_json::to_vec(&run).unwrap();
        let file = root.join("runs").join(format!("{id}.json"));
        fs::write(&file, &bytes).unwrap();
        let error = verify_at(&pool, repo, &root).await.unwrap_err();
        assert!(error.to_string().contains("fixed.json"));
        assert_eq!(fs::read(&file).unwrap(), bytes);
        let store = super::super::RepositoryMemoryStore::with_artifacts(
            &pool,
            repo,
            utils::repository_memory::RepositoryMemoryStore::at_artifacts(
                &temp.path().join("new-tmp"),
            )
            .unwrap(),
        );
        store
            .publish_event(&ChangeManifest {
                version: 1,
                event_id: id,
                repository_id: repo,
                workspace_id: run.workspace_id,
                task_id: None,
                created_at: run.created_at,
                base_commit: run.base_commit.clone(),
                source_commit: "b".repeat(40),
                target_branch: Some("main".into()),
                changed_paths: vec!["source.rs".into()],
                semantics: SemanticChanges {
                    summary: "Confirmed source".into(),
                    ..Default::default()
                },
            })
            .await
            .unwrap();
        verify_at(&pool, repo, &root).await.unwrap();
        fs::create_dir(root.join("wiki-setups")).unwrap();
        let mut setup = WikiSetupCheckpoint {
            version: 1,
            workspace_id: run.workspace_id,
            repository_path: run.repository_path.clone(),
            source_commit: run.base_commit.clone(),
            phase: WikiSetupPhase::Prepared,
            files: vec![],
        };
        let setup_path = root
            .join("wiki-setups")
            .join(format!("{}.json", run.workspace_id));
        let original = serde_json::to_vec(&setup).unwrap();
        fs::write(&setup_path, &original).unwrap();
        assert!(
            verify_at(&pool, repo, &root)
                .await
                .unwrap_err()
                .to_string()
                .contains("restoration")
        );
        assert_eq!(fs::read(&setup_path).unwrap(), original);
        // Only the fixture's simulated old-version restoration advances its journal.
        setup.phase = WikiSetupPhase::Restored;
        fs::write(&setup_path, serde_json::to_vec(&setup).unwrap()).unwrap();
        verify_at(&pool, repo, &root).await.unwrap();
        fs::create_dir(root.join("locks")).unwrap();
        let lock_path = root.join("locks/wiki-reconcile.lock");
        let lock = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(&lock_path)
            .unwrap();
        lock.try_lock().unwrap();
        assert!(
            verify_at(&pool, repo, &root)
                .await
                .unwrap_err()
                .to_string()
                .contains("legacy lock")
        );
        lock.unlock().unwrap();
        verify_at(&pool, repo, &root).await.unwrap();
        assert!(lock_path.exists());
        assert_eq!(fs::read(&file).unwrap(), bytes);
        assert_eq!(
            fs::read(root.join("events/malformed.json")).unwrap(),
            b"obsolete, never parsed"
        );
        crate::run_migrations(&pool).await.unwrap();
        assert!(store.event(id).await.unwrap().is_some());
    }
    #[tokio::test]
    async fn successful_cutover_never_inspects_future_user_knowledge_or_rewrites_import_history() {
        let temp = tempfile::tempdir().unwrap();
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        crate::run_migrations(&pool).await.unwrap();
        let repo = Uuid::new_v4();
        let state = utils::repository_memory::RepositoryMemoryState {
            artifact_layout: None,
            ..Default::default()
        };
        sqlx::query("INSERT INTO repository_wiki_state(repository_id,payload) VALUES(?,?)")
            .bind(repo)
            .bind(serde_json::to_string(&state).unwrap())
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO repository_manifest_imports(repository_id,imported_count) VALUES(?,387)",
        )
        .bind(repo)
        .execute(&pool)
        .await
        .unwrap();
        let legacy = temp.path().join("persistent/knowledge");
        assert!(
            verify_repository_at(&pool, repo, &legacy)
                .await
                .unwrap_err()
                .to_string()
                .contains("unavailable")
        );
        let layout: Option<i64> = sqlx::query_scalar("SELECT json_extract(payload,'$.artifact_layout') FROM repository_wiki_state WHERE repository_id=?")
            .bind(repo).fetch_one(&pool).await.unwrap();
        assert_eq!(
            layout, None,
            "missing storage must stay retryable, not acknowledged as empty"
        );
        fs::create_dir_all(&legacy).unwrap();
        verify_repository_at(&pool, repo, &legacy).await.unwrap();
        fs::create_dir_all(legacy.join("runs")).unwrap();
        fs::write(
            legacy.join("runs/user-note.txt"),
            b"user retained, never parse",
        )
        .unwrap();
        verify_repository_at(&pool, repo, &legacy).await.unwrap();
        assert_eq!(
            fs::read(legacy.join("runs/user-note.txt")).unwrap(),
            b"user retained, never parse"
        );
        let imported: i64 = sqlx::query_scalar(
            "SELECT imported_count FROM repository_manifest_imports WHERE repository_id=?",
        )
        .bind(repo)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(imported, 387);
    }
}
