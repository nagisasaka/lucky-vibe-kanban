use db::models::session::Session;
use executors::{
    profile::ExecutorConfig,
    runtime::{AgentRunPortSnapshot, AgentRunStatus, RunState},
};
use rmcp::{
    ErrorData, handler::server::wrapper::Parameters, model::CallToolResult, schemars, tool,
    tool_router,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::McpServer;

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct CreateSessionRequest {
    #[schemars(
        description = "Workspace ID to create the session in. Optional when running inside a scoped orchestrator MCP."
    )]
    workspace_id: Option<Uuid>,
    #[schemars(description = "Optional executor to pin this session to")]
    executor: Option<String>,
    #[schemars(description = "Optional display name for the session")]
    name: Option<String>,
}

#[derive(Debug, Serialize)]
struct CreateSessionPayload {
    workspace_id: Uuid,
    executor: Option<String>,
    name: Option<String>,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
struct SessionSummary {
    #[schemars(description = "Session ID")]
    id: String,
    #[schemars(description = "Workspace ID")]
    workspace_id: String,
    #[schemars(description = "Session display name (if set)")]
    name: Option<String>,
    #[schemars(description = "Session executor (if set)")]
    executor: Option<String>,
    #[schemars(description = "Creation timestamp")]
    created_at: String,
    #[schemars(description = "Last update timestamp")]
    updated_at: String,
    #[schemars(description = "True if this is the orchestrator session for this MCP server")]
    is_orchestrator_session: bool,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
struct CreateSessionResponse {
    session: SessionSummary,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct ListSessionsRequest {
    #[schemars(
        description = "Workspace ID to inspect. Optional when running inside a scoped orchestrator MCP."
    )]
    workspace_id: Option<Uuid>,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
struct ListSessionsResponse {
    #[schemars(description = "Workspace ID this result is scoped to")]
    workspace_id: String,
    total_count: usize,
    sessions: Vec<SessionSummary>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct RunCodingAgentInSessionRequest {
    project_id: Option<Uuid>,
    request_id: Option<Uuid>,
    #[serde(default)]
    attachment_ids: Vec<Uuid>,
    #[schemars(description = "Session ID to run the coding agent in")]
    session_id: Uuid,
    #[schemars(description = "Prompt for the coding agent")]
    prompt: String,
}

#[derive(Debug, Serialize)]
struct FollowUpPayload {
    prompt: String,
    executor_config: ExecutorConfig,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
struct RunCodingAgentInSessionResponse {
    session_id: String,
    agent_run_id: String,
    status: String,
    is_finished: bool,
    state: serde_json::Value,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct UpdateSessionRequest {
    #[schemars(description = "Session ID to update")]
    session_id: Uuid,
    #[schemars(description = "Set session display name (empty string clears it)")]
    name: Option<String>,
}

#[derive(Debug, Serialize)]
struct UpdateSessionPayload {
    name: Option<String>,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
struct UpdateSessionResponse {
    success: bool,
    session_id: String,
    name: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct GetExecutionRequest {
    #[schemars(description = "Execution ID to inspect")]
    execution_id: Uuid,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
struct GetExecutionResponse {
    execution_id: String,
    session_id: String,
    status: String,
    is_finished: bool,
    execution: serde_json::Value,
    #[schemars(
        description = "Latest immutable launch configuration for this session, including execution_mode; may differ from an older requested run."
    )]
    #[schemars(with = "Option<serde_json::Value>")]
    session_executor_config: Option<ExecutorConfig>,
    #[schemars(description = "Final assistant message/summary when execution has finished")]
    final_message: Option<String>,
}

#[tool_router(router = session_tools_router, vis = "pub")]
impl McpServer {
    #[tool(description = "Create a new session in a workspace.")]
    async fn create_session(
        &self,
        Parameters(CreateSessionRequest {
            workspace_id,
            executor,
            name,
        }): Parameters<CreateSessionRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        let workspace_id = match self.resolve_workspace_id(workspace_id) {
            Ok(id) => id,
            Err(error_result) => return Ok(Self::tool_error(error_result)),
        };
        if let Err(error_result) = self.scope_allows_workspace(workspace_id) {
            return Ok(Self::tool_error(error_result));
        }

        let payload = CreateSessionPayload {
            workspace_id,
            executor: executor.and_then(|value| {
                let trimmed = value.trim();
                if trimmed.is_empty() {
                    None
                } else {
                    Some(trimmed.to_string())
                }
            }),
            name: name.and_then(|value| {
                let trimmed = value.trim();
                if trimmed.is_empty() {
                    None
                } else {
                    Some(trimmed.to_string())
                }
            }),
        };

        let url = self.url("/api/sessions");
        let session: Session = match self.send_json(self.client.post(&url).json(&payload)).await {
            Ok(value) => value,
            Err(error_result) => return Ok(Self::tool_error(error_result)),
        };

        Self::success(&CreateSessionResponse {
            session: self.session_summary(session),
        })
    }

    #[tool(description = "List all sessions for a workspace.")]
    async fn list_sessions(
        &self,
        Parameters(ListSessionsRequest { workspace_id }): Parameters<ListSessionsRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        let workspace_id = match self.resolve_workspace_id(workspace_id) {
            Ok(id) => id,
            Err(error_result) => return Ok(Self::tool_error(error_result)),
        };
        if let Err(error_result) = self.scope_allows_workspace(workspace_id) {
            return Ok(Self::tool_error(error_result));
        }

        let url = self.url(&format!("/api/sessions?workspace_id={workspace_id}"));
        let sessions: Vec<Session> = match self.send_json(self.client.get(&url)).await {
            Ok(value) => value,
            Err(error_result) => return Ok(Self::tool_error(error_result)),
        };

        let sessions = sessions
            .into_iter()
            .map(|session| self.session_summary(session))
            .collect::<Vec<_>>();

        Self::success(&ListSessionsResponse {
            workspace_id: workspace_id.to_string(),
            total_count: sessions.len(),
            sessions,
        })
    }

    #[tool(description = "Update a session's name. `session_id` is required.")]
    async fn update_session(
        &self,
        Parameters(UpdateSessionRequest { session_id, name }): Parameters<UpdateSessionRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        // Verify session exists and check scope
        let session_url = self.url(&format!("/api/sessions/{session_id}"));
        let session: Session = match self.send_json(self.client.get(&session_url)).await {
            Ok(value) => value,
            Err(error_result) => return Ok(Self::tool_error(error_result)),
        };
        if let Err(error_result) = self.scope_allows_workspace(session.workspace_id) {
            return Ok(Self::tool_error(error_result));
        }

        let payload = UpdateSessionPayload {
            name: name.map(|value| value.trim().to_string()),
        };
        let url = self.url(&format!("/api/sessions/{session_id}"));
        let updated: Session = match self.send_json(self.client.put(&url).json(&payload)).await {
            Ok(value) => value,
            Err(error_result) => return Ok(Self::tool_error(error_result)),
        };

        Self::success(&UpdateSessionResponse {
            success: true,
            session_id: updated.id.to_string(),
            name: updated.name,
        })
    }

    #[tool(
        description = "Run a coding agent turn in an existing session and return immediately with the execution process."
    )]
    async fn run_session_prompt(
        &self,
        Parameters(RunCodingAgentInSessionRequest {
            session_id,
            prompt,
            project_id,
            request_id,
            attachment_ids,
        }): Parameters<RunCodingAgentInSessionRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        let prompt = prompt.trim();
        if prompt.is_empty() {
            return Self::err("prompt must not be empty", None);
        }

        let session_url = self.url(&format!("/api/sessions/{session_id}"));
        let session: Session = match self.send_json(self.client.get(&session_url)).await {
            Ok(value) => value,
            Err(error_result) => return Ok(Self::tool_error(error_result)),
        };
        if let Err(error_result) = self.scope_allows_workspace(session.workspace_id) {
            return Ok(Self::tool_error(error_result));
        }
        if self.orchestrator_session_id() == Some(session_id) {
            return Self::err(
                "Cannot run coding agent in the orchestrator session".to_string(),
                Some(
                    "Create or re-use a different session and run the coding agent there."
                        .to_string(),
                ),
            );
        }

        let executor_config = match self.executor_config_for_session(&session).await {
            Ok(Some(config)) => config,
            Ok(None) => {
                let executor = match Self::normalize_executor_name(session.executor.as_deref())
                    .and_then(|name| Self::parse_executor_agent(&name))
                {
                    Ok(executor) => executor,
                    Err(error_result) => return Ok(Self::tool_error(error_result)),
                };
                ExecutorConfig::new(executor)
            }
            Err(error_result) => return Ok(Self::tool_error(error_result)),
        };

        if !attachment_ids.is_empty() {
            let (Some(project_id), Some(request_id)) = (project_id, request_id) else {
                return Self::err(
                    "Attachments require project_id and request_id",
                    None::<&str>,
                );
            };
            let url = self.url("/api/attachment-transfer/continue");
            return match self.send_json::<serde_json::Value>(self.client.post(url).json(&serde_json::json!({"project_id":project_id,"request_id":request_id,"workspace_id":session.workspace_id,"session_id":session_id,"attachment_ids":attachment_ids,"prompt":prompt}))).await {
                Ok(value)=>Self::success(&value),Err(error)=>Ok(Self::tool_error(error)),
            };
        }
        let payload = FollowUpPayload {
            prompt: prompt.to_string(),
            executor_config,
        };

        let url = self.url(&format!("/api/sessions/{session_id}/follow-up"));
        let snapshot: AgentRunPortSnapshot =
            match self.send_json(self.client.post(&url).json(&payload)).await {
                Ok(value) => value,
                Err(error_result) => return Ok(Self::tool_error(error_result)),
            };
        let state = snapshot.state;

        Self::success(&RunCodingAgentInSessionResponse {
            session_id: session_id.to_string(),
            agent_run_id: snapshot.agent_run_id.to_string(),
            status: Self::agent_run_status_label(state.status).to_string(),
            is_finished: state.status.is_terminal(),
            state: serde_json::to_value(&state).unwrap_or(serde_json::Value::Null),
        })
    }

    #[tool(description = "Get canonical status for an AgentRun.")]
    async fn get_execution(
        &self,
        Parameters(GetExecutionRequest { execution_id }): Parameters<GetExecutionRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        let run_url = self.url(&format!("/api/agent-runs/{execution_id}"));
        let state: RunState = match self.send_json(self.client.get(&run_url)).await {
            Ok(value) => value,
            Err(error_result) => return Ok(Self::tool_error(error_result)),
        };

        let session_url = self.url(&format!("/api/sessions/{}", state.session_id));
        let session: Session = match self.send_json(self.client.get(&session_url)).await {
            Ok(value) => value,
            Err(error_result) => return Ok(Self::tool_error(error_result)),
        };
        if let Err(error_result) = self.scope_allows_workspace(session.workspace_id) {
            return Ok(Self::tool_error(error_result));
        }

        let session_executor_config = match self.executor_config_for_session(&session).await {
            Ok(config) => config,
            Err(error_result) => return Ok(Self::tool_error(error_result)),
        };

        Self::success(&GetExecutionResponse {
            execution_id: state.agent_run_id.to_string(),
            session_id: state.session_id.to_string(),
            status: Self::agent_run_status_label(state.status).to_string(),
            is_finished: state.status.is_terminal(),
            execution: serde_json::to_value(&state).unwrap_or(serde_json::Value::Null),
            session_executor_config,
            final_message: state
                .terminal_output
                .as_ref()
                .map(|message| message.content.clone()),
        })
    }
}

impl McpServer {
    async fn executor_config_for_session(
        &self,
        session: &Session,
    ) -> Result<Option<ExecutorConfig>, super::ToolError> {
        let url = self.url(&format!("/api/sessions/{}/executor-config", session.id));
        // A successful new Session returns data:null. The generic send_json
        // helper treats null as a missing payload, so decode this nullable API
        // explicitly and reject missing/malformed data rather than losing REVIEW.
        let response = self
            .client
            .get(&url)
            .send()
            .await
            .and_then(reqwest::Response::error_for_status)
            .map_err(|error| {
                super::ToolError::message(format!(
                    "Failed to read Session executor config: {error}"
                ))
            })?
            .json::<serde_json::Value>()
            .await
            .map_err(|error| {
                super::ToolError::message(format!(
                    "Invalid Session executor config response: {error}"
                ))
            })?;
        if response["success"] != true {
            return Err(super::ToolError::message(
                "Failed to read Session executor config",
            ));
        }
        let data = response.get("data").ok_or_else(|| {
            super::ToolError::message("Session executor config response missing data")
        })?;
        serde_json::from_value(data.clone()).map_err(|error| {
            super::ToolError::message(format!("Invalid Session executor config: {error}"))
        })
    }

    fn session_summary(&self, session: Session) -> SessionSummary {
        let is_orchestrator_session = self.orchestrator_session_id() == Some(session.id);
        SessionSummary {
            id: session.id.to_string(),
            workspace_id: session.workspace_id.to_string(),
            name: session.name,
            executor: session.executor,
            created_at: session.created_at.to_rfc3339(),
            updated_at: session.updated_at.to_rfc3339(),
            is_orchestrator_session,
        }
    }

    fn agent_run_status_label(status: AgentRunStatus) -> &'static str {
        match status {
            AgentRunStatus::Pending => "pending",
            AgentRunStatus::Starting => "starting",
            AgentRunStatus::Running => "running",
            AgentRunStatus::AwaitingInput => "awaiting_input",
            AgentRunStatus::AwaitingApproval => "awaiting_approval",
            AgentRunStatus::Cancelling => "cancelling",
            AgentRunStatus::Succeeded => "succeeded",
            AgentRunStatus::Failed => "failed",
            AgentRunStatus::Cancelled => "cancelled",
            AgentRunStatus::Crashed => "crashed",
            AgentRunStatus::AuditFailed => "audit_failed",
        }
    }
}

#[cfg(test)]
mod review_tests {
    use serde_json::json;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    use super::*;

    async fn api_fixture(
        responses: Vec<(u16, serde_json::Value)>,
    ) -> (String, tokio::task::JoinHandle<Vec<String>>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let task = tokio::spawn(async move {
            let mut requests = Vec::new();
            for (status, body) in responses {
                let (mut socket, _) = listener.accept().await.unwrap();
                let mut request = Vec::new();
                loop {
                    let mut bytes = [0; 4096];
                    let n = socket.read(&mut bytes).await.unwrap();
                    assert!(n > 0);
                    request.extend_from_slice(&bytes[..n]);
                    let text = String::from_utf8_lossy(&request);
                    if let Some(end) = text.find("\r\n\r\n") {
                        let length = text[..end]
                            .lines()
                            .find_map(|line| {
                                line.to_lowercase()
                                    .strip_prefix("content-length:")
                                    .map(|n| n.trim().parse::<usize>().unwrap())
                            })
                            .unwrap_or(0);
                        if request.len() >= end + 4 + length {
                            break;
                        }
                    }
                }
                requests.push(String::from_utf8(request).unwrap());
                let body = body.to_string();
                socket.write_all(format!("HTTP/1.1 {status} Fixture\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).as_bytes()).await.unwrap();
            }
            requests
        });
        (url, task)
    }

    fn session() -> Session {
        serde_json::from_value(json!({"id":Uuid::new_v4(),"workspace_id":Uuid::new_v4(),"name":null,"executor":"CODEX","created_at":"2026-10-29T00:00:00Z","updated_at":"2026-10-29T00:00:00Z"})).unwrap()
    }

    #[tokio::test]
    async fn review_followup_reads_and_preserves_saved_configuration() {
        let session = session();
        let config = json!({"executor":"CODEX","execution_mode":"review","permission_policy":"SUPERVISED","variant":"DEFAULT","model_id":"fixture-model"});
        // Deliberately reject after the POST: no real provider is started.
        let (url, task) = api_fixture(vec![
            (200, json!({"success":true,"data":session})),
            (200, json!({"success":true,"data":config})),
            (409, json!({})),
        ])
        .await;
        let result = McpServer::new_global(&url)
            .run_session_prompt(Parameters(RunCodingAgentInSessionRequest {
                session_id: session.id,
                prompt: "Investigate and report".into(),
            }))
            .await
            .unwrap();
        assert_eq!(result.is_error, Some(true));
        let requests = task.await.unwrap();
        assert!(requests[1].starts_with(&format!(
            "GET /api/sessions/{}/executor-config ",
            session.id
        )));
        assert!(requests[2].starts_with(&format!("POST /api/sessions/{}/follow-up ", session.id)));
        let posted: serde_json::Value =
            serde_json::from_str(requests[2].split_once("\r\n\r\n").unwrap().1).unwrap();
        assert_eq!(posted["executor_config"], config);
    }

    #[tokio::test]
    async fn nullable_config_allows_new_sessions_but_rejects_missing_or_invalid_snapshots() {
        for (data, valid) in [
            (json!({"success":true,"data":null}), true),
            (json!({"success":true}), false),
            (
                json!({"success":true,"data":{"executor":"CODEX","execution_mode":"unknown"}}),
                false,
            ),
            (json!({"success":false,"data":null}), false),
        ] {
            let (url, task) = api_fixture(vec![(200, data)]).await;
            let result = McpServer::new_global(&url)
                .executor_config_for_session(&session())
                .await;
            assert_eq!(result.is_ok(), valid);
            if valid {
                assert!(result.unwrap().is_none());
            }
            task.await.unwrap();
        }
    }
}
