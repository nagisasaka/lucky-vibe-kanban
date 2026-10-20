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
    // One issue can own many workspaces/sessions. Admission order, rather than
    // delayed event arrival or provider timestamps, fences older attempts.
    let issue: Option<(Uuid, Uuid)> = sqlx::query_as(
        r#"
        SELECT i.id, i.project_id FROM local_issues i
        JOIN local_workspace_links l ON l.issue_id = i.id
        JOIN agent_runs r ON r.workspace_id = l.workspace_id
        JOIN agent_run_attempts a ON a.agent_run_id = r.id
        WHERE a.id = ? AND (? = 0 OR NOT EXISTS (
            SELECT 1 FROM agent_run_attempts newer
            JOIN agent_runs nr ON nr.id = newer.agent_run_id
            JOIN local_workspace_links nl ON nl.workspace_id = nr.workspace_id
            WHERE nl.issue_id = i.id AND newer.rowid > a.rowid
        ))
    "#,
    )
    .bind(event.run_attempt_id)
    .bind(starting)
    .fetch_optional(&mut **tx)
    .await?;
    let Some((issue_id, project_id)) = issue else {
        return Ok(());
    };
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
                AND ((a.id = ? AND ? = 'succeeded') OR (r.id != ? AND r.status = 'succeeded' AND r.projection_status = 'current'))
                AND NOT EXISTS (
                    SELECT 1 FROM agent_run_attempts newer
                    JOIN agent_runs nr ON nr.id = newer.agent_run_id
                    JOIN local_workspace_links nl ON nl.workspace_id = nr.workspace_id
                    WHERE nl.issue_id = local_issues.id AND newer.rowid > a.rowid
                )
            )
            AND status_id IN (SELECT id FROM local_project_statuses WHERE project_id = ? AND name = 'In Progress' AND hidden = 0)
            AND NOT EXISTS (
                SELECT 1 FROM agent_runs r JOIN local_workspace_links l ON l.workspace_id = r.workspace_id
                WHERE l.issue_id = local_issues.id AND r.id != ?
                AND r.status IN ('pending','starting','running','awaiting_input','awaiting_approval','cancelling')
            )
        "#).bind(target_id).bind(issue_id).bind(event.run_attempt_id).bind(status).bind(event.agent_run_id).bind(project_id)
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
