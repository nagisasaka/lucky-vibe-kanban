use git::repository_read::{Request, Target};
use rmcp::{
    ErrorData, handler::server::wrapper::Parameters, model::CallToolResult, tool, tool_router,
};

use super::{McpMode, McpServer};

#[tool_router(router = repository_read_tools_router, vis = "pub")]
impl McpServer {
    #[tool(
        description = "Explore an authorised Project repository: commit refs (no Workspace required) or explicit live Workspace. Typed list/glob, literal/regex grep with content/files/count, line read, fixed-head log, direct/merge_base/parent diff. Returns full SHA/revision, budgets and exclusions. Repeat original arguments with next_cursor until complete; cursor expires on server restart. Workspace reads are non-atomic and revision mismatch requires restart. Same read service as remote MCP; no shell or model calls.",
        annotations(
            read_only_hint = true,
            destructive_hint = false,
            open_world_hint = false
        )
    )]
    async fn query_repository(
        &self,
        Parameters(request): Parameters<Request>,
    ) -> Result<CallToolResult, ErrorData> {
        if let Target::Workspace { workspace_id } = &request.target
            && let Err(e) = self.scope_allows_workspace(*workspace_id)
        {
            return Ok(Self::tool_error(e));
        }
        if matches!(self.mode(), McpMode::Orchestrator)
            && let Some(project) = self.context.as_ref().and_then(|c| c.project_id)
            && project != request.project_id
        {
            return Ok(Self::tool_error(super::ToolError::classified(
                super::Category::Permission,
                super::Outcome::Failed,
            )));
        }
        match self
            .send_read_json::<serde_json::Value>(
                self.client
                    .post(self.url("/api/repository/read"))
                    .json(&request),
            )
            .await
        {
            Ok(value) => Self::success(&value),
            Err(e) => Ok(Self::tool_error(e)),
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use uuid::Uuid;

    use super::*;
    #[tokio::test]
    async fn typed_tool_catalogues_and_scope_rejection_precede_http() {
        let global = McpServer::global_mode_router();
        let scoped = McpServer::orchestrator_mode_router();
        let a = global.list_all();
        let b = scoped.list_all();
        let a = a.iter().find(|t| t.name == "query_repository").unwrap();
        let b = b.iter().find(|t| t.name == "query_repository").unwrap();
        assert_eq!(a.input_schema, b.input_schema);
        for field in ["target", "operation", "cursor", "expected_revision"] {
            assert!(a.input_schema["properties"].get(field).is_some());
        }
        let project = Uuid::new_v4();
        let workspace = Uuid::new_v4();
        let mut server = McpServer::new_orchestrator("http://127.0.0.1:1");
        server.context = Some(crate::task_server::McpContext {
            organization_id: None,
            project_id: Some(project),
            issue_id: None,
            orchestrator_session_id: None,
            workspace_id: workspace,
            workspace_branch: "fixture".into(),
            workspace_repos: vec![],
        });
        for target in [
            json!({"mode":"workspace","workspace_id":Uuid::new_v4()}),
            json!({"mode":"commit","ref":"main"}),
        ] {
            let request:Request=serde_json::from_value(json!({"project_id":Uuid::new_v4(),"repository_id":Uuid::new_v4(),"target":target,"operation":{"kind":"list"}})).unwrap();
            let result = server.query_repository(Parameters(request)).await.unwrap();
            assert_eq!(result.is_error, Some(true));
            let text = serde_json::to_value(result).unwrap().to_string();
            assert!(!text.contains("connect to VK"));
        }
    }
}

#[cfg(test)]
mod transport_tests {
    use serde_json::{Value, json};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    use super::*;
    #[tokio::test]
    async fn stdio_forwards_typed_arguments_and_preserves_revision_failure() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let mut requests = Vec::new();
            for data in [
                json!({"success":true,"data":{"source_head":"fixed","revision":"observed","complete":true,"items":[]}}),
                json!({"success":false,"data":null,"message":"Revision mismatch; restart from first page"}),
            ] {
                let (mut stream, _) = listener.accept().await.unwrap();
                let mut bytes = Vec::new();
                let mut buffer = [0u8; 4096];
                let header_end = loop {
                    let n = stream.read(&mut buffer).await.unwrap();
                    assert!(n > 0);
                    bytes.extend_from_slice(&buffer[..n]);
                    if let Some(pos) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
                        break pos + 4;
                    }
                };
                let headers = std::str::from_utf8(&bytes[..header_end]).unwrap();
                assert!(headers.starts_with("POST /api/repository/read "));
                let length = headers
                    .lines()
                    .find_map(|l| {
                        l.to_ascii_lowercase()
                            .strip_prefix("content-length:")
                            .map(|v| v.trim().parse::<usize>().unwrap())
                    })
                    .unwrap();
                while bytes.len() < header_end + length {
                    let n = stream.read(&mut buffer).await.unwrap();
                    assert!(n > 0);
                    bytes.extend_from_slice(&buffer[..n]);
                }
                requests.push(
                    serde_json::from_slice::<Value>(&bytes[header_end..header_end + length])
                        .unwrap(),
                );
                let body = data.to_string();
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                stream.write_all(response.as_bytes()).await.unwrap();
            }
            requests
        });
        let mut mcp = McpServer::new_global(&format!("http://{address}"));
        mcp.client = reqwest::Client::builder()
            .no_proxy()
            .timeout(std::time::Duration::from_secs(3))
            .build()
            .unwrap();
        let args = json!({"project_id":uuid::Uuid::from_u128(1),"repository_id":uuid::Uuid::from_u128(2),"target":{"mode":"commit","ref":"main"},"operation":{"kind":"grep","query":"猫","syntax":"regex","case_sensitive":false,"output":"count","filter":{"glob":"src/**"}},"limit":3});
        let request: Request = serde_json::from_value(args).unwrap();
        let ok = mcp
            .query_repository(Parameters(request.clone()))
            .await
            .unwrap();
        assert_eq!(ok.is_error, Some(false));
        let failed = mcp
            .query_repository(Parameters(request.clone()))
            .await
            .unwrap();
        assert_eq!(failed.is_error, Some(true));
        let diagnostic = failed.structured_content.unwrap();
        assert_eq!(diagnostic["diagnostic"]["outcome"], "failed");
        assert_eq!(diagnostic["diagnostic"]["recovery"], "inspect_state");
        let received = tokio::time::timeout(std::time::Duration::from_secs(5), server)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(received, vec![serde_json::to_value(&request).unwrap(); 2]);
    }
}
