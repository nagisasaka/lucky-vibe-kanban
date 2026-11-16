//! Shared repository publication service for HTTP and remote MCP.
use anyhow::{Result, ensure};
use db::models::repo::Repo;
use git::repository_push::PushObservation;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::SqlitePool;
use ts_rs::TS;
use uuid::Uuid;

use crate::remote_mcp::operations::{Operations, Receipt};

#[derive(Clone, Debug, Deserialize, Serialize, TS, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RepositoryPushSelection {
    pub project_id: Uuid,
    pub remote: String,
    pub branch: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize, TS, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RepositoryPushPreview {
    pub repository_id: Uuid,
    pub repository_name: String,
    pub project_id: Uuid,
    #[serde(flatten)]
    #[ts(flatten)]
    pub target: PushObservation,
}
#[derive(Clone, Deserialize, Serialize, TS, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RepositoryPushRequest {
    pub request_id: Uuid,
    pub preview: RepositoryPushPreview,
}
#[derive(Serialize, TS)]
pub struct RepositoryPushResult {
    pub outcome: String,
}
#[derive(Serialize, TS)]
pub struct RepositoryPushReceipt {
    pub request_id: Uuid,
    pub status: String,
    pub result: Option<RepositoryPushResult>,
    pub preview: RepositoryPushPreview,
}
#[derive(Serialize, TS)]
pub struct RepositoryPushVerification {
    pub receipt: RepositoryPushReceipt,
    pub remote_sha: Option<String>,
    pub source_present: Option<bool>,
    pub verification_error: Option<String>,
}

pub(crate) fn authorize(actor: &server_access::Principal, write: bool) -> Result<()> {
    ensure!(
        actor.scopes.contains("lvk:read") && (!write || actor.scopes.contains("lvk:write")),
        "Read and write permissions are required for repository publication"
    );
    Ok(())
}
fn principal(actor: &server_access::Principal) -> String {
    json!([actor.issuer, actor.subject]).to_string()
}
fn operations(pool: &SqlitePool) -> Operations {
    Operations {
        pool: pool.clone(),
        process_instance: *crate::remote_mcp::PROCESS_INSTANCE.get_or_init(Uuid::new_v4),
    }
}
pub(crate) async fn root(pool: &SqlitePool, project: Uuid, id: Uuid) -> Result<Repo> {
    ensure!(
        crate::routes::local_remote::project_associated_repository_ids(pool, project)
            .await?
            .contains(&id),
        "Repository does not belong to Project"
    );
    Repo::find_by_id(pool, id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Repository not found"))
}
pub(crate) async fn preview(
    pool: &SqlitePool,
    id: Uuid,
    selection: RepositoryPushSelection,
) -> Result<RepositoryPushPreview> {
    let repo = root(pool, selection.project_id, id).await?;
    let path = repo.path.clone();
    let target = tokio::task::spawn_blocking(move || {
        git::repository_push::observe(&path, &selection.remote, &selection.branch)
    })
    .await?
    .map_err(|error| anyhow::anyhow!(git::repository_push::error_code(&error)))?;
    ensure!(
        root(pool, selection.project_id, id).await?.path == repo.path,
        "Repository registration changed"
    );
    Ok(RepositoryPushPreview {
        repository_id: id,
        repository_name: repo.display_name,
        project_id: selection.project_id,
        target,
    })
}
fn project_receipt(receipt: Receipt) -> Result<RepositoryPushReceipt> {
    let preview = serde_json::from_str(&receipt.targets)?;
    let result = receipt
        .result
        .as_deref()
        .map(serde_json::from_str::<Value>)
        .transpose()?
        .map(|v| RepositoryPushResult {
            outcome: v["outcome"].as_str().unwrap_or("unknown").into(),
        });
    Ok(RepositoryPushReceipt {
        request_id: receipt.request_id,
        status: receipt.status,
        result,
        preview,
    })
}
#[derive(Debug, thiserror::Error)]
#[error("Publication was not admitted ({0})")]
pub(crate) struct NotAdmitted(pub &'static str);
async fn validated_preview(
    pool: &SqlitePool,
    id: Uuid,
    input: &RepositoryPushPreview,
) -> Result<RepositoryPushPreview> {
    let current = preview(
        pool,
        id,
        RepositoryPushSelection {
            project_id: input.project_id,
            remote: input.target.remote.clone(),
            branch: input.target.branch.clone(),
        },
    )
    .await?;
    ensure!(
        current.target.source_sha == input.target.source_sha,
        "source_drift"
    );
    ensure!(
        current.target.remote_sha == input.target.remote_sha,
        "remote_drift"
    );
    ensure!(current == *input, "target_changed");
    Ok(current)
}
pub(crate) async fn push(
    pool: &SqlitePool,
    actor: &server_access::Principal,
    lease: server_access::Lease,
    id: Uuid,
    input: RepositoryPushRequest,
) -> Result<RepositoryPushReceipt> {
    authorize(actor, true)?;
    ensure!(lease.is_valid().await, NotAdmitted("permission_denied"));
    ensure!(
        input.preview.repository_id == id,
        "Repository target mismatch"
    );
    let store = operations(pool);
    let value = serde_json::to_value(&input)?;
    if let Some(receipt) = store
        .replay(
            &principal(actor),
            input.request_id,
            "push_repository_branch",
            &value,
        )
        .await?
    {
        root(pool, receipt.project_id, id).await?;
        return project_receipt(receipt);
    }
    // Never persist client-provided display/URL fields. Reconstruct every
    // target from the registration and Git before reserving a durable receipt.
    let expected = validated_preview(pool, id, &input.preview)
        .await
        .map_err(|error| NotAdmitted(git::repository_push::error_code(&error)))?;
    let repo = root(pool, expected.project_id, id)
        .await
        .map_err(|_| NotAdmitted("target_changed"))?;
    let pool_owned = pool.clone();
    let actor_owned = actor.clone();
    ensure!(lease.is_valid().await, NotAdmitted("permission_denied"));
    let targets = serde_json::to_value(&expected)?;
    let receipt = store.submit(principal(actor), input.request_id, expected.project_id, "push_repository_branch".into(), value, targets, move |_| async move {
        if authorize(&actor_owned, true).is_err() { return ("failed", json!({"outcome":"permission_denied"})); }
        match root(&pool_owned, expected.project_id, id).await {
            Ok(current) if current.path == repo.path => {}
            _ => return ("failed", json!({"outcome":"target_changed"})),
        }
        let selection = RepositoryPushSelection { project_id: expected.project_id, remote: expected.target.remote.clone(), branch: expected.target.branch.clone() };
        match preview(&pool_owned, id, selection).await {
            Ok(current) if current == expected => {}
            Ok(current) => return ("failed", json!({"outcome":if current.target.source_sha != expected.target.source_sha { "source_drift" } else { "target_changed" }})),
            Err(error) => return ("failed", json!({"outcome":git::repository_push::error_code(&error)})),
        }
        if !lease.is_valid().await { return ("failed", json!({"outcome":"permission_denied"})); }
        let path = repo.path;
        let result = tokio::task::spawn_blocking(move || git::repository_push::publish(&path, &expected.target)).await;
        let outcome = result.unwrap_or("unknown");
        let status = match outcome { "pushed" => "succeeded", "unknown" => "uncertain", _ => "failed" };
        (status, json!({"outcome":outcome}))
    }).await?;
    project_receipt(receipt)
}
pub(crate) async fn receipt(
    pool: &SqlitePool,
    actor: &server_access::Principal,
    id: Uuid,
    request: Uuid,
) -> Result<RepositoryPushReceipt> {
    authorize(actor, false)?;
    let receipt = operations(pool)
        .by_request(&principal(actor), request)
        .await?;
    ensure!(
        receipt.operation == "push_repository_branch",
        "Not a repository push operation"
    );
    root(pool, receipt.project_id, id).await?;
    let result = project_receipt(receipt)?;
    ensure!(
        result.preview.repository_id == id,
        "Repository target mismatch"
    );
    Ok(result)
}
pub(crate) async fn verify(
    pool: &SqlitePool,
    actor: &server_access::Principal,
    id: Uuid,
    request: Uuid,
) -> Result<RepositoryPushVerification> {
    let receipt = receipt(pool, actor, id, request).await?;
    let repo = root(pool, receipt.preview.project_id, id).await?;
    let target = receipt.preview.target.clone();
    let observed =
        tokio::task::spawn_blocking(move || git::repository_push::verify(&repo.path, &target))
            .await;
    let (remote_sha, source_present, verification_error) = match observed {
        Ok(Ok(sha)) => {
            let present = sha.as_deref() == Some(&receipt.preview.target.source_sha);
            (sha, Some(present), None)
        }
        Ok(Err(error)) => (
            None,
            None,
            Some(git::repository_push::error_code(&error).to_owned()),
        ),
        Err(_) => (None, None, Some("remote_verification_unavailable".into())),
    };
    Ok(RepositoryPushVerification {
        receipt,
        remote_sha,
        source_present,
        verification_error,
    })
}
