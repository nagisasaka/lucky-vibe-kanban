//! Wiki/integration operational state in the instance DB. Semantic artifacts and
//! frozen Git/filesystem inputs remain in the repository's shared folder.
use std::{io, ops::Deref, path::Path};

use sqlx::{Connection, SqliteConnection, SqlitePool};
use utils::repository_memory::{
    self as files, MemoryIntegration, RepositoryMemoryState, WikiPublication,
    WikiReconciliationReceipt,
};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct RepositoryMemoryStore {
    files: files::RepositoryMemoryStore,
    pool: SqlitePool,
    repository_id: Uuid,
    fixture: bool,
}

impl Deref for RepositoryMemoryStore {
    type Target = files::RepositoryMemoryStore;
    fn deref(&self) -> &Self::Target {
        &self.files
    }
}

fn invalid(error: impl std::fmt::Display) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, error.to_string())
}

impl RepositoryMemoryStore {
    pub fn for_repository(pool: &SqlitePool, name: &str, id: Uuid) -> io::Result<Self> {
        Ok(Self {
            files: files::RepositoryMemoryStore::for_repository(name, id)?,
            pool: pool.clone(),
            repository_id: id,
            fixture: false,
        })
    }

    pub async fn existing_for_repository(
        pool: &SqlitePool,
        name: &str,
        id: Uuid,
    ) -> io::Result<Option<Self>> {
        Self::read_only_configured_for_repository(pool, name, id)
            .await?
            .map(|_| Self::for_repository(pool, name, id))
            .transpose()
    }

    pub async fn read_only_configured_for_repository(
        pool: &SqlitePool,
        name: &str,
        id: Uuid,
    ) -> io::Result<Option<Self>> {
        let persistent = utils::path::shared_resources_dir(name, id).join("persistent");
        let configured: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM repository_wiki_state WHERE repository_id=?)",
        )
        .bind(id)
        .fetch_one(pool)
        .await
        .map_err(invalid)?;
        if !configured {
            if let Some(legacy) =
                files::RepositoryMemoryStore::read_only_at_persistent(&persistent)?
                && legacy.has_legacy_operational_state()?
            {
                return Err(invalid(
                    "Legacy operational state requires one-time migrate-repository-state; refusing empty DB fallback",
                ));
            }
            return Ok(None);
        }
        let files = files::RepositoryMemoryStore::read_only_at_persistent(&persistent)?
            .ok_or_else(|| invalid("Configured repository artifact directory is missing"))?;
        Ok(Some(Self {
            files,
            pool: pool.clone(),
            repository_id: id,
            fixture: false,
        }))
    }

    /// Isolated fixtures only: never resolves the instance DB or a live Repo.
    pub fn at_persistent(persistent: &Path) -> io::Result<Self> {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect_lazy("sqlite::memory:")
            .map_err(invalid)?;
        Ok(Self {
            files: files::RepositoryMemoryStore::at_persistent(persistent)?,
            pool,
            repository_id: Uuid::nil(),
            fixture: true,
        })
    }

    pub fn with_artifacts(
        pool: &SqlitePool,
        id: Uuid,
        files: files::RepositoryMemoryStore,
    ) -> Self {
        Self {
            files,
            pool: pool.clone(),
            repository_id: id,
            fixture: false,
        }
    }

    async fn connection(&self) -> io::Result<sqlx::pool::PoolConnection<sqlx::Sqlite>> {
        let mut conn = self.pool.acquire().await.map_err(invalid)?;
        if self.fixture {
            let schema =
                include_str!("../../migrations/20261106140000_repository_operational_state.sql")
                    .replace("CREATE TABLE ", "CREATE TABLE IF NOT EXISTS ");
            for statement in schema.split(';').filter(|s| !s.trim().is_empty()) {
                sqlx::query(statement)
                    .execute(&mut *conn)
                    .await
                    .map_err(invalid)?;
            }
        }
        let configured: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM repository_wiki_state WHERE repository_id=?)",
        )
        .bind(self.repository_id)
        .fetch_one(&mut *conn)
        .await
        .map_err(invalid)?;
        if !configured && self.files.has_legacy_operational_state()? {
            return Err(invalid(
                "Legacy operational state requires one-time migration; refusing empty DB fallback",
            ));
        }
        Ok(conn)
    }

    pub async fn state_on(
        conn: &mut SqliteConnection,
        id: Uuid,
    ) -> io::Result<Option<RepositoryMemoryState>> {
        let json: Option<String> =
            sqlx::query_scalar("SELECT payload FROM repository_wiki_state WHERE repository_id=?")
                .bind(id)
                .fetch_optional(conn)
                .await
                .map_err(invalid)?;
        json.map(|json| {
            let value: RepositoryMemoryState = serde_json::from_str(&json).map_err(invalid)?;
            if value.version != 1 {
                return Err(invalid("Unsupported repository state version"));
            }
            Ok(value)
        })
        .transpose()
    }

    pub async fn state(&self) -> io::Result<RepositoryMemoryState> {
        Self::state_on(&mut *self.connection().await?, self.repository_id)
            .await
            .map(|value| value.unwrap_or_default())
    }

    pub async fn initialize_defaults(&self, target: Option<String>) -> io::Result<()> {
        let _lock = self.try_lock()?;
        let mut conn = self.connection().await?;
        sqlx::query("INSERT INTO repository_wiki_state(repository_id,payload) VALUES(?,?) ON CONFLICT(repository_id) DO NOTHING").bind(self.repository_id).bind(serde_json::to_string(&RepositoryMemoryState::enabled_defaults(target)).map_err(invalid)?).execute(&mut *conn).await.map_err(invalid)?;
        Ok(())
    }

    pub async fn save_state(&self, state: &RepositoryMemoryState) -> io::Result<()> {
        self.save_reconciliation(state, &[]).await
    }

    pub async fn save_reconciliation(
        &self,
        state: &RepositoryMemoryState,
        receipts: &[WikiReconciliationReceipt],
    ) -> io::Result<()> {
        let mut conn = self.connection().await?;
        let mut tx = conn.begin_with("BEGIN IMMEDIATE").await.map_err(invalid)?;
        self.save_reconciliation_on(&mut tx, state, receipts)
            .await?;
        tx.commit().await.map_err(invalid)
    }

    pub async fn save_reconciliation_on(
        &self,
        conn: &mut SqliteConnection,
        state: &RepositoryMemoryState,
        receipts: &[WikiReconciliationReceipt],
    ) -> io::Result<()> {
        if state.version != 1 {
            return Err(invalid("Unsupported repository state version"));
        }
        for receipt in receipts {
            self.files.validate_receipt(receipt)?;
            self.save_on(
                conn,
                "repository_wiki_receipts",
                receipt.event_id,
                receipt,
                true,
            )
            .await?;
        }
        let json = serde_json::to_string(state).map_err(invalid)?;
        if json.len() > 128 * 1024 {
            return Err(invalid("Operational state is too large"));
        }
        sqlx::query("INSERT INTO repository_wiki_state(repository_id,payload) VALUES(?,?) ON CONFLICT(repository_id) DO UPDATE SET payload=excluded.payload").bind(self.repository_id).bind(json).execute(conn).await.map_err(invalid)?;
        Ok(())
    }

    async fn get<T: serde::de::DeserializeOwned>(
        &self,
        table: &str,
        id: Uuid,
    ) -> io::Result<Option<T>> {
        let json: Option<String> = sqlx::query_scalar(&format!(
            "SELECT payload FROM {table} WHERE repository_id=? AND id=?"
        ))
        .bind(self.repository_id)
        .bind(id)
        .fetch_optional(&mut *self.connection().await?)
        .await
        .map_err(invalid)?;
        json.map(|json| serde_json::from_str(&json).map_err(invalid))
            .transpose()
    }

    async fn save_on<T: serde::Serialize>(
        &self,
        conn: &mut SqliteConnection,
        table: &str,
        id: Uuid,
        value: &T,
        replace: bool,
    ) -> io::Result<()> {
        let json = serde_json::to_string(value).map_err(invalid)?;
        if json.len() > 128 * 1024 {
            return Err(invalid("Operational record is too large"));
        }
        if !replace {
            let old: Option<String> = sqlx::query_scalar(&format!(
                "SELECT payload FROM {table} WHERE repository_id=? AND id=?"
            ))
            .bind(self.repository_id)
            .bind(id)
            .fetch_optional(&mut *conn)
            .await
            .map_err(invalid)?;
            if let Some(old) = old {
                if old == json {
                    return Ok(());
                }
                return Err(invalid(
                    "Frozen Wiki publication conflicts with existing record",
                ));
            }
        }
        sqlx::query(&format!("INSERT INTO {table}(repository_id,id,payload) VALUES(?,?,?) ON CONFLICT(repository_id,id) DO UPDATE SET payload=excluded.payload")).bind(self.repository_id).bind(id).bind(json).execute(conn).await.map_err(invalid)?;
        Ok(())
    }

    async fn save<T: serde::Serialize>(
        &self,
        table: &str,
        id: Uuid,
        value: &T,
        replace: bool,
    ) -> io::Result<()> {
        let mut conn = self.connection().await?;
        let mut tx = conn.begin_with("BEGIN IMMEDIATE").await.map_err(invalid)?;
        self.save_on(&mut tx, table, id, value, replace).await?;
        tx.commit().await.map_err(invalid)
    }

    pub async fn integrations(&self) -> io::Result<Vec<MemoryIntegration>> {
        let rows: Vec<String> = sqlx::query_scalar(
            "SELECT payload FROM repository_integrations WHERE repository_id=? ORDER BY id",
        )
        .bind(self.repository_id)
        .fetch_all(&mut *self.connection().await?)
        .await
        .map_err(invalid)?;
        rows.into_iter()
            .map(|json| serde_json::from_str(&json).map_err(invalid))
            .collect()
    }
    pub async fn save_integration(&self, value: &MemoryIntegration) -> io::Result<()> {
        self.save("repository_integrations", value.id, value, true)
            .await
    }
    pub async fn publication(&self, id: Uuid) -> io::Result<Option<WikiPublication>> {
        let value: Option<WikiPublication> = self.get("repository_wiki_publications", id).await?;
        if value.as_ref().is_some_and(|value| value.run_id != id) {
            return Err(invalid("Publication identity mismatch"));
        }
        Ok(value)
    }
    pub async fn save_publication(&self, value: &WikiPublication) -> io::Result<()> {
        files::validate_publication(value)?;
        self.save("repository_wiki_publications", value.run_id, value, false)
            .await
    }
    pub async fn receipt(&self, id: Uuid) -> io::Result<Option<WikiReconciliationReceipt>> {
        let value: Option<WikiReconciliationReceipt> =
            self.get("repository_wiki_receipts", id).await?;
        if value.as_ref().is_some_and(|value| value.event_id != id) {
            return Err(invalid("Receipt identity mismatch"));
        }
        Ok(value)
    }
    pub async fn acknowledge(&self, value: &WikiReconciliationReceipt) -> io::Result<()> {
        self.files.validate_receipt(value)?;
        self.save("repository_wiki_receipts", value.event_id, value, true)
            .await
    }
}

/// Explicit offline, one-time development import. It does not run at startup,
/// audit other Repos, remove files, or create a permanent migration manager.
/// Operators must stop all writers; historical Run statuses and durable
/// integration reservations are preserved and are not evidence of live writers.
/// Fix any reported cause and retry; the state row commits with every imported
/// record, so a failed import cannot expose an empty or partial cutover.
pub async fn import_legacy_operations(
    pool: &SqlitePool,
    repository_id: Uuid,
    persistent: &Path,
) -> anyhow::Result<Option<[usize; 3]>> {
    use anyhow::{Context, ensure};
    let files = files::RepositoryMemoryStore::read_only_at_persistent(persistent)?
        .context("Legacy knowledge directory is missing")?;
    let _wiki_lock = files.try_lock()?;
    let _integration_lock = files.try_integration_lock()?;
    let mut tx = pool.begin_with("BEGIN IMMEDIATE").await?;
    let repos: Vec<Uuid> = sqlx::query_scalar("SELECT id FROM repos")
        .fetch_all(&mut *tx)
        .await?;
    ensure!(
        repos == vec![repository_id],
        "One-time development import expects exactly the selected registered Repo; inspect the actual Repo set before proceeding"
    );
    if RepositoryMemoryStore::state_on(&mut tx, repository_id)
        .await?
        .is_some()
    {
        return Ok(None);
    }
    for table in [
        "repository_wiki_receipts",
        "repository_wiki_publications",
        "repository_integrations",
    ] {
        let count: i64 = sqlx::query_scalar(&format!(
            "SELECT COUNT(*) FROM {table} WHERE repository_id=?"
        ))
        .bind(repository_id)
        .fetch_one(&mut *tx)
        .await?;
        ensure!(
            count == 0,
            "Operational records exist without the cutover state; inspect the database before retrying"
        );
    }
    let legacy = files.legacy_operational_state()?;
    ensure!(
        legacy.state.active_run_id.is_none() && legacy.state.bootstrap.is_none(),
        "Legacy Wiki maintenance is unfinished; settle it with the old version before importing"
    );
    let store = RepositoryMemoryStore::with_artifacts(pool, repository_id, files);
    for record in &legacy.integrations {
        for id in &record.event_ids {
            let event = store
                .event(*id)?
                .context("Legacy integration references a missing Manifest")?;
            ensure!(
                event.repository_id == repository_id,
                "Legacy Manifest belongs to a different Repo"
            );
        }
        store
            .save_on(&mut tx, "repository_integrations", record.id, record, true)
            .await?;
    }
    for record in &legacy.publications {
        store
            .save_on(
                &mut tx,
                "repository_wiki_publications",
                record.run_id,
                record,
                false,
            )
            .await?;
    }
    for record in &legacy.receipts {
        let event = store
            .event(record.event_id)?
            .context("Legacy receipt references a missing Manifest")?;
        ensure!(
            event.repository_id == repository_id,
            "Legacy receipt Manifest belongs to a different Repo"
        );
        store
            .save_on(
                &mut tx,
                "repository_wiki_receipts",
                record.event_id,
                record,
                true,
            )
            .await?;
    }
    sqlx::query("INSERT INTO repository_wiki_state(repository_id,payload) VALUES(?,?)")
        .bind(repository_id)
        .bind(serde_json::to_string(&legacy.state)?)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(Some([
        legacy.integrations.len(),
        legacy.publications.len(),
        legacy.receipts.len(),
    ]))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn new_repository_memory_is_enabled_without_starting_maintenance_and_preserves_opt_out() {
        let temp = tempfile::tempdir().unwrap();
        let store = RepositoryMemoryStore::at_persistent(temp.path()).unwrap();
        store
            .initialize_defaults(Some("main".into()))
            .await
            .unwrap();
        let state = store.state().await.unwrap();
        assert!(state.enabled);
        assert_eq!(state.target_branch.as_deref(), Some("main"));
        assert!(state.active_run_id.is_none());
        assert!(state.last_success.is_none());
        let opted_out = RepositoryMemoryState {
            target_branch: Some("custom".into()),
            ..Default::default()
        };
        store.save_state(&opted_out).await.unwrap();
        store
            .initialize_defaults(Some("main".into()))
            .await
            .unwrap();
        let state = store.state().await.unwrap();
        assert!(!state.enabled);
        assert_eq!(state.target_branch.as_deref(), Some("custom"));
    }

    #[tokio::test]
    async fn offline_import_preserves_blocked_candidates_runs_and_reservations() {
        use crate::models::integration::{
            IntegrationPublicationRequest, IntegrationRun, tests::fixture,
        };

        let temp = tempfile::tempdir().unwrap();
        files::RepositoryMemoryStore::at_persistent(temp.path()).unwrap();
        let state_path = temp.path().join("knowledge/state.json");
        let old_state = serde_json::to_vec(&RepositoryMemoryState::enabled_defaults(Some(
            "main".into(),
        )))
        .unwrap();
        std::fs::write(&state_path, &old_state).unwrap();
        let (pool, mut run, _) = fixture().await;
        let workspace = run.payload.sources[0].selection.workspace_id;
        let session: Uuid = sqlx::query_scalar("SELECT id FROM sessions WHERE workspace_id=?")
            .bind(workspace)
            .fetch_one(&pool)
            .await
            .unwrap();
        let mut agent = Uuid::nil();
        // Historical statuses remain even though no writer runs in this fixture.
        for status in [
            "pending",
            "starting",
            "running",
            "awaiting_input",
            "awaiting_approval",
            "cancelling",
        ] {
            agent = Uuid::new_v4();
            sqlx::query("INSERT INTO agent_runs(id,session_id,workspace_id,request_id,idempotency_key,correlation_id,schema_version,payload_version,runtime_profile_id,provider_id,workspace_mode,workspace_path,status,request_envelope) VALUES(?,?,?,?,?,?,1,1,'fixture','codex','shared_workspace','/fixture',?,'{}')")
                .bind(agent).bind(session).bind(workspace).bind(agent)
                .bind(agent.to_string()).bind(run.id).bind(status)
                .execute(&pool).await.unwrap();
        }
        run.status = "blocked".into();
        run.workspace_id = Some(workspace);
        run.session_id = Some(session);
        run.agent_run_id = Some(agent);
        run.error = Some("retained stop reason".into());
        run.payload.result_commit = Some("b".repeat(40));
        run.payload.publication_request = Some(IntegrationPublicationRequest {
            request_id: Uuid::new_v4(),
            agent_run_id: agent,
            code_commit: "b".repeat(40),
            wiki_commit: Some("c".repeat(40)),
            candidate_commit: "c".repeat(40),
        });
        run.payload.history.push(serde_json::json!({
            "status": "blocked", "candidate_commit": "c".repeat(40)
        }));
        run.save(&pool).await.unwrap();
        let other = Uuid::new_v4();
        sqlx::query("INSERT INTO integration_runs(id,request_key,project_id,repository_id,storage_identity,target_ref,status,workspace_id,session_id,agent_run_id,payload,error) SELECT ?,?,project_id,repository_id,storage_identity,target_ref,status,workspace_id,session_id,agent_run_id,payload,error FROM integration_runs WHERE id=?")
            .bind(other).bind(other.to_string()).bind(run.id)
            .execute(&pool).await.unwrap();

        let mut before = Vec::new();
        for id in [run.id, other] {
            before.push(
                serde_json::to_value(IntegrationRun::find(&pool, id).await.unwrap()).unwrap(),
            );
        }
        let agents_query = "SELECT id,session_id,workspace_id,correlation_id,status,request_envelope FROM agent_runs ORDER BY id";
        let agents = sqlx::query_as::<_, (Uuid, Uuid, Uuid, Uuid, String, String)>(agents_query)
            .fetch_all(&pool)
            .await
            .unwrap();
        let reservations_query = "SELECT resource_kind,resource_key,run_id FROM integration_reservations ORDER BY resource_kind,resource_key";
        let reservations = sqlx::query_as::<_, (String, String, Uuid)>(reservations_query)
            .fetch_all(&pool)
            .await
            .unwrap();
        assert!(!reservations.is_empty());

        for expected in [Some([0, 0, 0]), None] {
            assert_eq!(
                import_legacy_operations(&pool, run.repository_id, temp.path())
                    .await
                    .unwrap(),
                expected
            );
            for (index, id) in [run.id, other].into_iter().enumerate() {
                assert_eq!(
                    serde_json::to_value(IntegrationRun::find(&pool, id).await.unwrap()).unwrap(),
                    before[index]
                );
            }
            assert_eq!(
                sqlx::query_as::<_, (Uuid, Uuid, Uuid, Uuid, String, String)>(agents_query)
                    .fetch_all(&pool)
                    .await
                    .unwrap(),
                agents
            );
            assert_eq!(
                sqlx::query_as::<_, (String, String, Uuid)>(reservations_query)
                    .fetch_all(&pool)
                    .await
                    .unwrap(),
                reservations
            );
            assert_eq!(std::fs::read(&state_path).unwrap(), old_state);
        }
        pool.close().await;
    }

    #[tokio::test]
    async fn offline_import_rolls_back_retries_and_leaves_db_authoritative() {
        use utils::repository_memory::{
            ChangeManifest, ReconciliationResult, RepositoryWikiStatus, SemanticChanges,
        };
        let temp = tempfile::tempdir().unwrap();
        let files = files::RepositoryMemoryStore::at_persistent(temp.path()).unwrap();
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                sqlx::sqlite::SqliteConnectOptions::new()
                    .filename(temp.path().join("fixture.sqlite"))
                    .create_if_missing(true),
            )
            .await
            .unwrap();
        crate::run_migrations(&pool).await.unwrap();
        let repo = Uuid::new_v4();
        sqlx::query("INSERT INTO repos(id,path,name,display_name) VALUES(?,?,'fixture','Fixture')")
            .bind(repo)
            .bind(temp.path().to_string_lossy().as_ref())
            .execute(&pool)
            .await
            .unwrap();
        let event = ChangeManifest {
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
            semantics: SemanticChanges {
                goal: "Preserve development intent".into(),
                summary: "fix: fixture".into(),
                ..Default::default()
            },
        };
        files.publish_event(&event).unwrap();
        let mut state = RepositoryMemoryState::enabled_defaults(Some("main".into()));
        let integration = MemoryIntegration {
            id: Uuid::new_v4(),
            event_ids: vec![event.event_id],
            target_branch: "main".into(),
            before_commit: event.base_commit.clone(),
            integrated_commit: None,
            source_error: None,
        };
        let publication = WikiPublication {
            run_id: Uuid::new_v4(),
            source_commit: event.source_commit.clone(),
            target_branch: "main".into(),
            maintenance_commit: Some("c".repeat(40)),
            no_op: false,
        };
        let receipt = WikiReconciliationReceipt {
            event_id: event.event_id,
            reconciled_at: chrono::Utc::now(),
            target_commit: event.source_commit.clone(),
            wiki_commit: None,
            result: ReconciliationResult::Failed,
            error: Some("retry needed".into()),
        };
        let root = temp.path().join("knowledge");
        let write = |path: &Path, value: serde_json::Value| {
            std::fs::write(path, serde_json::to_vec(&value).unwrap()).unwrap();
        };
        for dir in ["integrations", "publications", "receipts"] {
            std::fs::create_dir(root.join(dir)).unwrap();
        }
        write(
            &root.join("state.json"),
            serde_json::to_value(&state).unwrap(),
        );
        write(
            &root.join(format!("integrations/{}.json", integration.id)),
            serde_json::to_value(&integration).unwrap(),
        );
        write(
            &root.join(format!("publications/{}.json", publication.run_id)),
            serde_json::to_value(&publication).unwrap(),
        );
        write(
            &root.join(format!("receipts/{}.json", event.event_id)),
            serde_json::to_value(&receipt).unwrap(),
        );
        let retained = root.join("sync-inputs/retained-input");
        std::fs::write(&retained, "fixed Manifest digest/identity").unwrap();
        let store = RepositoryMemoryStore::with_artifacts(&pool, repo, files);
        assert!(
            store
                .state()
                .await
                .unwrap_err()
                .to_string()
                .contains("one-time migration")
        );
        // An unfinished owner is preserved, not silently cleared or quarantined.
        state.active_run_id = Some(Uuid::new_v4());
        write(
            &root.join("state.json"),
            serde_json::to_value(&state).unwrap(),
        );
        assert!(
            import_legacy_operations(&pool, repo, temp.path())
                .await
                .is_err()
        );
        state.active_run_id = None;
        write(
            &root.join("state.json"),
            serde_json::to_value(&state).unwrap(),
        );
        let old_state = std::fs::read(root.join("state.json")).unwrap();
        let receipt_path = root.join(format!("receipts/{}.json", event.event_id));
        let old_receipt = std::fs::read(&receipt_path).unwrap();
        std::fs::write(&receipt_path, "invalid JSON").unwrap();
        let malformed = import_legacy_operations(&pool, repo, temp.path())
            .await
            .unwrap_err();
        assert!(
            malformed
                .to_string()
                .contains(receipt_path.to_string_lossy().as_ref())
        );
        std::fs::write(&receipt_path, &old_receipt).unwrap();

        // Fail after a mapping was inserted: the whole cutover must roll back.
        sqlx::query("CREATE TRIGGER fail_import BEFORE INSERT ON repository_wiki_publications BEGIN SELECT RAISE(ABORT,'fixture interruption'); END").execute(&pool).await.unwrap();
        assert!(
            import_legacy_operations(&pool, repo, temp.path())
                .await
                .is_err()
        );
        for table in [
            "repository_wiki_state",
            "repository_integrations",
            "repository_wiki_publications",
            "repository_wiki_receipts",
        ] {
            let count: i64 = sqlx::query_scalar(&format!("SELECT COUNT(*) FROM {table}"))
                .fetch_one(&pool)
                .await
                .unwrap();
            assert_eq!(count, 0);
        }
        assert_eq!(std::fs::read(root.join("state.json")).unwrap(), old_state);
        sqlx::query("DROP TRIGGER fail_import")
            .execute(&pool)
            .await
            .unwrap();
        assert_eq!(
            import_legacy_operations(&pool, repo, temp.path())
                .await
                .unwrap(),
            Some([1, 1, 1])
        );
        assert_eq!(
            serde_json::to_value(&store.integrations().await.unwrap()[0]).unwrap(),
            serde_json::to_value(&integration).unwrap()
        );
        assert_eq!(
            store
                .publication(publication.run_id)
                .await
                .unwrap()
                .unwrap()
                .maintenance_commit,
            publication.maintenance_commit
        );
        assert_eq!(
            store.receipt(event.event_id).await.unwrap().unwrap().result,
            ReconciliationResult::Failed
        );
        assert_eq!(
            std::fs::read_to_string(&retained).unwrap(),
            "fixed Manifest digest/identity"
        );
        assert!(store.event(event.event_id).unwrap().is_some());
        // A successful cutover never reimports stale files, even after retry.
        std::fs::write(root.join("state.json"), "corrupt stale file").unwrap();
        assert_eq!(
            import_legacy_operations(&pool, repo, temp.path())
                .await
                .unwrap(),
            None
        );
        assert!(store.state().await.unwrap().enabled);
        store.save_publication(&publication).await.unwrap();
        let mut conflicting = publication.clone();
        conflicting.no_op = true;
        assert!(store.save_publication(&conflicting).await.is_err());
        // Receipt and completed Wiki state are one transaction.
        let mut completed = state.clone();
        completed.status = RepositoryWikiStatus::Current;
        let mut success = receipt.clone();
        success.result = ReconciliationResult::NoOp;
        success.error = None;
        sqlx::query("CREATE TRIGGER fail_completion BEFORE UPDATE ON repository_wiki_state BEGIN SELECT RAISE(ABORT,'fixture interruption'); END").execute(&pool).await.unwrap();
        assert!(
            store
                .save_reconciliation(&completed, std::slice::from_ref(&success))
                .await
                .is_err()
        );
        assert_eq!(
            store.receipt(event.event_id).await.unwrap().unwrap().result,
            ReconciliationResult::Failed
        );
        sqlx::query("DROP TRIGGER fail_completion")
            .execute(&pool)
            .await
            .unwrap();
        store
            .save_reconciliation(&completed, &[success])
            .await
            .unwrap();
        assert_eq!(
            store.receipt(event.event_id).await.unwrap().unwrap().result,
            ReconciliationResult::NoOp
        );
        pool.close().await;
        let reopened = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                sqlx::sqlite::SqliteConnectOptions::new()
                    .filename(temp.path().join("fixture.sqlite")),
            )
            .await
            .unwrap();
        let durable = RepositoryMemoryStore::with_artifacts(
            &reopened,
            repo,
            files::RepositoryMemoryStore::read_only_at_persistent(temp.path())
                .unwrap()
                .unwrap(),
        );
        assert_eq!(
            durable.state().await.unwrap().status,
            RepositoryWikiStatus::Current
        );
        assert_eq!(
            durable
                .receipt(event.event_id)
                .await
                .unwrap()
                .unwrap()
                .result,
            ReconciliationResult::NoOp
        );
        assert_eq!(
            durable
                .publication(publication.run_id)
                .await
                .unwrap()
                .unwrap()
                .maintenance_commit,
            publication.maintenance_commit
        );
        reopened.close().await;
    }
}
