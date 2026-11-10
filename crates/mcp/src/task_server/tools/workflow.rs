use rmcp::{
    ErrorData, handler::server::wrapper::Parameters, model::CallToolResult, tool, tool_router,
};
use workflow::presets::WorkflowToolRequest as Request;

use super::McpServer;
#[tool_router(router=workflow_tools_router,vis="pub")]
impl McpServer {
    #[tool(
        description = "List/start the three fixed Workflow presets, read status/results, stop, or edit Project additions. Same parameters and execution service as UI. Reuse the exact start.request_id and parameters after uncertainty. Start admission is not completion. No arbitrary graphs, automatic adoption, repair, deletion, merge or publication."
    )]
    async fn workflow(
        &self,
        Parameters(args): Parameters<Request>,
    ) -> Result<CallToolResult, ErrorData> {
        match self
            .send_read_json::<serde_json::Value>(
                self.client
                    .post(self.url("/api/mcp-stdio/workflow"))
                    .json(&args),
            )
            .await
        {
            Ok(value) => Self::success(&value),
            Err(error) => Ok(Self::tool_error(error)),
        }
    }
}
