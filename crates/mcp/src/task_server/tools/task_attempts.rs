use db::models::requests::{CreateAndStartWorkspaceRequest, WorkspaceRepoInput};
use executors::profile::{ExecutionMode, ExecutorConfig};
use rmcp::{
    ErrorData, handler::server::wrapper::Parameters, model::CallToolResult, schemars, tool,
    tool_router,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::McpServer;

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct McpWorkspaceRepoInput {
    #[schemars(description = "The repository ID")]
    repo_id: Uuid,
    #[schemars(description = "The branch for this repository")]
    branch: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct StartWorkspaceRequest {
    #[schemars(description = "Name for the workspace")]
    name: String,
    #[serde(default)]
    attachment_ids: Vec<Uuid>,
    request_id: Option<Uuid>,
    expected_revision: Option<String>,
    project_id: Option<Uuid>,
    #[schemars(
        description = "Optional prompt for the first workspace session. If omitted/empty, the linked issue title/description is used."
    )]
    prompt: Option<String>,
    #[schemars(description = "The coding agent executor to run ('CLAUDE_CODE', 'CODEX')")]
    executor: String,
    #[schemars(description = "Optional executor variant, if needed")]
    variant: Option<String>,
    #[schemars(
        description = "Optional Codex execution mode. review uses Code execution for bounded review/investigation: report in conversation and stop; product implementation requires separate explicit instructions. Include target, scope and expected findings in prompt. Existing permissions still apply."
    )]
    execution_mode: Option<ExecutionMode>,
    #[schemars(description = "Repository selection for the workspace")]
    repositories: Vec<McpWorkspaceRepoInput>,
    #[schemars(
        description = "Optional issue ID to link the workspace to. When provided, the workspace will be associated with this remote issue."
    )]
    issue_id: Option<Uuid>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct LinkWorkspaceIssueRequest {
    #[schemars(description = "The workspace ID to link")]
    workspace_id: Uuid,
    #[schemars(description = "The issue ID to link the workspace to")]
    issue_id: Uuid,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
struct LinkWorkspaceIssueResponse {
    #[schemars(description = "Whether the linking was successful")]
    success: bool,
    #[schemars(description = "The workspace ID that was linked")]
    workspace_id: String,
    #[schemars(description = "The issue ID it was linked to")]
    issue_id: String,
}

#[tool_router(router = task_attempts_tools_router, vis = "pub")]
impl McpServer {
    #[tool(
        description = "Create a Workspace and start its first Session. Optional request_id gives same-principal durable replay protection (requires Project context or linked Issue); omission gives none. Optional expected_revision checks the linked Issue revision. Acceptance is not implementation completion; inspect get_operation after response loss."
    )]
    async fn start_workspace(
        &self,
        Parameters(StartWorkspaceRequest {
            name,
            prompt,
            executor,
            variant,
            execution_mode,
            repositories,
            issue_id,
            attachment_ids,
            request_id,
            expected_revision,
            project_id,
        }): Parameters<StartWorkspaceRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        if repositories.is_empty() {
            return Self::err("At least one repository must be specified.", None::<&str>);
        }

        let executor_trimmed = executor.trim();
        if executor_trimmed.is_empty() {
            return Self::err("Executor must not be empty.", None::<&str>);
        }

        let prompt = prompt.and_then(|prompt| {
            let trimmed = prompt.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        });

        let base_executor = match Self::parse_executor_agent(executor_trimmed) {
            Ok(exec) => exec,
            Err(_) => {
                return Self::err("Unknown executor", None::<&str>);
            }
        };

        if execution_mode == Some(ExecutionMode::Review)
            && base_executor != executors::executors::BaseCodingAgent::Codex
        {
            return Self::err("REVIEW execution mode requires CODEX", None::<&str>);
        }

        let variant = variant.and_then(|v| {
            let trimmed = v.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        });

        let workspace_repos: Vec<WorkspaceRepoInput> = repositories
            .into_iter()
            .map(|r| WorkspaceRepoInput {
                repo_id: r.repo_id,
                target_branch: r.branch,
            })
            .collect();

        // Resolve mutable linked-Issue data only after the server has checked
        // the durable receipt for this stable caller input.
        if prompt.is_none() && issue_id.is_none() {
            return Self::err(
                "Provide `prompt`, or `issue_id` with a non-empty title/description.",
                None::<&str>,
            );
        }
        let workspace_prompt = prompt.unwrap_or_default();

        let create_and_start_payload = CreateAndStartWorkspaceRequest {
            mode: db::models::requests::CreateWorkspaceMode::Worktree,
            name: Some(name.clone()),
            repos: workspace_repos,
            directory_path: None,
            linked_issue: None,
            executor_config: ExecutorConfig {
                executor: base_executor,
                variant,
                model_id: None,
                agent_id: None,
                reasoning_id: None,
                permission_policy: None,
                execution_mode,
                goal_token_budget: None,
                goal_max_concurrent_agents: None,
            },
            prompt: workspace_prompt,
            selected_skills: None,
            resume_session_id: None,
            resume_scope_path: None,
            attachment_ids: (!attachment_ids.is_empty()).then_some(attachment_ids),
        };

        let url = self.url("/api/mcp-stdio/start-workspace");
        let body = serde_json::json!({"issue_id":issue_id,"request_id":request_id,"project_id":project_id.or_else(||self.context.as_ref().and_then(|c|c.project_id)),"expected_revision":expected_revision,"request":create_and_start_payload});
        match self
            .send_json::<serde_json::Value>(self.client.post(url).json(&body))
            .await
        {
            Ok(value) => Self::operation_result(value),
            Err(error) => Ok(Self::tool_error(error)),
        }
    }

    #[tool(
        description = "Link an orphan workspace to an issue without changing repositories or history. Already-linked workspaces are rejected; explicitly unlink them from the old issue first."
    )]
    async fn link_workspace_issue(
        &self,
        Parameters(LinkWorkspaceIssueRequest {
            workspace_id,
            issue_id,
        }): Parameters<LinkWorkspaceIssueRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        if let Err(e) = self.link_workspace_to_issue(workspace_id, issue_id).await {
            return Ok(Self::tool_error(e));
        }

        McpServer::success(&LinkWorkspaceIssueResponse {
            success: true,
            workspace_id: workspace_id.to_string(),
            issue_id: issue_id.to_string(),
        })
    }
}

#[cfg(test)]
mod review_tests {
    use super::*;

    #[test]
    fn start_workspace_accepts_review_and_advertises_its_scope() {
        let request: StartWorkspaceRequest = serde_json::from_value(serde_json::json!({"name":"Review","executor":"CODEX","execution_mode":"review","repositories":[]})).unwrap();
        assert_eq!(request.execution_mode, Some(ExecutionMode::Review));
        let schema = serde_json::to_string(&schemars::schema_for!(StartWorkspaceRequest)).unwrap();
        assert!(schema.contains("review"));
        assert!(schema.contains("product implementation requires separate explicit instructions"));
    }
    #[tokio::test]
    async fn workspace_start_forwards_request_identity_for_both_attachment_branches() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        for attachments in [vec![], vec![Uuid::new_v4()]] {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let url = format!("http://{}", listener.local_addr().unwrap());
            let request_id = Uuid::new_v4();
            let project = Uuid::new_v4();
            let issue = Uuid::new_v4();
            let server = tokio::spawn(async move {
                let (mut socket, _) = listener.accept().await.unwrap();
                let mut bytes = Vec::new();
                loop {
                    let mut chunk = [0; 4096];
                    let n = socket.read(&mut chunk).await.unwrap();
                    assert!(n > 0);
                    bytes.extend_from_slice(&chunk[..n]);
                    let text = String::from_utf8_lossy(&bytes);
                    if let Some(end) = text.find("\r\n\r\n") {
                        let len = text[..end]
                            .lines()
                            .find_map(|l| {
                                l.to_ascii_lowercase()
                                    .strip_prefix("content-length:")
                                    .map(|v| v.trim().parse::<usize>().unwrap())
                            })
                            .unwrap_or(0);
                        if bytes.len() >= end + 4 + len {
                            break;
                        }
                    }
                }
                let text = String::from_utf8(bytes).unwrap();
                let body = r#"{"success":true,"data":{"status":"accepted"}}"#;
                socket.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).as_bytes()).await.unwrap();
                text
            });
            let args:StartWorkspaceRequest=serde_json::from_value(serde_json::json!({"name":"start","executor":"CODEX","issue_id":issue,"request_id":request_id,"project_id":project,"attachment_ids":attachments,"repositories":[{"repo_id":Uuid::new_v4(),"branch":"main"}]})).unwrap();
            let result = McpServer::new_global(&url)
                .start_workspace(Parameters(args))
                .await
                .unwrap();
            assert_ne!(result.is_error, Some(true));
            let text = server.await.unwrap();
            assert!(text.starts_with("POST /api/mcp-stdio/start-workspace "));
            let posted: serde_json::Value =
                serde_json::from_str(text.split_once("\r\n\r\n").unwrap().1).unwrap();
            assert_eq!(posted["request_id"], request_id.to_string());
            assert_eq!(posted["issue_id"], issue.to_string());
            assert_eq!(posted["request"]["prompt"], "");
            assert!(posted["request"]["linked_issue"].is_null());
            assert_eq!(posted["project_id"], project.to_string());
            if !attachments.is_empty() {
                assert_eq!(
                    posted["request"]["attachment_ids"],
                    serde_json::json!(attachments)
                );
            }
        }
    }
}
