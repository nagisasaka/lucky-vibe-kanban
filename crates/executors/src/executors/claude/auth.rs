//! Subscription preflight through the unmodified CLI's public auth metadata.
//! Never read credential files or include CLI output in diagnostics.

use std::{path::Path, process::Stdio, time::Duration};

use serde::Deserialize;
use tokio::process::Command;

use super::ClaudeCode;
use crate::{command::CommandBuilder, env::ExecutionEnv, executors::ExecutorError};

// Reject ambiguity rather than silently removing credentials the user configured.
const CONFLICTING_ENV: &[&str] = &[
    "ANTHROPIC_API_KEY",
    "ANTHROPIC_AUTH_TOKEN",
    "ANTHROPIC_CUSTOM_HEADERS",
    "ANTHROPIC_BASE_URL",
    "ANTHROPIC_PROFILE",
    "ANTHROPIC_BEDROCK_BASE_URL",
    "ANTHROPIC_VERTEX_BASE_URL",
    "CLAUDE_CODE_OAUTH_TOKEN",
    "CLAUDE_CODE_USE_BEDROCK",
    "CLAUDE_CODE_USE_VERTEX",
    "CLAUDE_CODE_USE_FOUNDRY",
];

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
        for key in CONFLICTING_ENV {
            let configured = effective_env
                .vars
                .get(*key)
                .map(|value| !value.is_empty())
                .unwrap_or_else(|| std::env::var_os(key).is_some_and(|value| !value.is_empty()));
            if configured {
                return Err(auth_error(&format!(
                    "Subscription login conflicts with {key}. Remove that setting from this profile/execution environment, or choose another profile. No alternative billing method was selected."
                )));
            }
        }
        let (program, args) = CommandBuilder::new("claude")
            .params([
                "--settings",
                super::command_adapter::SUBSCRIPTION_SETTINGS,
                "auth",
                "status",
            ])
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
        let mut env = ExecutionEnv::new(crate::env::RepoContext::default(), false, String::new());
        for key in CONFLICTING_ENV {
            env.insert(*key, "");
        }
        env
    }

    #[tokio::test]
    async fn rejects_each_conflicting_credential_without_reporting_values() {
        for key in CONFLICTING_ENV {
            let mut env = clean_env();
            env.insert(*key, "private-fixture-value");
            let error = subscription_agent()
                .check_subscription_login(Path::new("."), &env)
                .await
                .unwrap_err()
                .to_string();
            assert!(error.contains(key), "{error}");
            assert!(!error.contains("private-fixture-value"));
        }
    }

    #[tokio::test]
    async fn profile_environment_overrides_runtime_and_legacy_profiles_are_unchanged() {
        let mut agent = subscription_agent();
        agent.cmd.env = Some(std::collections::HashMap::from([(
            "ANTHROPIC_API_KEY".to_string(),
            "private-profile-value".to_string(),
        )]));
        let error = agent
            .check_subscription_login(Path::new("."), &clean_env())
            .await
            .unwrap_err()
            .to_string();
        assert!(error.contains("ANTHROPIC_API_KEY"));
        assert!(!error.contains("private-profile-value"));
        agent.require_subscription_login = None;
        assert!(
            agent
                .check_subscription_login(Path::new("."), &clean_env())
                .await
                .is_ok()
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
