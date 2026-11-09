//! Shared host guards and instructions; the resource product owns scheduling.
use anyhow::{Context, ensure};
use db::models::resource_coordination::{ResourceHolder, ResourceOperation};
use executors::actions::{ExecutorAction, ExecutorActionType, script::ScriptContext};
use sqlx::SqlitePool;
use uuid::Uuid;

pub fn marker(id: Uuid) -> String {
    format!("# LVK resource operation {id}\n")
}

pub fn operation_id(action: &ExecutorAction) -> anyhow::Result<Uuid> {
    let ExecutorActionType::ScriptRequest(script) = action.typ();
    Ok(script
        .script
        .lines()
        .next()
        .and_then(|l| l.strip_prefix("# LVK resource operation "))
        .context("Missing resource operation identity")?
        .parse()?)
}

pub fn command(op: &ResourceOperation, holders: &[ResourceHolder]) -> anyhow::Result<String> {
    let fences = serde_json::to_string(holders)?;
    // User scripts run in separate subshells, so exit cannot skip verification.
    // This is an execution protocol, not a shell sandbox or external fencing.
    Ok(format!(
        "{}export LVK_RESOURCE_OPERATION_ID='{}'\nexport LVK_RESOURCE_FENCES='{}'\n(\nset -eu\n{}\n)\nlvk_command_status=$?\nif [ \"$lvk_command_status\" -ne 0 ]; then exit \"$lvk_command_status\"; fi\n(\nset -eu\n{}\n)\n",
        marker(op.id),
        op.id,
        fences.replace('\'', "'\\''"),
        op.spec.script,
        op.spec.verification_script
    ))
}

pub async fn guard_script(
    pool: &SqlitePool,
    workspace: Uuid,
    session: Uuid,
    action: &ExecutorAction,
) -> anyhow::Result<()> {
    let ExecutorActionType::ScriptRequest(script) = action.typ();
    ensure!(
        script.context == ScriptContext::ResourceCommand && action.next_action.is_none(),
        "Invalid managed resource action"
    );
    let id = operation_id(action)?;
    let op = ResourceOperation::find(pool, id).await?;
    ensure!(
        op.workspace_id == workspace
            && op.session_id == session
            && op.status == "launching"
            && !op.cancel_requested,
        "Resource operation no longer owns dispatch"
    );
    let holders: Vec<ResourceHolder> =
        sqlx::query_as("SELECT * FROM resource_holders WHERE operation_id=? ORDER BY resource_id")
            .bind(id)
            .fetch_all(pool)
            .await?;
    ensure!(
        holders.len() == op.spec.claims.len(),
        "Incomplete resource bundle"
    );
    let stale: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM resource_holders h JOIN shared_resources r ON r.id=h.resource_id WHERE h.operation_id=? AND (h.fence<>r.fence OR r.health<>'ready'))")
        .bind(id).fetch_one(pool).await?;
    ensure!(
        !stale
            && script.script == command(&op, &holders)?
            && script.working_dir.as_deref() == Some(op.spec.working_dir.as_str()),
        "Stale or altered resource action"
    );
    Ok(())
}

pub async fn guard_workspace_idle(pool: &SqlitePool, workspace: Uuid) -> anyhow::Result<()> {
    let active: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM resource_operations WHERE workspace_id=? AND status IN ('queued','blocked','launching','running','recovery_required'))")
        .bind(workspace).fetch_one(pool).await?;
    ensure!(
        !active,
        "Workspace has unresolved shared resource operations; finish, cancel or recover them first"
    );
    Ok(())
}

pub fn instructions(port: u16, session: Uuid) -> String {
    format!(
        "Shared resource authority: http://127.0.0.1:{port}/api/resource-coordination; current session_id: {session}. Before shared device/DB/deployment access, read the snapshot (MCP list_shared_resources or GET /snapshot) and the guide's operation contract. Use the canonical resource identity, one atomic bundle, cleanup and idle/state verification; never bypass owners. Retry only with the same request_id and identical contents. Finish or cancel managed operations before completion; failures may require operator recovery. MCP authenticates automatically; read the Shared resources section of the LVK on-demand guide file for raw HTTP/Runner details when needed."
    )
}
