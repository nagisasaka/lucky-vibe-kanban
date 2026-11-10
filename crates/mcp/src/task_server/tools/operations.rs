use rmcp::{
    ErrorData, handler::server::wrapper::Parameters, model::CallToolResult, tool, tool_router,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::McpServer;
#[derive(Deserialize, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct Request {
    request_id: Uuid,
    project_id: Option<Uuid>,
}
#[tool_router(router = operation_tools_router, vis = "pub")]
impl McpServer {
    #[tool(
        description = "Read this authenticated principal's durable operation receipt using the original request_id. Optional project_id asserts its Project. Internal stdio uses a shared internal principal, not Run isolation; external principals' receipts are unavailable. Accepted/running is not domain or implementation completion. An omitted request_id on the original operation cannot be recovered after losing its first response. Never replay an unknown outcome under a new ID."
    )]
    async fn get_operation(
        &self,
        Parameters(args): Parameters<Request>,
    ) -> Result<CallToolResult, ErrorData> {
        match self
            .send_read_json::<serde_json::Value>(
                self.client
                    .post(self.url("/api/mcp-stdio/operation"))
                    .json(&args),
            )
            .await
        {
            Ok(value) => Self::success(&value),
            Err(error) => Ok(Self::tool_error(error)),
        }
    }
}
