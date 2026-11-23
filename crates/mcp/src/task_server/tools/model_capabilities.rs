use rmcp::{
    ErrorData, handler::server::wrapper::Parameters, model::CallToolResult, schemars, tool,
    tool_router,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::McpServer;

#[derive(Deserialize, Serialize, schemars::JsonSchema)]
struct ModelCapabilitiesRequest {
    /// Engine (CODEX or CLAUDE_CODE); omit or null for the configured default.
    executor: Option<String>,
    /// Profile variant; omit or null for the selected engine's default profile.
    variant: Option<String>,
    /// Discover in this existing Workspace's directory, if available.
    workspace_id: Option<Uuid>,
    /// Discover in this repository's directory; mutually exclusive with workspace_id.
    repo_id: Option<Uuid>,
}

#[tool_router(router = model_capabilities_tools_router, vis = "pub")]
impl McpServer {
    #[tool(
        description = "Discover models and reasoning options before starting a Workspace or Session. Uses the selected provider and installed CLI; no Session or Run is created. Choose a repository or Workspace for directory-dependent provider configuration; otherwise uses the server current directory. Repository results are provisional for a new Workspace; its actual root is checked before the first Run. Discovery failure is reported rather than treated as a valid selection.",
        annotations(
            read_only_hint = true,
            destructive_hint = false,
            open_world_hint = true
        )
    )]
    async fn get_initial_model_capabilities(
        &self,
        Parameters(request): Parameters<ModelCapabilitiesRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        match self
            .send_read_json::<serde_json::Value>(
                self.client
                    .post(self.url("/api/mcp-stdio/model-capabilities"))
                    .json(&request),
            )
            .await
        {
            Ok(value) => Self::success(&value),
            Err(error) => Ok(Self::tool_error(error)),
        }
    }
}
