//! Explicit decisions through the UI domain path; receipts never drive the engine.
use anyhow::ensure;
use db::models::integration::IntegrationRun;
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};
use uuid::Uuid;

use super::{
    Remote,
    auth::Principal,
    read::{arguments, definition},
};
use crate::routes::integrations::{DecisionExpectation, GateDecisionRequest};

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ApproveArgs {
    project_id: Uuid,
    integration_id: Uuid,
    request_id: Uuid,
    /// Exact pending review/validation gate ID from get_integration(section=pending_gates).
    gate_id: Uuid,
    expected: DecisionExpectation,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct CancelArgs {
    project_id: Uuid,
    integration_id: Uuid,
    request_id: Uuid,
}

pub(super) fn tools() -> Vec<rmcp::model::Tool> {
    [
        definition::<ApproveArgs>("approve_integration", "Explicitly approve the exact current formal Integration gate through the UI decision path. Read get_integration(section=pending_gates) first and copy gate_id and expected. A validation gate approval is an explicit candidate-bound exception to a failed check, NOT a test pass. Requires merge permission. Stale evidence/target is rejected. Preserve request_id and identical input on retry; read get_operation and get_integration afterwards. Never infer approval from a Goal or review comment."),
        definition::<CancelArgs>("cancel_integration", "Explicitly request cancellation of a formal Integration through the UI cancellation path. Requires merge permission. Acceptance is NOT stopped execution or released reservations. No Git undo after publication intent. Preserve request_id and input on retry; read get_operation and get_integration until cancellation_complete. Does not control Goals."),
    ].into_iter().map(|tool| {
        let mut value = serde_json::to_value(tool).unwrap();
        value["annotations"] = json!({"readOnlyHint":false,"destructiveHint":true,"idempotentHint":true,"openWorldHint":false});
        value["_meta"] = json!({"securitySchemes":[{"type":"oauth2","scopes":["lvk:read","lvk:merge"]}]});
        serde_json::from_value(value).unwrap()
    }).collect()
}

async fn find(remote: &Remote, project: Uuid, id: Uuid) -> anyhow::Result<IntegrationRun> {
    remote.require_project(project).await?;
    let run = IntegrationRun::find(&remote.pool, id).await?;
    ensure!(
        run.project_id == project,
        "Integration does not exist in this Project"
    );
    Ok(run)
}

/// Read run and reservations in one SQLite snapshot. Terminal status alone is
/// insufficient: the engine saves cancelled before releasing its reservations.
pub(super) async fn observation(remote: &Remote, project: Uuid, id: Uuid) -> anyhow::Result<Value> {
    remote.require_project(project).await?;
    let mut tx = remote.pool.begin().await?;
    let run: IntegrationRun =
        sqlx::query_as("SELECT * FROM integration_runs WHERE id=? AND project_id=?")
            .bind(id)
            .bind(project)
            .fetch_one(&mut *tx)
            .await?;
    let reserved: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM integration_reservations WHERE run_id=?)")
            .bind(id)
            .fetch_one(&mut *tx)
            .await?;
    tx.commit().await?;
    Ok(json!({
        "integration_id":id,"revision":run.updated_at,"status":run.status,
        "cancel_requested":run.cancel_requested,"published":run.payload.published,
        "publication_intent":run.payload.publication_intent,
        "terminal":matches!(run.status.as_str(),"cancelled"|"succeeded"|"failed"),
        "reservations_released":!reserved,
        "cancellation_complete":run.status=="cancelled" && !reserved,
    }))
}

pub(super) async fn call(
    remote: &Remote,
    principal: &Principal,
    name: &str,
    input: Value,
) -> anyhow::Result<Value> {
    ensure!(
        principal.allows("lvk:read") && principal.allows("lvk:merge"),
        "Read and merge permissions required"
    );
    let (project, id, request_id, approval) = match name {
        "approve_integration" => {
            let args: ApproveArgs = arguments(input.clone())?;
            (
                args.project_id,
                args.integration_id,
                args.request_id,
                Some((args.gate_id, args.expected)),
            )
        }
        "cancel_integration" => {
            let args: CancelArgs = arguments(input.clone())?;
            (args.project_id, args.integration_id, args.request_id, None)
        }
        _ => anyhow::bail!("Unknown Integration decision tool"),
    };
    find(remote, project, id).await?;
    let actions = remote
        .launch
        .clone()
        .ok_or_else(|| anyhow::anyhow!("Integration service unavailable"))?;
    let owned = remote.clone();
    let receipt = remote.operations().submit_and_wait(
        remote.principal_key(principal), request_id, project, name.into(), input,
        json!({"integration_id":id}), move |operation| async move {
            if let Err(error) = find(&owned, project, id).await {
                return ("failed", json!({"message":super::errors::message(&error)}));
            }
            let approving = approval.is_some();
            let result = match approval {
                Some((gate_id, expected)) => actions.decide_integration(id, GateDecisionRequest {
                    project_id:project, gate_id, request_id:operation, decision:"continue".into(),
                }, expected).await,
                None => actions.cancel_integration(id).await,
            };
            if let Err(error) = result {
                // A database failure can occur after mutation; never authorise replay.
                let status = if error.chain().any(|e| e.is::<sqlx::Error>()) { "uncertain" } else { "failed" };
                return (status, json!({"integration_id":id,"message":super::errors::message(&error),"read_back":"get_integration"}));
            }
            match observation(&owned, project, id).await {
                Ok(mut value) => {
                    value["decision_accepted"] = json!(approving);
                    value["cancellation_requested"] = json!(!approving && value["cancel_requested"] == true);
                    value["read_back"] = json!("get_integration");
                    ("succeeded", value)
                }
                Err(_) => ("uncertain", json!({"integration_id":id,"message":"Decision returned; current state unavailable. Read get_integration before any new request."})),
            }
        }
    ).await?;
    Ok(remote.receipt_value(receipt))
}
