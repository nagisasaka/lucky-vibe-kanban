//! Git identity checks and immutable initial worker dispatch. No merge planner.
use std::{path::Path, process::Command};

use anyhow::{Context, ensure};
use db::models::integration::{IntegrationRun, IntegrationValidationSuggestion};
use executors::{
    executors::{BaseCodingAgent, provider_adapter::DirectProvider},
    profile::ExecutorConfig,
    provider_policy::direct_provider_capability_snapshot,
    runtime::*,
};
use uuid::Uuid;

pub(super) fn git(root: &Path, args: &[&str]) -> anyhow::Result<String> {
    let result = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()?;
    ensure!(
        result.status.success(),
        "Git operation failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    Ok(String::from_utf8(result.stdout)?.trim().to_owned())
}

pub(super) fn require_private_branch(
    root: &Path,
    branch: &str,
    target: &str,
) -> anyhow::Result<()> {
    let expected = format!("refs/heads/{branch}");
    let observed = git(root, &["symbolic-ref", "HEAD"])
        .unwrap_or_else(|error| format!("unavailable: {error}"));
    ensure!(
        expected != target && observed == expected,
        "Auto-merge worktree is not on its private branch; refusing a Git mutation: path={}, expected={}, observed={}, target={}",
        root.display(),
        expected,
        observed,
        target
    );

    Ok(())
}

pub(super) fn validation_suggestions(
    root: &Path,
    base: &str,
) -> anyhow::Result<Vec<IntegrationValidationSuggestion>> {
    let text = git(
        root,
        &["show", &format!("{base}:.lvk/integration-validation.json")],
    )
    .context("No validation suggestions on the selected target")?;
    super::super::integration_validation::parse_commands(&text)?
        .into_iter()
        .map(|command| {
            Ok(IntegrationValidationSuggestion {
                command,
                cwd: ".".into(),
                evidence: format!("{base}:.lvk/integration-validation.json"),
                environment_requirements:
                    "Repository-configured environment; missing prerequisites fail validation"
                        .into(),
            })
        })
        .collect()
}

/// Shared UI/MCP input: the immutable dispatch retains this exact text. Full
/// adopted reference material stays in the existing payload, readable by inspect.
pub(super) fn worker_content(run: &IntegrationRun, identity: &str) -> anyhow::Result<String> {
    let targets = run
        .payload
        .sources
        .iter()
        .map(|source| {
            serde_json::json!({
                "issue_id":source.selection.card_id,"workspace_id":source.selection.workspace_id,
                "commit":source.commit,"branch":source.branch,"title":source.title
            })
        })
        .collect::<Vec<_>>();
    let skill = run
        .payload
        .reference_materials
        .as_ref()
        .and_then(|materials| materials.get("auto_merge_skill"))
        .and_then(serde_json::Value::as_str)
        .context("Missing adopted Auto-merge Skill")?;
    Ok(format!(
        "{skill}\n{identity}\n固定対象: {}\n追加指示（依頼経路から保存した原文）:\n{}\n検証省略の指定: {} / 参考検証: {} / OpenWiki有効: {}",
        serde_json::to_string(&targets)?,
        run.payload.instructions,
        run.payload.skip_validation,
        serde_json::to_string(&run.payload.validation_suggestions)?,
        run.payload.wiki_enabled
    ))
}

pub(super) fn worker_request(
    run: &IntegrationRun,
    workspace_path: String,
    content: String,
) -> anyhow::Result<AgentRunPortCommand> {
    let config: ExecutorConfig = serde_json::from_value(
        run.payload
            .executor_config
            .clone()
            .context("Missing worker executor")?,
    )?;
    ensure!(
        config.executor == BaseCodingAgent::Codex,
        "Auto-merge requires Codex"
    );
    let provider = DirectProvider::Codex;
    let now = chrono::Utc::now();
    let id = Uuid::new_v4();
    let session_id = run.session_id.context("Missing session")?;
    let workspace = WorkspaceReference {
        workspace_id: run.workspace_id.context("Missing workspace")?,
        mode: WorkspaceMode::IsolatedWorktree,
        path: workspace_path,
    };
    let profile = config.profile_id().cache_key();
    let request = AgentRunRequestEnvelope {
        schema_version: AGENT_REQUEST_SCHEMA_VERSION,
        payload_version: AGENT_REQUEST_PAYLOAD_VERSION,
        request_id: id,
        idempotency_key: format!("integration:{}:worker:{id}", run.id),
        session_id,
        agent_run_id: id,
        turn_id: Uuid::new_v4(),
        correlation_id: run.id,
        intent: AgentRunIntent::Initial,
        runtime_profile_id: profile.clone(),
        provider_id: provider.id().into(),
        workspace: workspace.clone(),
        input: CanonicalMessage {
            message_id: Uuid::new_v4(),
            role: AgentRuntimeMessageRole::User,
            content,
        },
        created_at: now,
    };
    let attempt = RunAttemptRequest {
        schema_version: AGENT_REQUEST_SCHEMA_VERSION,
        payload_version: AGENT_REQUEST_PAYLOAD_VERSION,
        request_id: id,
        idempotency_key: format!("{}:attempt:1", request.idempotency_key),
        session_id,
        agent_run_id: id,
        turn_id: request.turn_id,
        run_attempt_id: Uuid::new_v4(),
        attempt_number: 1,
        correlation_id: run.id,
        mode: RunAttemptMode::Launch,
        transport: provider.transport(),
        runtime_profile_id: profile.clone(),
        provider_id: provider.id().into(),
        workspace,
        capability_snapshot: direct_provider_capability_snapshot(provider, profile),
        executor_config: config,
        selected_skills: None,
        reset_to_message_id: None,
        provider_session: None,
        created_at: now,
    };
    Ok(AgentRunPortCommand::Create { request, attempt })
}
