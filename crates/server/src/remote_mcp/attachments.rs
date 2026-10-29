use anyhow::ensure;
use serde_json::{Value, json};

use super::{Remote, auth::Principal, read::definition};
use crate::routes::attachment_transfer::{HostFile, Place, Upload};
pub(super) fn tools() -> Vec<rmcp::model::Tool> {
    let mut upload=serde_json::to_value(definition::<Upload>("upload_attachment","Save a Project attachment without starting a run. Supply a host file (preferred) or inline base64 (decoded <=2MiB); host bytes <=20MiB. Relative paths become readable only after verified placement. Workspace attachments are shared across Sessions.")).unwrap();
    upload["inputSchema"]["properties"]["file"] =
        serde_json::to_value(schemars::schema_for!(HostFile)).unwrap();
    // Optional properties must be declared, but not nullable/required, for host file inputs.
    for key in ["mime_type", "file_name"] {
        upload["inputSchema"]["properties"]["file"]["properties"][key] = json!({"type":"string"});
    }
    let mut place=serde_json::to_value(definition::<Place>("place_attachments","Place saved Project attachments in a selected Session working directory, verify bytes, and associate with its Workspace. Starts no run. Partial failure returns per-file results; never interpret saved as readable.")).unwrap();
    for t in [&mut upload, &mut place] {
        t["annotations"] = json!({"readOnlyHint":false,"destructiveHint":false,"idempotentHint":true,"openWorldHint":true});
        t["_meta"] =
            json!({"securitySchemes":[{"type":"oauth2","scopes":["lvk:read","lvk:write"]}]});
    }
    upload["_meta"]["openai/fileParams"] = json!(["file"]);
    vec![
        serde_json::from_value(upload).unwrap(),
        serde_json::from_value(place).unwrap(),
    ]
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
    let project = if name == "upload_attachment" {
        serde_json::from_value::<Upload>(input.clone())?.project_id
    } else {
        let args: Place = serde_json::from_value(input.clone())?;
        remote
            .require_workspace(args.project_id, args.workspace_id)
            .await?;
        args.project_id
    };
    remote.require_project(project).await?;
    remote
        .launch
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Attachment service unavailable"))?
        .attachment_transfer(name, input, remote.principal_key(principal))
        .await
}
