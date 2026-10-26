//! MCP request types for passive official provider account reads.
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Provider {
    Codex,
    ClaudeCode,
}
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AccountContext {
    ServerDefault,
}
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub provider: Provider,
    /// Explicit server-managed DEFAULT launcher/auth context; administrator or authenticated LVK internal agent.
    /// Does not imply the credentials or environment of any Project/Workspace.
    pub account_context: AccountContext,
    #[serde(default)]
    pub include_usage: bool,
    /// Optional existing LVK Session; never accepts a native thread/session ID.
    pub session_id: Option<Uuid>,
    pub workspace_id: Option<Uuid>,
    pub project_id: Option<Uuid>,
}
