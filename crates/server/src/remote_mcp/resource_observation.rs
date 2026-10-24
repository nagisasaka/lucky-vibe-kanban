use serde_json::Value;

use super::read::{arguments, definition};

#[derive(serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct Args {}

pub(super) fn tools() -> Vec<rmcp::model::Tool> {
    vec![definition::<Args>(
        "get_resource_observation",
        "Observe LVK server-visible Linux memory, swap, online CPU capacity, load averages, worktree filesystem usage (used / total physical filesystem capacity) and a roughly 500ms CPU sample, with separately labelled cgroup v2 limits. Read-only, bounded and on-demand; unknown fields retain errors. Not physical-host visibility, reserved capacity, admission or a prediction of future execution safety. No arguments.",
    )]
}

pub(super) async fn call(args: Value) -> anyhow::Result<Value> {
    let _: Args = arguments(args)?;
    crate::resource_observation::observation().await
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    #[test]
    fn read_only_catalog_and_example() {
        let tool = super::super::tools()
            .into_iter()
            .find(|t| t.name == "get_resource_observation")
            .unwrap();
        assert_eq!(tool.input_schema["additionalProperties"], false);
        let value = serde_json::to_value(tool).unwrap();
        assert_eq!(value["annotations"]["readOnlyHint"], true);
        assert_eq!(
            value["_meta"]["securitySchemes"][0]["scopes"],
            json!(["lvk:read"])
        );
        let examples: Vec<Value> = serde_json::from_str(include_str!(
            "../../../../scripts/chatgpt-mcp/examples.json"
        ))
        .unwrap();
        let example = examples
            .iter()
            .find(|case| case["tool"] == "get_resource_observation")
            .unwrap();
        let _: Args = arguments(example["arguments"].clone()).unwrap();
        assert_eq!(example["scopes"], json!(["lvk:read"]));
    }
}
