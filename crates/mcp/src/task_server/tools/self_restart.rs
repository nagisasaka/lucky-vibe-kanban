//! Thin local stdio delegation to the existing durable switch engine.
use rmcp::{
    ErrorData, handler::server::wrapper::Parameters, model::CallToolResult, schemars, tool,
    tool_router,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use uuid::Uuid;

use super::McpServer;

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct Restart {
    project_id: Uuid,
    request_id: Uuid,
    purpose: String,
    bundle: String,
    no_new_work: bool,
}
#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct Status {
    project_id: Uuid,
    request_id: Uuid,
}

#[tool_router(router = self_restart_tools_router, vis = "pub")]
impl McpServer {
    #[tool(
        description = "Explicit developer-instance self restart through the installed common engine using local UID/instance trust, not an OAuth principal. Requires user authorisation, no_new_work coordination and enabled runtime execution. After durable admission end this Codex Run so its children/writers can settle; admission is not restart success. The engine waits 30 seconds and rechecks once only if its initial preflight is busy. Query get_self_restart_operation with the same request_id after reconnecting. Never substitute run_process or kill."
    )]
    async fn self_restart(
        &self,
        Parameters(a): Parameters<Restart>,
    ) -> Result<CallToolResult, ErrorData> {
        self.restart_via_engine("self_restart", json!(a)).await
    }
    #[tool(
        description = "Read the existing durable self restart receipt/result using the original project_id and request_id. Does not restart, retry or approve anything; admission is separate from successful cutover."
    )]
    async fn get_self_restart_operation(
        &self,
        Parameters(a): Parameters<Status>,
    ) -> Result<CallToolResult, ErrorData> {
        self.restart_via_engine("get_self_restart_operation", json!(a))
            .await
    }
}
impl McpServer {
    async fn restart_via_engine(
        &self,
        name: &str,
        input: Value,
    ) -> Result<CallToolResult, ErrorData> {
        let request_id = input
            .get("request_id")
            .and_then(Value::as_str)
            .and_then(|s| Uuid::parse_str(s).ok());
        match self.invoke_restart(name, input).await {
            Ok(value)
                if matches!(
                    value["status"].as_str(),
                    Some("refused" | "failed" | "interrupted")
                ) =>
            {
                // Failed cutover may have partial effects. Preserve the terminal
                // engine fact separately from uncertainty of external state.
                let unknown = value["status"] != "refused";
                let diagnostic = utils::mcp_error::Diagnostic::new(
                    if unknown {
                        super::Category::OutcomeUnknown
                    } else {
                        super::Category::Conflict
                    },
                    if unknown {
                        super::Outcome::Unknown
                    } else {
                        super::Outcome::Failed
                    },
                );
                let safe =
                    json!({"status":value["status"],"request_id":request_id,"error":diagnostic});
                let mut response =
                    CallToolResult::error(vec![rmcp::model::Content::text(safe.to_string())]);
                response.structured_content = Some(safe);
                Ok(response)
            }
            Ok(value) => Self::success(&value),
            Err(_) => Ok(Self::tool_error(super::ToolError::classified(
                super::Category::Unavailable,
                if name == "self_restart" {
                    super::Outcome::Unknown
                } else {
                    super::Outcome::Failed
                },
            ))),
        }
    }

    async fn invoke_restart(&self, name: &str, input: Value) -> anyhow::Result<Value> {
        use anyhow::{Context, ensure};
        let project: Uuid = serde_json::from_value(input["project_id"].clone())?;
        if let Some(bound) = self.context.as_ref().and_then(|c| c.project_id) {
            ensure!(project == bound, "Project is outside stdio context");
        }
        let backend = reqwest::Url::parse(&self.base_url)?;
        ensure!(
            backend.scheme() == "http"
                && backend.username().is_empty()
                && backend.password().is_none()
                && backend.query().is_none()
                && backend.fragment().is_none()
                && matches!(
                    backend.host_str(),
                    Some("127.0.0.1" | "localhost" | "[::1]")
                ),
            "Local stdio instance required"
        );
        let instance =
            utils::instance::instance_dir_read_only()?.context("Developer instance required")?;
        let engine = instance.join("switch/start_binary.py");
        ensure!(
            engine.canonicalize()? == engine,
            "Fixed engine cannot be aliased"
        );
        let request: Uuid = serde_json::from_value(input["request_id"].clone())?;
        let mut command = tokio::process::Command::new("/usr/bin/python3");
        command
            .arg(engine)
            .arg("--instance")
            .arg(instance)
            .arg("--stdio-project")
            .arg(project.to_string())
            .arg("--stdio-port")
            .arg(
                backend
                    .port_or_known_default()
                    .context("Backend port missing")?
                    .to_string(),
            )
            .arg("--request-id")
            .arg(request.to_string());
        if let Some(context) = &self.context {
            command
                .arg("--stdio-workspace")
                .arg(context.workspace_id.to_string());
        }
        match name {
            "self_restart" => {
                let a: Restart = serde_json::from_value(input)?;
                ensure!(
                    a.no_new_work
                        && !a.purpose.trim().is_empty()
                        && a.purpose.len() <= 1000
                        && std::path::Path::new(&a.bundle).is_absolute(),
                    "Explicit purpose, bundle and coordination required"
                );
                command.args([
                    "--async",
                    "--no-new-work",
                    "--purpose",
                    &a.purpose,
                    "--bundle",
                    &a.bundle,
                ]);
            }
            "get_self_restart_operation" => {
                let _: Status = serde_json::from_value(input)?;
                command.arg("--status");
            }
            _ => anyhow::bail!("Unsupported restart action"),
        }
        command
            .env_clear()
            .env("PATH", "/usr/local/bin:/usr/bin:/bin")
            .env("LANG", "C.UTF-8")
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .kill_on_drop(true);
        // Only admission/status is awaited. The existing detached worker owns the
        // durable lock and waits for this caller's writers to settle independently.
        let output =
            tokio::time::timeout(std::time::Duration::from_secs(15), command.output()).await??;
        ensure!(output.stdout.len() <= 65536, "Engine response too large");
        let value: Value = serde_json::from_slice(&output.stdout)?;
        ensure!(
            value.is_object()
                && (output.status.success()
                    || matches!(
                        value["status"].as_str(),
                        Some("refused" | "failed" | "interrupted")
                    )),
            "Engine response unavailable"
        );
        Ok(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stdio_cannot_override_local_authority_or_owner() {
        let request = json!({"project_id":Uuid::new_v4(),"request_id":Uuid::new_v4(),"purpose":"fixture","bundle":"/bundle","no_new_work":true});
        assert!(serde_json::from_value::<Restart>(request.clone()).is_ok());
        for field in [
            "instance",
            "owner",
            "engine",
            "stdio_project",
            "stdio_workspace",
            "snapshot",
        ] {
            let mut invalid = request.clone();
            invalid[field] = json!("override");
            assert!(serde_json::from_value::<Restart>(invalid).is_err());
        }
        for router in [
            McpServer::global_mode_router(),
            McpServer::orchestrator_mode_router(),
        ] {
            let tools = router.list_all();
            for name in ["self_restart", "get_self_restart_operation"] {
                assert!(tools.iter().any(|t| t.name == name));
            }
        }
    }
}
