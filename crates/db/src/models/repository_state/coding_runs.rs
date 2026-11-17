//! Durable coding history; mutable drafts and Git recovery snapshots stay files.
use files::RepositoryMemoryRun;

use super::*;

impl RepositoryMemoryStore {
    pub async fn save_run(&self, run: &RepositoryMemoryRun) -> io::Result<()> {
        if !self.fixture && run.repository_id != self.repository_id {
            return Err(invalid("Coding Run belongs to another repository"));
        }
        let json = serde_json::to_string(run).map_err(invalid)?;
        if json.len() > 128 * 1024 {
            return Err(invalid("Coding Run identity is too large"));
        }
        let mut conn = self.connection().await?;
        let mut tx = conn.begin_with("BEGIN IMMEDIATE").await.map_err(invalid)?;
        let old: Option<String> = sqlx::query_scalar(
            "SELECT payload FROM repository_coding_runs WHERE repository_id=? AND id=?",
        )
        .bind(self.repository_id)
        .bind(run.run_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(invalid)?;
        if let Some(old) = old {
            if old != json {
                return Err(invalid("Immutable coding Run identity conflict"));
            }
        } else {
            sqlx::query(
                "INSERT INTO repository_coding_runs(repository_id,id,payload) VALUES(?,?,?)",
            )
            .bind(self.repository_id)
            .bind(run.run_id)
            .bind(json)
            .execute(&mut *tx)
            .await
            .map_err(invalid)?;
        }
        tx.commit().await.map_err(invalid)
    }

    pub async fn runs(&self) -> io::Result<Vec<RepositoryMemoryRun>> {
        let rows: Vec<String> = sqlx::query_scalar(
            "SELECT payload FROM repository_coding_runs WHERE repository_id=? ORDER BY id",
        )
        .bind(self.repository_id)
        .fetch_all(&mut *self.connection().await?)
        .await
        .map_err(invalid)?;
        rows.into_iter()
            .map(|json| serde_json::from_str(&json).map_err(invalid))
            .collect()
    }

    pub async fn run(&self, id: Uuid) -> io::Result<Option<RepositoryMemoryRun>> {
        self.get("repository_coding_runs", id).await
    }

    pub async fn completed_without_change(&self, id: Uuid) -> io::Result<bool> {
        Ok(sqlx::query_scalar::<_, bool>(
            "SELECT no_source_change FROM repository_coding_runs WHERE repository_id=? AND id=?",
        )
        .bind(self.repository_id)
        .bind(id)
        .fetch_optional(&mut *self.connection().await?)
        .await
        .map_err(invalid)?
        .unwrap_or(false))
    }

    pub async fn mark_no_source_change(&self, id: Uuid) -> io::Result<()> {
        let result = sqlx::query(
            "UPDATE repository_coding_runs SET no_source_change=1 WHERE repository_id=? AND id=?",
        )
        .bind(self.repository_id)
        .bind(id)
        .execute(&mut *self.connection().await?)
        .await
        .map_err(invalid)?;
        if result.rows_affected() != 1 {
            return Err(invalid("Coding Run identity is missing"));
        }
        Ok(())
    }

    pub async fn record_coding_result(
        &self,
        run: &RepositoryMemoryRun,
        error: Option<String>,
    ) -> io::Result<()> {
        self.save_run(run).await?;
        let result = sqlx::query(
            "UPDATE repository_coding_runs SET completion_error=? WHERE repository_id=? AND id=?",
        )
        .bind(error)
        .bind(self.repository_id)
        .bind(run.run_id)
        .execute(&mut *self.connection().await?)
        .await
        .map_err(invalid)?;
        if result.rows_affected() != 1 {
            return Err(invalid("Coding Run identity is missing"));
        }
        Ok(())
    }

    pub async fn coding_errors(&self) -> io::Result<Vec<String>> {
        let rows: Vec<(String, String)> = sqlx::query_as("SELECT payload,completion_error FROM repository_coding_runs WHERE repository_id=? AND completion_error IS NOT NULL ORDER BY id")
            .bind(self.repository_id).fetch_all(&mut *self.connection().await?).await.map_err(invalid)?;
        rows.into_iter()
            .map(|(json, error)| {
                let run: RepositoryMemoryRun = serde_json::from_str(&json).map_err(invalid)?;
                Ok(format!(
                    "Workspace {} / run {}: {error}",
                    run.workspace_id, run.run_id
                ))
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn completed_history_is_db_owned_without_artifact_directories() {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        crate::run_migrations(&pool).await.unwrap();
        let repo = Uuid::new_v4();
        let reader_files = files::RepositoryMemoryStore::paths_for_repository(repo);
        let store = RepositoryMemoryStore::with_artifacts(&pool, repo, reader_files);
        // Save state directly: even defaults must not require a filesystem lock for reads.
        store
            .save_state(&RepositoryMemoryState::enabled_defaults(Some(
                "main".into(),
            )))
            .await
            .unwrap();
        let run = RepositoryMemoryRun {
            run_id: Uuid::new_v4(),
            repository_id: repo,
            workspace_id: Uuid::new_v4(),
            task_id: None,
            created_at: chrono::Utc::now(),
            base_commit: "a".repeat(40),
            target_branch: "main".into(),
            repository_path: "/nonexistent/fixture-worktree".into(),
            draft_path: store.draft_path(Uuid::new_v4()),
            wiki_status: files::RepositoryWikiStatus::Current,
            finalize_source: false,
        };
        store.save_run(&run).await.unwrap();
        store.mark_no_source_change(run.run_id).await.unwrap();
        store
            .record_coding_result(&run, Some("fixture diagnostic".into()))
            .await
            .unwrap();
        let reader =
            RepositoryMemoryStore::read_only_configured_for_repository(&pool, "absent", repo)
                .await
                .unwrap()
                .unwrap();
        assert!(reader.state().await.unwrap().enabled);
        assert_eq!(reader.runs().await.unwrap().len(), 1);
        assert!(reader.completed_without_change(run.run_id).await.unwrap());
        assert!(reader.coding_errors().await.unwrap()[0].contains("fixture diagnostic"));
        assert!(
            reader
                .discover_events(repo, None, 10)
                .await
                .unwrap()
                .records
                .is_empty()
        );
        assert!(!reader.draft_path(run.run_id).parent().unwrap().exists());
        store.record_coding_result(&run, None).await.unwrap();
        assert!(reader.coding_errors().await.unwrap().is_empty());
    }
}

#[cfg(test)]
mod upgrade_tests {
    use super::*;
    #[tokio::test]
    async fn existing_db_upgrade_preserves_applied_migrations_and_final_manifest_rows() {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        let all = sqlx::migrate!("./migrations");
        let old = sqlx::migrate::Migrator {
            migrations: std::borrow::Cow::Owned(
                all.iter()
                    .filter(|m| m.version < 20261117000000)
                    .cloned()
                    .collect(),
            ),
            ..sqlx::migrate::Migrator::DEFAULT
        };
        old.run(&pool).await.unwrap();
        let checksums: Vec<(i64, Vec<u8>)> =
            sqlx::query_as("SELECT version,checksum FROM _sqlx_migrations ORDER BY version")
                .fetch_all(&pool)
                .await
                .unwrap();
        let repo = Uuid::new_v4();
        let store = RepositoryMemoryStore::with_artifacts(
            &pool,
            repo,
            files::RepositoryMemoryStore::paths_for_repository(repo),
        );
        let manifest = files::ChangeManifest {
            version: 1,
            event_id: Uuid::new_v4(),
            repository_id: repo,
            workspace_id: Uuid::new_v4(),
            task_id: None,
            created_at: chrono::Utc::now(),
            base_commit: "a".repeat(40),
            source_commit: "b".repeat(40),
            target_branch: Some("main".into()),
            changed_paths: vec!["source.rs".into()],
            semantics: files::SemanticChanges {
                summary: "Retained final intent".into(),
                ..Default::default()
            },
        };
        store.publish_event(&manifest).await.unwrap();
        sqlx::query(
            "INSERT INTO repository_manifest_imports(repository_id,imported_count) VALUES(?,387)",
        )
        .bind(repo)
        .execute(&pool)
        .await
        .unwrap();
        crate::run_migrations(&pool).await.unwrap();
        assert_eq!(
            store.event(manifest.event_id).await.unwrap(),
            Some(manifest)
        );
        let after: Vec<(i64,Vec<u8>)> = sqlx::query_as("SELECT version,checksum FROM _sqlx_migrations WHERE version < 20261117000000 ORDER BY version")
            .fetch_all(&pool).await.unwrap();
        assert_eq!(after, checksums);
        let imported: i64 = sqlx::query_scalar(
            "SELECT imported_count FROM repository_manifest_imports WHERE repository_id=?",
        )
        .bind(repo)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(imported, 387);
        assert!(store.runs().await.unwrap().is_empty());
    }
}
