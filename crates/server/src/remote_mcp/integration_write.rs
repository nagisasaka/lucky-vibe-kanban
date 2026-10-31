//! Explicit decisions through the UI domain path; receipts never drive the engine.
use anyhow::ensure;
use db::models::integration::IntegrationRun;
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};
use utils::mcp_error::Category;
use uuid::Uuid;

use super::{
    Remote,
    auth::Principal,
    read::{arguments, definition},
};

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct CancelArgs {
    project_id: Uuid,
    integration_id: Uuid,
    request_id: Uuid,
}

pub(super) fn tools() -> Vec<rmcp::model::Tool> {
    [
        definition::<CancelArgs>("cancel_integration", "Explicitly request cancellation of an Auto-merge through the UI cancellation path. Requires merge permission. Acceptance is NOT stopped execution or released reservations. No Git undo after publication intent. Preserve request_id and input on retry; read get_operation and get_integration until cancellation_complete. Does not control Goals."),
    ].into_iter().map(|tool| {
        let mut value = serde_json::to_value(tool).unwrap();
        value["annotations"] = json!({"readOnlyHint":false,"destructiveHint":true,"idempotentHint":true,"openWorldHint":false});
        value["_meta"] = json!({"securitySchemes":[{"type":"oauth2","scopes":["lvk:read","lvk:merge"]}]});
        serde_json::from_value(value).unwrap()
    }).collect()
}

async fn find(remote: &Remote, project: Uuid, id: Uuid) -> anyhow::Result<IntegrationRun> {
    remote.require_project(project).await?;
    let run = IntegrationRun::find(&remote.pool, id)
        .await
        .map_err(|error| super::errors::rejection(super::todos::database_category(&error)))?;
    ensure!(
        run.project_id == project,
        super::errors::rejection_with_message(
            utils::mcp_error::Category::NotFound,
            "Auto-merge does not exist in this Project"
        )
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
        super::errors::rejection_with_message(
            utils::mcp_error::Category::Permission,
            "Read and merge permissions required"
        )
    );
    ensure!(
        name == "cancel_integration",
        super::errors::rejection_with_message(
            utils::mcp_error::Category::Validation,
            "Unknown Auto-merge operation"
        )
    );
    let args: CancelArgs = arguments(input.clone())?;
    let (project, id, request_id) = (args.project_id, args.integration_id, args.request_id);
    find(remote, project, id).await?;
    let actions = remote
        .launch
        .clone()
        .ok_or_else(|| super::errors::rejection(Category::Unavailable))?;
    let owned = remote.clone();
    let receipt = remote.operations().submit_and_wait(
        remote.principal_key(principal), request_id, project, name.into(), input,
        json!({"integration_id":id}), move |_operation| async move {
            if let Err(error) = find(&owned, project, id).await {
                return ("failed", json!({"message":super::errors::message(&error),"error":super::errors::diagnostic(&error,false).value()}));
            }
            let result = actions.cancel_integration(id).await;
            if let Err(error) = result {
                // Arbitrary domain errors can follow external effects. Only explicit
                // typed rejection evidence proves no effect; absence of a DB error does not.
                let diagnostic = super::errors::diagnostic(&error, true);
                let status = if diagnostic.outcome == utils::mcp_error::Outcome::Failed { "failed" } else { "uncertain" };
                return (status, json!({"integration_id":id,"message":super::errors::message(&error),"error":diagnostic.value(),"read_back":"get_integration"}));
            }
            match observation(&owned, project, id).await {
                Ok(mut value) => {
                    value["cancellation_requested"] = json!(value["cancel_requested"] == true);
                    value["read_back"] = json!("get_integration");
                    ("succeeded", value)
                }
                Err(_) => ("uncertain", json!({"integration_id":id,"message":"Decision returned; current state unavailable. Read get_integration before any new request."})),
            }
        }
    ).await?;
    Ok(remote.receipt_value(receipt))
}
