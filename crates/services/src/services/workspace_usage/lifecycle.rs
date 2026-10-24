//! Execution ownership is a temporary reservation, not a separate class of workspace.
use super::*;

/// Allow instructions only to the currently delegated, running Wiki agent.
/// Dispatch repeats the ownership check, so a stale UI cannot steer a later phase.
pub async fn allows_wiki_instruction(
    pool: &SqlitePool,
    workspace: &Workspace,
    request: &AgentRunRequestEnvelope,
) -> anyhow::Result<bool> {
    if !workspace.is_execution_only()
        || !workspace
            .execution_owner
            .as_ref()
            .is_some_and(|owner| matches!(owner.kind.as_str(), OPENWIKI_BOOTSTRAP | OPENWIKI_SYNC))
    {
        return Ok(false);
    }
    let Some(run) =
        db::models::agent_runtime::AgentRunRecord::find(pool, request.agent_run_id).await?
    else {
        return Ok(false);
    };
    if run.workspace_id != workspace.id
        || run.session_id != request.session_id
        || run.status.is_terminal()
        || run.status == executors::runtime::AgentRunStatus::Cancelling
    {
        return Ok(false);
    }
    Ok(validate_agent_owner(pool, workspace, request, true)
        .await
        .is_ok())
}

/// Create a normal Board card when repository membership identifies one Board.
/// Never choose arbitrarily between projects or replace a user's existing link.
/// Return false when no link can be established; new runs must reject that outcome.
pub async fn ensure_wiki_board_link(
    pool: &SqlitePool,
    workspace: &Workspace,
) -> anyhow::Result<bool> {
    let Some(owner) = workspace
        .execution_owner
        .as_ref()
        .filter(|owner| matches!(owner.kind.as_str(), OPENWIKI_BOOTSTRAP | OPENWIKI_SYNC))
    else {
        return Ok(false);
    };
    let Some(repo_id) = owner.repository_id else {
        return Ok(false);
    };
    let linked: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM local_workspace_links WHERE workspace_id=?)",
    )
    .bind(workspace.id)
    .fetch_one(pool)
    .await?;
    if linked {
        return Ok(true);
    }
    // Serialize the membership check and issue-number allocation without
    // generating a spurious Workspace update on every monitor tick.
    let mut tx = pool.begin_with("BEGIN IMMEDIATE").await?;
    let linked: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM local_workspace_links WHERE workspace_id=?)",
    )
    .bind(workspace.id)
    .fetch_one(&mut *tx)
    .await?;
    if linked {
        return Ok(true);
    }
    let projects: Vec<Uuid> = sqlx::query_scalar(
        "SELECT project_id FROM project_repos WHERE repo_id=?
         UNION SELECT i.project_id FROM workspace_repos wr
         JOIN local_workspace_links l ON l.workspace_id=wr.workspace_id
         JOIN local_issues i ON i.id=l.issue_id
         WHERE wr.repo_id=? ORDER BY project_id",
    )
    .bind(repo_id)
    .bind(repo_id)
    .fetch_all(&mut *tx)
    .await?;
    let [project_id] = projects.as_slice() else {
        return Ok(false);
    };
    let status: Option<Uuid> = sqlx::query_scalar("SELECT id FROM local_project_statuses WHERE project_id=? AND hidden=0 ORDER BY sort_order,id LIMIT 1")
        .bind(project_id).fetch_optional(&mut *tx).await?;
    let Some(status) = status else {
        return Ok(false);
    };
    let number: i64 = sqlx::query_scalar(
        "SELECT COALESCE(MAX(issue_number),0)+1 FROM local_issues WHERE project_id=?",
    )
    .bind(project_id)
    .fetch_one(&mut *tx)
    .await?;
    let issue_id = Uuid::new_v4();
    sqlx::query("INSERT INTO local_issues(id,project_id,issue_number,simple_id,status_id,title,description,sort_order) VALUES(?,?,?,?,?,?,?,0)")
        .bind(issue_id).bind(project_id).bind(number).bind(format!("LOCAL-{number}"))
        .bind(status).bind(workspace.name.as_deref().unwrap_or("OpenWiki"))
        .bind("OpenWiki repository maintenance. Execution history and operator instructions are available in the linked workspace.")
        .execute(&mut *tx).await?;
    sqlx::query(
        "INSERT INTO local_workspace_links(workspace_id,project_id,issue_id) VALUES(?,?,?)",
    )
    .bind(workspace.id)
    .bind(project_id)
    .bind(issue_id)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(true)
}

/// Release completed reservations only after all processes and product cleanup
/// have settled. Keep historical ownership/results for inspection and attribution.
pub async fn release_completed(pool: &SqlitePool) -> anyhow::Result<()> {
    // Compare the persisted JSON bytes, not a re-serialized struct: older
    // records may omit optional fields or use a different property order.
    let rows: Vec<(Uuid, String)> = sqlx::query_as(
        "SELECT id,execution_owner FROM workspaces WHERE usage='execution_only' AND execution_owner IS NOT NULL",
    ).fetch_all(pool).await?;
    for (id, expected_owner) in rows {
        let Some(workspace) = Workspace::find_by_id(pool, id).await? else {
            continue;
        };
        let idle = match cleanup_allowed(pool, id).await {
            Ok(idle) => idle,
            Err(error) => {
                tracing::warn!(workspace_id=%id, %error, "Workspace reservation retained");
                continue;
            }
        };
        if !idle {
            continue;
        }
        if let Some(owner) = &workspace.execution_owner
            && owner.kind == INTEGRATION
            && let Some(run_id) = owner.run_id
        {
            let run = db::models::integration::IntegrationRun::find(pool, run_id).await?;
            if !matches!(run.status.as_str(), "succeeded" | "failed" | "cancelled")
                || db::models::integration::workspace_owner(pool, id)
                    .await?
                    .is_some()
            {
                continue;
            }
        }
        sqlx::query("UPDATE workspaces SET usage='interactive',updated_at=datetime('now','subsec') WHERE id=? AND usage='execution_only' AND execution_owner=?")
            .bind(id).bind(expected_owner).execute(pool).await?;
    }
    Ok(())
}
