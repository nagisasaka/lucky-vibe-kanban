//! The final semantic record is immutable and keyed by (registered Repo, Run).
//! Legacy file imports are retired; all readers and writers use the instance DB.
use files::{ChangeManifest, ManifestDiscoveryError, ManifestDiscoveryPage, ManifestReference};

use super::*;

impl RepositoryMemoryStore {
    pub fn repository_id(&self) -> Uuid {
        self.repository_id
    }

    fn validate_event(repository_id: Uuid, value: &ChangeManifest, id: Uuid) -> io::Result<()> {
        value.validate()?;
        if value.repository_id != repository_id || value.event_id != id {
            return Err(invalid("Manifest repository/Run identity mismatch"));
        }
        Ok(())
    }

    async fn read_event_on(
        repository_id: Uuid,
        conn: &mut SqliteConnection,
        id: Uuid,
    ) -> io::Result<Option<ChangeManifest>> {
        let row: Option<(Uuid, String)> = sqlx::query_as(
            "SELECT workspace_id,payload FROM repository_change_manifests WHERE repository_id=? AND id=?",
        ).bind(repository_id).bind(id).fetch_optional(conn).await.map_err(invalid)?;
        row.map(|(workspace, json)| {
            let event: ChangeManifest = serde_json::from_str(&json).map_err(invalid)?;
            Self::validate_event(repository_id, &event, id)?;
            if workspace != event.workspace_id {
                return Err(invalid("Manifest Workspace identity mismatch"));
            }
            Ok(event)
        })
        .transpose()
    }

    async fn event_on(
        &self,
        conn: &mut SqliteConnection,
        id: Uuid,
    ) -> io::Result<Option<ChangeManifest>> {
        Self::read_event_on(self.repository_id, conn, id).await
    }

    /// Public record lookup without resolving or creating shared directories.
    pub async fn read_event(
        pool: &SqlitePool,
        repository_id: Uuid,
        id: Uuid,
    ) -> io::Result<Option<ChangeManifest>> {
        Self::read_event_on(
            repository_id,
            &mut *pool.acquire().await.map_err(invalid)?,
            id,
        )
        .await
    }

    pub async fn event(&self, id: Uuid) -> io::Result<Option<ChangeManifest>> {
        self.event_on(&mut *self.connection().await?, id).await
    }

    /// Narrow boundary for consumers such as Project portability: the caller
    /// resolves authorized completed Run IDs from that Project's Workspace links.
    /// Repository-wide Wiki history must not be substituted for this selection.
    /// Return absent records explicitly so the caller can distinguish expected
    /// no-change/control Runs from missing evidence. Membership is never inferred.
    pub async fn events_for_runs(
        &self,
        workspace_id: Uuid,
        run_ids: &[Uuid],
    ) -> io::Result<Vec<(Uuid, Option<ChangeManifest>)>> {
        let mut events = Vec::new();
        let mut seen = std::collections::HashSet::new();
        let mut conn = self.connection().await?;
        for id in run_ids.iter().copied().filter(|id| seen.insert(*id)) {
            let event = self.event_on(&mut conn, id).await?;
            if event
                .as_ref()
                .is_some_and(|event| event.workspace_id != workspace_id)
            {
                return Err(invalid(
                    "Selected Manifest does not belong to the requested Workspace",
                ));
            }
            events.push((id, event));
        }
        Ok(events)
    }

    async fn publish_event_on(
        &self,
        conn: &mut SqliteConnection,
        event: &ChangeManifest,
    ) -> io::Result<()> {
        Self::validate_event(self.repository_id, event, event.event_id)?;
        let json = serde_json::to_string(event).map_err(invalid)?;
        if json.len() > 128 * 1024 {
            return Err(invalid("Manifest is too large"));
        }
        if let Some(old) = self.event_on(conn, event.event_id).await? {
            if old == *event {
                return Ok(());
            }
            return Err(invalid(format!(
                "Immutable Manifest conflict: repository {} / event {}",
                self.repository_id, event.event_id
            )));
        }
        sqlx::query("INSERT INTO repository_change_manifests(repository_id,id,workspace_id,payload) VALUES(?,?,?,?)")
            .bind(self.repository_id).bind(event.event_id).bind(event.workspace_id).bind(json)
            .execute(conn).await.map_err(invalid)?;
        Ok(())
    }

    pub async fn publish_event(&self, event: &ChangeManifest) -> io::Result<()> {
        let mut conn = self.connection().await?;
        let mut tx = conn.begin_with("BEGIN IMMEDIATE").await.map_err(invalid)?;
        self.publish_event_on(&mut tx, event).await?;
        tx.commit().await.map_err(invalid)
    }

    pub async fn events(&self) -> io::Result<Vec<ChangeManifest>> {
        let mut conn = self.connection().await?;
        let ids: Vec<Uuid> = sqlx::query_scalar(
            "SELECT id FROM repository_change_manifests WHERE repository_id=? ORDER BY id",
        )
        .bind(self.repository_id)
        .fetch_all(&mut *conn)
        .await
        .map_err(invalid)?;
        let mut events = Vec::with_capacity(ids.len());
        for id in ids {
            events.push(
                self.event_on(&mut conn, id)
                    .await?
                    .ok_or_else(|| invalid("Manifest disappeared"))?,
            );
        }
        events.sort_by_key(|event| (event.created_at, event.event_id));
        Ok(events)
    }

    /// Stable ID pagination, retaining the previous UUID.json cursor spelling.
    /// Bad records advance the cursor and are reported; strict readers fail.
    pub async fn discover_events(
        &self,
        repository_id: Uuid,
        after: Option<&str>,
        limit: usize,
    ) -> io::Result<ManifestDiscoveryPage> {
        if repository_id != self.repository_id {
            return Err(invalid("Manifest discovery repository mismatch"));
        }
        let after = after
            .map(|cursor| {
                Uuid::parse_str(cursor.strip_suffix(".json").unwrap_or(cursor)).map_err(invalid)
            })
            .transpose()?;
        let limit = limit.clamp(1, 100);
        let mut conn = self.connection().await?;
        let mut ids: Vec<Uuid> = sqlx::query_scalar("SELECT id FROM repository_change_manifests WHERE repository_id=? AND (? IS NULL OR id>?) ORDER BY id LIMIT ?")
            .bind(self.repository_id).bind(after).bind(after).bind((limit+1) as i64)
            .fetch_all(&mut *conn).await.map_err(invalid)?;
        let mut page = ManifestDiscoveryPage::default();
        if ids.len() > limit {
            page.next_after = Some(format!("{}.json", ids[limit - 1]));
            ids.truncate(limit);
        }
        for id in ids {
            match self.event_on(&mut conn, id).await {
                Ok(Some(event)) => page.records.push(ManifestReference {
                    event_id: id,
                    repository_id: event.repository_id,
                    workspace_id: event.workspace_id,
                    base_commit: event.base_commit,
                    source_commit: event.source_commit,
                    target_branch: event.target_branch,
                    created_at: event.created_at,
                    url: format!("/api/repos/{repository_id}/change-manifests/{id}"),
                }),
                result => page.errors.push(ManifestDiscoveryError {
                    filename: format!("{id}.json"),
                    error: match result {
                        Err(error) => error.to_string(),
                        _ => "Manifest disappeared".into(),
                    },
                }),
            }
        }
        Ok(page)
    }

    pub(super) async fn validate_receipt_on(
        &self,
        conn: &mut SqliteConnection,
        value: &WikiReconciliationReceipt,
    ) -> io::Result<()> {
        files::validate_receipt_identity(value)?;
        if self.event_on(conn, value.event_id).await?.is_none() {
            return Err(invalid("Cannot acknowledge an unknown semantic event"));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
