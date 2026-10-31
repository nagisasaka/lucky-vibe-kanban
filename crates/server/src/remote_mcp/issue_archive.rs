//! Authenticated adapter; confirmation and execution are shared with UI/stdio.
use anyhow::ensure;
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};
use utils::mcp_error::Category;
use uuid::Uuid;

use super::{
    Remote,
    auth::Principal,
    read::{arguments, definition},
};
use crate::routes::{
    local_remote::issue_archive::{self, Confirm, Prepare},
    workspaces::archive::ConfirmationScope,
};
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct PrepareArgs {
    project_id: Uuid,
    issue_id: Uuid,
    archive_workspaces: bool,
}
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ConfirmArgs {
    project_id: Uuid,
    issue_id: Uuid,
    confirmation_token: Uuid,
    request_id: Uuid,
}
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct UnarchiveArgs {
    project_id: Uuid,
    issue_id: Uuid,
    request_id: Uuid,
}
pub(super) fn tools() -> Vec<rmcp::model::Tool> {
    [definition::<PrepareArgs>("prepare_issue_archive","Prepare one confirmation bound to this Issue, checkbox value and exact Workspace ID/path/branch set. No mutation. Show the message and all targets once. archive_workspaces=true physically removes idle unarchived worktrees while retaining history/branches. Busy child refuses the whole operation; false leaves all Workspaces unchanged."),
    definition::<ConfirmArgs>("confirm_issue_archive","Only after human confirmation, archive the prepared Issue and optional Workspaces. Does not change status. Keep request_id stable after response loss and inspect get_operation; uncertain outcomes must never be blindly replayed."),
    definition::<UnarchiveArgs>("unarchive_issue","Restore only the Issue to active, preserving status. Does not restore Workspaces or regenerate worktrees. Reuse request_id for receipt recovery.")]
    .into_iter().enumerate().map(|(index,tool)| {let mut value=serde_json::to_value(tool).unwrap();value["annotations"]=json!({"readOnlyHint":false,"destructiveHint":index==1,"idempotentHint":index>0,"openWorldHint":false});value["_meta"]=json!({"securitySchemes":[{"type":"oauth2","scopes":["lvk:read","lvk:write"]}]});serde_json::from_value(value).unwrap()}).collect()
}
pub(super) async fn call(
    remote: &Remote,
    principal: &Principal,
    name: &str,
    input: Value,
) -> anyhow::Result<Value> {
    ensure!(
        principal.allows("lvk:read") && principal.allows("lvk:write"),
        super::errors::rejection_with_message(
            utils::mcp_error::Category::Permission,
            "Read and write permissions required"
        )
    );
    let scope = |project_id| ConfirmationScope::Remote {
        principal: remote.principal_key(principal),
        project_id,
    };
    if name == "prepare_issue_archive" {
        let args: PrepareArgs = arguments(input)?;
        remote.require_issue(args.project_id, args.issue_id).await?;
        let runtime = remote
            .runtime
            .clone()
            .ok_or_else(|| super::errors::rejection(Category::Unavailable))?;
        return Ok(issue_archive::prepare_scoped(
            &remote.pool,
            args.issue_id,
            Prepare {
                project_id: args.project_id,
                archive_workspaces: args.archive_workspaces,
            },
            scope(args.project_id),
            |id| {
                let runtime = runtime.clone();
                async move { runtime.archive_scripts_settled(id).await }
            },
        )
        .await?);
    }
    let (project, issue, request, token) = if name == "confirm_issue_archive" {
        let a: ConfirmArgs = arguments(input.clone())?;
        (
            a.project_id,
            a.issue_id,
            a.request_id,
            Some(a.confirmation_token),
        )
    } else {
        ensure!(
            name == "unarchive_issue",
            super::errors::rejection_with_message(
                utils::mcp_error::Category::Validation,
                "Unknown Issue archive operation"
            )
        );
        let a: UnarchiveArgs = arguments(input.clone())?;
        (a.project_id, a.issue_id, a.request_id, None)
    };
    remote.require_issue(project, issue).await?;
    let owned = remote.clone();
    let scope = scope(project);
    let receipt = remote
        .operations()
        .submit_and_wait(
            remote.principal_key(principal),
            request,
            project,
            name.into(),
            input,
            json!({"issue_id":issue}),
            move |_| async move {
                if owned.require_issue(project, issue).await.is_err() {
                    return ("failed", json!({"code":"target_unavailable"}));
                }
                if let Some(token) = token {
                    let Some(runtime) = owned.runtime.clone() else {
                        return ("failed", json!({"message":"Archive runtime unavailable"}));
                    };
                    issue_archive::outcome(
                        issue_archive::archive_scoped(
                            &owned.pool,
                            issue,
                            &Confirm {
                                project_id: project,
                                request_id: request,
                                confirmation_token: token,
                            },
                            scope,
                            |id| {
                                let runtime = runtime.clone();
                                async move { runtime.archive_scripts_settled(id).await }
                            },
                        )
                        .await,
                    )
                } else {
                    issue_archive::outcome(
                        issue_archive::unarchive(&owned.pool, project, issue).await,
                    )
                }
            },
        )
        .await?;
    Ok(remote.receipt_value(receipt))
}
