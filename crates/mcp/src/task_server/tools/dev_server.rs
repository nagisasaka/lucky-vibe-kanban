use rmcp::{
    ErrorData,
    handler::server::wrapper::Parameters,
    model::{CallToolResult, Content},
    schemars, tool, tool_router,
};
use serde::Deserialize;
use uuid::Uuid;

use super::McpServer;

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct Target {
    project_id: Uuid,
    workspace_id: Uuid,
}

#[tool_router(router = dev_server_tools_router, vis = "pub")]
impl McpServer {
    #[tool(
        description = "Start the explicit Project/Workspace managed DevServer through the UI manager. Reuses running processes without restarting. Response distinguishes acceptance from observed process state; running is not application readiness."
    )]
    async fn start_dev_server(
        &self,
        Parameters(t): Parameters<Target>,
    ) -> Result<CallToolResult, ErrorData> {
        self.managed_dev_server(t, "start").await
    }
    #[tool(
        description = "Stop only the explicit Project/Workspace managed DevServer processes. Already stopped is a no-op. Inspect returned process observation; never substitute a generic shell command."
    )]
    async fn stop_dev_server(
        &self,
        Parameters(t): Parameters<Target>,
    ) -> Result<CallToolResult, ErrorData> {
        self.managed_dev_server(t, "stop").await
    }
    #[tool(
        description = "Read managed DevServer process state for an explicit Project/Workspace. Does not start or stop anything. Process running does not prove application readiness or health."
    )]
    async fn get_dev_server_status(
        &self,
        Parameters(t): Parameters<Target>,
    ) -> Result<CallToolResult, ErrorData> {
        self.managed_dev_server(t, "status").await
    }
}
impl McpServer {
    async fn managed_dev_server(
        &self,
        t: Target,
        action: &str,
    ) -> Result<CallToolResult, ErrorData> {
        if let Err(error) = self.scope_allows_workspace(t.workspace_id) {
            return Ok(Self::tool_error(error));
        }
        let url = self.url(&format!(
            "/api/workspaces/{}/managed-dev-server/{action}",
            t.workspace_id
        ));
        let request = if action == "status" {
            self.client.get(url)
        } else {
            self.client.post(url)
        };
        match self
            .send_json::<serde_json::Value>(request.query(&[("project_id", t.project_id)]))
            .await
        {
            Ok(mut value) if value["accepted"] == false => {
                if value.get("error").is_none() {
                    // An untyped domain rejection cannot establish that a partial
                    // start/stop did not occur. Current servers supply the phase.
                    value["error"] = utils::mcp_error::Diagnostic::new(
                        super::Category::OutcomeUnknown,
                        super::Outcome::Unknown,
                    )
                    .value();
                }
                let mut result = CallToolResult::error(vec![Content::text(value.to_string())]);
                result.structured_content = Some(value);
                Ok(result)
            }
            Ok(value) => Self::success(&value),
            Err(error) => Ok(Self::tool_error(error)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn dev_server_scoped_target_cannot_be_overridden() {
        let id = Uuid::new_v4();
        let mut server = McpServer::new_orchestrator("http://127.0.0.1:1");
        server.context = Some(crate::task_server::McpContext {
            organization_id: None,
            project_id: None,
            issue_id: None,
            orchestrator_session_id: None,
            workspace_id: id,
            workspace_branch: "fixture".into(),
            workspace_repos: vec![],
        });
        for action in ["status", "start", "stop"] {
            let result = server
                .managed_dev_server(
                    Target {
                        project_id: Uuid::new_v4(),
                        workspace_id: Uuid::new_v4(),
                    },
                    action,
                )
                .await
                .unwrap();
            assert_eq!(result.is_error, Some(true));
        }
        for router in [
            McpServer::global_mode_router(),
            McpServer::orchestrator_mode_router(),
        ] {
            let tools = router.list_all();
            for name in [
                "start_dev_server",
                "stop_dev_server",
                "get_dev_server_status",
            ] {
                assert!(tools.iter().any(|t| t.name == name));
            }
        }
    }
}
