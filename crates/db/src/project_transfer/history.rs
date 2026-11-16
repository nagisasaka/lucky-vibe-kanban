//! Display-only archive: never creates executable runtime identities or paths.
use executors::runtime::{
    AgentEventEnvelope, AgentEventPayload, AgentRunStatus, AgentRuntimeMessageRole,
    ProjectionStatus, RUN_STATE_REDUCER_VERSION, RUN_STATE_SCHEMA_VERSION, RunState,
};
use utils::repository_memory::{ChangeManifest, SemanticChanges};

use super::*;
pub(super) const OMISSIONS: &str = "Legacy logs, noncanonical/invalid events, thinking, tool I/O, provider metadata and ambiguous ownership are omitted. Only final assistant answers and canonical user message text are eligible; opt-out excludes all conversation text. Free text is preserved and may contain sensitive information.";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HistoryMessage {
    pub role: String,
    pub content: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HistoryManifest {
    pub id: Uuid,
    pub repo_id: Uuid,
    pub semantics: SemanticChanges,
    pub created_at: DateTime<Utc>,
    pub base_commit: String,
    pub source_commit: String,
    pub target_branch: Option<String>,
    pub changed_paths: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImportedHistory {
    pub issue_id: Uuid,
    pub workspace_id: Uuid,
    pub session_id: Uuid,
    pub run_id: Uuid,
    pub status: String,
    pub messages: Vec<HistoryMessage>,
    pub manifests: Vec<HistoryManifest>,
}
#[derive(FromRow)]
struct StateAnchor {
    state_json: String,
    turn_id: Uuid,
    attempt_id: Uuid,
    attempt_number: i64,
    state_schema_version: i64,
    reducer_version: i64,
    last_run_attempt_number: i64,
    last_event_sequence: i64,
    last_event_id: Option<Uuid>,
}
fn terminal(s: &str) -> bool {
    matches!(
        s,
        "succeeded" | "failed" | "cancelled" | "crashed" | "audit_failed"
    )
}

pub(super) async fn snapshot(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    project: Uuid,
    include: bool,
    repos: &mut Vec<Repository>,
) -> Result<Vec<ImportedHistory>> {
    let runs: Vec<(Uuid,Uuid,Uuid,Uuid,String)> = sqlx::query_as("SELECT r.id,r.workspace_id,r.session_id,l.issue_id,r.status FROM agent_runs r JOIN sessions s ON s.id=r.session_id AND s.workspace_id=r.workspace_id JOIN local_workspace_links l ON l.workspace_id=r.workspace_id JOIN local_issues i ON i.id=l.issue_id AND i.project_id=l.project_id WHERE l.project_id=? AND r.status IN ('succeeded','failed','cancelled','crashed','audit_failed') AND r.projection_status='current' AND EXISTS (SELECT 1 FROM agent_run_state st JOIN agent_run_attempts a ON a.id=st.last_run_attempt_id AND a.agent_run_id=r.id WHERE st.agent_run_id=r.id AND st.status=r.status AND st.projection_status='current' AND a.status=r.status AND a.attempt_number=(SELECT max(n.attempt_number) FROM agent_run_attempts n WHERE n.agent_run_id=r.id)) ORDER BY r.id").bind(project).fetch_all(&mut **tx).await?;
    let mut repo_ids: HashSet<_> = repos.iter().map(|r| r.id).collect();
    let mut result = Vec::new();
    for (run_id, workspace_id, session_id, issue_id, status) in runs {
        // The existing history reader returns typed state_json, not the ledger
        // columns alone. A stale/active/broken projection is never portable.
        let anchor=sqlx::query_as::<_,StateAnchor>("SELECT st.state_json,a.turn_id,a.id AS attempt_id,a.attempt_number,st.state_schema_version,st.reducer_version,st.last_run_attempt_number,st.last_event_sequence,st.last_event_id FROM agent_run_state st JOIN agent_run_attempts a ON a.id=st.last_run_attempt_id WHERE st.agent_run_id=?").bind(run_id).fetch_one(&mut **tx).await?;
        let Ok(state) = serde_json::from_str::<RunState>(&anchor.state_json) else {
            continue;
        };
        let ledger_status: AgentRunStatus =
            serde_json::from_value(serde_json::Value::String(status.clone()))?;
        if state.state_schema_version != RUN_STATE_SCHEMA_VERSION
            || state.reducer_version != RUN_STATE_REDUCER_VERSION
            || i64::from(state.state_schema_version) != anchor.state_schema_version
            || i64::from(state.reducer_version) != anchor.reducer_version
            || state.agent_run_id != run_id
            || state.session_id != session_id
            || state.turn_id != anchor.turn_id
            || !state.status.is_terminal()
            || state.status != ledger_status
            || state.projection_status != ProjectionStatus::Current
            || state.last_run_attempt_id != Some(anchor.attempt_id)
            || i64::from(state.last_run_attempt_number) != anchor.attempt_number
            || i64::from(state.last_run_attempt_number) != anchor.last_run_attempt_number
            || state.last_event_sequence != anchor.last_event_sequence as u64
            || state.last_event_id != anchor.last_event_id
        {
            continue;
        }
        let mut messages = Vec::new();
        if include {
            let events: Vec<(Uuid,Uuid,Uuid,i64,i64,String)> = sqlx::query_as("SELECT event_id,turn_id,run_attempt_id,run_attempt_number,sequence,event_envelope FROM agent_events WHERE agent_run_id=? AND session_id=? ORDER BY run_attempt_number,sequence").bind(run_id).bind(session_id).fetch_all(&mut **tx).await?;
            for (event_id, turn_id, attempt_id, attempt_number, sequence, raw) in events {
                let Ok(event) = serde_json::from_str::<AgentEventEnvelope>(&raw) else {
                    continue;
                };
                if event.agent_run_id != run_id
                    || event.session_id != session_id
                    || event.event_id != event_id
                    || event.turn_id != turn_id
                    || event.run_attempt_id != attempt_id
                    || i64::from(event.run_attempt_number) != attempt_number
                    || event.sequence != sequence as u64
                    || event.validate_for_projection().is_err()
                {
                    continue;
                }
                if let AgentEventPayload::Message {
                    message,
                    final_output,
                } = event.payload
                {
                    let role = match message.role {
                        AgentRuntimeMessageRole::User => "user",
                        AgentRuntimeMessageRole::Assistant if final_output => "assistant",
                        _ => continue,
                    };
                    messages.push(HistoryMessage {
                        role: role.into(),
                        content: message.content,
                    });
                }
            }
        }
        let rows: Vec<(Uuid,Uuid,String,String)> = sqlx::query_as("SELECT m.repository_id,m.workspace_id,m.payload,r.display_name FROM repository_change_manifests m JOIN workspace_repos w ON w.workspace_id=m.workspace_id AND w.repo_id=m.repository_id JOIN repos r ON r.id=m.repository_id WHERE m.id=? AND m.workspace_id=? ORDER BY m.repository_id").bind(run_id).bind(workspace_id).fetch_all(&mut **tx).await?;
        let mut manifests = Vec::new();
        for (repo, workspace, raw, display_name) in rows {
            let Ok(m) = serde_json::from_str::<ChangeManifest>(&raw) else {
                continue;
            };
            if m.validate().is_ok()
                && m.event_id == run_id
                && m.workspace_id == workspace
                && m.repository_id == repo
                && m.task_id.is_none_or(|id| id == issue_id)
            {
                if repo_ids.insert(repo) {
                    repos.push(Repository {
                        id: repo,
                        display_name,
                    });
                }
                manifests.push(HistoryManifest {
                    id: run_id,
                    repo_id: repo,
                    semantics: m.semantics,
                    created_at: m.created_at,
                    base_commit: m.base_commit,
                    source_commit: m.source_commit,
                    target_branch: m.target_branch,
                    changed_paths: m.changed_paths,
                });
            }
        }
        if include || !manifests.is_empty() {
            result.push(ImportedHistory {
                issue_id,
                workspace_id,
                session_id,
                run_id,
                status,
                messages,
                manifests,
            });
        }
    }
    // Re-export previously imported archives using the same allowlisted DTO.
    let archives: Vec<String> = sqlx::query_scalar(
        "SELECT payload FROM project_transfer_history WHERE project_id=? ORDER BY run_id",
    )
    .bind(project)
    .fetch_all(&mut **tx)
    .await?;
    for raw in archives {
        let mut h: ImportedHistory = serde_json::from_str(&raw)?;
        for manifest in &h.manifests {
            if repo_ids.insert(manifest.repo_id) {
                let repo =
                    sqlx::query_as::<_, Repository>("SELECT id,display_name FROM repos WHERE id=?")
                        .bind(manifest.repo_id)
                        .fetch_optional(&mut **tx)
                        .await?
                        .ok_or_else(|| invalid("Imported Manifest Repo is no longer registered"))?;
                repos.push(repo);
            }
        }
        if !include {
            h.messages.clear();
        }
        if include || !h.manifests.is_empty() {
            result.push(h);
        }
    }
    Ok(result)
}

pub(super) fn validate(
    history: &[ImportedHistory],
    include: bool,
    issues: &HashSet<Uuid>,
    record_ids: &HashSet<Uuid>,
    repos: &[Repository],
) -> Result<()> {
    let repos: HashSet<_> = repos.iter().map(|r| r.id).collect();
    let mut runs = HashSet::new();
    let mut workspaces = BTreeMap::new();
    let mut sessions = BTreeMap::new();
    for h in history {
        if !issues.contains(&h.issue_id)
            || [h.workspace_id, h.session_id, h.run_id]
                .iter()
                .any(Uuid::is_nil)
            || !runs.insert(h.run_id)
            || !terminal(&h.status)
            || (!include && !h.messages.is_empty())
            || h.messages
                .iter()
                .any(|m| !matches!(m.role.as_str(), "user" | "assistant"))
        {
            return Err(invalid("Invalid portable history closure"));
        }
        if workspaces
            .insert(h.workspace_id, h.issue_id)
            .is_some_and(|issue| issue != h.issue_id)
            || sessions
                .insert(h.session_id, h.workspace_id)
                .is_some_and(|workspace| workspace != h.workspace_id)
        {
            return Err(invalid("Ambiguous archive ownership"));
        }
        let mut manifests = HashSet::new();
        for m in &h.manifests {
            if m.id != h.run_id
                || !repos.contains(&m.repo_id)
                || !manifests.insert(m.repo_id)
                || m.semantics.validate().is_err()
                || (ChangeManifest {
                    version: 1,
                    event_id: m.id,
                    repository_id: m.repo_id,
                    workspace_id: h.workspace_id,
                    task_id: Some(h.issue_id),
                    created_at: m.created_at,
                    base_commit: m.base_commit.clone(),
                    source_commit: m.source_commit.clone(),
                    target_branch: m.target_branch.clone(),
                    changed_paths: m.changed_paths.clone(),
                    semantics: m.semantics.clone(),
                })
                .validate()
                .is_err()
            {
                return Err(invalid("Invalid portable Manifest closure"));
            }
        }
    }
    let groups = [
        record_ids.clone(),
        workspaces.keys().copied().collect(),
        sessions.keys().copied().collect(),
        runs,
    ];
    let mut seen = HashSet::new();
    for group in groups {
        for id in group {
            if !seen.insert(id) {
                return Err(invalid("Colliding portable history identity kinds"));
            }
        }
    }
    Ok(())
}

pub(super) async fn insert(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    project: Uuid,
    history: &[ImportedHistory],
    ids: &BTreeMap<Uuid, Uuid>,
    repos: &BTreeMap<Uuid, Uuid>,
) -> Result<()> {
    let mut archive_ids = BTreeMap::new();
    for h in history {
        for id in [h.workspace_id, h.session_id, h.run_id] {
            archive_ids.entry(id).or_insert_with(Uuid::new_v4);
        }
    }
    for source in history {
        let mut h = source.clone();
        h.issue_id = ids[&h.issue_id];
        h.workspace_id = archive_ids[&h.workspace_id];
        h.session_id = archive_ids[&h.session_id];
        h.run_id = archive_ids[&h.run_id];
        for m in &mut h.manifests {
            m.id = h.run_id;
            m.repo_id = repos[&m.repo_id];
        }
        sqlx::query("INSERT INTO project_transfer_history(project_id,issue_id,run_id,payload) VALUES(?,?,?,?)").bind(project).bind(h.issue_id).bind(h.run_id).bind(serde_json::to_string(&h)?).execute(&mut **tx).await?;
    }
    Ok(())
}

/// Read-only portable history. IDs do not identify executable runtime records.
pub async fn list_imported_history(
    pool: &SqlitePool,
    project: Uuid,
) -> Result<Vec<ImportedHistory>> {
    let rows: Vec<String> = sqlx::query_scalar(
        "SELECT payload FROM project_transfer_history WHERE project_id=? ORDER BY run_id",
    )
    .bind(project)
    .fetch_all(pool)
    .await?;
    rows.into_iter()
        .map(|raw| serde_json::from_str(&raw).map_err(TransferError::from))
        .collect()
}
