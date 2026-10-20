//! Integration owns Git progress. Agent execution is optional and only resolves conflicts.
use std::{path::Path, process::Command};

use anyhow::{Context, ensure};
use db::models::integration::{IntegrationRun, IntegrationValidation};
use executors::{
    executors::{BaseCodingAgent, provider_adapter::DirectProvider},
    profile::ExecutorConfig,
    provider_policy::direct_provider_capability_snapshot,
    runtime::*,
};
use serde::Deserialize;
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
    ensure!(
        expected != target && git(root, &["symbolic-ref", "HEAD"])? == expected,
        "Integration worktree is not on its private branch; refusing a Git mutation"
    );
    Ok(())
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ValidationPlan {
    commands: Vec<String>,
}

/// Read configuration from the frozen target, never from an AI response or a moving worktree.
pub(super) fn validation_plan(
    root: &Path,
    base: &str,
    skip: bool,
) -> anyhow::Result<Vec<IntegrationValidation>> {
    if skip {
        return Ok(vec![]);
    }
    let text = git(root, &["show", &format!("{base}:.lvk/integration-validation.json")])
        .context("VALIDATION_UNAVAILABLE: configure .lvk/integration-validation.json on the selected target, or explicitly select skipped validation")?;
    let plan: ValidationPlan = serde_json::from_str(&text)?;
    ensure!(
        !plan.commands.is_empty() && plan.commands.len() <= 32,
        "Validation requires 1–32 commands"
    );
    plan.commands
        .into_iter()
        .map(|command| {
            ensure!(
                !command.trim().is_empty() && !matches!(command.trim(), "true" | ":" | "exit 0"),
                "Empty/placeholder validation command"
            );
            Ok(IntegrationValidation {
                command,
                cwd: ".".into(),
                required: true,
                evidence: format!("{base}:.lvk/integration-validation.json"),
                environment_requirements:
                    "Repository-configured environment; missing prerequisites fail validation"
                        .into(),
                execution_process_id: None,
                exit_code: None,
                result: None,
            })
        })
        .collect()
}

pub(super) fn require_engine(run: &IntegrationRun) -> anyhow::Result<()> {
    ensure!(
        run.payload.mechanical,
        "LEGACY_INTEGRATION: retained history is read-only to this engine. Explicit checkpoint migration is required; no Agent retry or merge was started"
    );
    Ok(())
}

/// Prepare the index for a conflict job. An interrupted merge is reused only for this exact pair.
pub(super) fn prepare_conflict(root: &Path, before: &str, source: &str) -> anyhow::Result<()> {
    ensure!(
        git(root, &["rev-parse", "HEAD"])? == before,
        "Conflict base changed"
    );
    if let Ok(merge_head) = git(root, &["rev-parse", "--verify", "MERGE_HEAD"]) {
        ensure!(merge_head == source, "Another merge is in progress");
        return Ok(());
    }
    ensure!(
        git(root, &["status", "--porcelain", "--untracked-files=all"])?.is_empty(),
        "Unexpected edits before conflict merge"
    );
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args([
            "-c",
            "core.hooksPath=/dev/null",
            "-c",
            "rerere.enabled=false",
            "merge",
            "--no-commit",
            "--no-ff",
            "--no-edit",
            source,
        ])
        .output()?;
    ensure!(
        output.status.code() == Some(1),
        "Expected a conflicted merge; inspect retained index before continuing"
    );
    ensure!(
        git(root, &["rev-parse", "MERGE_HEAD"])? == source,
        "Conflict merge identity changed"
    );
    Ok(())
}

/// AI may edit/stage resolution only; the host creates the history-preserving commit.
pub(super) fn resolution_tree(root: &Path, before: &str, source: &str) -> anyhow::Result<String> {
    ensure!(
        git(root, &["rev-parse", "HEAD"])? == before,
        "Agent changed the integration branch"
    );
    ensure!(
        git(root, &["rev-parse", "MERGE_HEAD"])? == source,
        "Agent changed merge identity"
    );
    ensure!(
        git(root, &["ls-files", "--unmerged"])?.is_empty(),
        "Unresolved index entries"
    );
    ensure!(
        git(root, &["diff", "--name-only"])?.is_empty(),
        "Unstaged resolution changes"
    );
    ensure!(
        git(root, &["ls-files", "--others", "--exclude-standard"])?.is_empty(),
        "Untracked resolution files"
    );
    let protected = git(
        root,
        &["diff", "--cached", "--name-only", before, "--", "openwiki"],
    )?;
    ensure!(
        protected.is_empty(),
        "Conflict job changed canonical OpenWiki"
    );
    git(root, &["write-tree"])
}

pub(super) fn candidate(
    root: &Path,
    before: &str,
    source: &str,
    tree: &str,
    id: Uuid,
    step: usize,
) -> anyhow::Result<String> {
    // Already integrated sources require no redundant commit.
    if git(root, &["merge-base", before, source])? == source {
        return Ok(before.into());
    }
    git(
        root,
        &[
            "-c",
            "user.name=LVK Integration",
            "-c",
            "user.email=integration@localhost",
            "commit-tree",
            tree,
            "-p",
            before,
            "-p",
            source,
            "-m",
            &format!(
                "Integrate source {} (Integration {id}, step {})",
                source,
                step + 1
            ),
        ],
    )
}

/// Called only after the candidate OID is durable. Repeating an applied step is a read-only no-op.
pub(super) fn install(
    root: &Path,
    before: &str,
    source: &str,
    candidate: &str,
    conflict: bool,
) -> anyhow::Result<()> {
    let head = git(root, &["rev-parse", "HEAD"])?;
    if head == candidate {
        ensure!(
            git(root, &["status", "--porcelain", "--untracked-files=all"])?.is_empty(),
            "Applied candidate is dirty"
        );
        if let Ok(merge_head) = git(root, &["rev-parse", "--verify", "MERGE_HEAD"]) {
            ensure!(
                conflict && merge_head == source,
                "Applied candidate has an unrelated merge"
            );
            git(root, &["merge", "--quit"])?;
        }
        return Ok(());
    }
    ensure!(head == before, "Step HEAD changed; refusing to reset");
    let tree = git(root, &["rev-parse", &format!("{candidate}^{{tree}}")])?;
    if conflict {
        ensure!(
            resolution_tree(root, before, source)? == tree,
            "Resolution changed after candidate was recorded"
        );
        // The index and worktree already equal the candidate tree. CAS only our
        // branch, then clear merge metadata; a crash between them is recoverable above.
        git(root, &["update-ref", "HEAD", candidate, before])?;
        git(root, &["merge", "--quit"])?;
    } else {
        ensure!(
            git(root, &["status", "--porcelain", "--untracked-files=all"])?.is_empty(),
            "Candidate worktree is dirty"
        );
        git(
            root,
            &[
                "-c",
                "core.hooksPath=/dev/null",
                "merge",
                "--ff-only",
                "--no-edit",
                candidate,
            ],
        )?;
    }
    ensure!(
        git(root, &["rev-parse", "HEAD"])? == candidate
            && git(root, &["status", "--porcelain", "--untracked-files=all"])?.is_empty(),
        "Candidate installation incomplete; inspect retained state"
    );
    Ok(())
}

pub(super) fn conflict_request(
    run: &IntegrationRun,
    workspace_path: String,
    content: String,
) -> anyhow::Result<AgentRunPortCommand> {
    let config: ExecutorConfig = serde_json::from_value(
        run.payload
            .executor_config
            .clone()
            .context("Missing conflict executor")?,
    )?;
    let provider = match config.executor {
        BaseCodingAgent::Codex => DirectProvider::Codex,
        BaseCodingAgent::ClaudeCode => DirectProvider::ClaudeCode,
        BaseCodingAgent::Gemini => DirectProvider::Gemini,
        BaseCodingAgent::OhMyPi => DirectProvider::OhMyPi,
        #[cfg(feature = "qa-mode")]
        _ => anyhow::bail!("Unsupported conflict executor"),
    };
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
        idempotency_key: format!("integration:{}:conflict:{id}", run.id),
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

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Resolution {
    pub outcome: String,
    pub summary: String,
}

pub(super) fn resolution(text: &str) -> anyhow::Result<Resolution> {
    ensure!(text.len() <= 100_000, "Resolution response too large");
    let text = text
        .trim()
        .strip_prefix("```json")
        .unwrap_or(text.trim())
        .trim();
    let result: Resolution = serde_json::from_str(text.strip_suffix("```").unwrap_or(text).trim())?;
    ensure!(
        matches!(result.outcome.as_str(), "ready" | "blocked") && !result.summary.trim().is_empty(),
        "Invalid resolution response"
    );
    Ok(result)
}
