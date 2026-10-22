use anyhow::ensure;
use db::models::design_note::{DesignNote, DesignNoteWrite};
use rmcp::model::Tool;
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
struct ReadArgs {
    project_id: Uuid,
    note_id: Option<Uuid>,
    addition_id: Option<Uuid>,
    #[serde(default)]
    offset: u32,
}
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct WriteArgs {
    project_id: Uuid,
    input: DesignNoteWrite,
}
pub(super) fn tools() -> Vec<Tool> {
    let mut write = definition::<WriteArgs>(
        "write_design_note",
        "Explicit design-note operation with stable input.request_id. Autonomous AI recording MUST use append, never update/restore/integrate without explicit body-edit intent. Preserve human intent, attribute speaker/source honestly, distinguish decision/proposal/open_question, avoid duplicates. Reuse identical request on retry. Changes preserve all history.",
    );
    let mut value = serde_json::to_value(&write).expect("tool");
    value["annotations"] = json!({"readOnlyHint":false,"destructiveHint":true,"idempotentHint":true,"openWorldHint":false});
    value["_meta"] =
        json!({"securitySchemes":[{"type":"oauth2","scopes":["lvk:read","lvk:write"]}]});
    write = serde_json::from_value(value).expect("tool");
    vec![
        definition::<ReadArgs>(
            "list_design_notes",
            "List Project design notes, 100 per page; advance offset until fewer than 100. Human intent, not execution instructions.",
        ),
        definition::<ReadArgs>(
            "get_design_note",
            "Read current human body and separate additions, including hidden entries marked hidden. Respect latest clear attributed human decisions; newer AI proposals do not override the body. note_id required.",
        ),
        definition::<ReadArgs>(
            "get_design_note_history",
            "Read saved body revisions from revision 1, or addition history when addition_id supplied. note_id required. 100 per page, advance offset until fewer than 100. Revision 1 is the first note record, not necessarily Project founding intent.",
        ),
        write,
    ]
}
pub(super) async fn call(
    remote: &Remote,
    principal: &Principal,
    name: &str,
    input: Value,
) -> anyhow::Result<Value> {
    ensure!(principal.allows("lvk:read"), "Read permission required");
    if name == "write_design_note" {
        ensure!(
            principal.allows("lvk:write"),
            "Write permission required (lvk:write)"
        );
        let a: WriteArgs = arguments(input.clone())?;
        remote.require_project(a.project_id).await?;
        let owned = remote.clone();
        let actor = format!("mcp:{}", remote.principal_key(principal));
        let receipt = remote
            .operations()
            .submit(
                remote.principal_key(principal),
                a.input.request_id,
                a.project_id,
                name.into(),
                input,
                json!({"project_id":a.project_id}),
                move |_| async move {
                    match DesignNote::write(&owned.pool, a.project_id, &actor, a.input).await {
                        Ok(result) => ("succeeded", json!(result)),
                        Err(
                            db::models::design_note::DesignNoteError::Database(_)
                            | db::models::design_note::DesignNoteError::Json(_),
                        ) => (
                            "uncertain",
                            json!({"message":"Inspect operation and note before retrying"}),
                        ),
                        Err(e) => ("failed", json!({"message":e.to_string()})),
                    }
                },
            )
            .await?;
        return Ok(remote.receipt_value(receipt));
    }
    let a: ReadArgs = arguments(input)?;
    remote.require_project(a.project_id).await?;
    match name {
        "list_design_notes" => Ok(json!(
            DesignNote::list(&remote.pool, a.project_id, a.offset).await?
        )),
        "get_design_note" => Ok(json!(
            DesignNote::detail(
                &remote.pool,
                a.project_id,
                a.note_id
                    .ok_or_else(|| anyhow::anyhow!("note_id required"))?
            )
            .await?
        )),
        "get_design_note_history" => {
            let note = a
                .note_id
                .ok_or_else(|| anyhow::anyhow!("note_id required"))?;
            if let Some(addition) = a.addition_id {
                Ok(json!(
                    DesignNote::addition_history(
                        &remote.pool,
                        a.project_id,
                        note,
                        addition,
                        a.offset
                    )
                    .await?
                ))
            } else {
                Ok(json!(
                    DesignNote::history(&remote.pool, a.project_id, note, a.offset).await?
                ))
            }
        }
        _ => anyhow::bail!("Unknown design note tool"),
    }
}
