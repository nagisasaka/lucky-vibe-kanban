//! Formal Integration product state. Durable reservations are independent of
//! expiring Orchestration dispatcher leases. No conversation/semantic database.
use anyhow::{Context, ensure};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqlitePool};
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
pub struct IntegrationValidation {
    pub command: String,
    // Relative to the repository root, not the multi-repository container.
    pub cwd: String,
    pub required: bool,
    pub evidence: String,
    pub environment_requirements: String,
    pub execution_process_id: Option<Uuid>,
    pub exit_code: Option<i64>,
    pub result: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct IntegrationStop {
    pub stage: String,
    pub category: String,
    pub next_action: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct IntegrationIntervention {
    pub request_id: Uuid,
    pub prompt: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub pending: bool,
    pub retry_command: Option<serde_json::Value>,
    pub instruction_command: Option<serde_json::Value>,
}

/// A host-created, candidate-bound question. Old rows have no approvals.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct IntegrationGate {
    pub id: Uuid,
    pub validation_index: usize,
    pub scope: String,
    pub risk: String,
    pub evidence: IntegrationValidation,
    pub decision: Option<String>,
    pub actor: Option<Uuid>,
    pub decided_at: Option<String>,
    pub request_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct IntegrationStep {
    #[serde(default)]
    pub validation_skipped: bool,
    pub source_commit: String,
    pub result_commit: String,
    pub validation: Vec<IntegrationValidation>,
    pub gates: Vec<IntegrationGate>,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct IntegrationReview {
    pub id: Uuid,
    pub scope: String,
    pub step_index: usize,
    pub candidate: String,
    pub diff: String,
    pub explanation: String,
    pub decision: Option<String>,
    pub request_id: Option<Uuid>,
    pub actor: Option<Uuid>,
    pub decided_at: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, TS)]
pub struct IntegrationPayload {
    /// Host-owned merge engine. Legacy executions remain inspectable, never auto-replayed.
    #[serde(default)]
    pub mechanical: bool,
    /// Immutable Create envelope for the current conflict job; no Retry/Steer hand-off.
    #[serde(default)]
    pub conflict_dispatch: Option<serde_json::Value>,
    /// Frozen at admission; omitted in old records means validation is required.
    #[serde(default)]
    pub skip_validation: bool,
    #[serde(default)]
    pub card_id: Option<Uuid>,
    #[serde(default)]
    pub card_done_result: Option<String>,
    #[serde(default)]
    pub sequential: bool,
    #[serde(default)]
    pub step_index: usize,
    #[serde(default)]
    pub step_base: Option<String>,
    #[serde(default)]
    pub merge_tree: Option<String>,
    #[serde(default)]
    pub merge_conflicted: bool,
    #[serde(default)]
    pub next_step: bool,
    #[serde(default)]
    pub reviews: Vec<IntegrationReview>,
    #[serde(default)]
    pub steps: Vec<IntegrationStep>,
    #[serde(default)]
    pub gates: Vec<IntegrationGate>,
    #[serde(default)]
    pub resume_validation: bool,
    #[serde(default)]
    pub intervention: Option<IntegrationIntervention>,
    #[serde(default)]
    pub stop: Option<IntegrationStop>,
    #[serde(default)]
    pub history: Vec<serde_json::Value>,
    pub sources: Vec<IntegrationSource>,
    pub observed_target: String,
    pub base_commit: Option<String>,
    pub result_commit: Option<String>,
    pub validation: Vec<IntegrationValidation>,
    pub exclusions: Vec<String>,
    pub semantic_summary: Option<String>,
    pub semantics: Option<utils::repository_memory::SemanticChanges>,
    pub integration_manifest: Option<utils::repository_memory::ChangeManifest>,
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

impl IntegrationRun {
    pub fn validation_scope(&self) -> String {
        let mut scope = serde_json::json!({
            "integration": self.id, "project": self.project_id,
            "repository": self.repository_id, "target": self.target_ref,
            "base": self.payload.base_commit, "candidate": self.payload.result_commit,
            "exclusions": self.payload.exclusions,
            "sources": self.payload.sources.iter().map(|s| serde_json::json!({
                "selection": s.selection, "commit": s.commit, "branch": s.branch,
                "title": s.title, "description": s.description, "revision": s.requirements_revision,
                "status": s.status_id, "workspaces": s.related_workspaces, "events": s.event_ids
            })).collect::<Vec<_>>(),
            "plan": self.payload.validation.iter().map(|v| serde_json::json!({
                "command": v.command, "cwd": v.cwd, "required": v.required,
                "evidence": v.evidence, "environment": v.environment_requirements
            })).collect::<Vec<_>>()
        });
        if self.payload.skip_validation {
            scope["skip_validation"] = serde_json::json!(true);
        }
        scope.to_string()
    }

    pub fn validation_excepted(&self, index: usize) -> bool {
        self.payload.gates.iter().any(|gate| {
            gate.validation_index == index
                && gate.decision.as_deref() == Some("continue")
                && gate.actor.is_some()
                && gate.decided_at.is_some()
                && gate.request_id.is_some()
                && gate.scope == self.validation_scope()
                && serde_json::to_value(&gate.evidence).ok()
                    == serde_json::to_value(&self.payload.validation[index]).ok()
        })
    }
}

pub fn resource_key(id: Uuid) -> String {
    id.simple().to_string()
}

pub async fn workspace_owner(pool: &SqlitePool, id: Uuid) -> Result<Option<Uuid>, sqlx::Error> {
    sqlx::query_scalar("SELECT run_id FROM integration_reservations WHERE resource_kind='workspace' AND resource_key=?")
        .bind(resource_key(id)).fetch_optional(pool).await
}

pub async fn guard_workspace(pool: &SqlitePool, id: Uuid) -> Result<(), sqlx::Error> {
    if let Some(run) = workspace_owner(pool, id).await? {
        return Err(sqlx::Error::Protocol(format!(
            "Workspace reserved by Integration {run}; inspect /api/integrations/{run}"
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
    let product: Option<(Uuid, Option<Uuid>, String, bool)> = sqlx::query_as(
        "SELECT id,session_id,status,cancel_requested FROM integration_runs WHERE workspace_id=?",
    )
    .bind(workspace)
    .fetch_optional(pool)
    .await?;
    let owner = workspace_owner(pool, workspace).await?;
    if let Some((id, expected_session, status, cancelled)) = product {
        if owner != Some(id)
            || correlation != id
            || expected_session != Some(session)
            || cancelled
            || !matches!(status.as_str(), "preparing" | "integrating")
        {
            return Err(sqlx::Error::Protocol(format!(
                "Integration {id} does not permit new Agent dispatch in phase {status}; use Board progress/Cancel or a new explicit Integration"
            )));
        }
    } else if let Some(owner) = owner {
        return Err(sqlx::Error::Protocol(format!(
            "Source Workspace reserved by Integration {owner}"
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
        let card_id = self.payload.card_id.context("Missing Integration Card")?;
        let workspace_id = self.workspace_id.context("Missing Integration Workspace")?;
        let target = self
            .target_ref
            .strip_prefix("refs/heads/")
            .unwrap_or(&self.target_ref);
        let mut tx = pool.begin_with("BEGIN IMMEDIATE").await?;
        let status: Uuid = sqlx::query_scalar("SELECT id FROM local_project_statuses WHERE project_id=? AND hidden=0 ORDER BY (lower(name)='in progress') DESC,sort_order,id LIMIT 1")
        .bind(self.project_id).fetch_one(&mut *tx).await?;
        let number: i64 = sqlx::query_scalar(
            "SELECT COALESCE(MAX(issue_number),0)+1 FROM local_issues WHERE project_id=?",
        )
        .bind(self.project_id)
        .fetch_one(&mut *tx)
        .await?;
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
        .bind(card_id).bind(self.project_id).bind(number).bind(format!("LOCAL-{number}")).bind(status).bind(name).bind(description).bind(Uuid::from_u128(1)).execute(&mut *tx).await?;
        let owner = crate::models::workspace_usage::WorkspaceExecutionOwner::new(
            "integration",
            self.repository_id,
            Some(self.id),
        );
        sqlx::query("INSERT INTO workspaces(id,branch,name,usage,execution_owner) VALUES(?,?,?,'execution_only',?)")
        .bind(workspace_id).bind(branch).bind(name).bind(serde_json::to_string(&owner)?).execute(&mut *tx).await?;
        sqlx::query(
            "INSERT INTO workspace_repos(id,workspace_id,repo_id,target_branch) VALUES(?,?,?,?)",
        )
        .bind(Uuid::new_v4())
        .bind(workspace_id)
        .bind(self.repository_id)
        .bind(target)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "INSERT INTO local_workspace_links(workspace_id,project_id,issue_id) VALUES(?,?,?)",
        )
        .bind(workspace_id)
        .bind(self.project_id)
        .bind(card_id)
        .execute(&mut *tx)
        .await?;
        sqlx::query("INSERT INTO integration_runs(id,request_key,project_id,repository_id,storage_identity,target_ref,status,workspace_id,payload) VALUES(?,?,?,?,?,?,'queued',?,?)")
        .bind(self.id).bind(&self.request_key).bind(self.project_id).bind(self.repository_id).bind(&self.storage_identity).bind(&self.target_ref).bind(workspace_id).bind(&self.payload).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn decide_validation_gate(
        pool: &SqlitePool,
        id: Uuid,
        project_id: Uuid,
        gate_id: Uuid,
        request_id: Uuid,
        decision: &str,
        expected_scope: &str,
    ) -> anyhow::Result<()> {
        ensure!(
            matches!(decision, "continue" | "retry" | "cancel"),
            "Invalid decision"
        );
        let mut tx = pool.begin_with("BEGIN IMMEDIATE").await?;
        let mut current: IntegrationRun =
            sqlx::query_as("SELECT * FROM integration_runs WHERE id=?")
                .bind(id)
                .fetch_one(&mut *tx)
                .await?;
        ensure!(current.project_id == project_id, "Project mismatch");
        let index = current
            .payload
            .gates
            .iter()
            .position(|g| g.id == gate_id)
            .context("Confirmation disappeared")?;
        if current.payload.gates[index].request_id == Some(request_id) {
            ensure!(
                current.payload.gates[index].decision.as_deref() == Some(decision),
                "Idempotency conflict"
            );
            return Ok(());
        }
        ensure!(
            current.status == "blocked"
                && !current.cancel_requested
                && !current.payload.publication_intent,
            "Integration no longer awaits this decision"
        );
        ensure!(
            current.payload.gates[index].decision.is_none()
                && current.payload.gates[index].scope == expected_scope
                && current.validation_scope() == expected_scope,
            "Stale confirmation"
        );
        current.payload.stop = None;
        let evidence_index = current.payload.gates[index].validation_index;
        ensure!(
            current
                .payload
                .validation
                .get(evidence_index)
                .is_some_and(|v| serde_json::to_value(v).ok()
                    == serde_json::to_value(&current.payload.gates[index].evidence).ok()),
            "Validation evidence changed"
        );
        let recorded = &mut current.payload.gates[index];
        recorded.decision = Some(decision.to_owned());
        // Local Board mutations use the existing local operator identity, under
        // the same Origin/relay-signature boundary as other local owner actions.
        recorded.actor = Some(Uuid::from_u128(1));
        recorded.decided_at = Some(chrono::Utc::now().to_rfc3339());
        recorded.request_id = Some(request_id);
        if decision == "cancel" {
            current.status = "cancelling".into();
            current.cancel_requested = true;
        } else {
            if decision == "retry" {
                let item = &mut current.payload.validation[evidence_index];
                item.execution_process_id = None;
                item.exit_code = None;
                item.result = None;
            }
            current.status = "validating".into();
            current.payload.resume_validation = true;
        }
        sqlx::query("UPDATE integration_runs SET status=?,payload=?,cancel_requested=?,error=NULL,updated_at=datetime('now','subsec') WHERE id=?")
        .bind(&current.status).bind(&current.payload).bind(current.cancel_requested).bind(id).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn decide_review(
        pool: &SqlitePool,
        id: Uuid,
        project: Uuid,
        review_id: Uuid,
        request_id: Uuid,
        decision: &str,
        scope: &str,
    ) -> anyhow::Result<()> {
        ensure!(
            matches!(decision, "continue" | "cancel"),
            "Invalid review decision"
        );
        let mut tx = pool.begin_with("BEGIN IMMEDIATE").await?;
        let mut run: Self = sqlx::query_as("SELECT * FROM integration_runs WHERE id=?")
            .bind(id)
            .fetch_one(&mut *tx)
            .await?;
        ensure!(run.project_id == project, "Project mismatch");
        let index = run
            .payload
            .reviews
            .iter()
            .position(|r| r.id == review_id)
            .context("Unknown review")?;
        let review = &run.payload.reviews[index];
        if review.request_id == Some(request_id) {
            ensure!(
                review.decision.as_deref() == Some(decision),
                "Idempotency conflict"
            );
            return Ok(());
        }
        ensure!(
            run.status == "blocked" && !run.cancel_requested && !run.payload.publication_intent,
            "Review is no longer pending"
        );
        ensure!(
            review.decision.is_none() && review.scope == scope && run.validation_scope() == scope,
            "Stale resolution review"
        );
        let review = &mut run.payload.reviews[index];
        review.decision = Some(decision.into());
        review.request_id = Some(request_id);
        review.actor = Some(Uuid::from_u128(1));
        review.decided_at = Some(chrono::Utc::now().to_rfc3339());
        run.payload.stop = None;
        run.error = None;
        if decision == "cancel" {
            run.cancel_requested = true;
            run.status = "cancelling".into();
        } else {
            run.status = if run.payload.mechanical {
                "integrating"
            } else {
                "validating"
            }
            .into();
            run.payload.resume_validation = !run.payload.mechanical;
        }
        sqlx::query("UPDATE integration_runs SET status=?,payload=?,cancel_requested=?,error=NULL,updated_at=datetime('now','subsec') WHERE id=?")
            .bind(&run.status).bind(&run.payload).bind(run.cancel_requested).bind(id).execute(&mut *tx).await?;
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
    pub async fn fence_interrupted_preparations(pool: &SqlitePool) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE integration_runs SET status='cancelling',error='Preparation interrupted by service restart; confirm the accepted execution has stopped, inspect the retained Workspace, then resume this Integration',updated_at=datetime('now','subsec') WHERE status='preparing'")
            .execute(pool).await?;
        Ok(())
    }
    pub async fn request_cancel(pool: &SqlitePool, id: Uuid) -> Result<Self, sqlx::Error> {
        sqlx::query("UPDATE integration_runs SET cancel_requested=1,status=CASE WHEN status='blocked' THEN 'cancelling' ELSE status END,updated_at=datetime('now','subsec') WHERE id=? AND json_extract(payload,'$.publication_intent')=0 AND status IN ('queued','preparing','integrating','validating','cancelling','blocked')")
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
    pub async fn find(pool: &SqlitePool, id: Uuid) -> Result<Self, sqlx::Error> {
        sqlx::query_as("SELECT * FROM integration_runs WHERE id=?")
            .bind(id)
            .fetch_one(pool)
            .await
    }
    pub async fn save(&self, pool: &SqlitePool) -> Result<(), sqlx::Error> {
        // Cancellation is never overwritten by a worker's older observation.
        sqlx::query("UPDATE integration_runs SET status=?,workspace_id=?,session_id=?,agent_run_id=?,payload=?,error=?,updated_at=datetime('now','subsec') WHERE id=?")
            .bind(&self.status).bind(self.workspace_id).bind(self.session_id).bind(self.agent_run_id)
            .bind(&self.payload).bind(&self.error).bind(self.id).execute(pool).await?;
        Ok(())
    }
    /// Card effect and its durable receipt commit together. Reload under the
    /// SQLite writer lock, so concurrent recovery cannot re-Done a reopened Card.
    pub async fn complete_cards(pool: &SqlitePool, id: Uuid) -> Result<Self, sqlx::Error> {
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
                    Some("not changed: Card reserved by another Integration".into());
                continue;
            }
            sqlx::query("DELETE FROM integration_reservations WHERE run_id=? AND resource_kind='card' AND resource_key=?")
                .bind(id).bind(resource_key(source.selection.card_id)).execute(&mut *tx).await?;
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
            let count = if let Some(done) = done {
                sqlx::query("UPDATE local_issues SET status_id=?,completed_at=datetime('now','subsec'),updated_at=datetime('now','subsec') WHERE id=? AND project_id=?")
                    .bind(done).bind(card).bind(run.project_id).execute(&mut *tx).await?.rows_affected()
            } else {
                0
            };
            run.payload.card_done_result = Some(
                if count == 1 {
                    "done"
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

    #[tokio::test]
    async fn only_durable_integration_intervention_can_reopen_orchestration() {
        use executors::runtime::*;

        use crate::models::orchestration::{
            OrchestrationEventRecord, OrchestrationNodeExecutionRecord, OrchestrationRunRecord,
        };
        let (pool, mut run, _) = fixture().await;
        let plan = OrchestrationPlanSnapshot {
            schema_version: ORCHESTRATION_PLAN_SCHEMA_VERSION,
            plan_id: run.id,
            source_definition_id: run.id,
            source_definition_version: "test".into(),
            product_kind: OrchestrationProductKind::Integration,
            workspace_mode: WorkspaceMode::IsolatedWorktree,
            created_at: chrono::Utc::now(),
            nodes: vec![OrchestrationPlanNode {
                node_key: "integrate".into(),
                requires_product_validation: true,
                stable_order: 0,
                dependencies: vec![],
                join: OrchestrationJoinPolicy::All,
                failure_policy: OrchestrationFailurePolicy::FailFast,
                remaining_upstreams: RemainingUpstreamsPolicy::Continue,
                each_downstream_execution: EachDownstreamExecution::Serial,
                retry: OrchestrationRetryPolicy::default(),
                runtime_profile_id: None,
                provider_id: None,
                provider_config: None,
            }],
        };
        OrchestrationRunRecord::persist_before_dispatch(
            &pool, run.id, run.id, "recovery", run.id, &plan,
        )
        .await
        .unwrap();
        let node = Uuid::new_v4();
        OrchestrationNodeExecutionRecord::persist_identity_before_dispatch(
            &pool,
            node,
            run.id,
            "integrate",
            0,
            0,
        )
        .await
        .unwrap();
        let event = |sequence, payload| OrchestrationEventEnvelope {
            schema_version: ORCHESTRATION_EVENT_SCHEMA_VERSION,
            payload_version: ORCHESTRATION_EVENT_PAYLOAD_VERSION,
            event_id: Uuid::new_v4(),
            orchestration_run_id: run.id,
            sequence,
            correlation_id: run.id,
            timestamp: chrono::Utc::now(),
            payload,
        };
        OrchestrationEventRecord::append_and_project(
            &pool,
            &event(
                1,
                OrchestrationEventPayload::LifecycleChanged {
                    status: OrchestrationRunStatus::Failed,
                },
            ),
        )
        .await
        .unwrap();
        let resume = event(
            2,
            OrchestrationEventPayload::IntegrationResumed {
                node_execution_id: node,
            },
        );
        assert!(
            OrchestrationEventRecord::append_and_project(&pool, &resume)
                .await
                .is_err()
        );
        run.status = "integrating".into();
        run.payload.intervention = Some(IntegrationIntervention {
            request_id: Uuid::new_v4(),
            prompt: "repair environment".into(),
            created_at: chrono::Utc::now(),
            pending: true,
            retry_command: None,
            instruction_command: None,
        });
        run.save(&pool).await.unwrap();
        OrchestrationEventRecord::append_and_project(&pool, &resume)
            .await
            .unwrap();
        OrchestrationEventRecord::append_and_project(&pool, &resume)
            .await
            .unwrap();
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM orchestration_events WHERE orchestration_run_id=?",
        )
        .bind(run.id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(count, 2);
        let status: String = sqlx::query_scalar("SELECT status FROM orchestration_runs WHERE id=?")
            .bind(run.id)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(status, "running");
        OrchestrationEventRecord::append_and_project(
            &pool,
            &event(
                3,
                OrchestrationEventPayload::LifecycleChanged {
                    status: OrchestrationRunStatus::Failed,
                },
            ),
        )
        .await
        .unwrap();
        OrchestrationEventRecord::append_and_project(
            &pool,
            &event(
                4,
                OrchestrationEventPayload::LifecycleChanged {
                    status: OrchestrationRunStatus::Running,
                },
            ),
        )
        .await
        .unwrap();
        let status: String = sqlx::query_scalar("SELECT status FROM orchestration_runs WHERE id=?")
            .bind(run.id)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(
            status, "failed",
            "ordinary observations must not reopen terminal runs"
        );
        IntegrationRun::request_cancel(&pool, run.id).await.unwrap();
        assert!(
            OrchestrationEventRecord::append_and_project(
                &pool,
                &event(
                    5,
                    OrchestrationEventPayload::IntegrationResumed {
                        node_execution_id: node
                    }
                )
            )
            .await
            .is_err()
        );
    }

    #[tokio::test]
    async fn explicit_gate_decisions_are_atomic_scoped_audited_and_replayable() {
        let (pool, mut run, _) = fixture().await;
        run.status = "blocked".into();
        run.payload.result_commit = Some("candidate".into());
        run.payload.validation.push(IntegrationValidation {
            command: "npm test".into(),
            cwd: ".".into(),
            required: true,
            evidence: "selected acceptance".into(),
            environment_requirements: "node".into(),
            execution_process_id: Some(Uuid::new_v4()),
            exit_code: Some(1),
            result: Some("failed".into()),
        });
        let scope = run.validation_scope();
        let gate = Uuid::new_v4();
        let evidence = run.payload.validation[0].clone();
        run.payload.gates.push(IntegrationGate {
            id: gate,
            validation_index: 0,
            scope: scope.clone(),
            risk: "Regression risk".into(),
            evidence,
            decision: None,
            actor: None,
            decided_at: None,
            request_id: None,
        });
        run.save(&pool).await.unwrap();
        let request = Uuid::new_v4();
        assert!(
            IntegrationRun::decide_validation_gate(
                &pool,
                run.id,
                Uuid::new_v4(),
                gate,
                request,
                "continue",
                &scope
            )
            .await
            .is_err()
        );
        assert!(
            IntegrationRun::decide_validation_gate(
                &pool,
                run.id,
                run.project_id,
                gate,
                request,
                "approve everything",
                &scope
            )
            .await
            .is_err()
        );
        assert!(
            IntegrationRun::decide_validation_gate(
                &pool,
                run.id,
                run.project_id,
                gate,
                request,
                "continue",
                "old scope"
            )
            .await
            .is_err()
        );
        let (first, second) = tokio::join!(
            IntegrationRun::decide_validation_gate(
                &pool,
                run.id,
                run.project_id,
                gate,
                request,
                "continue",
                &scope
            ),
            IntegrationRun::decide_validation_gate(
                &pool,
                run.id,
                run.project_id,
                gate,
                Uuid::new_v4(),
                "cancel",
                &scope
            )
        );
        assert_ne!(first.is_ok(), second.is_ok(), "exactly one decision wins");
        let observed = IntegrationRun::find(&pool, run.id).await.unwrap();
        let decision = &observed.payload.gates[0];
        assert!(decision.actor.is_some() && decision.decided_at.is_some());
        assert_eq!(
            observed.payload.validation[0].result.as_deref(),
            Some("failed")
        );
        IntegrationRun::decide_validation_gate(
            &pool,
            run.id,
            run.project_id,
            gate,
            decision.request_id.unwrap(),
            decision.decision.as_deref().unwrap(),
            &scope,
        )
        .await
        .unwrap();
        assert_eq!(observed.payload.gates.len(), 1);
        assert!(!observed.payload.published);
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
        assert!(IntegrationRun::complete_cards(&pool, run.id).await.is_err());
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
    async fn cancellation_fences_a_late_agent_insert_at_the_database_boundary() {
        let (pool, mut run, _) = fixture().await;
        run.workspace_id = Some(run.payload.sources[0].selection.workspace_id);
        run.session_id = sqlx::query_scalar("SELECT id FROM sessions WHERE workspace_id=?")
            .bind(run.workspace_id)
            .fetch_optional(&pool)
            .await
            .unwrap();
        run.status = "preparing".into();
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
        let complete = IntegrationRun::complete_cards(&pool, run.id).await.unwrap();
        assert!(
            complete.payload.sources[0]
                .done_result
                .as_ref()
                .unwrap()
                .contains("another Integration")
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
        let completed = IntegrationRun::complete_cards(&pool, run.id).await.unwrap();
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
        IntegrationRun::complete_cards(&pool, run.id).await.unwrap();
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
        let completed = IntegrationRun::complete_cards(&pool, run.id).await.unwrap();
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
            let completed = IntegrationRun::complete_cards(&pool, run.id).await.unwrap();
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
        let result = IntegrationRun::complete_cards(&pool, run.id).await.unwrap();
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
        assert!(IntegrationRun::complete_cards(&pool, run.id).await.is_err());
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
            IntegrationRun::complete_cards(&pool, run.id)
                .await
                .unwrap()
                .payload
                .sources[0]
                .done_result
                .as_deref(),
            Some("done")
        );
    }

    #[tokio::test]
    async fn startup_fence_prevents_redelivery_and_retains_business_reservations() {
        let (pool, mut run, _) = fixture().await;
        let ws = run.payload.sources[0].selection.workspace_id;
        let session: Uuid = sqlx::query_scalar("SELECT id FROM sessions WHERE workspace_id=?")
            .bind(ws)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert!(
            guard_agent_dispatch(&pool, ws, session, Uuid::new_v4())
                .await
                .is_err()
        );
        run.workspace_id = Some(ws);
        run.session_id = Some(session);
        run.status = "preparing".into();
        run.save(&pool).await.unwrap();
        guard_agent_dispatch(&pool, ws, session, run.id)
            .await
            .unwrap();
        assert!(
            guard_agent_dispatch(&pool, ws, Uuid::new_v4(), run.id)
                .await
                .is_err()
        );
        IntegrationRun::fence_interrupted_preparations(&pool)
            .await
            .unwrap();
        assert!(
            guard_agent_dispatch(&pool, ws, session, run.id)
                .await
                .is_err()
        );
        assert_eq!(workspace_owner(&pool, ws).await.unwrap(), Some(run.id));
        let fenced = IntegrationRun::find(&pool, run.id).await.unwrap();
        assert_eq!(fenced.status, "cancelling");
        assert!(fenced.error.unwrap().contains("restart"));
        sqlx::query("DELETE FROM integration_reservations WHERE run_id=?")
            .bind(run.id)
            .execute(&pool)
            .await
            .unwrap();
        assert!(
            guard_agent_dispatch(&pool, ws, session, run.id)
                .await
                .is_err()
        );
    }
    #[tokio::test]
    async fn skipped_validation_survives_storage_and_is_visible_on_the_integration_card() {
        let (pool, mut run, _) = fixture().await;
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
        assert!(saved.payload.validation.is_empty());
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
    async fn admission_creates_card_workspace_and_link_atomically_without_source_reservations() {
        let (pool, original, done) = fixture().await;
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
        run.payload.sequential = true;
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
        let completed = IntegrationRun::complete_cards(&pool, run.id).await.unwrap();
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
    }

    #[tokio::test]
    async fn resolution_approval_is_durable_idempotent_and_candidate_bound() {
        let (pool, mut run, _) = fixture().await;
        run.status = "blocked".into();
        let review = Uuid::new_v4();
        let scope = run.validation_scope();
        run.payload.reviews.push(IntegrationReview {
            id: review,
            scope: scope.clone(),
            step_index: 0,
            candidate: "candidate".into(),
            diff: "+ preserves both features".into(),
            explanation: "resolution".into(),
            decision: None,
            request_id: None,
            actor: None,
            decided_at: None,
        });
        run.save(&pool).await.unwrap();
        let request = Uuid::new_v4();
        let mut changed = run.clone();
        changed.payload.result_commit = Some("different".into());
        changed.save(&pool).await.unwrap();
        assert!(
            IntegrationRun::decide_review(
                &pool,
                run.id,
                run.project_id,
                review,
                request,
                "continue",
                &scope
            )
            .await
            .is_err()
        );
        run.save(&pool).await.unwrap();
        IntegrationRun::decide_review(
            &pool,
            run.id,
            run.project_id,
            review,
            request,
            "continue",
            &scope,
        )
        .await
        .unwrap();
        IntegrationRun::decide_review(
            &pool,
            run.id,
            run.project_id,
            review,
            request,
            "continue",
            &scope,
        )
        .await
        .unwrap();
        let accepted = IntegrationRun::find(&pool, run.id).await.unwrap();
        assert_eq!(accepted.status, "validating");
        assert!(accepted.payload.resume_validation);
        assert_eq!(
            accepted.payload.reviews[0].decision.as_deref(),
            Some("continue")
        );
        assert!(
            IntegrationRun::decide_review(
                &pool,
                run.id,
                run.project_id,
                review,
                Uuid::new_v4(),
                "continue",
                &scope
            )
            .await
            .is_err()
        );
    }
}
