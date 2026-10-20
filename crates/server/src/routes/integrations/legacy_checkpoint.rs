//! One retained LOCAL-9 checkpoint, explicitly adopted by an operator.
//! No automatic migration, Agent replay, Git mutation or projection repair.
use std::path::Path;

use anyhow::{Context, ensure};
use axum::{
    Json,
    extract::{Path as RoutePath, State},
};
use db::models::integration::IntegrationRun;
use deployment::Deployment;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use super::{admission, mechanical, runtime};
use crate::{DeploymentImpl, error::ApiError};

const RUN: &str = "c68ce019-9349-4679-976f-326120191c08";
const CANDIDATE: &str = "8d0eda6b75cce0d0930a17c46d43c5d162f58e7b";
const PAYLOAD: &str = "d29ed8880dc32be3eba3e8727af1d4217ae314bf07d864734666b4d7c7e85e6c";
const BASE: &str = "b385a2d2d1a8683fbdcb8d931356d34f4264169c";
const SOURCES: [&str; 4] = [
    "7922bf778fa5837762f056bcf7322b5191d4d0ab",
    "4a11c7a304891f1cfe95f9daecd4a5e319de51d0",
    "b8f954948d615e66866bba85b5deb6882e71d55e",
    "40487d16a091b0f67eb24ecd011e507a5f42b8db",
];

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Request {
    pub project_id: Uuid,
    pub request_id: Uuid,
    pub candidate: String,
    /// Required: inspection and execution are separate explicit operator actions.
    pub dry_run: bool,
}

pub(super) async fn adopt(
    State(deployment): State<DeploymentImpl>,
    RoutePath(id): RoutePath<Uuid>,
    Json(request): Json<Request>,
) -> Result<Json<utils::response::ApiResponse<serde_json::Value>>, ApiError> {
    let result = apply(&deployment, id, request)
        .await
        .map_err(super::error)?;
    Ok(Json(utils::response::ApiResponse::success(result)))
}

pub(super) async fn apply(
    deployment: &DeploymentImpl,
    id: Uuid,
    request: Request,
) -> anyhow::Result<serde_json::Value> {
    ensure!(
        id.to_string() == RUN && request.candidate == CANDIDATE,
        "Unsupported legacy checkpoint"
    );
    let _guard = runtime::reserve_intervention(id)?;
    let _admission = services::services::integration_admission::MUTATIONS
        .lock()
        .await;
    let pool = &deployment.db().pool;
    let mut run = IntegrationRun::find(pool, id).await?;
    ensure!(run.project_id == request.project_id, "Project mismatch");
    if run.payload.mechanical {
        ensure!(
            run.payload
                .history
                .iter()
                .any(|h| h["operation"] == "adopt_legacy_checkpoint"
                    && h["request_id"] == request.request_id.to_string()
                    && h["candidate"] == request.candidate),
            "Already adopted by another request"
        );
        return Ok(serde_json::json!({"adopted":true,"replayed":true,"integration_id":id}));
    }
    let raw: String = sqlx::query_scalar("SELECT payload FROM integration_runs WHERE id=?")
        .bind(id)
        .fetch_one(pool)
        .await?;
    ensure!(
        format!("{:x}", Sha256::digest(raw.as_bytes())) == PAYLOAD,
        "Legacy payload changed; inspect before adoption"
    );
    let (repo, workspace, _, root) = runtime::context(deployment, &run).await?;
    admission::idle(deployment, workspace.id).await?;
    let missing_exit: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM agent_run_attempts a JOIN agent_runs r ON r.id=a.agent_run_id LEFT JOIN agent_process_registry p ON p.run_attempt_id=a.id WHERE r.workspace_id=? AND (p.registry_status IS NULL OR p.registry_status!='exited'))")
        .bind(workspace.id).fetch_one(pool).await?;
    ensure!(
        !missing_exit,
        "Historical provider exit evidence is incomplete"
    );
    verify_git_and_approval(&run, &root)?;
    mechanical::require_private_branch(&root, &workspace.branch, &run.target_ref)?;
    deployment.git().require_clean_source(&root, CANDIDATE)?;
    ensure!(
        mechanical::git(&repo.path, &["rev-parse", "refs/heads/main"])? == BASE,
        "TARGET_CHANGED"
    );
    let unsettled: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM orchestration_outbox WHERE orchestration_run_id=? AND delivery_status!='delivered') OR EXISTS(SELECT 1 FROM agent_run_commands c JOIN agent_runs a ON a.id=c.agent_run_id WHERE a.workspace_id=? AND c.delivery_status NOT IN ('delivered','failed'))")
        .bind(id).bind(workspace.id).fetch_one(pool).await?;
    ensure!(!unsettled, "Historical command delivery is not settled");
    // Preserve this known pre-send failed instruction, never replay or mark it delivered.
    let failures: Vec<(Uuid, String)> = sqlx::query_as(
        "SELECT c.command_id,c.command_envelope FROM agent_run_commands c JOIN agent_runs a ON a.id=c.agent_run_id WHERE a.workspace_id=? AND c.delivery_status='failed'")
        .bind(workspace.id).fetch_all(pool).await?;
    ensure!(
        failures.len() == 1
            && failures[0].0.to_string() == "d74d0df1-6ddd-45dd-b806-010bad8c73d7"
            && format!("{:x}", Sha256::digest(failures[0].1.as_bytes()))
                == "bb8152fd5f282a1b6396fbd3f752ba3c29dcde260c34806fd97ca89ec6b24976",
        "Unknown historical command failure"
    );
    let nodes: Vec<(Uuid, String, String)> = sqlx::query_as(
        "SELECT id,node_key,status FROM orchestration_node_executions WHERE orchestration_run_id=?",
    )
    .bind(id)
    .fetch_all(pool)
    .await?;
    ensure!(
        nodes.len() == 1
            && nodes[0].1 == "integrate"
            && matches!(
                nodes[0].2.as_str(),
                "cancelling" | "cancelled" | "succeeded"
            ),
        "Historical product ownership changed"
    );
    if nodes[0].2 == "succeeded" {
        // Startup may settle the cancelled parent from its already terminal child.
        // This is historical child execution evidence, never Integration success.
        let parent: String = sqlx::query_scalar("SELECT status FROM orchestration_runs WHERE id=?")
            .bind(id)
            .fetch_one(pool)
            .await?;
        let events: Vec<(i64, String)> = sqlx::query_as("SELECT sequence,event_envelope FROM orchestration_events WHERE orchestration_run_id=? ORDER BY sequence DESC LIMIT 2")
            .bind(id).fetch_all(pool).await?;
        ensure!(
            parent == "cancelled" && events.len() == 2 && events[0].0 == 110 && events[1].0 == 109,
            "Unproven historical startup settlement"
        );
        let terminal: serde_json::Value = serde_json::from_str(&events[0].1)?;
        let child: serde_json::Value = serde_json::from_str(&events[1].1)?;
        ensure!(
            terminal["payload"]
                == serde_json::json!({"type":"lifecycle_changed","data":{"status":"cancelled"}})
                && child["payload"]
                    == serde_json::json!({"type":"node_status_changed","data":{"node_execution_id":nodes[0].0,"status":"succeeded"}}),
            "Historical startup settlement evidence changed"
        );
        let linked: i64 = sqlx::query_scalar("SELECT count(*) FROM orchestration_agent_run_links l JOIN agent_runs a ON a.id=l.agent_run_id WHERE l.orchestration_run_id=? AND l.node_execution_id=? AND a.id=? AND a.status='succeeded'")
            .bind(id).bind(nodes[0].0).bind(run.agent_run_id).fetch_one(pool).await?;
        ensure!(linked == 1, "Historical child identity changed");
    }
    if request.dry_run {
        return Ok(
            serde_json::json!({"adopted":false,"eligible":true,"candidate":CANDIDATE,"next_source":SOURCES[3]}),
        );
    }
    // Idempotent legacy settlement precedes adoption. A storage failure here leaves the
    // unchanged payload blocked; repeat this same request, never recreate the Integration.
    if nodes[0].2 == "cancelling" {
        services::services::orchestration::OrchestrationService::new(
            pool.clone(),
            std::sync::Arc::new(deployment.agent_run_port().clone()),
        )
        .complete_product_node(
            id,
            nodes[0].0,
            executors::runtime::OrchestrationNodeStatus::Cancelled,
        )
        .await?;
    }
    let original: serde_json::Value = serde_json::from_str(&raw)?;
    run.payload.history.push(serde_json::json!({
        "operation":"adopt_legacy_checkpoint","request_id":request.request_id,
        "candidate":CANDIDATE,"source_payload_sha256":PAYLOAD,
        "previous_payload":original,"previous_error":run.error,"previous_agent_run_id":run.agent_run_id,
        "at":chrono::Utc::now(),
    }));
    run.payload.mechanical = true;
    run.payload.conflict_dispatch = None;
    run.payload.intervention = None;
    run.payload.merge_tree = None; // recompute step 4 from the approved prefix
    run.payload.merge_conflicted = false;
    run.payload.next_step = false;
    run.payload.resume_validation = false;
    run.agent_run_id = None;
    run.error = None;
    run.payload.stop = Some(db::models::integration::IntegrationStop {
        stage: "blocked".into(),
        category: "checkpoint_adopted".into(),
        next_action:
            "Checkpoint adopted without execution. Explicitly Resume to integrate source 4.".into(),
    });
    let mut tx = pool.begin_with("BEGIN IMMEDIATE").await?;
    let changed = sqlx::query("UPDATE integration_runs SET payload=?,agent_run_id=NULL,error=NULL,updated_at=datetime('now','subsec') WHERE id=? AND payload=? AND status='blocked' AND cancel_requested=0")
        .bind(&run.payload).bind(id).bind(&raw).execute(&mut *tx).await?.rows_affected();
    ensure!(
        changed == 1,
        "Checkpoint changed concurrently; no adoption committed"
    );
    tx.commit().await?;
    Ok(
        serde_json::json!({"adopted":true,"integration_id":id,"status":"blocked","next_source":SOURCES[3]}),
    )
}

pub(super) fn verify_git_and_approval(run: &IntegrationRun, root: &Path) -> anyhow::Result<()> {
    ensure!(
        run.status == "blocked"
            && !run.cancel_requested
            && !run.payload.publication_intent
            && !run.payload.published
            && run.payload.result_commit.is_none()
            && run.payload.skip_validation
            && run.payload.validation.is_empty()
            && run.payload.gates.is_empty()
            && run.payload.step_index == 3
            && run.payload.steps.len() == 3
            && run.payload.step_base.as_deref() == Some(CANDIDATE)
            && run.payload.base_commit.as_deref() == Some(BASE)
            && run
                .payload
                .sources
                .iter()
                .map(|s| s.commit.as_str())
                .eq(SOURCES),
        "Checkpoint invariants changed"
    );
    let mut before = BASE;
    for (index, step) in run.payload.steps.iter().enumerate() {
        ensure!(
            step.validation_skipped
                && step.validation.is_empty()
                && step.gates.is_empty()
                && step.source_commit == SOURCES[index],
            "Completed step evidence changed"
        );
        let parents = mechanical::git(root, &["show", "-s", "--format=%P", &step.result_commit])?;
        ensure!(
            parents == format!("{before} {}", SOURCES[index]),
            "Unexpected merge parents"
        );
        ensure!(
            mechanical::git(root, &["rev-parse", &run.payload.sources[index].branch])?
                == SOURCES[index],
            "Source ref changed"
        );
        before = &step.result_commit;
    }
    ensure!(
        before == CANDIDATE
            && mechanical::git(root, &["rev-parse", &run.payload.sources[3].branch])? == SOURCES[3],
        "Checkpoint/source changed"
    );
    ensure!(
        mechanical::git(root, &["merge-base", CANDIDATE, SOURCES[3]])? != SOURCES[3],
        "Source 4 already applied"
    );
    let review = run
        .payload
        .reviews
        .iter()
        .find(|r| {
            r.step_index == 2
                && r.candidate == CANDIDATE
                && r.decision.as_deref() == Some("continue")
                && r.actor.is_some()
                && r.request_id.is_some()
                && r.decided_at.is_some()
        })
        .context("Missing human approval")?;
    let mut approved = run.clone();
    approved.payload.result_commit = Some(CANDIDATE.into());
    ensure!(
        review.scope == approved.validation_scope(),
        "Approval scope changed"
    );
    let auto = mechanical::git(root, &["rev-parse", &format!("{}^1", CANDIDATE)])?;
    approved.payload.step_base = Some(auto);
    approved.payload.step_index = 2;
    super::steps::prepare(root, &mut approved)?;
    let diff = mechanical::git(
        root,
        &[
            "diff",
            "--no-ext-diff",
            "--no-textconv",
            approved.payload.merge_tree.as_deref().unwrap(),
            CANDIDATE,
            "--",
        ],
    )?;
    ensure!(
        diff.trim() == review.diff.trim(),
        "Approved resolution diff changed"
    );
    Ok(())
}
