use rmcp::{
    ServerHandler,
    model::{Implementation, ProtocolVersion, ServerCapabilities, ServerInfo},
    tool_handler,
};

use super::{McpMode, McpServer};

#[tool_handler(router = self.tool_router)]
impl ServerHandler for McpServer {
    async fn call_tool(
        &self,
        request: rmcp::model::CallToolRequestParams,
        context: rmcp::service::RequestContext<rmcp::RoleServer>,
    ) -> Result<rmcp::model::CallToolResult, rmcp::ErrorData> {
        let known = self.tool_router.get(request.name.as_ref()).is_some();
        let route = if known {
            self.tool_router.map.get(request.name.as_ref())
        } else {
            None
        };
        let Some(route) = route else {
            let diagnostic = utils::mcp_error::Diagnostic::new(
                utils::mcp_error::Category::Validation,
                utils::mcp_error::Outcome::Failed,
            );
            return Err(rmcp::ErrorData::invalid_params(
                "Unknown tool",
                Some(serde_json::json!({"error":diagnostic})),
            ));
        };
        let tcc = rmcp::handler::server::tool::ToolCallContext::new(self, request, context);
        // Invoke the same registered handler before rmcp converts serde errors
        // into public tool text (which can include the supplied input value).
        match (route.call)(tcc).await {
            Ok(result) => Ok(result),
            Err(error) if error.code == rmcp::model::ErrorCode::INVALID_PARAMS => {
                let diagnostic = utils::mcp_error::Diagnostic::new(
                    utils::mcp_error::Category::Validation,
                    utils::mcp_error::Outcome::Failed,
                );
                let value = serde_json::json!({"error":diagnostic});
                let mut result =
                    rmcp::model::CallToolResult::error(vec![rmcp::model::Content::text(
                        diagnostic.message,
                    )]);
                result.structured_content = Some(value);
                Ok(result)
            }
            Err(mut error) => {
                let diagnostic = utils::mcp_error::Diagnostic::new(
                    utils::mcp_error::Category::Internal,
                    utils::mcp_error::Outcome::Unknown,
                );
                error.message = diagnostic.message.into();
                error.data = Some(serde_json::json!({"error":diagnostic}));
                Err(error)
            }
        }
    }

    fn get_info(&self) -> ServerInfo {
        let preamble = match self.mode() {
            McpMode::Global => {
                "A Vibe Kanban MCP server for task, issue, repository, workspace, and session management."
            }
            McpMode::Orchestrator => {
                "An orchestrator-scoped Vibe Kanban MCP server with tools limited to the configured workspace and orchestrator session context."
            }
        };
        let mut instruction = format!(
            "{} Use list/read tools first when you need IDs or current state.",
            preamble
        );
        if self.context.is_some() {
            instruction = format!(
                "Use 'get_context' to fetch project, issue, workspace, and orchestrator-session metadata for the active MCP context when available. {}",
                instruction
            );
        }

        instruction.push_str(" Error diagnostics separate category from outcome. Unknown outcomes require the original receipt and target state; reuse the original request ID with identical input only for tools promising durable deduplication. Never automatically replay under a new ID. Accepted/running is not completed success.");

        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("vibe-kanban-mcp", "1.0.0"))
            .with_protocol_version(ProtocolVersion::V_2025_03_26)
            .with_instructions(instruction)
    }
}
