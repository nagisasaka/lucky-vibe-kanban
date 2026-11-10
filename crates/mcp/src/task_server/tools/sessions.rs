use db::models::session::Session;
use executors::{
    profile::{ExecutorConfig, NextRunOverrides},
    runtime::{AgentRunStatus, RunState},
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
    request_id: Option<Uuid>,
    project_id: Option<Uuid>,
    #[schemars(
        description = "Workspace ID to create the session in. Optional when current Workspace context is available."
    )]
    workspace_id: Option<Uuid>,
    #[schemars(
        description = "Engine for this new conversation (for example CODEX or CLAUDE_CODE). Omit for existing default selection. Reuses Workspace code, not another provider native conversation."
    )]
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
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct ListSessionsRequest {
    #[schemars(
        description = "Workspace ID to inspect. Optional when current Workspace context is available."
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
    #[schemars(
        description = "Optional caller-selected latest finished Run. Omitted selects the current Run at admission and gives no caller-observed Run guarantee."
    )]
    agent_run_id: Option<Uuid>,
    #[serde(default)]
    #[schemars(
        description = "Apply only to the next Run. Omitted fields preserve saved settings; use action reset explicitly. Null is invalid."
    )]
    overrides: NextRunOverrides,
    #[serde(default)]
    attachment_ids: Vec<Uuid>,
    #[schemars(description = "Session ID to run the coding agent in")]
    session_id: Uuid,
    #[schemars(description = "Prompt for the coding agent")]
    prompt: String,
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
struct GetSessionExecutionSettingsRequest {
    #[schemars(description = "Session whose saved settings and capabilities should be inspected")]
    session_id: Uuid,
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
    #[tool(
        description = "Create a new conversation. Optional request_id gives durable replay protection (requires a Project context or linked Workspace); omission gives none. Read get_operation after response loss. In an existing workspace, optionally selecting its engine. Does not inherit another Session native conversation; include relevant goals, decisions and remaining work in its first prompt."
    )]
    async fn create_session(
        &self,
        Parameters(CreateSessionRequest {
            workspace_id,
            executor,
            name,
            request_id,
            project_id,
        }): Parameters<CreateSessionRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        let workspace_id = match self.resolve_workspace_id(workspace_id) {
            Ok(id) => id,
            Err(error_result) => return Ok(Self::tool_error(error_result)),
        };

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

        let mut body = serde_json::to_value(&payload).expect("Session input");
        body["request_id"] = serde_json::json!(request_id);
        body["project_id"] = serde_json::json!(
            project_id.or_else(|| self.context.as_ref().and_then(|c| c.project_id))
        );
        match self
            .send_json::<serde_json::Value>(
                self.client
                    .post(self.url("/api/mcp-stdio/create-session"))
                    .json(&body),
            )
            .await
        {
            Ok(value) => Self::operation_result(value),
            Err(error) => Ok(Self::tool_error(error)),
        }
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
        let _session: Session = match self.send_json(self.client.get(&session_url)).await {
            Ok(value) => value,
            Err(error_result) => return Ok(Self::tool_error(error_result)),
        };

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
        description = "Run a turn in an existing Session. Optional request_id provides durable replay protection independently of explicit Run/overrides/attachments; omission provides none. Continue requires a latest finished Run; use steering for running execution. To change next-Run mode/reasoning, supply overrides; explicitly pin agent_run_id for caller-observed latest-Run protection. Project context is required for durable receipts. Omission preserves saved settings. Engine changes require create_session; REVIEW grants no additional permissions."
    )]
    async fn run_session_prompt(
        &self,
        Parameters(RunCodingAgentInSessionRequest {
            session_id,
            prompt,
            project_id,
            request_id,
            attachment_ids,
            agent_run_id,
            overrides,
        }): Parameters<RunCodingAgentInSessionRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        let prompt = prompt.trim();
        if prompt.is_empty() {
            return Self::err("prompt must not be empty", None);
        }

        let body = serde_json::json!({"project_id":project_id.or_else(||self.context.as_ref().and_then(|c|c.project_id)),"request_id":request_id,"agent_run_id":agent_run_id,"session_id":session_id,"prompt":prompt,"attachment_ids":attachment_ids,"overrides":overrides});
        match self
            .send_json::<serde_json::Value>(
                self.client
                    .post(self.url("/api/mcp-stdio/continue"))
                    .json(&body),
            )
            .await
        {
            Ok(mut value) => {
                if value["status"] == "succeeded"
                    && let Some(id) = value["agent_run_id"].as_str()
                {
                    let state: RunState = match self
                        .send_json(self.client.get(self.url(&format!("/api/agent-runs/{id}"))))
                        .await
                    {
                        Ok(state) => state,
                        Err(mut e) => {
                            e.diagnostic.outcome = utils::mcp_error::Outcome::Unknown;
                            return Ok(Self::tool_error(e));
                        }
                    };
                    value["operation_status"] = value["status"].clone();
                    value["status"] = serde_json::json!(Self::agent_run_status_label(state.status));
                    value["is_finished"] = serde_json::json!(state.status.is_terminal());
                    value["state"] = serde_json::to_value(state).expect("Run state");
                    return Self::success(&value);
                }
                Self::operation_result(value)
            }
            Err(error) => Ok(Self::tool_error(error)),
        }
    }

    #[tool(
        description = "Read saved launch settings and supported next-Run mode/reasoning choices for a Session, including native-adopted change restrictions. This is configuration/capability evidence, not confirmation of the provider internal adopted value."
    )]
    async fn get_session_execution_settings(
        &self,
        Parameters(GetSessionExecutionSettingsRequest { session_id }): Parameters<
            GetSessionExecutionSettingsRequest,
        >,
    ) -> Result<CallToolResult, ErrorData> {
        let session_url = self.url(&format!("/api/sessions/{session_id}"));
        let _session: Session = match self.send_json(self.client.get(&session_url)).await {
            Ok(value) => value,
            Err(error) => return Ok(Self::tool_error(error)),
        };
        let url = self.url(&format!("/api/sessions/{session_id}/execution-settings"));
        match self
            .send_json::<serde_json::Value>(self.client.get(url))
            .await
        {
            Ok(value) => Self::success(&value),
            Err(error) => Ok(Self::tool_error(error)),
        }
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
        let (response, _) = self.send_response(self.client.get(&url), true).await?;
        let response = response.json::<serde_json::Value>().await.map_err(|_| {
            super::ToolError::classified(super::Category::Internal, super::Outcome::Failed)
        })?;
        if response["success"] != true {
            return Err(super::ToolError::classified(
                super::Category::Internal,
                super::Outcome::Failed,
            ));
        }
        let data = response.get("data").ok_or_else(|| {
            super::ToolError::classified(super::Category::Internal, super::Outcome::Failed)
        })?;
        serde_json::from_value(data.clone()).map_err(|_| {
            super::ToolError::classified(super::Category::Internal, super::Outcome::Failed)
        })
    }

    fn session_summary(&self, session: Session) -> SessionSummary {
        SessionSummary {
            id: session.id.to_string(),
            workspace_id: session.workspace_id.to_string(),
            name: session.name,
            executor: session.executor,
            created_at: session.created_at.to_rfc3339(),
            updated_at: session.updated_at.to_rfc3339(),
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
    async fn continuation_preserves_request_id_with_or_without_explicit_run_overrides_and_attachments()
     {
        for explicit in [false, true] {
            let request = Uuid::new_v4();
            let run = Uuid::new_v4();
            let overrides = if explicit {
                json!({"reasoning_id":{"action":"reset"}})
            } else {
                json!({})
            };
            let attachments = if explicit {
                vec![Uuid::new_v4()]
            } else {
                vec![]
            };
            let (url, task) = api_fixture(vec![(
                200,
                json!({"success":true,"data":{"status":"accepted","request_id":request}}),
            )])
            .await;
            let result = McpServer::new_global(&url)
                .run_session_prompt(Parameters(RunCodingAgentInSessionRequest {
                    session_id: Uuid::new_v4(),
                    project_id: Some(Uuid::new_v4()),
                    request_id: Some(request),
                    agent_run_id: explicit.then_some(run),
                    overrides: serde_json::from_value(overrides.clone()).unwrap(),
                    attachment_ids: attachments.clone(),
                    prompt: "Continue".into(),
                }))
                .await
                .unwrap();
            assert_ne!(result.is_error, Some(true));
            let requests = task.await.unwrap();
            assert_eq!(requests.len(), 1);
            assert!(requests[0].starts_with("POST /api/mcp-stdio/continue "));
            let posted: serde_json::Value =
                serde_json::from_str(requests[0].split_once("\r\n\r\n").unwrap().1).unwrap();
            assert_eq!(posted["request_id"], request.to_string());
            assert_eq!(posted["overrides"], overrides);
            assert_eq!(posted["attachment_ids"], json!(attachments));
        }
    }

    #[tokio::test]
    async fn settings_read_checks_session_and_returns_capability_evidence() {
        let session = session();
        let settings = json!({"session_id":session.id,"provider_adoption":"not_observed","configuration_changes_allowed":false});
        let (url, task) = api_fixture(vec![
            (200, json!({"success":true,"data":session})),
            (200, json!({"success":true,"data":settings})),
        ])
        .await;
        let result = McpServer::new_global(&url)
            .get_session_execution_settings(Parameters(GetSessionExecutionSettingsRequest {
                session_id: session.id,
            }))
            .await
            .unwrap();
        assert_ne!(result.is_error, Some(true));
        let requests = task.await.unwrap();
        assert!(requests[1].starts_with(&format!(
            "GET /api/sessions/{}/execution-settings ",
            session.id
        )));
    }

    #[test]
    fn next_run_overrides_reject_null() {
        let base = json!({"session_id":Uuid::new_v4(),"prompt":"Continue"});
        assert!(serde_json::from_value::<RunCodingAgentInSessionRequest>(base.clone()).is_ok());
        for overrides in [
            json!(null),
            json!({"execution_mode":null}),
            json!({"reasoning_id":null}),
        ] {
            let mut input = base.clone();
            input["overrides"] = overrides;
            assert!(serde_json::from_value::<RunCodingAgentInSessionRequest>(input).is_err());
        }
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
