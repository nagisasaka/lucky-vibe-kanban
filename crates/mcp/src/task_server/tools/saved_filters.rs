//! Both MCP transports call the same authenticated server adapter and service.
use api_types::*;
use rmcp::{
    ErrorData, handler::server::wrapper::Parameters, model::CallToolResult, tool, tool_router,
};

use super::McpServer;
#[tool_router(router = saved_filter_tools_router, vis = "pub")]
impl McpServer {
    #[tool(
        description = "List shared Project saved conditions. Follow next_offset with page.offset.",
        annotations(
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn list_saved_filters(
        &self,
        Parameters(a): Parameters<SavedFilterListArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        let url = self.url("/api/mcp-stdio/saved-filters/list_saved_filters");
        match self
            .send_read_json::<serde_json::Value>(self.client.post(url).json(&a))
            .await
        {
            Ok(v) => Self::success(&v),
            Err(e) => Ok(Self::tool_error(e)),
        }
    }
    #[tool(
        description = "Get a current saved filter definition and revision; ID survives renaming.",
        annotations(
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn get_saved_filter(
        &self,
        Parameters(a): Parameters<SavedFilterReadArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        let url = self.url("/api/mcp-stdio/saved-filters/get_saved_filter");
        match self
            .send_read_json::<serde_json::Value>(self.client.post(url).json(&a))
            .await
        {
            Ok(v) => Self::success(&v),
            Err(e) => Ok(Self::tool_error(e)),
        }
    }
    #[tool(
        description = "Read machine results and cached semantic state without starting AI. Unevaluated/failed is not a complete zero result. Follow next_offset for all matches.",
        annotations(
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn get_saved_filter_results(
        &self,
        Parameters(a): Parameters<SavedFilterReadArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        let url = self.url("/api/mcp-stdio/saved-filters/get_saved_filter_results");
        match self
            .send_read_json::<serde_json::Value>(self.client.post(url).json(&a))
            .await
        {
            Ok(v) => Self::success(&v),
            Err(e) => Ok(Self::tool_error(e)),
        }
    }
    #[tool(
        description = "Get a stable board URL selecting this saved filter. Opens current definition; does not control another screen.",
        annotations(
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn get_saved_filter_link(
        &self,
        Parameters(a): Parameters<SavedFilterReadArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        let url = self.url("/api/mcp-stdio/saved-filters/get_saved_filter_link");
        match self
            .send_read_json::<serde_json::Value>(self.client.post(url).json(&a))
            .await
        {
            Ok(v) => Self::success(&v),
            Err(e) => Ok(Self::tool_error(e)),
        }
    }
    #[tool(
        description = "Create shared conditions, not a fixed Issue set. Literal case-insensitive title/description search, field AND, selection OR, tag any/all. Returns operation receipt. Reuse input.request_id and identical input; get_operation resolves uncertain outcomes."
    )]
    async fn create_saved_filter(
        &self,
        Parameters(a): Parameters<SavedFilterCreateArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        let url = self.url("/api/mcp-stdio/saved-filters/create_saved_filter");
        match self
            .send_json::<serde_json::Value>(self.client.post(url).json(&a))
            .await
        {
            Ok(v) => Self::success(&v),
            Err(e) => Ok(Self::tool_error(e)),
        }
    }
    #[tool(
        description = "Replace definition at expected_revision, preserving ID. Reuse input.request_id and identical input. Returns durable operation receipt; inspect get_operation."
    )]
    async fn update_saved_filter(
        &self,
        Parameters(a): Parameters<SavedFilterUpdateArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        let url = self.url("/api/mcp-stdio/saved-filters/update_saved_filter");
        match self
            .send_json::<serde_json::Value>(self.client.post(url).json(&a))
            .await
        {
            Ok(v) => Self::success(&v),
            Err(e) => Ok(Self::tool_error(e)),
        }
    }
    #[tool(
        description = "Delete saved definition/cache only at expected_revision; preserve Issues and relationships. Reuse input.request_id; inspect get_operation."
    )]
    async fn delete_saved_filter(
        &self,
        Parameters(a): Parameters<SavedFilterDeleteArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        let url = self.url("/api/mcp-stdio/saved-filters/delete_saved_filter");
        match self
            .send_json::<serde_json::Value>(self.client.post(url).json(&a))
            .await
        {
            Ok(v) => Self::success(&v),
            Err(e) => Ok(Self::tool_error(e)),
        }
    }
    #[tool(
        description = "Evaluate the full Project with current conditions and valid caches; may start server-side AI. Reuse request_id and identical input. Returns receipt; get_operation and get_saved_filter_results show progress."
    )]
    async fn evaluate_saved_filter(
        &self,
        Parameters(a): Parameters<SavedFilterEvaluateArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        let url = self.url("/api/mcp-stdio/saved-filters/evaluate_saved_filter");
        match self
            .send_json::<serde_json::Value>(self.client.post(url).json(&a))
            .await
        {
            Ok(v) => Self::success(&v),
            Err(e) => Ok(Self::tool_error(e)),
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
    };
    use uuid::Uuid;

    use super::*;
    #[tokio::test]
    async fn saved_filter_stdio_passes_project_definition_and_receipt_unchanged() {
        super::super::tests::install_rustls_provider();
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let project = Uuid::new_v4();
        let request = Uuid::new_v4();
        let expected = json!({"project_id":project,"input":{"request_id":request,"definition":{"name":"My slice","machine":{"search":null,"status_ids":[],"priorities":[],"assignee_ids":[],"tag_ids":[],"tag_mode":"any","archived":"active","created_at":null,"updated_at":null,"relation":null},"natural_language":null,"context":null}}});
        let backend = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut header = Vec::new();
            while !header.ends_with(b"\r\n\r\n") {
                header.push(socket.read_u8().await.unwrap());
                assert!(header.len() < 8192);
            }
            let header = String::from_utf8(header).unwrap();
            assert!(
                header
                    .starts_with("POST /api/mcp-stdio/saved-filters/create_saved_filter HTTP/1.1")
            );
            let len: usize = header
                .lines()
                .find_map(|s| {
                    s.to_lowercase()
                        .strip_prefix("content-length: ")
                        .map(str::to_owned)
                })
                .unwrap()
                .parse()
                .unwrap();
            let mut bytes = vec![0; len];
            socket.read_exact(&mut bytes).await.unwrap();
            assert_eq!(serde_json::from_slice::<Value>(&bytes).unwrap(), expected);
            let body=json!({"success":true,"data":{"request_id":request,"status":"accepted"},"message":null}).to_string();
            socket.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).as_bytes()).await.unwrap();
        });
        let server = McpServer::new_global(&format!("http://{address}"));
        let result = server
            .create_saved_filter(Parameters(SavedFilterCreateArgs {
                project_id: project,
                input: CreateSavedFilterRequest {
                    request_id: request,
                    definition: SavedFilterDefinition {
                        name: "My slice".into(),
                        machine: MachineFilter::default(),
                        natural_language: None,
                        context: None,
                    },
                },
            }))
            .await
            .unwrap();
        assert_ne!(result.is_error, Some(true));
        let value: Value =
            serde_json::from_str(&result.content[0].as_text().unwrap().text).unwrap();
        assert_eq!(value["status"], "accepted");
        assert_eq!(value["request_id"], request.to_string());
        backend.await.unwrap();
        let tools = McpServer::saved_filter_tools_router().list_all();
        assert_eq!(tools.len(), 8);
        let results = tools
            .iter()
            .find(|t| t.name == "get_saved_filter_results")
            .unwrap();
        assert_eq!(
            results.annotations.as_ref().unwrap().read_only_hint,
            Some(true)
        );
    }
}
