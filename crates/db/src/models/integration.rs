//! Formal Integration product state. Durable reservations are independent of
//! expiring Orchestration dispatcher leases. No conversation/semantic database.
use anyhow::Context;
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqliteConnection, SqlitePool};
use ts_rs::TS;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct IntegrationSelection {
    pub card_id: Uuid,
    pub workspace_id: Uuid,
    // HEAD observed when the user adopted this Workspace, not a moving ref.
    pub expected_commit: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct IntegrationSource {
    pub selection: IntegrationSelection,
    pub branch: String,
    pub commit: String,
    pub title: String,
    pub description: Option<String>,
    pub status_id: Uuid,
    pub requirements_revision: i64,
    pub event_ids: Vec<Uuid>,
    // Historical attribution; source Workspaces are not reserved by new runs.
    pub related_workspaces: Vec<Uuid>,
    pub done_result: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct IntegrationValidationSuggestion {
    pub command: String,
    // Relative to the repository root, not the multi-repository container.
    pub cwd: String,
    pub evidence: String,
    pub environment_requirements: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct IntegrationStop {
    pub stage: String,
    pub category: String,
    pub next_action: String,
}

/// One immutable observation/decision on a code candidate. Results on earlier
/// candidates remain history, never a pass on a later repaired candidate.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct IntegrationEvidence {
    pub request_id: Uuid,
    pub agent_run_id: Uuid,
    pub code_commit: String,
    pub command: Option<String>,
    pub process_id: Option<Uuid>,
    pub result: String,
    pub actor: String,
    pub reason: String,
    pub remaining_risk: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct IntegrationPublicationRequest {
    pub request_id: Uuid,
    pub agent_run_id: Uuid,
    pub code_commit: String,
    pub wiki_commit: Option<String>,
    pub candidate_commit: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, TS)]
pub struct IntegrationPayload {
    #[serde(default)]
    pub wiki_enabled: bool,
    #[serde(default)]
    pub evidence: Vec<IntegrationEvidence>,
    #[serde(default)]
    pub publication_request: Option<IntegrationPublicationRequest>,
    #[serde(default)]
    pub instructions: String,
    /// Immutable initial worker Create envelope for restart/redelivery.
    #[serde(default)]
    pub worker_dispatch: Option<serde_json::Value>,
    /// Adopted reference Skill and public Manifests, saved alongside the initial dispatch.
    #[serde(default)]
    pub reference_materials: Option<serde_json::Value>,
    /// Initial user instruction to omit validation; actual decisions are recorded in evidence.
    #[serde(default)]
    pub skip_validation: bool,
    #[serde(default)]
    pub card_id: Option<Uuid>,
    #[serde(default)]
    pub card_done_result: Option<String>,
    #[serde(default)]
    pub stop: Option<IntegrationStop>,
    #[serde(default)]
    pub history: Vec<serde_json::Value>,
    pub sources: Vec<IntegrationSource>,
    pub observed_target: String,
    pub base_commit: Option<String>,
    pub result_commit: Option<String>,
    #[serde(default)]
    pub validation_suggestions: Vec<IntegrationValidationSuggestion>,
    pub semantic_summary: Option<String>,
    pub publication_intent: bool,
    pub published: bool,
    pub wiki_result: Option<String>,
    pub executor_config: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, FromRow)]
pub struct IntegrationRun {
    pub id: Uuid,
    pub request_key: String,
    pub project_id: Uuid,
    pub repository_id: Uuid,
    pub storage_identity: String,
    pub target_ref: String,
    pub status: String,
    pub workspace_id: Option<Uuid>,
    pub session_id: Option<Uuid>,
    pub agent_run_id: Option<Uuid>,
    #[ts(type = "IntegrationPayload")]
    pub payload: sqlx::types::Json<IntegrationPayload>,
    pub cancel_requested: bool,
    pub error: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl IntegrationRun {}

pub fn resource_key(id: Uuid) -> String {
    id.simple().to_string()
}

pub async fn workspace_owner(pool: &SqlitePool, id: Uuid) -> Result<Option<Uuid>, sqlx::Error> {
    sqlx::query_scalar("SELECT run_id FROM integration_reservations WHERE resource_kind='workspace' AND resource_key=?")
        .bind(resource_key(id)).fetch_optional(pool).await
}

pub async fn guard_workspace(pool: &SqlitePool, id: Uuid) -> Result<(), sqlx::Error> {
    if let Some(run) = workspace_owner(pool, id).await? {
        let product = IntegrationRun::find(pool, run).await?;
        if !product.cancel_requested
            && product.status == "integrating"
            && product.payload.publication_request.is_none()
        {
            return Ok(());
        }
        return Err(sqlx::Error::Protocol(format!(
            "Workspace reserved by Auto-merge {run}; inspect /api/integrations/{run}"
        )));
    }
    Ok(())
}

pub async fn guard_agent_dispatch(
    pool: &SqlitePool,
    workspace: Uuid,
    session: Uuid,
    correlation: Uuid,
) -> Result<(), sqlx::Error> {
    let conversation: Option<IntegrationRun> =
        sqlx::query_as("SELECT * FROM integration_runs WHERE workspace_id=?")
            .bind(workspace)
            .fetch_optional(pool)
            .await?;
    if let Some(run) = conversation {
        if correlation != run.id {
            return Err(sqlx::Error::Protocol(
                "Auto-merge correlation mismatch".into(),
            ));
        }
        if run.session_id == Some(session)
            && (run.payload.published
                || (run.status == "cancelled" && !run.payload.publication_intent))
        {
            return Ok(()); // an explicit normal follow-up may reopen settled work
        }
        if run.session_id == Some(session)
            && !run.cancel_requested
            && !run.payload.publication_intent
            && run.payload.publication_request.is_none()
            && matches!(run.status.as_str(), "integrating" | "blocked")
        {
            return Ok(());
        }
        return Err(sqlx::Error::Protocol(
            "Auto-merge owner is not accepting execution".into(),
        ));
    }
    if let Some(owner) = workspace_owner(pool, workspace).await? {
        return Err(sqlx::Error::Protocol(format!(
            "Workspace reserved by Auto-merge {owner}"
        )));
    }
    Ok(())
}

impl IntegrationRun {
    pub async fn create_with_card(
        &self,
        pool: &SqlitePool,
        branch: &str,
        name: &str,
    ) -> anyhow::Result<()> {
        // The request, Card and linked Workspace are one admission transaction.
        // Retries return the same run, including while it is waiting for the target.
        let card_id = self.payload.card_id.context("Missing Auto-merge Card")?;
        let workspace_id = self.workspace_id.context("Missing Auto-merge Workspace")?;
        let target = self
            .target_ref
            .strip_prefix("refs/heads/")
            .unwrap_or(&self.target_ref);
        let mut tx = pool.begin_with("BEGIN IMMEDIATE").await?;
        let status: Uuid = sqlx::query_scalar("SELECT id FROM local_project_statuses WHERE project_id=? AND hidden=0 ORDER BY (lower(name)='in progress') DESC,sort_order,id LIMIT 1")
        .bind(self.project_id).fetch_one(&mut *tx).await?;
        let (number, simple_id) =
            crate::models::issue_number::next_issue_number(&mut tx, self.project_id).await?;
        let description = self
            .payload
            .sources
            .iter()
            .map(|s| {
                format!(
                    "- [{}](/projects/{}/issues/{}): [Workspace](/projects/{}/issues/{}/workspaces/{}) · `{}`",
                    s.title.replace(['[', ']'], ""), self.project_id, s.selection.card_id,
                    self.project_id, s.selection.card_id, s.selection.workspace_id, s.commit
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        let description = if self.payload.skip_validation {
            format!("Validation skipped by explicit request — unverified.\n\n{description}")
        } else {
            description
        };
        sqlx::query("INSERT INTO local_issues(id,project_id,issue_number,simple_id,status_id,title,description,sort_order,creator_user_id) VALUES(?,?,?,?,?,?,?,0,?)")
        .bind(card_id).bind(self.project_id).bind(number).bind(simple_id).bind(status).bind(name).bind(description).bind(Uuid::from_u128(1)).execute(&mut *tx).await?;
        crate::models::board_tag::attach_system(&mut tx, self.project_id, card_id).await?;
        let owner = crate::models::workspace_usage::WorkspaceExecutionOwner::new(
            "integration",
            self.repository_id,
            Some(self.id),
        );
        sqlx::query(
            "INSERT INTO workspaces(id,branch,name,usage,execution_owner) VALUES(?,?,?,?,?)",
        )
        .bind(workspace_id)
        .bind(branch)
        .bind(name)
        .bind("interactive")
        .bind(serde_json::to_string(&owner)?)
        .execute(&mut *tx)
        .await?;
        super::workspace_repo::WorkspaceRepo::create_many_in_transaction(
            &mut tx,
            workspace_id,
            &[super::workspace_repo::CreateWorkspaceRepo {
                repo_id: self.repository_id,
                target_branch: target.into(),
            }],
        )
        .await?;
        sqlx::query(
            "INSERT INTO local_workspace_links(workspace_id,project_id,issue_id) VALUES(?,?,?)",
        )
        .bind(workspace_id)
        .bind(self.project_id)
        .bind(card_id)
        .execute(&mut *tx)
        .await?;
        sqlx::query("INSERT INTO generated_issue_workspaces(workspace_id,issue_id,project_id,owner_kind,repository_id,owner_run_id) VALUES(?,?,?,'integration',?,?)")
            .bind(workspace_id).bind(card_id).bind(self.project_id).bind(self.repository_id).bind(self.id.to_string())
            .execute(&mut *tx).await?;
        sqlx::query("INSERT INTO integration_runs(id,request_key,project_id,repository_id,storage_identity,target_ref,status,workspace_id,payload) VALUES(?,?,?,?,?,?,'queued',?,?)")
        .bind(self.id).bind(&self.request_key).bind(self.project_id).bind(self.repository_id).bind(&self.storage_identity).bind(&self.target_ref).bind(workspace_id).bind(&self.payload).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn allows_instruction(
        pool: &SqlitePool,
        workspace: Uuid,
        session: Uuid,
        agent: Uuid,
    ) -> Result<bool, sqlx::Error> {
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM integration_runs i JOIN integration_reservations r ON r.run_id=i.id AND r.resource_kind='workspace' AND r.resource_key=? WHERE i.workspace_id=? AND i.session_id=? AND i.agent_run_id=? AND i.status='integrating' AND i.cancel_requested=0 AND json_extract(i.payload,'$.publication_intent')=0)")
            .bind(resource_key(workspace)).bind(workspace).bind(session).bind(agent).fetch_one(pool).await
    }

    /// Execute before the generic Orchestration outbox is redelivered. A
    /// half-prepared product must never start a new child just because its
    /// dispatcher lease disappeared. Keep reservations until stop confirmation.
    pub async fn request_cancel(pool: &SqlitePool, id: Uuid) -> Result<Self, sqlx::Error> {
        sqlx::query("UPDATE integration_runs SET cancel_requested=1,status=CASE WHEN status='blocked' THEN 'cancelling' ELSE status END,updated_at=datetime('now','subsec') WHERE id=? AND json_extract(payload,'$.publication_intent')=0 AND status IN ('queued','integrating','validating','cancelling','blocked')")
            .bind(id).execute(pool).await?;
        Self::find(pool, id).await
    }

    /// Used inside the same writer transaction as target reservations. Exactly
    /// one of accepted cancellation or publication intent can win.
    pub async fn record_publication_intent(
        connection: &mut sqlx::SqliteConnection,
        run: &Self,
    ) -> Result<(), sqlx::Error> {
        if !run.payload.publication_intent || run.payload.published {
            return Err(sqlx::Error::Protocol("Invalid publication intent".into()));
        }
        let changed=sqlx::query("UPDATE integration_runs SET status='publishing',payload=?,updated_at=datetime('now','subsec') WHERE id=? AND cancel_requested=0 AND status='validating'")
            .bind(&run.payload).bind(run.id).execute(connection).await?.rows_affected();
        if changed != 1 {
            return Err(sqlx::Error::Protocol(
                "Cancelled or ownership changed before publication".into(),
            ));
        }
        Ok(())
    }
    /// Only the current admission schema is executable. Retired persisted jobs
    /// are neither migrated into a new worker nor rewritten by the monitor.
    pub async fn active(pool: &SqlitePool) -> Result<Vec<Self>, sqlx::Error> {
        sqlx::query_as("SELECT * FROM integration_runs WHERE json_type(payload,'$.validation_suggestions')='array' AND status NOT IN ('succeeded','blocked','failed','cancelled','recovery_required') ORDER BY created_at,id")
            .fetch_all(pool).await
    }
    pub async fn find(pool: &SqlitePool, id: Uuid) -> Result<Self, sqlx::Error> {
        sqlx::query_as("SELECT * FROM integration_runs WHERE id=?")
            .bind(id)
            .fetch_one(pool)
            .await
    }
    pub async fn save(&self, pool: &SqlitePool) -> Result<(), sqlx::Error> {
        self.save_on(&mut *pool.acquire().await?).await
    }

    pub async fn save_on(&self, connection: &mut SqliteConnection) -> Result<(), sqlx::Error> {
        // Cancellation is never overwritten by a worker's older observation.
        sqlx::query("UPDATE integration_runs SET status=?,workspace_id=?,session_id=?,agent_run_id=?,payload=?,error=?,updated_at=datetime('now','subsec') WHERE id=?")
            .bind(&self.status).bind(self.workspace_id).bind(self.session_id).bind(self.agent_run_id)
            .bind(&self.payload).bind(&self.error).bind(self.id).execute(connection).await?;
        Ok(())
    }
    /// Card effect and its durable receipt commit together. Reload under the
    /// SQLite writer lock, so concurrent recovery cannot re-Done a reopened Card.
    pub async fn complete_cards(
        pool: &SqlitePool,
        id: Uuid,
        source_heads: &std::collections::HashMap<Uuid, String>,
    ) -> Result<Self, sqlx::Error> {
        let mut tx = pool.begin_with("BEGIN IMMEDIATE").await?;
        let mut run: Self = sqlx::query_as("SELECT * FROM integration_runs WHERE id=?")
            .bind(id)
            .fetch_one(&mut *tx)
            .await?;
        if !run.payload.published {
            return Err(sqlx::Error::Protocol(
                "Cannot update Cards before confirmed Git publication".into(),
            ));
        }
        let done:Option<Uuid>=sqlx::query_scalar("SELECT id FROM local_project_statuses WHERE project_id=? AND hidden=0 ORDER BY (lower(name)='done') DESC,sort_order DESC,id LIMIT 1")
            .bind(run.project_id).fetch_optional(&mut *tx).await?;
        for source in &mut run.payload.sources {
            if source.done_result.is_some() {
                continue;
            }
            let owner: Option<Uuid> = sqlx::query_scalar("SELECT run_id FROM integration_reservations WHERE resource_kind='card' AND resource_key=?")
                .bind(resource_key(source.selection.card_id)).fetch_optional(&mut *tx).await?;
            if owner.is_some_and(|owner| owner != id) {
                source.done_result =
                    Some("not changed: Card reserved by another Auto-merge".into());
                continue;
            }
            sqlx::query("DELETE FROM integration_reservations WHERE run_id=? AND resource_kind='card' AND resource_key=?")
                .bind(id).bind(resource_key(source.selection.card_id)).execute(&mut *tx).await?;
            let active: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM agent_runs WHERE workspace_id=? AND status NOT IN ('succeeded','failed','cancelled','crashed','audit_failed'))")
                .bind(source.selection.workspace_id).fetch_one(&mut *tx).await?;
            let Some(head) = source_heads.get(&source.selection.workspace_id) else {
                source.done_result = Some("not changed: source HEAD unavailable".into());
                continue;
            };
            if head != &source.commit || active {
                source.done_result = Some(
                    match (head != &source.commit, active) {
                        (true, true) => "not changed: source HEAD changed and Run active",
                        (true, false) => "not changed: source HEAD changed",
                        (false, true) => "not changed: source Run active",
                        _ => unreachable!(),
                    }
                    .into(),
                );
                continue;
            }
            let count = if let Some(done) = done {
                sqlx::query("UPDATE local_issues SET status_id=?,completed_at=datetime('now','subsec'),updated_at=datetime('now','subsec') WHERE id=? AND project_id=?")
                    .bind(done).bind(source.selection.card_id).bind(run.project_id).execute(&mut *tx).await?.rows_affected()
            } else {
                0
            };
            source.done_result = Some(
                if count == 1 {
                    "done"
                } else {
                    "not changed: Card or destination column is unavailable"
                }
                .into(),
            );
        }
        if let Some(card) = run.payload.card_id
            && run.payload.card_done_result.is_none()
        {
            // New dedicated Issues have a publication-driven projection and
            // receipt. Replaying post-processing must not re-Done an operator's
            // reopened/cancelled Issue. Historical cards keep their old path.
            let projected: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM generated_issue_status s JOIN generated_issue_workspaces g ON g.workspace_id=s.workspace_id WHERE g.issue_id=? AND g.owner_run_id=?)")
                .bind(card).bind(id.to_string()).fetch_one(&mut *tx).await?;
            let count = if projected {
                sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM local_issues i JOIN local_project_statuses s ON s.id=i.status_id WHERE i.id=? AND lower(s.name)='done'")
                    .bind(card).fetch_one(&mut *tx).await? as u64
            } else if let Some(done) = done {
                sqlx::query("UPDATE local_issues SET status_id=?,completed_at=datetime('now','subsec'),updated_at=datetime('now','subsec') WHERE id=? AND project_id=? AND status_id NOT IN (SELECT id FROM local_project_statuses WHERE lower(name) IN ('cancelled','canceled'))")
                    .bind(done).bind(card).bind(run.project_id).execute(&mut *tx).await?.rows_affected()
            } else {
                0
            };
            run.payload.card_done_result = Some(
                if count == 1 {
                    "done"
                } else if projected {
                    "managed by generated Issue publication status"
                } else {
                    "Card or destination column is unavailable"
                }
                .into(),
            );
        }
        sqlx::query(
            "UPDATE integration_runs SET payload=?,updated_at=datetime('now','subsec') WHERE id=?",
        )
        .bind(&run.payload)
        .bind(id)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(run)
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::models::{
        repo::Repo,
        session::{CreateSession, Session},
        workspace::{CreateWorkspace, Workspace},
    };

    pub(crate) async fn fixture() -> (SqlitePool, IntegrationRun, Uuid) {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        let project = Uuid::new_v4();
        let card = Uuid::new_v4();
        let todo = Uuid::new_v4();
        let done = Uuid::new_v4();
        sqlx::query("INSERT INTO projects(id,name) VALUES(?,'test')")
            .bind(project)
            .execute(&pool)
            .await
            .unwrap();
        for (id, name) in [(todo, "In Progress"), (done, "Done")] {
            sqlx::query("INSERT INTO local_project_statuses(id,project_id,name,color,sort_order) VALUES(?,?,?,'blue',0)").bind(id).bind(project).bind(name).execute(&pool).await.unwrap();
        }
        sqlx::query("INSERT INTO local_issues(id,project_id,issue_number,simple_id,status_id,title,sort_order) VALUES(?,?,1,'TEST-1',?,'request',0)").bind(card).bind(project).bind(todo).execute(&pool).await.unwrap();
        let ws = Workspace::create(
            &pool,
            &CreateWorkspace {
                branch: "source".into(),
                name: None,
            },
            Uuid::new_v4(),
        )
        .await
        .unwrap();
        Session::create(
            &pool,
            &CreateSession {
                executor: None,
                name: None,
            },
            Uuid::new_v4(),
            ws.id,
        )
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO local_workspace_links(workspace_id,issue_id,project_id) VALUES(?,?,?)",
        )
        .bind(ws.id)
        .bind(card)
        .bind(project)
        .execute(&pool)
        .await
        .unwrap();
        let repo = Repo::find_or_create(
            &pool,
            std::path::Path::new("/test-integration-fixture"),
            "test",
        )
        .await
        .unwrap();
        let id = Uuid::new_v4();
        let payload = IntegrationPayload {
            sources: vec![IntegrationSource {
                selection: IntegrationSelection {
                    card_id: card,
                    workspace_id: ws.id,
                    expected_commit: "a".repeat(40),
                },
                branch: "source".into(),
                commit: "a".repeat(40),
                title: "request".into(),
                description: None,
                status_id: todo,
                requirements_revision: 1,
                event_ids: vec![],
                related_workspaces: vec![ws.id],
                done_result: None,
            }],
            ..Default::default()
        };
        sqlx::query("INSERT INTO integration_runs(id,request_key,project_id,repository_id,storage_identity,target_ref,status,payload) VALUES(?,?,?,?, '/git/common','refs/heads/test','queued',?)")
            .bind(id).bind(id.to_string()).bind(project).bind(repo.id).bind(sqlx::types::Json(payload)).execute(&pool).await.unwrap();
        for (kind, id) in [("card", card), ("workspace", ws.id)] {
            sqlx::query("INSERT INTO integration_reservations VALUES(?,?,?)")
                .bind(kind)
                .bind(resource_key(id))
                .bind(
                    sqlx::query_scalar::<_, Uuid>("SELECT id FROM integration_runs")
                        .fetch_one(&pool)
                        .await
                        .unwrap(),
                )
                .execute(&pool)
                .await
                .unwrap();
        }
        (
            pool.clone(),
            IntegrationRun::find(&pool, id).await.unwrap(),
            done,
        )
    }

    async fn complete_cards(pool: &SqlitePool, id: Uuid) -> Result<IntegrationRun, sqlx::Error> {
        let run = IntegrationRun::find(pool, id).await?;
        let heads = run
            .payload
            .sources
            .iter()
            .map(|s| (s.selection.workspace_id, s.commit.clone()))
            .collect();
        IntegrationRun::complete_cards(pool, id, &heads).await
    }

    #[tokio::test]
    async fn done_is_held_only_for_adopted_head_or_active_run() {
        for (changed, active) in [(true, false), (false, true), (true, true), (false, false)] {
            let (pool, mut run, done) = fixture().await;
            let source = run.payload.sources[0].clone();
            run.payload.published = true;
            run.save(&pool).await.unwrap();
            // Source reservations are historical fixture data; normal admission has none.
            sqlx::query("DELETE FROM integration_reservations")
                .execute(&pool)
                .await
                .unwrap();
            if active {
                let session = Session::create(
                    &pool,
                    &CreateSession {
                        executor: None,
                        name: None,
                    },
                    Uuid::new_v4(),
                    source.selection.workspace_id,
                )
                .await
                .unwrap();
                let agent = Uuid::new_v4();
                sqlx::query("INSERT INTO agent_runs(id,session_id,workspace_id,request_id,idempotency_key,correlation_id,schema_version,payload_version,runtime_profile_id,provider_id,workspace_mode,workspace_path,status,request_envelope) VALUES(?,?,?,?,?,?,1,1,'CODEX','codex','isolated_worktree','/fixture','running','{}')")
                    .bind(agent).bind(session.id).bind(source.selection.workspace_id).bind(Uuid::new_v4()).bind(agent.to_string()).bind(Uuid::new_v4()).execute(&pool).await.unwrap();
            }
            // Text/revision changes are irrelevant, including alongside both negative checks.
            sqlx::query("UPDATE local_issues SET title='recorded decision',description='new text' WHERE id=?").bind(source.selection.card_id).execute(&pool).await.unwrap();
            let heads = [(
                source.selection.workspace_id,
                if changed {
                    "b".repeat(40)
                } else {
                    source.commit.clone()
                },
            )]
            .into_iter()
            .collect();
            let completed = IntegrationRun::complete_cards(&pool, run.id, &heads)
                .await
                .unwrap();
            assert!(completed.payload.published);
            let status: Uuid = sqlx::query_scalar("SELECT status_id FROM local_issues WHERE id=?")
                .bind(source.selection.card_id)
                .fetch_one(&pool)
                .await
                .unwrap();
            assert_eq!(
                status,
                if changed || active {
                    source.status_id
                } else {
                    done
                }
            );
            assert_eq!(
                completed.payload.sources[0].done_result.as_deref() == Some("done"),
                !changed && !active
            );
        }
    }

    #[tokio::test]
    async fn active_run_in_another_workspace_of_same_issue_does_not_hold_done() {
        let (pool, mut run, _) = fixture().await;
        sqlx::query("DELETE FROM integration_reservations")
            .execute(&pool)
            .await
            .unwrap();
        let peer = Workspace::create(
            &pool,
            &CreateWorkspace {
                branch: "other".into(),
                name: None,
            },
            Uuid::new_v4(),
        )
        .await
        .unwrap();
        let session = Session::create(
            &pool,
            &CreateSession {
                executor: None,
                name: None,
            },
            Uuid::new_v4(),
            peer.id,
        )
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO local_workspace_links(workspace_id,issue_id,project_id) VALUES(?,?,?)",
        )
        .bind(peer.id)
        .bind(run.payload.sources[0].selection.card_id)
        .bind(run.project_id)
        .execute(&pool)
        .await
        .unwrap();
        let agent = Uuid::new_v4();
        sqlx::query("INSERT INTO agent_runs(id,session_id,workspace_id,request_id,idempotency_key,correlation_id,schema_version,payload_version,runtime_profile_id,provider_id,workspace_mode,workspace_path,status,request_envelope) VALUES(?,?,?,?,?,?,1,1,'CODEX','codex','isolated_worktree','/fixture','running','{}')")
            .bind(agent).bind(session.id).bind(peer.id).bind(Uuid::new_v4()).bind(agent.to_string()).bind(Uuid::new_v4()).execute(&pool).await.unwrap();
        run.workspace_id = Some(peer.id); // Even the Auto-merge Workspace is outside Done eligibility.
        run.payload.published = true;
        run.save(&pool).await.unwrap();
        let completed = complete_cards(&pool, run.id).await.unwrap();
        assert_eq!(
            completed.payload.sources[0].done_result.as_deref(),
            Some("done")
        );
    }

    #[tokio::test]
    async fn missing_head_observation_does_not_mean_unchanged() {
        let (pool, mut run, _) = fixture().await;
        run.payload.published = true;
        run.save(&pool).await.unwrap();
        let original = run.payload.sources[0].status_id;
        let result = IntegrationRun::complete_cards(&pool, run.id, &Default::default())
            .await
            .unwrap();
        assert_eq!(
            result.payload.sources[0].done_result.as_deref(),
            Some("not changed: source HEAD unavailable")
        );
        let status: Uuid = sqlx::query_scalar("SELECT status_id FROM local_issues WHERE id=?")
            .bind(run.payload.sources[0].selection.card_id)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(status, original);
        let replay = complete_cards(&pool, run.id).await.unwrap();
        assert_eq!(
            replay.payload.sources[0].done_result,
            result.payload.sources[0].done_result
        );
    }

    #[tokio::test]
    async fn instruction_permission_is_limited_to_exact_owned_integration_execution() {
        let (pool, mut run, _) = fixture().await;
        let workspace = run.payload.sources[0].selection.workspace_id;
        let session: Uuid = sqlx::query_scalar("SELECT id FROM sessions WHERE workspace_id=?")
            .bind(workspace)
            .fetch_one(&pool)
            .await
            .unwrap();
        let agent = Uuid::new_v4();
        run.workspace_id = Some(workspace);
        run.session_id = Some(session);
        run.agent_run_id = Some(agent);
        run.status = "integrating".into();
        run.save(&pool).await.unwrap();
        assert!(
            IntegrationRun::allows_instruction(&pool, workspace, session, agent)
                .await
                .unwrap()
        );
        assert!(
            !IntegrationRun::allows_instruction(&pool, workspace, Uuid::new_v4(), agent)
                .await
                .unwrap()
        );
        assert!(
            !IntegrationRun::allows_instruction(&pool, workspace, session, Uuid::new_v4())
                .await
                .unwrap()
        );
        for phase in ["validating", "publishing", "blocked", "succeeded"] {
            run.status = phase.into();
            run.save(&pool).await.unwrap();
            assert!(
                !IntegrationRun::allows_instruction(&pool, workspace, session, agent)
                    .await
                    .unwrap()
            );
        }
    }

    #[tokio::test]
    async fn reservations_survive_dispatcher_restart_and_fence_real_mutations() {
        let (pool, run, _) = fixture().await;
        let s = &run.payload.sources[0];
        crate::models::orchestration::OrchestrationLeaseRecord::reconcile_startup(&pool)
            .await
            .unwrap();
        assert_eq!(
            workspace_owner(&pool, s.selection.workspace_id)
                .await
                .unwrap(),
            Some(run.id)
        );
        assert!(
            sqlx::query("UPDATE local_issues SET title='changed' WHERE id=?")
                .bind(s.selection.card_id)
                .execute(&pool)
                .await
                .is_err()
        );
        assert!(
            sqlx::query("DELETE FROM local_workspace_links WHERE workspace_id=?")
                .bind(s.selection.workspace_id)
                .execute(&pool)
                .await
                .is_err()
        );
        assert!(
            sqlx::query("UPDATE workspaces SET worktree_deleted=1 WHERE id=?")
                .bind(s.selection.workspace_id)
                .execute(&pool)
                .await
                .is_err()
        );
        assert!(
            Session::create(
                &pool,
                &CreateSession {
                    executor: None,
                    name: None
                },
                Uuid::new_v4(),
                s.selection.workspace_id
            )
            .await
            .is_err()
        );
        assert!(complete_cards(&pool, run.id).await.is_err());
        assert!(
            sqlx::query("INSERT INTO integration_reservations VALUES('workspace',?,?)")
                .bind(resource_key(s.selection.workspace_id))
                .bind(run.id)
                .execute(&pool)
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn monitor_does_not_reinterpret_or_rewrite_retired_job_payloads() {
        let (pool, run, _) = fixture().await;
        assert_eq!(IntegrationRun::active(&pool).await.unwrap().len(), 1);
        sqlx::query("UPDATE integration_runs SET payload=json_remove(payload,'$.validation_suggestions') WHERE id=?").bind(run.id).execute(&pool).await.unwrap();
        let before: String = sqlx::query_scalar("SELECT payload FROM integration_runs WHERE id=?")
            .bind(run.id)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert!(IntegrationRun::active(&pool).await.unwrap().is_empty());
        let after: String = sqlx::query_scalar("SELECT payload FROM integration_runs WHERE id=?")
            .bind(run.id)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(before, after);
    }

    #[tokio::test]
    async fn cancellation_fences_a_late_agent_insert_at_the_database_boundary() {
        let (pool, mut run, _) = fixture().await;
        run.workspace_id = Some(run.payload.sources[0].selection.workspace_id);
        run.session_id = sqlx::query_scalar("SELECT id FROM sessions WHERE workspace_id=?")
            .bind(run.workspace_id)
            .fetch_optional(&pool)
            .await
            .unwrap();
        run.status = "integrating".into();
        run.save(&pool).await.unwrap();
        guard_agent_dispatch(
            &pool,
            run.workspace_id.unwrap(),
            run.session_id.unwrap(),
            run.id,
        )
        .await
        .unwrap();
        // Cancellation wins after the optimistic guard, before the actual INSERT.
        IntegrationRun::request_cancel(&pool, run.id).await.unwrap();
        let result=sqlx::query("INSERT INTO agent_runs(id,session_id,workspace_id,request_id,idempotency_key,correlation_id,schema_version,payload_version,runtime_profile_id,provider_id,workspace_mode,workspace_path,request_envelope) VALUES(?,?,?,?,?,?,1,1,'CODEX','codex','isolated_worktree','/fixture','{}')")
            .bind(Uuid::new_v4()).bind(run.session_id).bind(run.workspace_id).bind(Uuid::new_v4()).bind("late-dispatch").bind(run.id).execute(&pool).await;
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("reserved by formal Integration")
        );
        assert_eq!(
            workspace_owner(&pool, run.workspace_id.unwrap())
                .await
                .unwrap(),
            Some(run.id)
        );
    }

    #[tokio::test]
    async fn reserved_sources_reject_goal_reactivation_and_script_admission_until_release() {
        let (pool, run, _) = fixture().await;
        let workspace = run.payload.sources[0].selection.workspace_id;
        let session: Uuid = sqlx::query_scalar("SELECT id FROM sessions WHERE workspace_id=?")
            .bind(workspace)
            .fetch_one(&pool)
            .await
            .unwrap();
        sqlx::query("DELETE FROM integration_reservations WHERE run_id=?")
            .bind(run.id)
            .execute(&pool)
            .await
            .unwrap();
        let agent = Uuid::new_v4();
        sqlx::query("INSERT INTO agent_runs(id,session_id,workspace_id,request_id,idempotency_key,correlation_id,schema_version,payload_version,runtime_profile_id,provider_id,workspace_mode,workspace_path,status,request_envelope) VALUES(?,?,?,?,?,?,1,1,'CODEX','codex','isolated_worktree','/fixture','succeeded','{}')")
            .bind(agent).bind(session).bind(workspace).bind(Uuid::new_v4())
            .bind(agent.to_string()).bind(Uuid::new_v4()).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO integration_reservations VALUES('workspace',?,?)")
            .bind(resource_key(workspace))
            .bind(run.id)
            .execute(&pool)
            .await
            .unwrap();
        for status in [
            "pending",
            "starting",
            "running",
            "awaiting_input",
            "awaiting_approval",
        ] {
            let result = sqlx::query("UPDATE agent_runs SET status=? WHERE id=?")
                .bind(status)
                .bind(agent)
                .execute(&pool)
                .await;
            assert!(
                result
                    .unwrap_err()
                    .to_string()
                    .contains("reserved by formal Integration")
            );
        }
        use executors::actions::{
            ExecutorAction, ExecutorActionType,
            script::{ScriptContext, ScriptRequest, ScriptRequestLanguage},
        };

        use crate::models::execution_process::{
            CreateExecutionProcess, ExecutionProcess, ExecutionProcessRunReason,
        };
        let request = CreateExecutionProcess {
            session_id: session,
            executor_action: ExecutorAction::new(
                ExecutorActionType::ScriptRequest(ScriptRequest {
                    script: "echo setup".into(),
                    language: ScriptRequestLanguage::Bash,
                    context: ScriptContext::SetupScript,
                    working_dir: None,
                }),
                None,
            ),
            run_reason: ExecutionProcessRunReason::SetupScript,
        };
        assert!(
            ExecutionProcess::create(&pool, &request, Uuid::new_v4(), &[])
                .await
                .unwrap_err()
                .to_string()
                .contains("reserved by formal Integration")
        );
        guard_workspace(&pool, workspace).await.unwrap_err();
        sqlx::query("DELETE FROM integration_reservations WHERE run_id=?")
            .bind(run.id)
            .execute(&pool)
            .await
            .unwrap();
        guard_workspace(&pool, workspace).await.unwrap();
        sqlx::query("UPDATE agent_runs SET status='pending' WHERE id=?")
            .bind(agent)
            .execute(&pool)
            .await
            .unwrap();
        ExecutionProcess::create(&pool, &request, Uuid::new_v4(), &[])
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn old_publication_never_completes_a_card_reserved_by_a_new_run() {
        let (pool, mut run, _) = fixture().await;
        run.payload.published = true;
        run.save(&pool).await.unwrap();
        let next = Uuid::new_v4();
        sqlx::query("INSERT INTO integration_runs(id,request_key,project_id,repository_id,storage_identity,target_ref,status,payload) SELECT ?,?,project_id,repository_id,storage_identity,target_ref,'queued',payload FROM integration_runs WHERE id=?")
            .bind(next).bind(next.to_string()).bind(run.id).execute(&pool).await.unwrap();
        sqlx::query("UPDATE integration_reservations SET run_id=? WHERE resource_kind='card'")
            .bind(next)
            .execute(&pool)
            .await
            .unwrap();
        let complete = complete_cards(&pool, run.id).await.unwrap();
        assert!(
            complete.payload.sources[0]
                .done_result
                .as_ref()
                .unwrap()
                .contains("another Auto-merge")
        );
        let status: Uuid = sqlx::query_scalar("SELECT status_id FROM local_issues WHERE id=?")
            .bind(run.payload.sources[0].selection.card_id)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(status, run.payload.sources[0].status_id);
    }

    #[tokio::test]
    async fn done_receipt_is_atomic_idempotent_and_never_undoes_reopening() {
        let (pool, mut run, done) = fixture().await;
        run.payload.published = true;
        run.save(&pool).await.unwrap();
        let completed = complete_cards(&pool, run.id).await.unwrap();
        let source = &completed.payload.sources[0];
        assert_eq!(source.done_result.as_deref(), Some("done"));
        let status: Uuid = sqlx::query_scalar("SELECT status_id FROM local_issues WHERE id=?")
            .bind(source.selection.card_id)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(status, done);
        sqlx::query("UPDATE local_issues SET status_id=? WHERE id=?")
            .bind(source.status_id)
            .bind(source.selection.card_id)
            .execute(&pool)
            .await
            .unwrap();
        complete_cards(&pool, run.id).await.unwrap();
        let status: Uuid = sqlx::query_scalar("SELECT status_id FROM local_issues WHERE id=?")
            .bind(source.selection.card_id)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(status, source.status_id);
    }

    #[tokio::test]
    async fn changed_requirements_do_not_block_the_selected_cards_column_update() {
        let (pool, mut run, _) = fixture().await;
        run.payload.published = true;
        run.save(&pool).await.unwrap();
        sqlx::query("DELETE FROM integration_reservations WHERE resource_kind='card'")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("UPDATE local_issues SET description='new request' WHERE id=?")
            .bind(run.payload.sources[0].selection.card_id)
            .execute(&pool)
            .await
            .unwrap();
        let completed = complete_cards(&pool, run.id).await.unwrap();
        assert!(
            completed.payload.sources[0]
                .done_result
                .as_deref()
                .unwrap()
                .eq("done")
        );
        let fk: Vec<sqlx::sqlite::SqliteRow> = sqlx::query("PRAGMA foreign_key_check")
            .fetch_all(&pool)
            .await
            .unwrap();
        assert!(fk.is_empty());
    }

    #[tokio::test]
    async fn relinking_does_not_change_which_selected_card_moves_to_done() {
        for restore_original_link in [false, true] {
            let (pool, mut run, done) = fixture().await;
            run.payload.published = true;
            run.save(&pool).await.unwrap();
            let source = &run.payload.sources[0];
            sqlx::query("DELETE FROM integration_reservations WHERE run_id=?")
                .bind(run.id)
                .execute(&pool)
                .await
                .unwrap();
            let other_card = Uuid::new_v4();
            sqlx::query("INSERT INTO local_issues(id,project_id,issue_number,simple_id,status_id,title,sort_order) VALUES(?,?,2,'TEST-2',?,'new task',1)")
                .bind(other_card).bind(run.project_id).bind(source.status_id)
                .execute(&pool).await.unwrap();
            sqlx::query("UPDATE local_workspace_links SET issue_id=? WHERE workspace_id=?")
                .bind(other_card)
                .bind(source.selection.workspace_id)
                .execute(&pool)
                .await
                .unwrap();
            if restore_original_link {
                sqlx::query("UPDATE local_workspace_links SET issue_id=? WHERE workspace_id=?")
                    .bind(source.selection.card_id)
                    .bind(source.selection.workspace_id)
                    .execute(&pool)
                    .await
                    .unwrap();
            }
            let completed = complete_cards(&pool, run.id).await.unwrap();
            assert!(
                completed.payload.sources[0]
                    .done_result
                    .as_deref()
                    .unwrap()
                    .eq("done")
            );
            for card in [source.selection.card_id, other_card] {
                let status: Uuid =
                    sqlx::query_scalar("SELECT status_id FROM local_issues WHERE id=?")
                        .bind(card)
                        .fetch_one(&pool)
                        .await
                        .unwrap();
                assert_eq!(
                    status,
                    if card == source.selection.card_id {
                        done
                    } else {
                        source.status_id
                    }
                );
            }
            assert!(
                IntegrationRun::find(&pool, run.id)
                    .await
                    .unwrap()
                    .payload
                    .published
            );
        }
    }

    #[tokio::test]
    async fn cancel_and_publication_have_one_durable_winner() {
        let (pool, mut run, _) = fixture().await;
        run.status = "validating".into();
        run.save(&pool).await.unwrap();
        assert!(
            IntegrationRun::request_cancel(&pool, run.id)
                .await
                .unwrap()
                .cancel_requested
        );
        run.payload.publication_intent = true;
        let mut tx = pool.begin().await.unwrap();
        assert!(
            IntegrationRun::record_publication_intent(&mut tx, &run)
                .await
                .is_err()
        );
        tx.rollback().await.unwrap();
        assert!(
            !IntegrationRun::find(&pool, run.id)
                .await
                .unwrap()
                .payload
                .publication_intent
        );

        let (pool, mut run, _) = fixture().await;
        run.status = "validating".into();
        run.save(&pool).await.unwrap();
        run.payload.publication_intent = true;
        let mut tx = pool.begin().await.unwrap();
        IntegrationRun::record_publication_intent(&mut tx, &run)
            .await
            .unwrap();
        tx.commit().await.unwrap();
        let late = IntegrationRun::request_cancel(&pool, run.id).await.unwrap();
        assert!(!late.cancel_requested);
        assert_eq!(late.status, "publishing");
        assert!(late.payload.publication_intent);
    }

    #[tokio::test]
    async fn requirements_revisions_are_not_done_eligibility_checks() {
        let (pool, mut run, _) = fixture().await;
        run.payload.published = true;
        run.save(&pool).await.unwrap();
        sqlx::query("DELETE FROM integration_reservations WHERE resource_kind='card'")
            .execute(&pool)
            .await
            .unwrap();
        let card = run.payload.sources[0].selection.card_id;
        for title in ["new requirement", "request"] {
            sqlx::query("UPDATE local_issues SET title=? WHERE id=?")
                .bind(title)
                .bind(card)
                .execute(&pool)
                .await
                .unwrap();
        }
        let result = complete_cards(&pool, run.id).await.unwrap();
        assert!(
            result.payload.sources[0]
                .done_result
                .as_deref()
                .unwrap()
                .eq("done")
        );
    }

    #[tokio::test]
    async fn done_and_receipt_roll_back_together_on_database_failure() {
        let (pool, mut run, _) = fixture().await;
        run.payload.published = true;
        run.save(&pool).await.unwrap();
        sqlx::query("CREATE TRIGGER fail_integration_receipt BEFORE UPDATE OF payload ON integration_runs BEGIN SELECT RAISE(ABORT,'injected receipt failure'); END;").execute(&pool).await.unwrap();
        assert!(complete_cards(&pool, run.id).await.is_err());
        let status: Uuid = sqlx::query_scalar("SELECT status_id FROM local_issues WHERE id=?")
            .bind(run.payload.sources[0].selection.card_id)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(status, run.payload.sources[0].status_id);
        assert!(
            IntegrationRun::find(&pool, run.id)
                .await
                .unwrap()
                .payload
                .sources[0]
                .done_result
                .is_none()
        );
        sqlx::query("DROP TRIGGER fail_integration_receipt")
            .execute(&pool)
            .await
            .unwrap();
        assert_eq!(
            complete_cards(&pool, run.id).await.unwrap().payload.sources[0]
                .done_result
                .as_deref(),
            Some("done")
        );
    }

    #[tokio::test]
    async fn skipped_validation_survives_storage_and_is_visible_on_the_integration_card() {
        let (pool, mut run, _) = fixture().await;
        sqlx::query("DELETE FROM integration_reservations")
            .execute(&pool)
            .await
            .unwrap();
        run.id = Uuid::new_v4();
        run.request_key = run.id.to_string();
        run.workspace_id = Some(Uuid::new_v4());
        run.payload.card_id = Some(Uuid::new_v4());
        run.payload.skip_validation = true;
        run.create_with_card(&pool, "unverified", "Auto merge without validation")
            .await
            .unwrap();
        let saved = IntegrationRun::find(&pool, run.id).await.unwrap();
        assert!(saved.payload.skip_validation);
        assert!(saved.payload.validation_suggestions.is_empty());
        let description: String =
            sqlx::query_scalar("SELECT description FROM local_issues WHERE id=?")
                .bind(saved.payload.card_id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert!(description.contains("unverified"));
        assert!(description.contains(&run.payload.sources[0].commit));
        let mut resumed = saved;
        resumed.status = "integrating".into();
        resumed.save(&pool).await.unwrap();
        assert!(
            IntegrationRun::find(&pool, run.id)
                .await
                .unwrap()
                .payload
                .skip_validation
        );
    }

    #[tokio::test]
    async fn admission_keeps_existing_project_local_prefix() {
        let (pool, mut run, _) = fixture().await;
        // A deleted highest Issue must also stay retired for system cards.
        let deleted = Uuid::new_v4();
        sqlx::query("INSERT INTO local_issues(id,project_id,issue_number,simple_id,status_id,title,sort_order) SELECT ?,project_id,205,'LOCAL-205',status_id,'deleted highest',0 FROM local_issues LIMIT 1")
            .bind(deleted).execute(&pool).await.unwrap();
        sqlx::query("DELETE FROM local_issues WHERE id=?")
            .bind(deleted)
            .execute(&pool)
            .await
            .unwrap();
        run.id = Uuid::new_v4();
        run.request_key = run.id.to_string();
        run.workspace_id = Some(Uuid::new_v4());
        run.session_id = None;
        run.payload.card_id = Some(Uuid::new_v4());
        run.create_with_card(&pool, "integration", "Auto-merge")
            .await
            .unwrap();
        assert_eq!(
            sqlx::query_scalar::<_, String>("SELECT simple_id FROM local_issues WHERE id=?")
                .bind(run.payload.card_id)
                .fetch_one(&pool)
                .await
                .unwrap(),
            "LOCAL-206"
        );
    }

    #[tokio::test]
    async fn admission_creates_card_workspace_and_link_atomically_without_source_reservations() {
        let (pool, original, done) = fixture().await;
        sqlx::query("UPDATE projects SET key='LVK' WHERE id=?")
            .bind(original.project_id)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("DELETE FROM integration_reservations")
            .execute(&pool)
            .await
            .unwrap();
        let mut run = original.clone();
        run.id = Uuid::new_v4();
        run.request_key = run.id.to_string();
        run.workspace_id = Some(Uuid::new_v4());
        run.session_id = None;
        run.payload.card_id = Some(Uuid::new_v4());
        run.create_with_card(&pool, "integration", "Auto merge: selected → main")
            .await
            .unwrap();
        let linked: Uuid =
            sqlx::query_scalar("SELECT issue_id FROM local_workspace_links WHERE workspace_id=?")
                .bind(run.workspace_id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(Some(linked), run.payload.card_id);
        assert_eq!(
            sqlx::query_scalar::<_, String>("SELECT simple_id FROM local_issues WHERE id=?")
                .bind(linked)
                .fetch_one(&pool)
                .await
                .unwrap(),
            "LVK-2"
        );
        let labels: Vec<String> = sqlx::query_scalar("SELECT t.name FROM local_issue_tags it JOIN local_tags t ON t.id=it.tag_id WHERE it.issue_id=?")
            .bind(linked).fetch_all(&pool).await.unwrap();
        assert_eq!(labels, vec!["system"]);
        let priority: String = sqlx::query_scalar("SELECT priority FROM local_issues WHERE id=?")
            .bind(linked)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(priority, "medium");
        assert!(
            workspace_owner(&pool, original.payload.sources[0].selection.workspace_id)
                .await
                .unwrap()
                .is_none()
        );
        sqlx::query("UPDATE local_issues SET title='follow-up' WHERE id=?")
            .bind(original.payload.sources[0].selection.card_id)
            .execute(&pool)
            .await
            .unwrap();
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM local_issues")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert!(
            run.create_with_card(&pool, "integration", "duplicate")
                .await
                .is_err()
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM local_issues")
                .fetch_one(&pool)
                .await
                .unwrap(),
            count
        );
        run.payload.published = true;
        run.save(&pool).await.unwrap();
        let completed = complete_cards(&pool, run.id).await.unwrap();
        assert_eq!(completed.payload.card_done_result.as_deref(), Some("done"));
        for card in [linked, original.payload.sources[0].selection.card_id] {
            assert_eq!(
                sqlx::query_scalar::<_, Uuid>("SELECT status_id FROM local_issues WHERE id=?")
                    .bind(card)
                    .fetch_one(&pool)
                    .await
                    .unwrap(),
                done
            );
        }
        let mut failed = run.clone();
        failed.id = Uuid::new_v4();
        failed.request_key = failed.id.to_string();
        failed.payload.card_id = Some(Uuid::new_v4());
        failed.workspace_id = Some(Uuid::new_v4());
        sqlx::query("CREATE TRIGGER reject_integration_link BEFORE INSERT ON local_workspace_links BEGIN SELECT RAISE(ABORT,'injected link failure'); END").execute(&pool).await.unwrap();
        assert!(
            failed
                .create_with_card(&pool, "another", "rollback")
                .await
                .is_err()
        );
        assert!(
            !sqlx::query_scalar::<_, bool>("SELECT EXISTS(SELECT 1 FROM local_issues WHERE id=?)")
                .bind(failed.payload.card_id)
                .fetch_one(&pool)
                .await
                .unwrap()
        );
        assert!(
            !sqlx::query_scalar::<_, bool>("SELECT EXISTS(SELECT 1 FROM workspaces WHERE id=?)")
                .bind(failed.workspace_id)
                .fetch_one(&pool)
                .await
                .unwrap()
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT count(*) FROM local_issue_tags WHERE issue_id=?")
                .bind(failed.payload.card_id)
                .fetch_one(&pool)
                .await
                .unwrap(),
            0
        );
        let mut tx = pool.begin_with("BEGIN IMMEDIATE").await.unwrap();
        assert_eq!(
            crate::models::issue_number::next_issue_number(&mut tx, run.project_id)
                .await
                .unwrap(),
            (3, "LVK-3".into())
        );
        tx.rollback().await.unwrap();
    }
}

#[cfg(test)]
mod conversation_tests {
    use super::*;
    #[tokio::test]
    async fn conversation_owner_allows_same_session_repairs_and_fences_publication() {
        let (pool, mut run, _) = super::tests::fixture().await;
        let workspace = run.payload.sources[0].selection.workspace_id;
        let session: Uuid = sqlx::query_scalar("SELECT id FROM sessions WHERE workspace_id=?")
            .bind(workspace)
            .fetch_one(&pool)
            .await
            .unwrap();
        run.workspace_id = Some(workspace);
        run.session_id = Some(session);

        run.status = "integrating".into();
        run.save(&pool).await.unwrap();
        guard_agent_dispatch(&pool, workspace, session, run.id)
            .await
            .unwrap();
        assert!(
            guard_agent_dispatch(&pool, workspace, session, Uuid::new_v4())
                .await
                .is_err()
        );
        guard_workspace(&pool, workspace).await.unwrap();
        assert!(
            guard_agent_dispatch(&pool, workspace, Uuid::new_v4(), run.id)
                .await
                .is_err()
        );
        run.payload.publication_request = Some(IntegrationPublicationRequest {
            request_id: Uuid::new_v4(),
            agent_run_id: Uuid::new_v4(),
            code_commit: "a".repeat(40),
            wiki_commit: None,
            candidate_commit: "a".repeat(40),
        });
        run.save(&pool).await.unwrap();
        assert!(
            guard_agent_dispatch(&pool, workspace, session, run.id)
                .await
                .is_err()
        );
        assert!(guard_workspace(&pool, workspace).await.is_err());
        let event = serde_json::json!({"event":"publication_not_applied","request":run.payload.publication_request.clone()});
        run.payload.history.push(event);
        run.payload.publication_request = None;
        run.status = "blocked".into();
        run.save(&pool).await.unwrap();
        sqlx::query("DELETE FROM integration_reservations WHERE run_id=?")
            .bind(run.id)
            .execute(&pool)
            .await
            .unwrap();
        guard_agent_dispatch(&pool, workspace, session, run.id)
            .await
            .unwrap();
        assert_eq!(
            IntegrationRun::find(&pool, run.id)
                .await
                .unwrap()
                .payload
                .history
                .len(),
            1
        );
        run.cancel_requested = true;
        sqlx::query("UPDATE integration_runs SET cancel_requested=1 WHERE id=?")
            .bind(run.id)
            .execute(&pool)
            .await
            .unwrap();
        assert!(
            guard_agent_dispatch(&pool, workspace, session, run.id)
                .await
                .is_err()
        );
    }
}
