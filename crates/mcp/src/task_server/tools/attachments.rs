use rmcp::{
    ErrorData, handler::server::wrapper::Parameters, model::CallToolResult, tool, tool_router,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::McpServer;
#[derive(Deserialize, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct Upload {
    project_id: Uuid,
    request_id: Uuid,
    original_name: String,
    /// Standard base64, decoded size at most 2MiB. Never supply a local path or Library ID.
    #[schemars(length(max = 2796204))]
    data_base64: String,
    expected_sha256: Option<String>,
}
#[derive(Deserialize, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct Place {
    project_id: Uuid,
    workspace_id: Uuid,
    session_id: Uuid,
    request_id: Uuid,
    attachment_ids: Vec<Uuid>,
}
#[derive(Deserialize, Serialize, schemars::JsonSchema)]
struct Status {
    project_id: Uuid,
    request_id: Uuid,
}
#[tool_router(router=attachment_tools_router,vis="pub")]
impl McpServer {
    #[tool(
        description = "Read an attachment save/place/start/continue receipt using the original request_id. Inspect accepted/running/uncertain before retrying; this never starts a run."
    )]
    async fn get_attachment_operation(
        &self,
        Parameters(args): Parameters<Status>,
    ) -> Result<CallToolResult, ErrorData> {
        let url = self.url("/api/attachment-transfer/status");
        match self
            .send_json::<serde_json::Value>(self.client.post(url).json(&args))
            .await
        {
            Ok(value) => Self::success(&value),
            Err(error) => Ok(Self::tool_error(error)),
        }
    }

    #[tool(
        description = "Save exact attachment bytes in a Project without starting a run. Decoded base64 <=2MiB. Reuse request_id for retries; inspect the returned operation receipt. Saved does not mean placed."
    )]
    async fn upload_attachment(
        &self,
        Parameters(args): Parameters<Upload>,
    ) -> Result<CallToolResult, ErrorData> {
        if args.data_base64.len() > 2796204 {
            return Self::err("Inline attachment too large", None::<&str>);
        }
        let url = self.url("/api/attachment-transfer/upload");
        match self
            .send_json::<serde_json::Value>(self.client.post(url).json(&args))
            .await
        {
            Ok(value) => Self::success(&value),
            Err(error) => Ok(Self::tool_error(error)),
        }
    }
    #[tool(
        description = "Place Project attachments in a selected Session directory and verify size/SHA-256. Workspace files are shared across Sessions. Starts no run. Inspect all_verified and per-file results; partial failure blocks execution."
    )]
    async fn place_attachments(
        &self,
        Parameters(args): Parameters<Place>,
    ) -> Result<CallToolResult, ErrorData> {
        if let Err(error) = self.scope_allows_workspace(args.workspace_id) {
            return Ok(Self::tool_error(error));
        }
        let url = self.url("/api/attachment-transfer/place");
        match self
            .send_json::<serde_json::Value>(self.client.post(url).json(&args))
            .await
        {
            Ok(value) => Self::success(&value),
            Err(error) => Ok(Self::tool_error(error)),
        }
    }
}
