use executors::account_usage::Request;
use rmcp::{
    ErrorData, handler::server::wrapper::Parameters, model::CallToolResult, tool, tool_router,
};

use super::McpServer;

#[tool_router(router = provider_usage_tools_router, vis = "pub")]
impl McpServer {
    #[tool(
        description = "Read official Codex/Claude account quota and optional usage from explicit server_default auth/launcher context (administrator or authenticated LVK internal agent). No Workspace is required for account scope. Uses a short-lived non-model control connection with timeout/cleanup; no prompt, transcript scan, login or execution records. For existing session usage supply authorized LVK project/workspace/session IDs. Values are cumulative snapshots, not additions; estimated USD is not subscription billing.",
        annotations(
            read_only_hint = true,
            destructive_hint = false,
            open_world_hint = true
        )
    )]
    async fn read_provider_usage(
        &self,
        Parameters(request): Parameters<Request>,
    ) -> Result<CallToolResult, ErrorData> {
        if let Some(workspace) = request.workspace_id
            && let Err(error) = self.scope_allows_workspace(workspace)
        {
            return Ok(Self::tool_error(error));
        }
        let url = self.url("/api/provider-usage/read");
        match self
            .send_read_json::<serde_json::Value>(self.client.post(url).json(&request))
            .await
        {
            Ok(value) => Self::success(&value),
            Err(error) => Ok(Self::tool_error(error)),
        }
    }
}
