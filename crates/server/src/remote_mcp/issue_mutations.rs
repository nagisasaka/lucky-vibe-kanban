//! Shared Issue mutation/CAS/receipt service. Authentication and legacy input
//! resolution belong to the adapters; no caller-provided identity is accepted here.
use api_types::{CreateIssueRequest, UpdateIssueRequest};
use serde_json::{Value, json};
use sqlx::SqlitePool;
use utils::mcp_error::Category;
use uuid::Uuid;

use super::operations::Receipt;
use crate::{error::ApiError, routes::local_remote};

pub(super) enum Mutation {
    Create {
        request: CreateIssueRequest,
        tag_ids: Vec<Uuid>,
    },
    Update {
        issue_id: Uuid,
        changes: UpdateIssueRequest,
        expected_revision: Option<String>,
    },
    Delete {
        issue_id: Uuid,
        project_id: Uuid,
        missing_is_success: bool,
    },
}
impl Mutation {
    pub(super) async fn apply(self, pool: &SqlitePool, id: Uuid) -> (&'static str, Value) {
        let result = match self {
            Self::Create {
                mut request,
                tag_ids,
            } => {
                request.id = Some(id);
                local_remote::create_local_issue_with_tags_outcome(pool, request, &tag_ids).await
            }
            Self::Update {
                issue_id,
                changes,
                expected_revision,
            } => {
                let revision = match expected_revision.as_deref().map(parse_revision).transpose() {
                    Ok(value) => value,
                    Err(_) => {
                        return domain_failure(local_remote::IssueMutationError::Failed(
                            ApiError::BadRequest("Invalid revision".into()),
                        ));
                    }
                };
                local_remote::update_local_issue_checked_outcome(pool, issue_id, changes, revision)
                    .await
            }
            Self::Delete {
                issue_id,
                project_id,
                missing_is_success,
            } => {
                return match local_remote::delete_local_issue_outcome(
                    pool,
                    issue_id,
                    Some(project_id),
                )
                .await
                {
                    Ok(true) => ("succeeded", json!({"deleted_issue_id":issue_id})),
                    Ok(false) if missing_is_success => {
                        ("succeeded", json!({"deleted_issue_id":issue_id}))
                    }
                    Ok(false) => domain_failure(local_remote::IssueMutationError::Failed(
                        ApiError::BadRequest("Issue not found".into()),
                    )),
                    Err(error) => domain_failure(error),
                };
            }
        };
        match result {
            Ok(issue) => (
                "succeeded",
                json!({"issue_id":issue.id,"implementation_started":false}),
            ),
            Err(error) => domain_failure(error),
        }
    }
}

pub(super) fn receipt_value(receipt: Receipt, project_url: impl Fn(Uuid) -> String) -> Value {
    let object = |raw: &str| {
        serde_json::from_str::<Value>(raw)
            .ok()
            .filter(Value::is_object)
    };
    let targets = object(&receipt.targets);
    let result = receipt.result.as_deref().and_then(object);
    // Older saved successful design-note receipts may contain full snapshots.
    // Project at every delivery (including get_operation/retry), without rewriting
    // the stored evidence or replacing committed revisions with current state.
    let result = if receipt.operation == "write_design_note" && receipt.status == "succeeded" {
        result
            .and_then(|v| {
                serde_json::from_value::<db::models::design_note::DesignNoteReceipt>(v).ok()
            })
            .and_then(|v| serde_json::to_value(v).ok())
    } else {
        result
    };
    let unreadable = targets.is_none()
        || (receipt.result.is_some() && result.is_none())
        || (matches!(receipt.status.as_str(), "succeeded" | "failed") && result.is_none());
    let status = if unreadable {
        "uncertain"
    } else {
        &receipt.status
    };
    let result = if unreadable {
        Some(
            json!({"code":"receipt_unreadable","message":"Stored operation evidence is incomplete or unreadable. Inspect these targets and the original request ID before any new request."}),
        )
    } else {
        result
    };
    let mut result = result;
    let diagnostic = if matches!(status, "failed" | "uncertain") {
        use utils::mcp_error::{Category, Diagnostic, Outcome};
        let category = result
            .as_ref()
            .and_then(|v| v.get("error"))
            .and_then(|v| v.get("category"))
            .and_then(|v| serde_json::from_value::<Category>(v.clone()).ok());
        let mut diagnostic = if status == "uncertain" {
            Diagnostic::new(Category::OutcomeUnknown, Outcome::Unknown)
        } else if let Some(category) = category {
            Diagnostic::new(category, Outcome::Failed)
        } else {
            super::errors::receipt_diagnostic(status, result.as_ref())
        };
        diagnostic.correlation_id = receipt.id;
        let diagnostic = diagnostic.value();
        if let Some(result) = result.as_mut().and_then(Value::as_object_mut)
            && result.contains_key("error")
        {
            result.insert("error".into(), diagnostic.clone());
        }
        Some(diagnostic)
    } else {
        None
    };
    let mut value = json!({"operation_id":receipt.id,"request_id":receipt.request_id,"project_id":receipt.project_id,"operation":receipt.operation,"targets":targets,"status":status,"result":result,"created_at":receipt.created_at,"updated_at":receipt.updated_at,"url":project_url(receipt.project_id)});
    if let Some(diagnostic) = diagnostic {
        value["error"] = diagnostic;
        utils::mcp_error::redact_failure_context(&mut value);
    }
    value
}

pub(super) fn parse_revision(value: &str) -> anyhow::Result<chrono::DateTime<chrono::Utc>> {
    value
        .parse::<chrono::DateTime<chrono::Utc>>()
        .or_else(|_| {
            chrono::NaiveDateTime::parse_from_str(value, "%Y-%m-%d %H:%M:%S%.f")
                .map(|v| v.and_utc())
        })
        .map_err(|_| super::errors::rejection(Category::Validation))
}

// Only our explicit validation errors leave the boundary. Domain errors may
// include SQL, filesystem paths or user-supplied data and are never forwarded.
pub(super) fn domain_failure(error: local_remote::IssueMutationError) -> (&'static str, Value) {
    use utils::mcp_error::{Category, Diagnostic, Outcome};
    let (status, category, outcome, code, message) = match error {
        local_remote::IssueMutationError::OutcomeUnknown(_) => (
            "uncertain",
            Category::OutcomeUnknown,
            Outcome::Unknown,
            "domain_outcome_unavailable",
            "Inspect this receipt and the Issue using the original request ID before any new request.",
        ),
        local_remote::IssueMutationError::Failed(error) => {
            let (category, code, message) = match error {
                ApiError::Conflict(_) => (
                    Category::Conflict,
                    "revision_conflict",
                    "Issue changed; read its current requirements before submitting a new update.",
                ),
                ApiError::BadRequest(ref message) if message == "Issue not found" => {
                    (Category::NotFound, "not_found", "Issue was not found.")
                }
                ApiError::BadRequest(_) | ApiError::PayloadTooLarge => (
                    Category::Validation,
                    "validation_failed",
                    "Issue changes failed validation.",
                ),
                ApiError::Unauthorized | ApiError::Forbidden(_) => (
                    Category::Permission,
                    "permission_denied",
                    "Permission is required for this operation.",
                ),
                ApiError::Database(sqlx::Error::RowNotFound) => (
                    Category::NotFound,
                    "not_found",
                    "The requested target was not found.",
                ),
                ApiError::Database(sqlx::Error::Database(ref error))
                    if error
                        .code()
                        .and_then(|code| code.parse::<i32>().ok())
                        .is_some_and(|code| matches!(code & 0xff, 5 | 6)) =>
                {
                    (
                        Category::Busy,
                        "resource_busy",
                        "The database is busy; this Issue write was not committed.",
                    )
                }
                ApiError::Database(sqlx::Error::PoolTimedOut | sqlx::Error::PoolClosed) => (
                    Category::Unavailable,
                    "unavailable",
                    "The database is unavailable; this Issue write was not committed.",
                ),
                _ => (
                    Category::Internal,
                    "internal_error",
                    "The Issue write failed before commit.",
                ),
            };
            ("failed", category, Outcome::Failed, code, message)
        }
    };
    (
        status,
        json!({"code":code,"message":message,"error":Diagnostic::new(category, outcome).value()}),
    )
}
