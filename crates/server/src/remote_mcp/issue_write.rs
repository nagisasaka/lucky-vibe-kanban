//! Ordinary Issue mutations, separate from execution. Domain writes reuse the
//! local Board services; the remote boundary owns scope and request receipts.
use anyhow::ensure;
use api_types::issue::{CreateIssueRequest, UpdateIssueRequest};
use rmcp::model::Tool;
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};
use utils::mcp_error::Category;
use uuid::Uuid;

use super::{
    Remote,
    auth::Principal,
    issue_mutations::{Mutation, domain_failure, parse_revision},
    operations::{Operations, Receipt},
    read::{arguments, definition},
};
#[cfg(test)]
use crate::error::ApiError;
use crate::routes::local_remote;

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
    /// Defaults to medium, including legacy explicit null.
    priority: Option<api_types::IssuePriority>,
    #[serde(default)]
    #[schemars(length(max = 50))]
    tag_ids: Vec<Uuid>,
    /// Optional parent in the same Project; does not start implementation.
    parent_issue_id: Option<Uuid>,
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
    /// Omitted/null preserves priority. Set medium explicitly to reset.
    priority: Option<api_types::IssuePriority>,
    /// Set or replace the parent. Omitted/null preserves the existing parent.
    parent_issue_id: Option<Uuid>,
    /// Explicitly detach only the parent relationship. Cannot combine with a parent ID.
    clear_parent: Option<bool>,
}
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct DeleteArgs {
    project_id: Uuid,
    request_id: Uuid,
    issue_id: Uuid,
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
            "Create a normal Issue containing agreed requirements; does not create a Workspace or start implementation. Optional parent_issue_id creates a subissue in the same Project. Read get_issue to verify its parent. Use one stable request_id and inspect get_operation after a lost response.",
        ),
        definition::<DeleteArgs>(
            "delete_issue",
            "Permanently delete an Issue and related database records according to existing Board constraints. Unlike archive, the Issue cannot be restored. Does not physically delete Workspace directories or worktrees. Obtain explicit user authorization for this target and respect host confirmation controls. Reuse the exact request_id after response loss and inspect get_operation; never retry an unknown outcome with a new ID.",
        ),
        definition::<UpdateArgs>(
            "update_issue",
            "Apply Issue changes, including conversation-based priority classification, against the revision from get_issue. Does not steer existing runs or start implementation. Omitted fields and priority:null stay unchanged; set priority:\"medium\" explicitly to reset. parent_issue_id sets/replaces a parent; null preserves it. Only clear_parent:true detaches it. Read get_issue before and after; parent cycles are rejected.",
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
        .await
        .map_err(|error| super::errors::rejection(super::errors::database_category(&error)))?;
        ensure!(
            exists,
            super::errors::rejection_with_message(
                utils::mcp_error::Category::NotFound,
                "Status is not in this Project"
            )
        );
        Ok(())
    }
    pub(super) fn receipt_value(&self, receipt: Receipt) -> Value {
        super::issue_mutations::receipt_value(receipt, |project| self.config.project_url(project))
    }
}

fn validate_text(text: &str, max: usize, required: bool) -> anyhow::Result<()> {
    ensure!(
        text.chars().count() <= max && (!required || !text.trim().is_empty()),
        super::errors::rejection_with_message(
            utils::mcp_error::Category::Validation,
            "Issue text is empty or exceeds the tool limit"
        )
    );
    Ok(())
}

fn validation_failure(error: anyhow::Error) -> (&'static str, Value) {
    let error = match error.downcast::<sqlx::Error>() {
        Ok(error) => return domain_failure(local_remote::IssueMutationError::Failed(error.into())),
        Err(error) => error,
    };
    let message = super::errors::message(&error);
    let diagnostic = super::errors::diagnostic(&error, false);
    (
        "failed",
        json!({"code":"target_validation_failed","message":message,"error":diagnostic.value()}),
    )
}

pub(super) async fn call(
    remote: &Remote,
    principal: &Principal,
    name: &str,
    input: Value,
) -> anyhow::Result<Value> {
    ensure!(
        principal.allows("lvk:read"),
        super::errors::rejection_with_message(
            utils::mcp_error::Category::Permission,
            "Read permission required"
        )
    );
    if name == "get_operation" {
        let args: OperationArgs = arguments(input)?;
        remote.require_project(args.project_id).await?;
        let receipt = remote
            .operations()
            .by_request(&remote.principal_key(principal), args.request_id)
            .await?;
        ensure!(
            receipt.project_id == args.project_id,
            super::errors::rejection_with_message(
                utils::mcp_error::Category::NotFound,
                "Operation not found in this Project"
            )
        );
        if matches!(
            receipt.operation.as_str(),
            "run_process" | "stop_process" | "change_runtime_path" | "self_restart"
        ) {
            ensure!(
                principal.allows("lvk:runtime:read"),
                super::errors::rejection_with_message(
                    utils::mcp_error::Category::Permission,
                    "Runtime read permission required"
                )
            );
        }
        return Ok(remote.receipt_value(receipt));
    }
    ensure!(
        principal.allows("lvk:write"),
        super::errors::rejection_with_message(
            utils::mcp_error::Category::Permission,
            "Write permission required (lvk:write)"
        )
    );
    let owned = remote.clone();
    let key = remote.principal_key(principal);
    let receipt = match name {
        "create_issue" => {
            let args: CreateArgs = arguments(input.clone())?;
            ensure!(
                args.tag_ids.len() <= 50,
                super::errors::rejection_with_message(
                    utils::mcp_error::Category::Validation,
                    "At most 50 tags per Issue creation"
                )
            );
            validate_text(&args.title, 500, true)?;
            validate_text(&args.description, 64000, true)?;
            remote.require_project(args.project_id).await?;
            remote.operations().submit(key, args.request_id, args.project_id, name.into(), input, json!({"project_id":args.project_id,"status_id":args.status_id}), move |id| async move {
                if let Err(error) = owned.require_status(args.project_id, args.status_id).await { return validation_failure(error); }
                // The receipt ID is also the domain's client-generated Issue ID.
                let request: CreateIssueRequest = serde_json::from_value(json!({"id":id,"project_id":args.project_id,"status_id":args.status_id,"title":args.title,"description":args.description,"priority":args.priority,"sort_order":0.0,"extension_metadata":null,"parent_issue_id":args.parent_issue_id})).expect("Issue request fields");
                let (status, mut value) = Mutation::Create { request, tag_ids: args.tag_ids }.apply(&owned.pool, id).await;
                if status == "succeeded" { value["url"] = json!(owned.config.issue_url(args.project_id, id)); }
                (status, value)
            }).await?
        }
        "delete_issue" => {
            let args: DeleteArgs = arguments(input.clone())?;
            remote.require_project(args.project_id).await?;
            remote
                .operations()
                .submit(
                    key,
                    args.request_id,
                    args.project_id,
                    name.into(),
                    input,
                    json!({"project_id":args.project_id,"issue_id":args.issue_id}),
                    move |_| async move {
                        Mutation::Delete {
                            issue_id: args.issue_id,
                            project_id: args.project_id,
                            missing_is_success: false,
                        }
                        .apply(&owned.pool, Uuid::nil())
                        .await
                    },
                )
                .await?
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
                args.title.is_some()
                    || args.description.is_some()
                    || args.status_id.is_some()
                    || input.get("priority").is_some()
                    || args.parent_issue_id.is_some()
                    || args.clear_parent == Some(true),
                super::errors::rejection_with_message(
                    utils::mcp_error::Category::Validation,
                    "No Issue changes supplied"
                )
            );
            ensure!(
                !(args.parent_issue_id.is_some() && args.clear_parent == Some(true)),
                super::errors::rejection(Category::Validation)
            );
            parse_revision(&args.expected_revision)?;
            remote.require_project(args.project_id).await?;
            remote
                .operations()
                .submit(
                    key,
                    args.request_id,
                    args.project_id,
                    name.into(),
                    input,
                    json!({"project_id":args.project_id,"issue_id":args.issue_id}),
                    move |_| async move {
                        if let Err(error) =
                            owned.require_issue(args.project_id, args.issue_id).await
                        {
                            return validation_failure(error);
                        }
                        if let Some(status) = args.status_id
                            && let Err(error) = owned.require_status(args.project_id, status).await
                        {
                            return validation_failure(error);
                        }
                        let mut changes = json!({});
                        if let Some(title) = args.title {
                            changes["title"] = json!(title);
                        }
                        if let Some(description) = args.description {
                            changes["description"] = json!(description);
                        }
                        if let Some(status) = args.status_id {
                            changes["status_id"] = json!(status);
                        }
                        if let Some(priority) = args.priority {
                            changes["priority"] = json!(priority);
                        }
                        if let Some(parent) = args.parent_issue_id {
                            changes["parent_issue_id"] = json!(parent);
                        }
                        if args.clear_parent == Some(true) {
                            changes["parent_issue_id"] = Value::Null;
                        }
                        let request: UpdateIssueRequest =
                            serde_json::from_value(changes).expect("Issue update fields");
                        let (status, mut value) = Mutation::Update {
                            issue_id: args.issue_id,
                            changes: request,
                            expected_revision: Some(args.expected_revision),
                        }
                        .apply(&owned.pool, Uuid::nil())
                        .await;
                        if status == "succeeded" {
                            value["url"] =
                                json!(owned.config.issue_url(args.project_id, args.issue_id));
                        }
                        (status, value)
                    },
                )
                .await?
        }
        _ => return Err(super::errors::rejection(Category::Validation)),
    };
    Ok(remote.receipt_value(receipt))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_issue_text_is_a_proven_preflight_rejection() {
        let error = validate_text(" ", 500, true).unwrap_err();
        let diagnostic = super::super::errors::diagnostic(&error, true);
        assert_eq!(diagnostic.category, Category::Validation);
        assert_eq!(diagnostic.outcome, utils::mcp_error::Outcome::Failed);
        let (_, value) = validation_failure(super::super::errors::rejection(Category::NotFound));
        assert_eq!(value["error"]["category"], "not_found");
        assert_eq!(value["error"]["outcome"], "failed");
    }

    #[test]
    fn domain_failures_preserve_terminal_evidence_without_exposing_details() {
        use local_remote::IssueMutationError::{Failed, OutcomeUnknown};
        for (error, expected_status, expected_code) in [
            (
                Failed(ApiError::BadRequest(
                    "private-token /host/path SELECT secret".into(),
                )),
                "failed",
                "validation_failed",
            ),
            (
                Failed(ApiError::Conflict("private-token".into())),
                "failed",
                "revision_conflict",
            ),
            (
                Failed(ApiError::BadRequest("Issue not found".into())),
                "failed",
                "not_found",
            ),
            (
                Failed(ApiError::Forbidden("private-token".into())),
                "failed",
                "permission_denied",
            ),
            (
                Failed(ApiError::Database(sqlx::Error::PoolClosed)),
                "failed",
                "unavailable",
            ),
            (
                Failed(ApiError::Io(std::io::Error::other("private-token"))),
                "failed",
                "internal_error",
            ),
            (
                OutcomeUnknown(ApiError::BadRequest("private-token".into())),
                "uncertain",
                "domain_outcome_unavailable",
            ),
        ] {
            let (status, value) = domain_failure(error);
            assert_eq!(status, expected_status);
            assert_eq!(value["code"], expected_code);
            assert!(!value.to_string().contains("private-token"));
            assert!(!value.to_string().contains("/host/path"));
            assert!(!value.to_string().contains("SELECT"));
        }
    }

    #[tokio::test]
    async fn writer_contention_is_a_definite_failure_without_sql_details() {
        let dir = tempfile::tempdir().unwrap();
        let options = sqlx::sqlite::SqliteConnectOptions::new()
            .filename(dir.path().join("private.sqlite"))
            .create_if_missing(true)
            .busy_timeout(std::time::Duration::from_millis(25));
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(2)
            .connect_with(options)
            .await
            .unwrap();
        sqlx::query("CREATE TABLE private_input(value TEXT)")
            .execute(&pool)
            .await
            .unwrap();
        let held = pool.begin_with("BEGIN IMMEDIATE").await.unwrap();
        let error = sqlx::query("INSERT INTO private_input VALUES('SECRET')")
            .execute(&pool)
            .await
            .unwrap_err();
        let (status, value) =
            domain_failure(local_remote::IssueMutationError::Failed(error.into()));
        assert_eq!(status, "failed");
        assert_eq!(value["error"]["category"], "busy");
        assert_eq!(value["error"]["outcome"], "failed");
        for marker in ["INSERT", "private_input", "SECRET", "private.sqlite"] {
            assert!(!value.to_string().contains(marker));
        }
        held.rollback().await.unwrap();
        let count: i64 = sqlx::query_scalar("SELECT count(*) FROM private_input")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 0);
        pool.close().await;
    }

    #[tokio::test]
    async fn unreadable_receipts_never_report_success_or_leak_the_damaged_payload() {
        let (remote, _, principal, target, _) = super::super::runtime_write_tests::fixture().await;
        let request = Uuid::new_v4();
        let targets = json!({"workspace_id":target.workspace_id});
        let receipt = remote
            .operations()
            .submit_and_wait(
                remote.principal_key(&principal),
                request,
                target.project_id,
                "fixture".into(),
                json!({}),
                targets.clone(),
                |_| async { ("succeeded", json!({"accepted":true})) },
            )
            .await
            .unwrap();
        let operation = receipt.id;
        let valid = remote.receipt_value(receipt);
        assert_eq!(valid["status"], "succeeded");
        assert_eq!(valid["targets"], targets);
        for (stored_targets, stored_result, status) in [
            (
                targets.to_string(),
                Some("{private-token-not-json"),
                "succeeded",
            ),
            (targets.to_string(), None, "succeeded"),
            (targets.to_string(), Some("null"), "failed"),
            ("private-token-not-json".into(), Some("{}"), "succeeded"),
        ] {
            sqlx::query("UPDATE remote_mcp_operations SET targets=?,result=?,status=? WHERE id=?")
                .bind(stored_targets)
                .bind(stored_result)
                .bind(status)
                .bind(operation)
                .execute(&remote.pool)
                .await
                .unwrap();
            let value = call(
                &remote,
                &principal,
                "get_operation",
                json!({"project_id":target.project_id,"request_id":request}),
            )
            .await
            .unwrap();
            assert_eq!(value["status"], "uncertain");
            assert_eq!(value["result"]["code"], "receipt_unreadable");
            assert_eq!(value["operation_id"], operation.to_string());
            assert!(!value.to_string().contains("private-token"));
        }
        sqlx::query("UPDATE remote_mcp_operations SET targets=?,result=?,status='failed' WHERE id=?")
            .bind(json!({"workspace_id":target.workspace_id,"purpose":"SECRET user text","target":{"path":"/private/SECRET"}}).to_string())
            .bind(json!({"code":"resource_busy","path":"/private/SECRET","partial_possible":true}).to_string()).bind(operation)
            .execute(&remote.pool).await.unwrap();
        let safe = remote.receipt_value(
            remote
                .operations()
                .by_request(&remote.principal_key(&principal), request)
                .await
                .unwrap(),
        );
        assert_eq!(
            safe["targets"]["workspace_id"],
            target.workspace_id.to_string()
        );
        assert_eq!(safe["result"]["partial_possible"], true);
        assert!(!safe.to_string().contains("SECRET"));
        assert!(!safe.to_string().contains("/private"));
        // Stored diagnostic fields are untrusted, even in a valid JSON object.
        for damaged in [
            json!("SELECT credential FROM /private/SECRET"),
            json!(["SECRET"]),
            json!({"category":"conflict","outcome":"failed","message":"SECRET","extra":"/private"}),
        ] {
            sqlx::query(
                "UPDATE remote_mcp_operations SET targets=?,result=?,status='uncertain' WHERE id=?",
            )
            .bind(targets.to_string())
            .bind(json!({"error":damaged}).to_string())
            .bind(operation)
            .execute(&remote.pool)
            .await
            .unwrap();
            let value = remote.receipt_value(
                remote
                    .operations()
                    .by_request(&remote.principal_key(&principal), request)
                    .await
                    .unwrap(),
            );
            assert_eq!(value["error"]["outcome"], "unknown");
            assert_eq!(value["result"]["error"]["outcome"], "unknown");
            assert!(!value.to_string().contains("SECRET"));
            assert!(!value.to_string().contains("/private"));
        }
        // A pending receipt has no final result yet; that absence is normal.
        sqlx::query(
            "UPDATE remote_mcp_operations SET targets=?,result=NULL,status='running' WHERE id=?",
        )
        .bind(targets.to_string())
        .bind(operation)
        .execute(&remote.pool)
        .await
        .unwrap();
        let value = call(
            &remote,
            &principal,
            "get_operation",
            json!({"project_id":target.project_id,"request_id":request}),
        )
        .await
        .unwrap();
        assert_eq!(value["status"], "running");
        assert!(value["result"].is_null());
        let count: i64 = sqlx::query_scalar("SELECT count(*) FROM remote_mcp_operations")
            .fetch_one(&remote.pool)
            .await
            .unwrap();
        assert_eq!(count, 1);
    }
}
