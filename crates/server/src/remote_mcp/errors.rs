//! Public diagnostics must be static validation text or an explicitly safe typed
//! hostname diagnostic, never a generic error or its context chain.
pub(super) fn message(error: &anyhow::Error) -> String {
    if let Some(message) = crate::routes::attachment_transfer::host_file_diagnostic(error) {
        return message;
    }
    error.downcast_ref::<&'static str>().copied().unwrap_or(
        "LVK operation unavailable; refresh state and inspect the existing request before retrying",
    ).to_owned()
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
