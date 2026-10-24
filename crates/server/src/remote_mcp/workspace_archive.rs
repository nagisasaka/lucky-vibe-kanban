//! Remote confirmation is bound to the authenticated connection and never
//! exposes a token accepted by the local UI/stdio routes. Deletion stays shared.
use anyhow::{Context, ensure};
use db::models::workspace::Workspace;
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};
use uuid::Uuid;

use super::{
    Remote,
    auth::Principal,
    read::{arguments, definition},
};
use crate::routes::workspaces::archive::{self, ConfirmationScope};

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct PrepareArgs {
    project_id: Uuid,
    workspace_id: Uuid,
}
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ConfirmArgs {
    project_id: Uuid,
    workspace_id: Uuid,
    confirmation_token: Uuid,
    /// Stable request identity. After a lost response use get_operation or
    /// repeat this exact request; never issue another deletion blindly.
    request_id: Uuid,
}

pub(super) fn tools() -> Vec<rmcp::model::Tool> {
    [
        definition::<PrepareArgs>("prepare_workspace_archive", "Prepare manual archive for one explicit Project/Workspace. No deletion or archived change. Show the returned target and exact message to the user once; only after confirmation call confirm_workspace_archive with this caller-bound expiring token. Cancel by not confirming."),
        definition::<ConfirmArgs>("confirm_workspace_archive", "After the user's single confirmation, archive and physically remove the prepared worktree while retaining history and Git branch. Busy targets are refused. Reuse the exact request_id after response loss; inspect get_operation. archived and worktree_deleted are separate outcomes; deletion failure still archives and requires manual cleanup."),
    ].into_iter().enumerate().map(|(index, tool)| {
        let mut tool = serde_json::to_value(tool).unwrap();
        tool["annotations"] = json!({"readOnlyHint":false,"destructiveHint":index==1,"idempotentHint":index==1,"openWorldHint":false});
        tool["_meta"] = json!({"securitySchemes":[{"type":"oauth2","scopes":["lvk:read","lvk:write"]}]});
        serde_json::from_value(tool).unwrap()
    }).collect()
}

pub(super) async fn call(
    remote: &Remote,
    principal: &Principal,
    name: &str,
    input: Value,
) -> anyhow::Result<Value> {
    ensure!(
        principal.allows("lvk:read") && principal.allows("lvk:write"),
        "Read and write permissions required"
    );
    let runtime = remote
        .runtime
        .clone()
        .context("Archive runtime unavailable")?;
    if name == "prepare_workspace_archive" {
        let args: PrepareArgs = arguments(input)?;
        remote
            .require_workspace(args.project_id, args.workspace_id)
            .await?;
        let _lifecycle = services::services::workspace_lifecycle::lock(args.workspace_id).await;
        ensure!(
            runtime.archive_scripts_settled(args.workspace_id).await?,
            "使用中のWorkspaceはarchiveできません。"
        );
        let workspace = Workspace::find_by_id(&remote.pool, args.workspace_id)
            .await?
            .context("Workspace missing")?;
        let prepared = archive::prepare_scoped(
            &remote.pool,
            &workspace,
            ConfirmationScope::Remote {
                principal: remote.principal_key(principal),
                project_id: args.project_id,
            },
        )
        .await?;
        Ok(
            json!({"project_id":args.project_id,"workspace_id":args.workspace_id,"branch":workspace.branch,
            "confirmation_token":prepared.confirmation_token,"expires_at":prepared.expires_at,"message":archive::CONFIRMATION}),
        )
    } else {
        ensure!(
            name == "confirm_workspace_archive",
            "Unknown archive operation"
        );
        let args: ConfirmArgs = arguments(input.clone())?;
        remote
            .require_workspace(args.project_id, args.workspace_id)
            .await?;
        let scope = ConfirmationScope::Remote {
            principal: remote.principal_key(principal),
            project_id: args.project_id,
        };
        let owned = remote.clone();
        let receipt = remote.operations().submit_and_wait(
            remote.principal_key(principal), args.request_id, args.project_id, name.into(), input,
            json!({"workspace_id":args.workspace_id}), move |_| async move {
                let _admission = services::services::integration_admission::MUTATIONS.lock().await;
                if owned.require_workspace(args.project_id, args.workspace_id).await.is_err() {
                    return ("failed", json!({"code":"target_unavailable"}));
                }
                match archive::archive_scoped(&owned.pool, args.workspace_id, args.confirmation_token, scope,
                    || async { runtime.archive_scripts_settled(args.workspace_id).await }).await {
                    Ok(result) => ("succeeded", json!({"workspace_id":args.workspace_id,"archived":result.workspace.archived,
                        "worktree_deleted":result.workspace.worktree_deleted,"deletion_error":result.deletion_error})),
                    Err(error) => ("failed", json!({"code":"archive_refused","message":super::errors::message(&error.into())})),
                }
            },
        ).await?;
        Ok(remote.receipt_value(receipt))
    }
}

#[cfg(test)]
mod tests;
