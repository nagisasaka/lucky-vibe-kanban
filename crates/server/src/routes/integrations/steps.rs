//! Sequential merge evidence. Git supplies the automatic merge tree; the
//! difference from that tree is exactly what the human is asked to review.
use std::{path::Path, process::Command};

use anyhow::{Context, ensure};
use db::models::integration::{IntegrationReview, IntegrationRun, IntegrationStep};
use uuid::Uuid;

fn git(root: &Path, args: &[&str]) -> anyhow::Result<std::process::Output> {
    Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .context("Cannot run Git")
}

pub(super) fn prepare(root: &Path, run: &mut IntegrationRun) -> anyhow::Result<()> {
    let before = run
        .payload
        .step_base
        .as_deref()
        .context("Missing step base")?;
    let source = run
        .payload
        .sources
        .get(run.payload.step_index)
        .context("Missing merge source")?;
    let output = git(
        root,
        &["merge-tree", "--write-tree", before, &source.commit],
    )?;
    ensure!(
        matches!(output.status.code(), Some(0 | 1)),
        "Cannot inspect merge: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let text = String::from_utf8(output.stdout)?;
    let tree = text.lines().next().context("Git returned no merge tree")?;
    ensure!(
        tree.len() == 40 && tree.bytes().all(|c| c.is_ascii_hexdigit()),
        "Invalid automatic merge tree"
    );
    run.payload.merge_tree = Some(tree.into());
    run.payload.merge_conflicted = !output.status.success();
    Ok(())
}

pub(super) fn record_review(root: &Path, run: &mut IntegrationRun) -> anyhow::Result<()> {
    if !run.payload.sequential {
        return Ok(());
    }
    let candidate = run
        .payload
        .result_commit
        .as_deref()
        .context("Missing candidate")?;
    let tree = run
        .payload
        .merge_tree
        .as_deref()
        .context("Missing automatic merge tree")?;
    let output = git(
        root,
        &[
            "diff",
            "--no-ext-diff",
            "--no-textconv",
            tree,
            candidate,
            "--",
        ],
    )?;
    ensure!(output.status.success(), "Cannot read resolution diff");
    let diff = String::from_utf8_lossy(&output.stdout).into_owned();
    if !run.payload.merge_conflicted
        && diff.is_empty()
        && !run
            .payload
            .reviews
            .iter()
            .any(|r| r.step_index == run.payload.step_index)
    {
        return Ok(());
    }
    let review = IntegrationReview {
        id: Uuid::new_v4(),
        scope: run.validation_scope(),
        step_index: run.payload.step_index,
        candidate: candidate.into(),
        diff,
        explanation: run.payload.semantic_summary.clone().unwrap_or_default(),
        decision: None,
        actor: None,
        request_id: None,
        decided_at: None,
    };
    run.payload.reviews.push(review);
    Ok(())
}

pub(super) fn review_approved(run: &IntegrationRun) -> bool {
    let relevant: Vec<_> = run
        .payload
        .reviews
        .iter()
        .filter(|r| r.step_index == run.payload.step_index)
        .collect();
    (relevant.is_empty() && !run.payload.merge_conflicted)
        || relevant.iter().any(|r| {
            r.scope == run.validation_scope()
                && r.decision.as_deref() == Some("continue")
                && r.actor.is_some()
                && r.request_id.is_some()
                && r.decided_at.is_some()
        })
}

pub(super) fn complete(run: &mut IntegrationRun) -> anyhow::Result<()> {
    let result = run
        .payload
        .result_commit
        .clone()
        .context("Missing tested candidate")?;
    if let Some(step) = run.payload.steps.get(run.payload.step_index) {
        ensure!(step.result_commit == result, "Completed merge step changed");
        return Ok(());
    }
    ensure!(
        run.payload.steps.len() == run.payload.step_index,
        "Merge step has already completed"
    );
    let step = IntegrationStep {
        source_commit: run.payload.sources[run.payload.step_index].commit.clone(),
        result_commit: result,
        validation: run.payload.validation.clone(),
        gates: run.payload.gates.clone(),
        summary: run.payload.semantic_summary.clone().unwrap_or_default(),
    };
    run.payload.steps.push(step);
    if run.payload.steps.len() == run.payload.sources.len() {
        let summary = run
            .payload
            .steps
            .iter()
            .enumerate()
            .map(|(i, step)| format!("{}. {}", i + 1, step.summary))
            .collect::<Vec<_>>()
            .join("\n");
        run.payload.semantic_summary = Some(summary.clone());
        if let Some(semantics) = &mut run.payload.semantics {
            semantics.summary = summary.clone();
        }
        if let Some(manifest) = &mut run.payload.integration_manifest {
            manifest.semantics.summary = summary;
        }
    }
    Ok(())
}
