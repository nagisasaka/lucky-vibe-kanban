//! The same fixed preset service as the Issue start dialog.
use serde_json::Value;
use uuid::Uuid;
pub use workflow::presets::{WorkflowAction as Action, WorkflowToolRequest as WorkflowArgs};

use super::{
    Remote,
    auth::Principal,
    read::{arguments, definition},
};
use crate::{DeploymentImpl, routes::workflow_presets};
pub(super) fn tools() -> Vec<rmcp::model::Tool> {
    let mut tools = vec![definition::<WorkflowArgs>(
        "workflow",
        "List/start fixed Workflow presets, retrieve their existing Run/Session results, stop, or edit Project additions. Uses the same parameters/service as UI. Start requires a stable request_id; replay unchanged after uncertainty. No arbitrary graph or automatic adoption/fix/merge/publication. Start acceptance is not completion. status reports retained successful results, failures and remaining nodes.",
    )];
    for tool in &mut tools {
        if let Some(a) = &mut tool.annotations {
            a.read_only_hint = Some(false);
        }
    }
    tools
}
pub(super) async fn call(
    remote: &Remote,
    principal: &Principal,
    args: Value,
) -> anyhow::Result<Value> {
    let a: WorkflowArgs = arguments(args)?;
    anyhow::ensure!(principal.allows("lvk:read"), "lvk:read required");
    remote.require_project(a.project_id).await?;
    if matches!(
        a.action,
        Action::Start | Action::Stop | Action::SaveDefaults
    ) {
        anyhow::ensure!(principal.scopes.contains("lvk:write"), "lvk:write required");
    }
    if let Some(start) = &a.start {
        anyhow::ensure!(start.project_id == a.project_id, "Project mismatch");
        remote.require_issue(a.project_id, start.issue_id).await?;
        if let Some(w) = start.workspace_id {
            anyhow::ensure!(
                remote.require_workspace(a.project_id, w).await? == start.issue_id,
                "Workspace belongs to another Issue"
            );
        }
    }
    if let Some(id) = a.run_id {
        let issue: Option<Uuid> =
            sqlx::query_scalar("SELECT issue_id FROM workflow_runs WHERE id=?")
                .bind(id)
                .fetch_one(&remote.pool)
                .await?;
        remote
            .require_issue(
                a.project_id,
                issue.ok_or_else(|| {
                    anyhow::anyhow!("Repository-owned workflows use their owner API")
                })?,
            )
            .await?;
    }
    if matches!(a.action, Action::List) {
        return Ok(workflow_presets::catalog());
    }
    remote
        .launch
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Workflow runtime unavailable"))?
        .workflow(a)
        .await
}
pub(super) async fn execute(d: &DeploymentImpl, a: WorkflowArgs) -> anyhow::Result<Value> {
    use deployment::Deployment;
    Ok(match a.action {
        Action::List => workflow_presets::catalog(),
        Action::Start => serde_json::to_value(
            workflow_presets::start(
                d,
                a.start
                    .ok_or_else(|| anyhow::anyhow!("start parameters required"))?,
            )
            .await?,
        )?,
        Action::Status => serde_json::to_value(
            workflow_presets::status(
                d,
                a.run_id.ok_or_else(|| anyhow::anyhow!("run_id required"))?,
            )
            .await?,
        )?,
        Action::Stop => serde_json::to_value(
            workflow_presets::stop(
                d,
                a.run_id.ok_or_else(|| anyhow::anyhow!("run_id required"))?,
            )
            .await?,
        )?,
        Action::Defaults => {
            serde_json::to_value(workflow_presets::defaults(&d.db().pool, a.project_id).await?)?
        }
        Action::SaveDefaults => {
            workflow_presets::save_defaults(
                &d.db().pool,
                a.project_id,
                a.defaults
                    .ok_or_else(|| anyhow::anyhow!("defaults required"))?,
            )
            .await?;
            serde_json::json!({"saved":true})
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn preset_mcp_authorizes_before_runtime_or_default_mutation() {
        let (remote, _, mut principal, target, _) =
            super::super::runtime_write_tests::fixture().await;
        principal.scopes = std::collections::BTreeSet::from(["lvk:read".to_owned()]);
        let list = call(
            &remote,
            &principal,
            serde_json::json!({"project_id":target.project_id,"action":"list"}),
        )
        .await
        .unwrap();
        assert_eq!(list["presets"].as_array().unwrap().len(), 3);
        assert!(
            call(
                &remote,
                &principal,
                serde_json::json!({"project_id":Uuid::new_v4(),"action":"list"})
            )
            .await
            .is_err()
        );
        assert!(call(&remote,&principal,serde_json::json!({"project_id":target.project_id,"action":"save_defaults","defaults":{"preset":"inventory_reduce","additions":"unapproved","revision":0}})).await.is_err());
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM workflow_preset_defaults")
            .fetch_one(&remote.pool)
            .await
            .unwrap();
        assert_eq!(count, 0);
    }
}
