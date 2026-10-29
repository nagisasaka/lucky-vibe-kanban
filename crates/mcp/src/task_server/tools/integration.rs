//! Thin worker adapter. Upstream AI uses start/get/steer; this operation is bound
//! to the local coding runtime and never accepts a repository/target override.
use rmcp::{
    ErrorData, handler::server::wrapper::Parameters, model::CallToolResult, schemars, tool,
    tool_router,
};
use serde::Deserialize;
use uuid::Uuid;

use super::McpServer;

#[derive(Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct IntegrationAction {
    request_id: Uuid,
    /// inspect, verify (managed command), skip (reason/risk), publish (deferred until writer settlement).
    action: String,
    expected_commit: Option<String>,
    code_commit: Option<String>,
    wiki_commit: Option<String>,
    command: Option<String>,
    /// Skip provenance: user or agent. Attribution only; never an execution approval.
    decision_actor: Option<String>,
    #[serde(default)]
    reason: String,
    #[serde(default)]
    remaining_risk: String,
}

#[tool_router(router = integration_tools_router, vis = "pub")]
impl McpServer {
    #[tool(
        description = "Worker-only Auto-merge operation in the current Workspace. inspect reads frozen targets/evidence; verify executes a command on expected clean SHA; skip records reason and remaining risk; publish saves a request for host execution AFTER this Run and all children settle. Never use shell to update the target. Reuse request_id and identical input after uncertainty; inspect existing receipt/process, never blindly rerun. Publication acceptance is not publication success. Wiki-enabled work supplies code_commit and wiki_commit separately. Ordinary commands/skip decisions need no repeated approval."
    )]
    async fn integration_action(
        &self,
        Parameters(input): Parameters<IntegrationAction>,
    ) -> Result<CallToolResult, ErrorData> {
        let Some(context) = &self.context else {
            return Self::err("Current runtime context required", None);
        };
        let agent = match std::env::var("VK_AGENT_RUN_ID")
            .ok()
            .and_then(|s| Uuid::parse_str(&s).ok())
        {
            Some(id) => id,
            None => return Self::err("Current AgentRun binding required", None),
        };
        let body = serde_json::json!({"workspace_id":context.workspace_id,"agent_run_id":agent,"request_id":input.request_id,"action":input.action,"expected_commit":input.expected_commit,"code_commit":input.code_commit,"wiki_commit":input.wiki_commit,"command":input.command,"decision_actor":input.decision_actor,"reason":input.reason,"remaining_risk":input.remaining_risk});
        match self
            .send_json::<serde_json::Value>(
                self.client
                    .post(self.url("/api/integrations/worker"))
                    .json(&body),
            )
            .await
        {
            Ok(value) => Self::success(&value),
            Err(error) => Ok(Self::tool_error(error)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn worker_request_cannot_override_its_runtime_target() {
        let request = serde_json::json!({"request_id":Uuid::new_v4(),"action":"skip","expected_commit":"code","reason":"existing tested source","remaining_risk":"environment not rerun","decision_actor":"user"});
        assert!(serde_json::from_value::<IntegrationAction>(request.clone()).is_ok());
        for name in [
            "workspace_id",
            "repository_id",
            "session_id",
            "agent_run_id",
            "target_branch",
        ] {
            let mut attempted = request.clone();
            attempted[name] = serde_json::json!(Uuid::new_v4());
            assert!(
                serde_json::from_value::<IntegrationAction>(attempted).is_err(),
                "{name} must be bound by the host"
            );
        }
        let tools = McpServer::integration_tools_router().list_all();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name, "integration_action");
    }
}
