//! New no-change verification at the ordinary trusted-development-command level.
//! Never reconstruct old Runs or infer a continuously immutable source tree.
use std::{collections::HashMap, path::Path, process::Command};

use anyhow::{Context, ensure};
use db::models::agent_runtime::{AgentEventRecord, AgentRunAttemptRecord, AgentRunRecord};
use executors::runtime::{
    AgentEventPayload, AgentRunStatus, AgentRuntimeToolStatus, ProjectionStatus,
};
use utils::repository_memory::{
    ChangeManifest, MemoryTestResult, RepositoryMemoryRun, RepositoryMemoryStore,
    verification::{
        CommandExit, NoChangeVerification, VerificationOutcome, VerificationSource,
        VerificationStart,
    },
};

pub fn source(root: &Path) -> anyhow::Result<VerificationSource> {
    let git = git::GitService::new();
    let head = git.get_head_info(root)?;
    let status = Command::new("git")
        .arg("-C")
        .arg(root)
        .args([
            "status",
            "--porcelain=v1",
            "--untracked-files=all",
            "--ignore-submodules=none",
        ])
        .output()?;
    ensure!(
        status.status.success(),
        "Cannot observe verification source"
    );
    let after = git.get_head_info(root)?;
    ensure!(
        after.oid == head.oid && after.branch == head.branch,
        "Source moved during observation"
    );
    Ok(VerificationSource {
        source_commit: head.oid,
        branch: head.branch,
        clean: status.stdout.is_empty(),
        observed_at: chrono::Utc::now(),
    })
}

pub fn capture_start(
    store: &RepositoryMemoryStore,
    run: &RepositoryMemoryRun,
) -> anyhow::Result<()> {
    let observed = source(&run.repository_path)?;
    ensure!(
        observed.source_commit == run.base_commit,
        "Source moved before verification start"
    );
    store.save_verification_start(&VerificationStart {
        run_id: run.run_id,
        repository_id: run.repository_id,
        workspace_id: run.workspace_id,
        source: observed,
    })?;
    Ok(())
}

/// Only the normal successful completion/reconciliation path calls this.
/// The DB transaction fences new managed admissions; the existing completion
/// lock fences source finalisation and immutable verification publication.
pub async fn complete(
    pool: &sqlx::SqlitePool,
    store: &RepositoryMemoryStore,
    run: &RepositoryMemoryRun,
) -> anyhow::Result<()> {
    if !run.finalize_source
        || store.verification(run.run_id)?.is_some()
        || store.verification_start(run.run_id)?.is_none()
    {
        return Ok(());
    }
    let mut tx = pool.begin_with("BEGIN IMMEDIATE").await?;
    let current: AgentRunRecord = sqlx::query_as("SELECT * FROM agent_runs WHERE id=?")
        .bind(run.run_id)
        .fetch_one(&mut *tx)
        .await?;
    ensure!(
        current.workspace_id == run.workspace_id
            && current.status == AgentRunStatus::Succeeded
            && current.projection_status == ProjectionStatus::Current,
        "Verification requires a normally succeeded current Run"
    );
    let owns: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM workspace_repos wr JOIN sessions s ON s.workspace_id=wr.workspace_id WHERE wr.workspace_id=? AND wr.repo_id=? AND s.id=?)").bind(run.workspace_id).bind(run.repository_id).bind(current.session_id).fetch_one(&mut *tx).await?;
    ensure!(owns, "Verification Run/Workspace/repository mismatch");
    if super::super::workspace_lifecycle::has_active_writers(&mut tx, run.workspace_id).await? {
        tracing::warn!(run_id=%run.run_id, "No-change verification deferred: writer containment unsettled");
        return Ok(());
    }
    let attempts: Vec<AgentRunAttemptRecord> = sqlx::query_as(
        "SELECT * FROM agent_run_attempts WHERE agent_run_id=? ORDER BY attempt_number",
    )
    .bind(run.run_id)
    .fetch_all(&mut *tx)
    .await?;
    ensure!(!attempts.is_empty(), "Verification attempt missing");
    let mut known = HashMap::new();
    for (index, attempt) in attempts.into_iter().enumerate() {
        attempt
            .request_envelope
            .0
            .validate_for_run(&current.request_envelope.0)?;
        ensure!(
            attempt.attempt_number == index as i64 + 1
                && attempt.status.is_terminal()
                && attempt.projection_status == ProjectionStatus::Current,
            "Verification attempt incomplete"
        );
        let exited: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM agent_process_registry p JOIN native_audit_streams n ON n.run_attempt_id=p.run_attempt_id WHERE p.run_attempt_id=? AND p.registry_status='exited' AND p.observed_exited_at IS NOT NULL AND n.integrity_status='complete' AND n.closed_at IS NOT NULL)").bind(attempt.id).fetch_one(&mut *tx).await?;
        ensure!(exited, "Verification process/audit termination unknown");
        known.insert(attempt.id, attempt);
    }
    let events: Vec<AgentEventRecord> = sqlx::query_as("SELECT event_id,session_id,agent_run_id,turn_id,run_attempt_id,run_attempt_number,sequence,correlation_id,schema_version,payload_version,json_remove(event_envelope,'$.payload.data.result.aggregatedOutput') AS event_envelope,created_at FROM agent_events WHERE agent_run_id=? ORDER BY run_attempt_number,sequence LIMIT 1001").bind(run.run_id).fetch_all(&mut *tx).await?;
    if events.len() > 1000 {
        let _lock = store.try_workspace_completion_lock(run.workspace_id)?;
        if store.verification(run.run_id)?.is_none() && store.event(run.run_id)?.is_none() {
            store.publish_verification(
                run.run_id,
                &VerificationOutcome {
                    record: None,
                    reason: Some("verification_evidence_limit_exceeded".into()),
                },
            )?;
        }
        tx.commit().await?;
        return Ok(());
    }
    let mut exits = Vec::new();
    let mut children = HashMap::new();
    for event in events {
        let envelope = &event.event_envelope.0;
        envelope.validate_for_projection()?;
        let attempt = known
            .get(&event.run_attempt_id)
            .context("Unknown verification attempt")?;
        ensure!(
            event.event_id == envelope.event_id
                && event.session_id == envelope.session_id
                && event.turn_id == envelope.turn_id
                && event.run_attempt_number == i64::from(envelope.run_attempt_number)
                && event.correlation_id == envelope.correlation_id
                && envelope.correlation_id == current.correlation_id
                && event.schema_version == i64::from(envelope.schema_version)
                && event.payload_version == i64::from(envelope.payload_version)
                && envelope.agent_run_id == run.run_id
                && envelope.session_id == current.session_id
                && envelope.run_attempt_id == attempt.id
                && envelope.turn_id == attempt.turn_id
                && envelope.run_attempt_number == attempt.attempt_number as u32
                && event.sequence == envelope.sequence as i64,
            "Verification event identity mismatch"
        );
        match &envelope.payload {
            AgentEventPayload::AgentActivity { activity } => {
                if matches!(
                    activity.kind.as_str(),
                    "completed" | "failed" | "interrupted" | "shutdown"
                ) {
                    children.insert(activity.thread_id.clone(), true);
                } else if !matches!(
                    activity.kind.as_str(),
                    "observed" | "commentary" | "answer" | "tool"
                ) {
                    children.insert(activity.thread_id.clone(), false);
                } else {
                    children.entry(activity.thread_id.clone()).or_insert(false);
                }
            }
            AgentEventPayload::ToolCall {
                tool_name,
                status,
                arguments: Some(args),
                result: Some(result),
                ..
            } if current.provider_id == "codex"
                && tool_name == "Shell"
                && matches!(
                    status,
                    AgentRuntimeToolStatus::Succeeded | AgentRuntimeToolStatus::Failed
                ) =>
            {
                if let (Some(command), Some(cwd), Some(exit_code)) = (
                    args["command"].as_str(),
                    args["cwd"].as_str(),
                    result["exitCode"].as_i64(),
                ) && !command.trim().is_empty()
                    && Path::new(cwd).canonicalize().ok() == run.repository_path.canonicalize().ok()
                {
                    // Older Codex mappers call native "completed" Succeeded even
                    // for exit=1. The observed integer exit determines the claim.
                    exits.push(CommandExit {
                        event_id: event.event_id,
                        run_attempt_id: attempt.id,
                        command: command.into(),
                        cwd: cwd.into(),
                        exit_code,
                    });
                }
            }
            _ => {}
        }
    }
    ensure!(
        children.values().all(|done| *done),
        "Verification child execution unfinished or unknown"
    );
    let (store, run) = (store.clone(), run.clone());
    tokio::task::spawn_blocking(move || finish(&store, &run, exits)).await??;
    tx.commit().await?;
    Ok(())
}

fn finish(
    store: &RepositoryMemoryStore,
    run: &RepositoryMemoryRun,
    command_exits: Vec<CommandExit>,
) -> anyhow::Result<()> {
    let _lock = store.try_workspace_completion_lock(run.workspace_id)?;
    if store.verification(run.run_id)?.is_some() || store.event(run.run_id)?.is_some() {
        return Ok(());
    }
    let start = store
        .verification_start(run.run_id)?
        .context("Verification start missing")?;
    ensure!(
        start.run_id == run.run_id
            && start.repository_id == run.repository_id
            && start.workspace_id == run.workspace_id
            && start.source.source_commit == run.base_commit,
        "Verification start identity mismatch"
    );
    let end = source(&run.repository_path)?;
    let reason = if !start.source.clean {
        Some("dirty_start")
    } else if !end.clean {
        Some("dirty_end")
    } else if end.source_commit != start.source.source_commit || end.branch != start.source.branch {
        Some("source_changed")
    } else if command_exits.is_empty() {
        Some("no_current_command_exit_evidence")
    } else {
        None
    };
    let (draft, draft_problem) = if reason.is_some() {
        (None, None)
    } else {
        match store.read_draft(run.run_id) {
            Ok(draft) => (draft, None),
            Err(error) if error.kind() == std::io::ErrorKind::InvalidData => {
                (None, Some("invalid_current_draft"))
            }
            Err(error) => return Err(error.into()),
        }
    };
    let mut outcome = if let Some(reason) = reason.or(draft_problem) {
        VerificationOutcome {
            record: None,
            reason: Some(reason.into()),
        }
    } else if let Some(mut semantics) = draft.filter(|draft| !draft.tests.is_empty()) {
        for test in &mut semantics.tests {
            if test.result == MemoryTestResult::NotRun {
                continue;
            }
            let matches: Vec<_> = command_exits
                .iter()
                .filter(|exit| test.command.as_deref() == Some(exit.command.as_str()))
                .collect();
            if matches.len() != 1
                || (matches[0].exit_code == 0) != (test.result == MemoryTestResult::Passed)
            {
                test.result = MemoryTestResult::NotRun;
                test.summary = Some("Not established: this Run has no unambiguous matching command exit for the draft claim; original draft preserved.".into());
            }
        }
        VerificationOutcome {
            reason: None,
            record: Some(NoChangeVerification {
                manifest: ChangeManifest {
                    version: 1,
                    event_id: run.run_id,
                    repository_id: run.repository_id,
                    workspace_id: run.workspace_id,
                    task_id: run.task_id,
                    created_at: chrono::Utc::now(),
                    base_commit: run.base_commit.clone(),
                    source_commit: end.source_commit.clone(),
                    target_branch: Some(run.target_branch.clone()),
                    changed_paths: vec![],
                    semantics,
                },
                start,
                end,
                command_exits,
            }),
        }
    } else {
        VerificationOutcome {
            record: None,
            reason: Some("no_current_test_claims".into()),
        }
    };
    // Data/size limits decline only this verification. Genuine filesystem
    // failures and immutable-identity conflicts remain errors to its caller.
    if outcome
        .record
        .as_ref()
        .is_some_and(|record| record.manifest.validate().is_err())
    {
        outcome = VerificationOutcome {
            record: None,
            reason: Some("invalid_verification_snapshot".into()),
        };
    } else if serde_json::to_vec_pretty(&outcome)?.len()
        > utils::repository_memory::verification::MAX_VERIFICATION_BYTES
    {
        outcome = VerificationOutcome {
            record: None,
            reason: Some("verification_record_too_large".into()),
        };
    }
    // Filesystem snapshot + draft claims + command facts share one atomic file.
    store.publish_verification(run.run_id, &outcome)?;
    Ok(())
}

#[cfg(test)]
#[path = "verification_tests.rs"]
mod tests;
