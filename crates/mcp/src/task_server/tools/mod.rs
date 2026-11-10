use std::str::FromStr;

use api_types::{Issue, ListProjectStatusesResponse, ProjectStatus};
use executors::executors::BaseCodingAgent;
use rmcp::{
    ErrorData,
    model::{CallToolResult, Content},
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use thiserror::Error;
use utils::mcp_error::{Category, Diagnostic, Outcome};
use uuid::Uuid;

use super::{ApiResponseEnvelope, McpServer};

type ToolCallResult = Result<CallToolResult, ErrorData>;

#[derive(Debug, Error)]
#[error("{message}")]
struct ToolError {
    message: &'static str,
    diagnostic: Diagnostic,
}

impl ToolError {
    fn new(message: &'static str, _details: Option<impl Into<String>>) -> Self {
        Self {
            message,
            diagnostic: Diagnostic::new(Category::Validation, Outcome::Failed),
        }
    }

    fn message(message: &'static str) -> Self {
        Self::new(message, None::<String>)
    }

    fn classified(category: Category, outcome: Outcome) -> Self {
        let diagnostic = Diagnostic::new(category, outcome);
        Self {
            message: diagnostic.message,
            diagnostic,
        }
    }
}

mod attachments;
mod context;
mod design_notes;
mod dev_server;
#[cfg(test)]
mod error_contract_tests;
mod integration;
mod issue_assignees;
mod issue_relationships;
mod issue_tags;
mod operations;
mod organizations;
mod provider_usage;
mod remote_issues;
mod remote_projects;
mod repos;
mod repository_read;
mod resources;
mod self_restart;
mod sessions;
mod task_attempts;
mod todos;
mod workflow;
mod workspaces;

impl McpServer {
    pub fn global_mode_router() -> rmcp::handler::server::tool::ToolRouter<Self> {
        Self::context_tools_router()
            + Self::operation_tools_router()
            + Self::attachment_tools_router()
            + Self::integration_tools_router()
            + Self::provider_usage_tools_router()
            + Self::workflow_tools_router()
            + Self::resource_tools_router()
            + Self::repository_read_tools_router()
            + Self::dev_server_tools_router()
            + Self::self_restart_tools_router()
            + Self::workspaces_tools_router()
            + Self::organizations_tools_router()
            + Self::repos_tools_router()
            + Self::remote_projects_tools_router()
            + Self::remote_issues_tools_router()
            + Self::todo_tools_router()
            + Self::design_note_tools_router()
            + Self::issue_assignees_tools_router()
            + Self::issue_tags_tools_router()
            + Self::issue_relationships_tools_router()
            + Self::task_attempts_tools_router()
            + Self::session_tools_router()
    }
}

impl McpServer {
    fn scoped_workspace_id(&self) -> Option<Uuid> {
        self.context.as_ref().map(|ctx| ctx.workspace_id)
    }

    fn operation_result(value: serde_json::Value) -> ToolCallResult {
        if matches!(value["status"].as_str(), Some("failed" | "uncertain")) {
            let mut result = CallToolResult::error(vec![Content::text(value.to_string())]);
            result.structured_content = Some(value);
            Ok(result)
        } else {
            Self::success(&value)
        }
    }

    fn success<T: Serialize>(data: &T) -> ToolCallResult {
        match serde_json::to_string_pretty(data) {
            Ok(text) => Ok(CallToolResult::success(vec![Content::text(text)])),
            // A mutation may already have completed before response encoding.
            Err(_) => Ok(Self::tool_error(ToolError::classified(
                Category::Internal,
                Outcome::Unknown,
            ))),
        }
    }

    fn err(msg: &'static str, details: Option<&str>) -> ToolCallResult {
        Ok(Self::tool_error(ToolError::new(msg, details)))
    }

    fn tool_error(error: ToolError) -> CallToolResult {
        // Neither backend response bodies nor reqwest/serde diagnostics are
        // public data. They may include credentials, SQL, paths or input text.
        tracing::warn!(correlation_id=%error.diagnostic.correlation_id,
            category=?error.diagnostic.category, outcome=?error.diagnostic.outcome,
            "MCP tool failed");
        let value = serde_json::json!({
            "success": false,
            "error": error.message,
            "diagnostic": error.diagnostic,
        });
        let mut result = CallToolResult::error(vec![Content::text(value.to_string())]);
        result.structured_content = Some(value);
        result
    }

    async fn send_response(
        &self,
        rb: reqwest::RequestBuilder,
        read_only: bool,
    ) -> Result<(reqwest::Response, bool), ToolError> {
        let request = rb
            .build()
            .map_err(|_| ToolError::classified(Category::Validation, Outcome::Failed))?;
        let read = read_only
            || matches!(
                *request.method(),
                reqwest::Method::GET | reqwest::Method::HEAD
            );
        let response = self.client.execute(request).await.map_err(|error| {
            let outcome = if read || error.is_connect() {
                Outcome::Failed
            } else {
                Outcome::Unknown
            };
            ToolError::classified(Category::Unavailable, outcome)
        })?;
        let status = response.status();
        if !status.is_success() {
            let outcome = if read || matches!(status.as_u16(), 401 | 403) {
                Outcome::Failed
            } else {
                Outcome::Unknown
            };
            return Err(ToolError::classified(
                utils::mcp_error::http_category(status.as_u16()),
                outcome,
            ));
        }
        Ok((response, read))
    }

    async fn send_json<T: DeserializeOwned>(
        &self,
        rb: reqwest::RequestBuilder,
    ) -> Result<T, ToolError> {
        self.send_json_with_effects(rb, false).await
    }

    async fn send_read_json<T: DeserializeOwned>(
        &self,
        rb: reqwest::RequestBuilder,
    ) -> Result<T, ToolError> {
        self.send_json_with_effects(rb, true).await
    }

    async fn send_json_with_effects<T: DeserializeOwned>(
        &self,
        rb: reqwest::RequestBuilder,
        read_only: bool,
    ) -> Result<T, ToolError> {
        let (response, read) = self.send_response(rb, read_only).await?;
        let unavailable = || {
            ToolError::classified(
                Category::Internal,
                if read {
                    Outcome::Failed
                } else {
                    Outcome::Unknown
                },
            )
        };
        let api_response = response
            .json::<ApiResponseEnvelope<T>>()
            .await
            .map_err(|_| unavailable())?;
        if !api_response.success {
            return Err(unavailable());
        }
        api_response.data.ok_or_else(unavailable)
    }

    async fn send_empty_json(&self, rb: reqwest::RequestBuilder) -> Result<(), ToolError> {
        let (response, read) = self.send_response(rb, false).await?;
        let unavailable = || {
            ToolError::classified(
                Category::Internal,
                if read {
                    Outcome::Failed
                } else {
                    Outcome::Unknown
                },
            )
        };
        #[derive(Deserialize)]
        struct EmptyApiResponse {
            success: bool,
        }
        let value = response
            .json::<EmptyApiResponse>()
            .await
            .map_err(|_| unavailable())?;
        if !value.success {
            return Err(unavailable());
        }
        Ok(())
    }

    fn resolve_workspace_id(&self, explicit: Option<Uuid>) -> Result<Uuid, ToolError> {
        if let Some(id) = explicit {
            return Ok(id);
        }
        if let Some(workspace_id) = self.scoped_workspace_id() {
            return Ok(workspace_id);
        }
        Err(ToolError::message(
            "workspace_id is required (not available from current MCP context)",
        ))
    }

    // Expands @tagname references in text by replacing them with tag content.

    // Resolves a project_id from an explicit parameter or falls back to context.
    fn resolve_project_id(&self, explicit: Option<Uuid>) -> Result<Uuid, ToolError> {
        if let Some(id) = explicit {
            return Ok(id);
        }
        if let Some(ctx) = &self.context
            && let Some(id) = ctx.project_id
        {
            return Ok(id);
        }
        Err(ToolError::message(
            "project_id is required (not available from workspace context)",
        ))
    }

    // Resolves an organization_id from an explicit parameter or falls back to context.
    fn resolve_organization_id(&self, explicit: Option<Uuid>) -> Result<Uuid, ToolError> {
        if let Some(id) = explicit {
            return Ok(id);
        }
        if let Some(ctx) = &self.context
            && let Some(id) = ctx.organization_id
        {
            return Ok(id);
        }
        Err(ToolError::message(
            "organization_id is required (not available from workspace context)",
        ))
    }

    // Fetches project statuses for a project.
    async fn fetch_project_statuses(
        &self,
        project_id: Uuid,
    ) -> Result<Vec<ProjectStatus>, ToolError> {
        let url = self.url(&format!(
            "/api/board/project-statuses?project_id={}",
            project_id
        ));
        let response: ListProjectStatusesResponse = self.send_json(self.client.get(&url)).await?;
        Ok(response.project_statuses)
    }

    // Resolves a status name to status_id.

    // Gets the default status_id for a project (first non-hidden status by sort_order).

    // Resolves a status_id to its display name. Falls back to UUID string if lookup fails.
    async fn resolve_status_name(&self, project_id: Uuid, status_id: Uuid) -> String {
        match self.fetch_project_statuses(project_id).await {
            Ok(statuses) => statuses
                .iter()
                .find(|s| s.id == status_id)
                .map(|s| s.name.clone())
                .unwrap_or_else(|| status_id.to_string()),
            Err(_) => status_id.to_string(),
        }
    }

    // Links a workspace to a remote issue by fetching issue.project_id and calling link endpoint.
    async fn link_workspace_to_issue(
        &self,
        workspace_id: Uuid,
        issue_id: Uuid,
    ) -> Result<(), ToolError> {
        let issue_url = self.url(&format!("/api/board/issues/{}", issue_id));
        let issue: Issue = self.send_json(self.client.get(&issue_url)).await?;

        let link_url = self.url(&format!("/api/workspaces/{}/links", workspace_id));
        let link_payload = serde_json::json!({
            "project_id": issue.project_id,
            "issue_id": issue_id,
        });
        self.send_empty_json(self.client.post(&link_url).json(&link_payload))
            .await
    }

    fn parse_executor_agent(executor: &str) -> Result<BaseCodingAgent, ToolError> {
        let normalized = executor.replace('-', "_").to_ascii_uppercase();
        BaseCodingAgent::from_str(&normalized).map_err(|_| ToolError::message("Unknown executor"))
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeSet, sync::Once};

    use super::McpServer;

    static RUSTLS_PROVIDER: Once = Once::new();

    pub(super) fn install_rustls_provider() {
        RUSTLS_PROVIDER.call_once(|| {
            rustls::crypto::aws_lc_rs::default_provider()
                .install_default()
                .expect("Failed to install rustls crypto provider");
        });
    }

    fn tool_names(router: rmcp::handler::server::tool::ToolRouter<McpServer>) -> BTreeSet<String> {
        let tools = router.list_all();
        let names: BTreeSet<_> = tools.iter().map(|tool| tool.name.to_string()).collect();
        assert_eq!(names.len(), tools.len(), "Tool names must be unique");
        names
    }

    #[test]
    fn global_mode_keeps_workspace_admin_and_discovery_tools() {
        let actual = tool_names(McpServer::global_mode_router());

        for name in [
            "read_provider_usage",
            "start_dev_server",
            "stop_dev_server",
            "get_dev_server_status",
        ] {
            assert!(actual.contains(name));
        }
        assert!(actual.contains("get_operation"));
        assert!(actual.contains("list_workspaces"));
        assert!(actual.contains("delete_workspace"));
        assert!(actual.contains("run_resource_operation"));
        assert!(!actual.contains("output_markdown"));
    }
}

#[cfg(test)]
mod stdio_tests;
