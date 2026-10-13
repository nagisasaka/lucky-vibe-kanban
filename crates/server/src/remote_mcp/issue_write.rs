//! Ordinary Issue mutations, separate from execution. Domain writes reuse the
//! local Board services; the remote boundary owns scope and request receipts.
use anyhow::ensure;
use api_types::issue::{CreateIssueRequest, UpdateIssueRequest};
use rmcp::model::Tool;
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};
use uuid::Uuid;

use super::{
    Remote,
    auth::Principal,
    operations::{Operations, Receipt},
    read::{arguments, definition},
};
use crate::{error::ApiError, routes::local_remote};

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct CreateArgs {
    project_id: Uuid,
    request_id: Uuid,
    status_id: Uuid,
    #[schemars(length(min = 1, max = 500))]
    title: String,
    /// Self-contained agreed requirements and acceptance criteria. No execution starts.
    #[schemars(length(min = 1, max = 64000))]
    description: String,
}
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct UpdateArgs {
    project_id: Uuid,
    request_id: Uuid,
    issue_id: Uuid,
    expected_revision: String,
    #[schemars(length(min = 1, max = 500))]
    title: Option<String>,
    /// Omit to preserve requirements; an empty string explicitly clears them.
    #[schemars(length(max = 64000))]
    description: Option<String>,
    status_id: Option<Uuid>,
}
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct OperationArgs {
    project_id: Uuid,
    request_id: Uuid,
}

pub(super) fn tools() -> Vec<Tool> {
    let mut tools = vec![
        definition::<CreateArgs>(
            "create_issue",
            "Create a normal Issue containing agreed requirements; does not create a Workspace or start implementation. Use one stable request_id and inspect get_operation after a lost response.",
        ),
        definition::<UpdateArgs>(
            "update_issue",
            "Apply explicitly requested Issue changes against the revision from get_issue. Does not steer existing runs or start implementation. Omitted fields stay unchanged.",
        ),
    ];
    for tool in &mut tools {
        let mut value = serde_json::to_value(&*tool).expect("tool");
        value["annotations"] = json!({"readOnlyHint":false,"destructiveHint":true,"idempotentHint":true,"openWorldHint":false});
        value["_meta"] =
            json!({"securitySchemes":[{"type":"oauth2","scopes":["lvk:read","lvk:write"]}]});
        *tool = serde_json::from_value(value).expect("write tool");
    }
    tools.push(definition::<OperationArgs>("get_operation", "Read this authenticated owner's durable request receipt. Acceptance, domain completion and implementation completion are distinct. Never replay an uncertain operation under a new key."));
    tools
}

impl Remote {
    pub(super) fn operations(&self) -> Operations {
        Operations {
            pool: self.pool.clone(),
            process_instance: self.process_instance,
        }
    }
    pub(super) fn principal_key(&self, principal: &Principal) -> String {
        // JSON framing prevents delimiter collisions; issuer changes never grant
        // access to receipts belonging to the old authorization authority.
        json!([self.config.issuer, principal.subject]).to_string()
    }
    async fn require_status(&self, project: Uuid, status: Uuid) -> anyhow::Result<()> {
        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM local_project_statuses WHERE project_id=? AND id=?)",
        )
        .bind(project)
        .bind(status)
        .fetch_one(&self.pool)
        .await?;
        ensure!(exists, "Status is not in this Project");
        Ok(())
    }
    fn receipt_value(&self, receipt: Receipt) -> Value {
        json!({"operation_id":receipt.id,"request_id":receipt.request_id,"project_id":receipt.project_id,"operation":receipt.operation,"status":receipt.status,"result":receipt.result.and_then(|s| serde_json::from_str::<Value>(&s).ok()),"created_at":receipt.created_at,"updated_at":receipt.updated_at,"url":self.config.project_url(receipt.project_id),"implementation_started":false})
    }
}

fn validate_text(text: &str, max: usize, required: bool) -> anyhow::Result<()> {
    ensure!(
        text.chars().count() <= max && (!required || !text.trim().is_empty()),
        "Issue text is empty or exceeds the tool limit"
    );
    Ok(())
}

// Only our explicit validation errors leave the boundary. Domain errors may
// include SQL, filesystem paths or user-supplied data and are never forwarded.
fn domain_failure(error: ApiError) -> (&'static str, Value) {
    match error {
        ApiError::Conflict(_) => (
            "failed",
            json!({"code":"revision_conflict","message":"Issue changed; read its current requirements before submitting a new update"}),
        ),
        _ => (
            "uncertain",
            json!({"code":"domain_outcome_unavailable","message":"Inspect the Issue and this receipt before any new request"}),
        ),
    }
}
fn validation_failure(error: anyhow::Error) -> (&'static str, Value) {
    let message = if error.downcast_ref::<sqlx::Error>().is_some() {
        "Target validation unavailable".into()
    } else {
        error.to_string()
    };
    (
        "failed",
        json!({"code":"target_validation_failed","message":message}),
    )
}

pub(super) async fn call(
    remote: &Remote,
    principal: &Principal,
    name: &str,
    input: Value,
) -> anyhow::Result<Value> {
    ensure!(principal.allows("lvk:read"), "Read permission required");
    if name == "get_operation" {
        let args: OperationArgs = arguments(input)?;
        remote.require_project(args.project_id).await?;
        let receipt = remote
            .operations()
            .by_request(&remote.principal_key(principal), args.request_id)
            .await?;
        ensure!(
            receipt.project_id == args.project_id,
            "Operation not found in this Project"
        );
        return Ok(remote.receipt_value(receipt));
    }
    ensure!(
        principal.allows("lvk:write"),
        "Write permission required (lvk:write)"
    );
    let owned = remote.clone();
    let key = remote.principal_key(principal);
    let receipt = match name {
        "create_issue" => {
            let args: CreateArgs = arguments(input.clone())?;
            validate_text(&args.title, 500, true)?;
            validate_text(&args.description, 64000, true)?;
            remote.require_project(args.project_id).await?;
            remote.operations().submit(key, args.request_id, args.project_id, name.into(), input, json!({"project_id":args.project_id,"status_id":args.status_id}), move |id| async move {
                if let Err(error) = owned.require_status(args.project_id, args.status_id).await { return validation_failure(error); }
                // The receipt ID is also the domain's client-generated Issue ID.
                let request: CreateIssueRequest = serde_json::from_value(json!({"id":id,"project_id":args.project_id,"status_id":args.status_id,"title":args.title,"description":args.description,"sort_order":0.0,"extension_metadata":null})).expect("Issue request fields");
                match local_remote::create_local_issue(&owned.pool, request).await {
                    Ok(issue) => ("succeeded", json!({"issue_id":issue.id,"url":owned.config.issue_url(issue.project_id, issue.id),"implementation_started":false})),
                    Err(error) => domain_failure(error),
                }
            }).await?
        }
        "update_issue" => {
            let args: UpdateArgs = arguments(input.clone())?;
            if let Some(title) = &args.title {
                validate_text(title, 500, true)?;
            }
            if let Some(description) = &args.description {
                validate_text(description, 64000, false)?;
            }
            ensure!(
                args.title.is_some() || args.description.is_some() || args.status_id.is_some(),
                "No Issue changes supplied"
            );
            let revision = args
                .expected_revision
                .parse::<chrono::DateTime<chrono::Utc>>()
                .or_else(|_| {
                    chrono::NaiveDateTime::parse_from_str(
                        &args.expected_revision,
                        "%Y-%m-%d %H:%M:%S%.f",
                    )
                    .map(|value| value.and_utc())
                })
                .map_err(|_| anyhow::anyhow!("Invalid Issue revision; read get_issue again"))?;
            remote.require_project(args.project_id).await?;
            remote.operations().submit(key, args.request_id, args.project_id, name.into(), input, json!({"project_id":args.project_id,"issue_id":args.issue_id}), move |_| async move {
                if let Err(error) = owned.require_issue(args.project_id, args.issue_id).await { return validation_failure(error); }
                if let Some(status) = args.status_id {
                    if let Err(error) = owned.require_status(args.project_id, status).await { return validation_failure(error); }
                }
                let mut changes = json!({});
                if let Some(title) = args.title { changes["title"] = json!(title); }
                if let Some(description) = args.description { changes["description"] = json!(description); }
                if let Some(status) = args.status_id { changes["status_id"] = json!(status); }
                let request: UpdateIssueRequest = serde_json::from_value(changes).expect("Issue update fields");
                match local_remote::update_local_issue_checked(&owned.pool, args.issue_id, request, Some(revision)).await {
                    Ok(issue) => ("succeeded", json!({"issue_id":issue.id,"url":owned.config.issue_url(issue.project_id, issue.id),"implementation_started":false})),
                    Err(error) => domain_failure(error),
                }
            }).await?
        }
        _ => anyhow::bail!("Unknown Issue operation"),
    };
    Ok(remote.receipt_value(receipt))
}
