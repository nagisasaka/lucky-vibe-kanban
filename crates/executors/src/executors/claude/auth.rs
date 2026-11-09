//! Subscription preflight through the unmodified CLI's public auth metadata.
//! Never read credential files or include CLI output in diagnostics.

use std::{path::Path, process::Stdio, time::Duration};

use serde::Deserialize;
use tokio::process::Command;

use super::ClaudeCode;
use crate::{env::ExecutionEnv, executors::ExecutorError};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AuthStatus {
    logged_in: bool,
    auth_method: String,
    api_provider: Option<String>,
}

fn verify_status(success: bool, stdout: &[u8]) -> Result<(), ExecutorError> {
    let status: AuthStatus = serde_json::from_slice(stdout).map_err(|_| {
        auth_error("Unable to verify Claude CLI authentication. Use a CLI version supporting `claude auth status`, then run `claude auth login` in the LVK execution environment.")
    })?;
    if !success || !status.logged_in {
        return Err(auth_error(
            "Claude CLI login is missing or expired. Run `claude auth login` with your Claude subscription in the LVK execution environment.",
        ));
    }
    if status.auth_method != "claude.ai" || status.api_provider.as_deref() != Some("firstParty") {
        return Err(auth_error(
            "This profile requires a Claude subscription CLI login. Another authentication method or provider is selected. Check `claude auth status` and CLI settings, or choose a different LVK profile.",
        ));
    }
    Ok(())
}

fn auth_error(message: &str) -> ExecutorError {
    ExecutorError::AuthRequired(message.to_owned())
}

impl ClaudeCode {
    pub(super) fn validate_subscription_profile(&self) -> Result<(), ExecutorError> {
        if !self.require_subscription_login.unwrap_or(false) {
            return Ok(());
        }
        // An arbitrary wrapper or extra CLI flags could change authentication
        // after the preflight. Other profiles retain these existing features.
        if self.claude_code_router.unwrap_or(false)
            || self.cmd.base_command_override.is_some()
            || self
                .cmd
                .additional_params
                .as_ref()
                .is_some_and(|args| !args.is_empty())
        {
            return Err(auth_error(
                "Subscription login requires the official `claude` command on PATH without a router, base command override, or additional parameters. Use a separate profile for custom commands.",
            ));
        }
        // Only reject contradictions explicitly saved in this profile. The CLI
        // settings neutralise inherited credentials; public metadata decides
        // the selected method rather than a second provider precedence table.
        for key in [
            "ANTHROPIC_API_KEY",
            "ANTHROPIC_AUTH_TOKEN",
            "ANTHROPIC_CUSTOM_HEADERS",
        ] {
            if self
                .cmd
                .env
                .as_ref()
                .is_some_and(|env| env.contains_key(key))
            {
                return Err(auth_error(&format!(
                    "Subscription login conflicts with profile env {key}. Remove that entry from this profile, or choose another profile."
                )));
            }
        }
        Ok(())
    }

    pub async fn check_subscription_login(
        &self,
        current_dir: &Path,
        env: &ExecutionEnv,
    ) -> Result<(), ExecutorError> {
        if !self.require_subscription_login.unwrap_or(false) {
            return Ok(());
        }
        self.validate_subscription_profile()?;
        let effective_env = env.clone().with_profile(&self.cmd);
        let (program, args) = super::command_adapter::ClaudeCodeCommandAdapter::new(self)
            .build_auth_probe()
            .build_initial()?
            .into_resolved()
            .await?;
        let mut command = Command::new(program);
        command
            .args(args)
            .current_dir(current_dir)
            .kill_on_drop(true)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        effective_env.apply_to_command(&mut command);
        run_auth_probe(command).await
    }
}

async fn run_auth_probe(mut command: Command) -> Result<(), ExecutorError> {
    let output = tokio::time::timeout(Duration::from_secs(10), command.output())
            .await
            .map_err(|_| auth_error("Claude CLI authentication check timed out. Check `claude auth status` in the LVK execution environment."))?
            .map_err(|_| auth_error("Unable to run `claude auth status`. Install the official Claude CLI in the LVK execution environment and log in."))?;
    verify_status(output.status.success(), &output.stdout)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn subscription_agent() -> ClaudeCode {
        serde_json::from_value(serde_json::json!({"require_subscription_login":true})).unwrap()
    }

    fn clean_env() -> ExecutionEnv {
        ExecutionEnv::new(crate::env::RepoContext::default())
    }

    #[test]
    fn rejects_only_explicit_profile_credential_entries_without_reporting_values() {
        for key in [
            "ANTHROPIC_API_KEY",
            "ANTHROPIC_AUTH_TOKEN",
            "ANTHROPIC_CUSTOM_HEADERS",
        ] {
            for value in ["", "private-profile-value"] {
                let mut agent = subscription_agent();
                agent.cmd.env = Some(std::collections::HashMap::from([(
                    key.into(),
                    value.into(),
                )]));
                let error = agent
                    .validate_subscription_profile()
                    .unwrap_err()
                    .to_string();
                assert!(error.contains(key), "{error}");
                assert!(!error.contains("private-profile-value"));
                agent.require_subscription_login = None;
                assert!(agent.validate_subscription_profile().is_ok());
            }
        }
    }

    #[test]
    fn other_authentication_inputs_are_left_to_cli_status() {
        let mut agent = subscription_agent();
        agent.cmd.env = Some(std::collections::HashMap::from([
            ("CLAUDE_CODE_OAUTH_TOKEN".into(), "private-token".into()),
            ("CLAUDE_CODE_USE_BEDROCK".into(), "1".into()),
        ]));
        assert!(agent.validate_subscription_profile().is_ok());
        // This is not support for token authentication or a cloud provider.
        // verify_status still rejects their reported methods/providers.
        assert!(
            verify_status(
                true,
                br#"{"loggedIn":true,"authMethod":"oauth_token","apiProvider":"firstParty"}"#
            )
            .is_err()
        );
        assert!(
            verify_status(
                true,
                br#"{"loggedIn":true,"authMethod":"third_party","apiProvider":"bedrock"}"#
            )
            .is_err()
        );
    }

    #[tokio::test]
    async fn refuses_wrappers_router_and_extra_arguments_before_spawning() {
        for profile in [
            serde_json::json!({"claude_code_router":true}),
            serde_json::json!({"base_command_override":"private-wrapper"}),
            serde_json::json!({"additional_params":["--settings", "private-file"]}),
        ] {
            let mut profile = profile;
            profile["require_subscription_login"] = true.into();
            let agent: ClaudeCode = serde_json::from_value(profile).unwrap();
            let error = agent
                .check_subscription_login(Path::new("."), &clean_env())
                .await
                .unwrap_err()
                .to_string();
            assert!(error.contains("official `claude`"));
            assert!(!error.contains("private-"));
        }
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn effort_probe_never_executes_a_rejected_wrapper() {
        let temp = tempfile::tempdir().unwrap();
        let marker = temp.path().join("executed");
        let wrapper = temp.path().join("wrapper.sh");
        std::fs::write(
            &wrapper,
            format!(
                "touch {}\n",
                shlex::try_quote(marker.to_str().unwrap()).unwrap()
            ),
        )
        .unwrap();
        let mut agent = subscription_agent();
        agent.cmd.base_command_override = Some(format!(
            "/bin/sh {}",
            shlex::try_quote(wrapper.to_str().unwrap()).unwrap()
        ));
        assert!(agent.supports_effort_flag().await.is_err());
        assert!(!marker.exists());
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn cli_stub_checks_exit_status_and_suppresses_diagnostics() {
        for (json, exit, succeeds) in [
            (
                r#"{"loggedIn":true,"authMethod":"claude.ai","apiProvider":"firstParty"}"#,
                "0",
                true,
            ),
            (r#"{"loggedIn":false,"authMethod":"none"}"#, "1", false),
            (
                r#"{"loggedIn":true,"authMethod":"api_key_helper","apiProvider":"firstParty"}"#,
                "0",
                false,
            ),
            ("not-json-private-value", "0", false),
        ] {
            let mut command = Command::new("/bin/sh");
            command
                .args([
                    "-c",
                    "printf '%s' \"$1\"; printf 'private-stderr' >&2; exit \"$2\"",
                    "fixture",
                    json,
                    exit,
                ])
                .kill_on_drop(true)
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::null());
            let result = run_auth_probe(command).await;
            assert_eq!(result.is_ok(), succeeds);
            if let Err(error) = result {
                assert!(!error.to_string().contains("private-"));
            }
        }
    }

    #[test]
    fn accepts_only_confirmed_first_party_claude_login() {
        assert!(
            verify_status(
                true,
                br#"{"loggedIn":true,"authMethod":"claude.ai","apiProvider":"firstParty"}"#
            )
            .is_ok()
        );
        for method in [
            "none",
            "api_key",
            "api_key_helper",
            "oauth_token",
            "third_party",
            "unknown",
        ] {
            let output =
                serde_json::json!({"loggedIn":true,"authMethod":method,"apiProvider":"firstParty"});
            assert!(verify_status(true, &serde_json::to_vec(&output).unwrap()).is_err());
        }
        assert!(
            verify_status(
                true,
                br#"{"loggedIn":true,"authMethod":"claude.ai","apiProvider":"bedrock"}"#
            )
            .is_err()
        );
        assert!(verify_status(true, br#"{"loggedIn":true,"authMethod":"claude.ai"}"#).is_err());
    }

    #[test]
    fn fails_closed_without_exposing_cli_output() {
        for output in [
            br#"{"loggedIn":false,"authMethod":"none"}"#.as_slice(),
            br#"{"unexpected":"private-value"}"#.as_slice(),
            b"private-value".as_slice(),
        ] {
            let error = verify_status(true, output).unwrap_err().to_string();
            assert!(!error.contains("private-value"));
        }
        assert!(
            verify_status(
                false,
                br#"{"loggedIn":true,"authMethod":"claude.ai","apiProvider":"firstParty"}"#
            )
            .is_err()
        );
    }
}
