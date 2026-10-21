use chrono::{DateTime, Utc};
use executors::runtime::{AgentEventEnvelope, AgentEventPayload, AgentRunStatus};
use uuid::Uuid;

/// Apply board effects alongside canonical event persistence, never on process exit
/// or projection rebuild. Status writes invalidate ownership, so replay and manual
/// edits cannot complete/reopen a card on behalf of an earlier run.
pub(super) async fn sync_kanban_lifecycle(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    event: &AgentEventEnvelope,
    previous: &(AgentRunStatus, Option<Uuid>),
) -> Result<(), sqlx::Error> {
    let AgentEventPayload::LifecycleChanged { status } = event.payload else {
        return Ok(());
    };
    let starting = status == AgentRunStatus::Running
        && (matches!(
            previous.0,
            AgentRunStatus::Pending | AgentRunStatus::Starting
        ) || previous.1 != Some(event.run_attempt_id));
    let terminal = status.is_terminal() && !previous.0.is_terminal();
    if !starting && !terminal {
        return Ok(());
    }
    let issue: Option<(Uuid, Uuid)> = sqlx::query_as(
        r#"
        SELECT i.id, i.project_id FROM local_issues i
        JOIN local_workspace_links l ON l.issue_id = i.id
        JOIN agent_runs r ON r.workspace_id = l.workspace_id
        JOIN agent_run_attempts a ON a.agent_run_id = r.id
        WHERE a.id = ?
        "#,
    )
    .bind(event.run_attempt_id)
    .fetch_optional(&mut **tx)
    .await?;
    let Some((issue_id, project_id)) = issue else {
        return Ok(());
    };
    // The process host timestamps Started after successful provider spawn. The
    // registry retains that timestamp across completion/replay; NULL means no
    // start evidence. Neither admission nor notification arrival orders work.
    // Compare parsed timestamps without SQLite's millisecond rounding. UUID is
    // only a stable tie-break, not evidence of physical order at equal times.
    let starts: Vec<(Uuid, DateTime<Utc>)> = sqlx::query_as(
        r#"
        SELECT a.id, p.process_started_at FROM agent_run_attempts a
        JOIN agent_process_registry p ON p.run_attempt_id = a.id
        JOIN agent_runs r ON r.id = a.agent_run_id
        JOIN local_workspace_links l ON l.workspace_id = r.workspace_id
        WHERE l.issue_id = ? AND p.process_started_at IS NOT NULL
    "#,
    )
    .bind(issue_id)
    .fetch_all(&mut **tx)
    .await?;
    let Some((latest_attempt, _)) = starts.into_iter().max_by_key(|(id, at)| (*at, *id)) else {
        return Ok(());
    };
    if starting && latest_attempt != event.run_attempt_id {
        return Ok(());
    }
    let target = if starting { "In Progress" } else { "In Review" };
    let target_id: Option<Uuid> = sqlx::query_scalar(
        "SELECT id FROM local_project_statuses WHERE project_id = ? AND name = ? AND hidden = 0 ORDER BY sort_order LIMIT 1",
    ).bind(project_id).bind(target).fetch_optional(&mut **tx).await?;
    let Some(target_id) = target_id else {
        return Ok(());
    };
    let changed = if starting {
        sqlx::query(
            r#"
            UPDATE local_issues SET status_id = ?, updated_at = strftime('%Y-%m-%d %H:%M:%f', MAX(julianday('now'), COALESCE(julianday(updated_at), 0) + 0.001 / 86400.0))
            WHERE id = ? AND status_id IN (
                SELECT id FROM local_project_statuses WHERE project_id = ?
                AND name IN ('Not Started', 'In Progress', 'In Review') AND hidden = 0
            )
        "#,
        )
        .bind(target_id)
        .bind(issue_id)
        .bind(project_id)
        .execute(&mut **tx)
        .await?
    } else {
        sqlx::query(r#"
            UPDATE local_issues SET status_id = ?, updated_at = strftime('%Y-%m-%d %H:%M:%f', MAX(julianday('now'), COALESCE(julianday(updated_at), 0) + 0.001 / 86400.0))
            WHERE id = ? AND agent_status_owner IN (
                SELECT a.id FROM agent_run_attempts a JOIN agent_runs r ON r.id = a.agent_run_id
                JOIN local_workspace_links l ON l.workspace_id = r.workspace_id
                WHERE l.issue_id = local_issues.id
                AND ((a.id = ? AND ? = 'succeeded') OR (a.id != ? AND a.status = 'succeeded' AND a.projection_status = 'current'))
                AND a.id = ?
            )
            AND status_id IN (SELECT id FROM local_project_statuses WHERE project_id = ? AND name = 'In Progress' AND hidden = 0)
            AND NOT EXISTS (
                SELECT 1 FROM agent_runs r JOIN local_workspace_links l ON l.workspace_id = r.workspace_id
                WHERE l.issue_id = local_issues.id AND r.id != ?
                AND r.status IN ('pending','starting','running','awaiting_input','awaiting_approval','cancelling')
            )
        "#).bind(target_id).bind(issue_id).bind(event.run_attempt_id).bind(status).bind(event.run_attempt_id).bind(latest_attempt).bind(project_id)
            .bind(event.agent_run_id).execute(&mut **tx).await?
    };
    if starting && changed.rows_affected() == 1 {
        sqlx::query("UPDATE local_issues SET agent_status_owner = ? WHERE id = ?")
            .bind(event.run_attempt_id)
            .bind(issue_id)
            .execute(&mut **tx)
            .await?;
    }
    Ok(())
}
