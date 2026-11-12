use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::Value;
use uuid::Uuid;

use super::{
    Remote,
    read::{arguments, definition},
};
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Conversation {
    session_id: Uuid,
}
pub(super) fn tools() -> Vec<rmcp::model::Tool> {
    let tools = vec![
        definition::<crate::routes::help::AskHelp>(
            "ask_help",
            "Ask the configured coding agent general LVK help using bundled source matching the running build. Starts an ordinary Review Session without a Project, or continues the supplied help session_id. Uses existing credentials/billing. Accepted run is not an answer: poll get_help_conversation. Does not implement or restart LVK. Not idempotent: do not automatically replay an unknown outcome.",
        ),
        definition::<Conversation>(
            "get_help_conversation",
            "Read help runs, progress, answers and failures. Inspect runs[].state.status, terminal_output and last_error; accepted/running is not success. Missing source is an explicit error.",
        ),
        definition::<Conversation>(
            "cancel_help_conversation",
            "Cancel a help conversation's latest run through ordinary agent control.",
        ),
        definition::<Conversation>(
            "delete_help_conversation",
            "Delete a stopped help conversation using ordinary Workspace deletion. Running conversations must be cancelled first.",
        ),
    ];
    tools
        .into_iter()
        .map(|tool| {
            if tool.name == "get_help_conversation" {
                return tool;
            }
            let mut value = serde_json::to_value(tool).expect("tool");
            value["annotations"]["readOnlyHint"] = serde_json::json!(false);
            value["annotations"]["destructiveHint"] =
                serde_json::json!(value["name"] == "delete_help_conversation");
            value["_meta"]["securitySchemes"] =
                serde_json::json!([{"type":"oauth2","scopes":["lvk:read","lvk:write"]}]);
            serde_json::from_value(value).expect("help tool")
        })
        .collect()
}
pub(super) async fn call(remote: &Remote, name: &str, args: Value) -> anyhow::Result<Value> {
    if name == "ask_help" {
        let _: crate::routes::help::AskHelp = arguments(args.clone())?;
    } else {
        let request: Conversation = arguments(args.clone())?;
        let _ = request.session_id;
    }
    Ok(remote
        .runtime
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Help runtime unavailable"))?
        .help(name, args)
        .await?)
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    #[tokio::test]
    async fn help_transport_reports_missing_runtime_and_rejects_extra_target() {
        let (_temp, remote, _ids) = super::super::code_read::tests::fixture().await;
        let error = call(
            &remote,
            "ask_help",
            serde_json::json!({"prompt":"How do I use LVK?"}),
        )
        .await
        .unwrap_err();
        assert!(error.to_string().contains("runtime unavailable"));
        assert!(
            call(
                &remote,
                "get_help_conversation",
                serde_json::json!({"session_id":Uuid::new_v4(),"project_id":Uuid::new_v4()})
            )
            .await
            .is_err()
        );
    }
    #[test]
    fn help_mutations_advertise_write_and_read_is_read_only() {
        for tool in tools() {
            let value = serde_json::to_value(&tool).unwrap();
            let read = tool.name == "get_help_conversation";
            assert_eq!(value["annotations"]["readOnlyHint"], read);
            assert_eq!(
                super::super::operation_scope(tool.name.as_ref()),
                if read { None } else { Some("lvk:write") }
            );
        }
    }
}
