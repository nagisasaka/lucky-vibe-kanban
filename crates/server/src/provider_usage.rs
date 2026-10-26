//! Shared stdio/HTTP MCP account read boundary. No execution records or logs.
use anyhow::ensure;
pub use executors::account_usage::{Provider, Request};
use executors::{
    executors::{BaseCodingAgent, CodingAgent},
    profile::{ExecutorConfigs, ExecutorProfileId},
};
use serde_json::{Value, json};
use sqlx::SqlitePool;

pub async fn read(pool: &SqlitePool, request: Request, authorized: bool) -> anyhow::Result<Value> {
    ensure!(authorized, "Authorized server account context required");
    let base = match request.provider {
        Provider::Codex => BaseCodingAgent::Codex,
        Provider::ClaudeCode => BaseCodingAgent::ClaudeCode,
    };
    let profile = ExecutorProfileId::new(base);
    let native_id = resolve_native(pool, &request, &profile).await?;
    let agent = ExecutorConfigs::get_cached()
        .get_coding_agent(&profile)
        .ok_or_else(|| anyhow::anyhow!("Server default provider profile unavailable"))?;
    let scope = if native_id.is_some() {
        "account_quota_and_existing_session_usage"
    } else {
        "account"
    };
    let result = match agent {
        CodingAgent::Codex(agent) => agent.read_account(native_id, request.include_usage).await,
        CodingAgent::ClaudeCode(agent) => {
            agent
                .read_account_usage(native_id.is_some(), request.include_usage)
                .await
        }
        _ => anyhow::bail!("Provider profile mismatch"),
    };
    Ok(
        json!({"provider":request.provider,"account_context":"server_default","scope":scope,"source":"official_cli_control","result":result,"semantics":{"observedAt":"time observed, not provider accounting time or immediate posting guarantee","quota":"account scope; never summed across workers","usage":"cumulative provider snapshot, not added to Workspace tokens; parent/child inclusion unknown","money":"provider estimates; micros are millionths; not subscription invoice amounts","remaining":"if displayed, derived as 100 minus matching usedPercent"}}),
    )
}

pub(crate) async fn resolve_native(
    pool: &SqlitePool,
    request: &Request,
    profile: &ExecutorProfileId,
) -> anyhow::Result<Option<String>> {
    let native_id = match (request.session_id, request.workspace_id, request.project_id) {
        (None, None, None) => None,
        (Some(session), Some(workspace), Some(project)) => {
            ensure!(request.include_usage, "Session requires include_usage=true");
            let row: Option<(String,String)> = sqlx::query_as("SELECT p.provider_session_id,p.runtime_profile_id FROM agent_provider_sessions p JOIN sessions s ON s.id=p.session_id JOIN local_workspace_links l ON l.workspace_id=s.workspace_id JOIN local_issues i ON i.id=l.issue_id WHERE s.id=? AND s.workspace_id=? AND l.project_id=? AND i.project_id=? AND p.provider_id=?")
                .bind(session).bind(workspace).bind(project).bind(project)
                .bind(match request.provider {Provider::Codex=>"codex",Provider::ClaudeCode=>"claude_code"})
                .fetch_optional(pool).await?;
            let (native, runtime) = row.ok_or_else(|| {
                anyhow::anyhow!("Session does not have a matching authorized provider reference")
            })?;
            ensure!(
                executors::profile::runtime_profile_ids_match(&runtime, &profile.to_string()),
                "Session uses a different launcher context"
            );
            ensure!(
                !native.trim().is_empty() && native.len() <= 512,
                "Invalid stored native session reference"
            );
            Some(native)
        }
        _ => anyhow::bail!(
            "Supply project_id, workspace_id and session_id together for existing-session usage"
        ),
    };
    Ok(native_id)
}

/// Internal is an authenticated LVK server capability, never a caller field or
/// loopback assumption. It permits this read but cannot manage authentication.
pub(crate) fn allows_server_context(admin: bool, channel: &str, read_scope: bool) -> bool {
    read_scope && (admin || channel == "internal")
}
#[cfg(test)]
mod authorization_tests {
    #[test]
    fn provider_usage_server_context_is_explicitly_authorized() {
        assert!(super::allows_server_context(false, "internal", true));
        assert!(super::allows_server_context(true, "mcp", true));
        assert!(!super::allows_server_context(false, "mcp", true));
        assert!(!super::allows_server_context(false, "web", true));
        assert!(!super::allows_server_context(true, "internal", false));
    }
}
