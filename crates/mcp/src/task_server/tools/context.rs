use rmcp::{
    ErrorData, handler::server::wrapper::Parameters, model::CallToolResult, tool, tool_router,
};
use schemars::JsonSchema;
use serde::Deserialize;

use super::McpServer;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ContextRequest {
    /// Optional on-demand guide. Omit for identity metadata only.
    pub guide: Option<ContextGuide>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ContextGuide {
    Coding,
    Parallel,
    Resources,
}

#[tool_router(router = context_tools_router, vis = "pub")]
impl McpServer {
    #[tool(
        description = "Return project, issue, and workspace metadata. Request guide=coding for Manifest format, parallel for discovery/preview APIs, or resources for raw HTTP/Runner details. Coding agents perform their assigned work directly; orchestration delegation instructions apply to coordinating agents."
    )]
    async fn get_context(
        &self,
        Parameters(request): Parameters<ContextRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        let context = self.context.as_ref().expect("VK context should exist");
        let mut response = serde_json::json!(context);
        match request.guide {
            Some(ContextGuide::Coding) => {
                response["coding_run_guide"] =
                    serde_json::json!(utils::repository_memory::CODING_RUN_GUIDE);
            }
            Some(ContextGuide::Parallel) => {
                response["parallel_api_guide"] =
                    serde_json::json!(utils::agent_guides::PARALLEL_GUIDE);
            }
            Some(ContextGuide::Resources) => {
                response["resource_access_guide"] =
                    serde_json::json!(utils::agent_guides::resource_guide());
            }
            None => {}
        }
        McpServer::success(&response)
    }
}
