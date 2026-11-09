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
        description = "Return project, issue, workspace, and orchestrator-session metadata. Request guide=coding for Manifest format, parallel for discovery/preview APIs, or resources for raw HTTP/Runner details. Coding agents perform their assigned work directly; orchestration delegation instructions apply to coordinating agents."
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
                response["parallel_api_guide"] = serde_json::json!(
                    "Use CURRENT discovery_url for public repository activity. Follow next_workspace as after_workspace and manifests.next_after as after_manifest only for needed ranges; errors are not zero activity. Read pinned source at /api/repos/{repository_id}/snapshot?commit=<full-oid>&path=<url-encoded-path>; omit path to enumerate, follow next_after as after_path. For a test-only combination POST /api/workspaces/{workspace_id}/integration/preview with repository_id, base_commit (clean full HEAD), peers [{workspace_id,commit}]. Only combine in its returned detached trial path. Preview is not formal publication or Auto-merge."
                );
            }
            Some(ContextGuide::Resources) => {
                let token = utils::assets::asset_dir().join("access-internal.token");
                response["resource_access_guide"] = serde_json::json!(format!(
                    "Prefer stdio MCP resource and execution-bridge tools, whose contracts and schemas describe atomic claims, waiting, recovery and Runner finish. For raw loopback HTTP only, read {} and send X-LVK-Internal-Token in memory; never print it, put it in URLs/command arguments, send it to other hosts or follow redirects. This capability cannot manage server authentication. Resource API: /api/resource-coordination (snapshot, operations, operations/{{id}}?wait_seconds=25, operations/{{id}}/events and operations/{{id}}/cancel). Runner API: /api/execution-bridge/runners, /sources and /operations/{{id}}/commands. Capture an immutable source with session_id and repository working_dir; select runner_id explicitly and claim its execution slot and every device in the same bundle. Runner working_dir is snapshot-relative; Windows uses PowerShell. Interactive commands require stable id, kind step or finish, and script. Finish runs cleanup and verification; disconnect/cancel/timeout do not prove remote termination. Do not complete with an open operation; only the operator confirms recovery after settled containment.",
                    token.display()
                ));
            }
            None => {}
        }
        McpServer::success(&response)
    }
}
