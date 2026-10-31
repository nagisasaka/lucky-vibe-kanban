//! Public diagnostics must be static validation text or an explicitly safe typed
//! hostname diagnostic, never a generic error or its context chain.
pub(super) fn message(error: &anyhow::Error) -> String {
    if let Some(diagnostic) = error.downcast_ref::<utils::mcp_error::Diagnostic>() {
        return diagnostic.message.to_owned();
    }
    if let Some(message) = crate::routes::attachment_transfer::host_file_diagnostic(error) {
        return message;
    }
    error.downcast_ref::<&'static str>().copied().unwrap_or(
        "LVK operation unavailable; refresh state and inspect the existing request before retrying",
    ).to_owned()
}

pub(super) fn rejection(category: utils::mcp_error::Category) -> anyhow::Error {
    utils::mcp_error::Diagnostic::new(category, utils::mcp_error::Outcome::Failed).into()
}

pub(super) fn rejection_with_message(
    category: utils::mcp_error::Category,
    message: &'static str,
) -> anyhow::Error {
    let mut diagnostic =
        utils::mcp_error::Diagnostic::new(category, utils::mcp_error::Outcome::Failed);
    diagnostic.message = message;
    diagnostic.into()
}

pub(super) fn unknown() -> anyhow::Error {
    utils::mcp_error::Diagnostic::new(
        utils::mcp_error::Category::OutcomeUnknown,
        utils::mcp_error::Outcome::Unknown,
    )
    .into()
}

pub(super) fn diagnostic(error: &anyhow::Error, mutation: bool) -> utils::mcp_error::Diagnostic {
    use utils::mcp_error::{Category, Diagnostic, Outcome};
    if let Some(value) = error.downcast_ref::<Diagnostic>() {
        return value.clone();
    }
    let category = if let Some(error) = error.downcast_ref::<crate::error::ApiError>() {
        match error {
            crate::error::ApiError::Conflict(_) => Category::Conflict,
            crate::error::ApiError::Unauthorized | crate::error::ApiError::Forbidden(_) => {
                Category::Permission
            }
            crate::error::ApiError::BadRequest(_) | crate::error::ApiError::PayloadTooLarge => {
                Category::Validation
            }
            crate::error::ApiError::TooManyRequests(_) => Category::Busy,
            _ => Category::Internal,
        }
    } else if let Some(error) = error.downcast_ref::<sqlx::Error>() {
        super::todos::database_category(error)
    } else {
        Category::Internal
    };
    // An error type alone never proves that an arbitrary mutation had no
    // effects. Proven preflight/transaction failures carry a typed Diagnostic.
    let outcome = if mutation {
        Outcome::Unknown
    } else {
        Outcome::Failed
    };
    Diagnostic::new(category, outcome)
}

/// Existing domain codes stay intact; only known code facts are classified.
/// Unknown codes are never interpreted by matching user-controlled prose.
pub(super) fn receipt_diagnostic(
    status: &str,
    result: Option<&serde_json::Value>,
) -> utils::mcp_error::Diagnostic {
    use utils::mcp_error::{Category, Diagnostic, Outcome};
    if status == "uncertain" {
        return Diagnostic::new(Category::OutcomeUnknown, Outcome::Unknown);
    }
    let category = match result.and_then(|v| v["code"].as_str()) {
        Some(
            "revision_conflict"
            | "state_conflict"
            | "question_conflict"
            | "goal_state_conflict"
            | "comment_revision_conflict"
            | "workspace_state_conflict",
        ) => Category::Conflict,
        Some(
            "comment_owner_required"
            | "repository_scope_changed"
            | "selection_scope_changed"
            | "workspace_scope_changed",
        ) => Category::Permission,
        Some(
            "target_validation_failed" | "admission_validation_failed" | "attachments_not_verified",
        ) => Category::Validation,
        Some("target_unavailable" | "repository_unavailable") => Category::NotFound,
        Some("attachment_service_unavailable") => Category::Unavailable,
        _ => Category::Internal,
    };
    Diagnostic::new(category, Outcome::Failed)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn diagnostic_chains_and_dynamic_domain_messages_do_not_leave_the_boundary() {
        let validation = anyhow::anyhow!("Run changed; refresh before acting");
        assert_eq!(message(&validation), "Run changed; refresh before acting");
        let marker = "/private/credentials/token-123".to_owned();
        let errors = [
            anyhow::Error::from(std::io::Error::other(marker.clone())),
            anyhow::Error::from(sqlx::Error::Protocol(marker.clone())).context(marker.clone()),
            anyhow::Error::from(crate::error::ApiError::BadRequest(marker.clone())),
            anyhow::anyhow!(marker.clone()),
        ];
        for error in errors {
            assert!(!message(&error).contains(&marker));
        }
        assert_eq!(
            message(&validation.context(marker)),
            "Run changed; refresh before acting"
        );
    }
}
