use anyhow::ensure;
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};
use uuid::Uuid;

use super::{
    Remote,
    auth::Principal,
    read::{arguments, definition},
};

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Target {
    project_id: Uuid,
    workspace_id: Uuid,
}
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Change {
    project_id: Uuid,
    workspace_id: Uuid,
    request_id: Uuid,
}

pub(super) fn tools() -> Vec<rmcp::model::Tool> {
    let mut tools = vec![
        definition::<Target>(
            "get_dev_server_status",
            "Observe managed DevServer process records for an explicit Project/Workspace without starting or stopping. Running does not prove application readiness.",
        ),
        definition::<Change>(
            "start_dev_server",
            "Start the Workspace managed DevServer using the UI manager. Existing running processes are reused without restart. Returns an acceptance receipt; read get_operation then get_dev_server_status. Never use generic shell as an alternative.",
        ),
        definition::<Change>(
            "stop_dev_server",
            "Stop only this Workspace's managed DevServer processes. Already stopped is a no-op. Returns an acceptance receipt; inspect get_operation and get_dev_server_status.",
        ),
    ];
    for tool in &mut tools {
        let read = tool.name == "get_dev_server_status";
        let mut value = serde_json::to_value(&*tool).expect("tool");
        value["annotations"] = json!({"readOnlyHint":read,"destructiveHint":!read,"idempotentHint":true,"openWorldHint":false});
        value["_meta"] = json!({"securitySchemes":[{"type":"oauth2","scopes":if read {vec!["lvk:read","lvk:runtime:read"]} else {vec!["lvk:read","lvk:runtime:read","lvk:runtime:execute"]}}]});
        *tool = serde_json::from_value(value).expect("tool");
    }
    tools
}

pub(super) async fn call(
    remote: &Remote,
    principal: &Principal,
    name: &str,
    input: Value,
) -> anyhow::Result<Value> {
    ensure!(
        principal.allows("lvk:read") && principal.allows("lvk:runtime:read"),
        "Read and runtime read permissions required"
    );
    if name == "get_dev_server_status" {
        let t: Target = arguments(input)?;
        remote
            .require_workspace(t.project_id, t.workspace_id)
            .await?;
        let runtime = remote
            .runtime
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("DevServer runtime unavailable"))?;
        return Ok(runtime
            .dev_server_status(t.project_id, t.workspace_id)
            .await?);
    }
    ensure!(
        principal.allows("lvk:runtime:execute"),
        "Runtime execute permission required"
    );
    let t: Change = arguments(input.clone())?;
    remote
        .require_workspace(t.project_id, t.workspace_id)
        .await?;
    let runtime = remote
        .runtime
        .clone()
        .ok_or_else(|| anyhow::anyhow!("DevServer runtime unavailable"))?;
    let owned = remote.clone();
    let start = match name {
        "start_dev_server" => true,
        "stop_dev_server" => false,
        _ => anyhow::bail!("Unknown DevServer operation"),
    };
    let receipt = remote.operations().submit(remote.principal_key(principal),t.request_id,t.project_id,name.into(),input,json!({"workspace_id":t.workspace_id}),move |_| async move {
        if owned.require_workspace(t.project_id,t.workspace_id).await.is_err() {
            return ("failed",json!({"code":"workspace_scope_changed"}));
        }
        match runtime.dev_server(t.project_id,t.workspace_id,start).await {
            Ok(value) => (if value["accepted"] == false {"failed"} else {"succeeded"},value),
            Err(crate::error::ApiError::Conflict(message)) =>
                ("failed",json!({"code":"workspace_state_conflict","message":message})),
            Err(error) => {
                tracing::warn!(%error,"Managed DevServer outcome unconfirmed");
                ("uncertain",json!({"code":"dev_server_outcome_unconfirmed","message":"Managed execution may have changed before observation failed; inspect this receipt and current status before a new request"}))
            }
        }
    }).await?;
    Ok(remote.receipt_value(receipt))
}
