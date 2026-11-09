//! Claude Code-native command construction and capability probes.

use std::path::PathBuf;

use uuid::Uuid;

use super::{ClaudeCode, PermissionMode, base_command};
use crate::{
    command::{CommandBuildError, CommandBuilder, CommandParts, apply_overrides},
    executors::provider_adapter::{DirectControl, DirectIntent, encode_stdio_rpc},
};

// One authentication policy for status, discovery, runs and passive usage.
// Empty values neutralise inherited credentials and ordinary settings;
// managed settings retain their native higher priority.
pub(super) const SUBSCRIPTION_SETTINGS: &str = r#"{"forceLoginMethod":"claudeai","env":{"ANTHROPIC_API_KEY":"","ANTHROPIC_AUTH_TOKEN":"","ANTHROPIC_CUSTOM_HEADERS":""}}"#;

pub struct ClaudeCodeCommandAdapter<'a> {
    agent: &'a ClaudeCode,
}

impl<'a> ClaudeCodeCommandAdapter<'a> {
    pub fn new(agent: &'a ClaudeCode) -> Self {
        Self { agent }
    }

    fn with_authentication_settings(&self, builder: CommandBuilder) -> CommandBuilder {
        if self.agent.require_subscription_login.unwrap_or(false) {
            builder.extend_params(["--settings", SUBSCRIPTION_SETTINGS])
        } else {
            builder
        }
    }

    pub(super) fn build_auth_probe(&self) -> CommandBuilder {
        self.with_authentication_settings(CommandBuilder::new("claude"))
            .extend_params(["auth", "status"])
    }

    /// Official non-model control lane. Do not inherit Project settings,
    /// hooks, MCP servers or persistence. --bare is unsuitable for OAuth.
    pub(super) fn build_passive_usage(&self) -> Result<CommandBuilder, CommandBuildError> {
        let mut settings = if self.agent.require_subscription_login.unwrap_or(false) {
            serde_json::from_str::<serde_json::Value>(SUBSCRIPTION_SETTINGS)
                .expect("static subscription settings")
        } else {
            serde_json::json!({})
        };
        settings["disableAllHooks"] = true.into();
        let settings = settings.to_string();
        let builder = CommandBuilder::new(base_command(false)).params([
            "-p",
            "--verbose",
            "--input-format=stream-json",
            "--output-format=stream-json",
            "--no-session-persistence",
            "--setting-sources",
            "user",
            "--strict-mcp-config",
            "--mcp-config",
            "{\"mcpServers\":{}}",
            "--tools",
            "",
            "--settings",
            &settings,
        ]);
        apply_overrides(builder, &self.agent.cmd)
    }

    pub fn build_runtime(
        &self,
        supports_effort: bool,
    ) -> Result<CommandBuilder, CommandBuildError> {
        if self.agent.cmd.base_command_override.is_some()
            && self.agent.claude_code_router.unwrap_or(false)
        {
            tracing::warn!(
                "base_command_override is set, this will override the claude_code_router setting"
            );
        }

        let mut builder =
            CommandBuilder::new(base_command(self.agent.claude_code_router.unwrap_or(false)))
                .params(["-p"]);

        let plan = self.agent.plan.unwrap_or(false);
        let approvals = self.agent.approvals.unwrap_or(false);
        if plan && approvals {
            tracing::warn!("Both plan and approvals are enabled. Plan will take precedence.");
        }
        if plan || approvals {
            builder = builder
                .extend_params(["--permission-prompt-tool=stdio"])
                .extend_params([format!(
                    "--permission-mode={}",
                    PermissionMode::BypassPermissions
                )]);
        } else {
            builder = builder.extend_params(["--disallowedTools=AskUserQuestion"]);
        }
        if self.agent.dangerously_skip_permissions.unwrap_or(false) {
            builder = builder.extend_params(["--dangerously-skip-permissions"]);
        }
        if let Some(model) = &self.agent.model {
            builder = builder.extend_params(["--model", model]);
        }
        if let Some(effort) = &self.agent.effort {
            if supports_effort {
                builder = builder.extend_params(["--effort", effort.as_ref()]);
            } else {
                tracing::warn!(
                    "Claude Code binary does not advertise --effort; omitting configured effort"
                );
            }
        }
        if let Some(agent) = &self.agent.agent {
            builder = builder.extend_params(["--agent", agent]);
        }
        builder = builder.extend_params([
            "--verbose",
            "--output-format=stream-json",
            "--input-format=stream-json",
            "--include-partial-messages",
            "--replay-user-messages",
        ]);

        apply_overrides(self.with_authentication_settings(builder), &self.agent.cmd)
    }

    pub fn build_for_intent(
        &self,
        supports_effort: bool,
        intent: DirectIntent,
        session_id: Option<&str>,
        reset_to_message_id: Option<&str>,
    ) -> Result<CommandParts, CommandBuildError> {
        let builder = self.build_runtime(supports_effort)?;
        match intent {
            DirectIntent::Initial => builder.build_initial(),
            DirectIntent::FollowUp | DirectIntent::Resume | DirectIntent::Review => {
                let Some(session_id) = session_id else {
                    return builder.build_initial();
                };
                let mut args = vec!["--resume".to_string(), session_id.to_string()];
                if let Some(message_id) = reset_to_message_id {
                    args.extend(["--resume-session-at".to_string(), message_id.to_string()]);
                }
                builder.build_follow_up(&args)
            }
        }
    }

    pub fn build_effort_probe(&self) -> Result<CommandBuilder, CommandBuildError> {
        let mut builder =
            CommandBuilder::new(base_command(self.agent.claude_code_router.unwrap_or(false)))
                .extend_params(["--help"]);
        if let Some(base) = &self.agent.cmd.base_command_override {
            builder = builder.override_base(base.clone());
        }
        Ok(builder)
    }

    pub fn build_discovery_probe(
        &self,
        repository_roots: &[PathBuf],
    ) -> Result<CommandBuilder, CommandBuildError> {
        let mut builder =
            CommandBuilder::new(base_command(self.agent.claude_code_router.unwrap_or(false)))
                .params(["-p"])
                .extend_params([
                    "--verbose",
                    "--output-format=stream-json",
                    "--max-turns",
                    "1",
                ]);
        for root in repository_roots {
            builder =
                builder.extend_params(["--add-dir".into(), root.to_string_lossy().into_owned()]);
        }
        // Keep repository/profile options before the end-of-options marker:
        // anything after it is part of the discovery prompt, not CLI options.
        Ok(
            apply_overrides(self.with_authentication_settings(builder), &self.agent.cmd)?
                .extend_params(["--", "/"]),
        )
    }
}

pub(crate) fn encode_control(control: DirectControl) -> Result<Vec<u8>, serde_json::Error> {
    let request = match control {
        DirectControl::Cancel => serde_json::json!({
            "type":"control_request",
            "request_id": Uuid::new_v4().to_string(),
            "request":{"subtype":"interrupt"}
        }),
        DirectControl::Approve {
            request_id,
            approved,
            reason,
        } => serde_json::json!({
            "type":"control_response",
            "response":{"subtype":"success","request_id":request_id,"response": if approved {
                serde_json::json!({"behavior":"allow","updatedInput":{}})
            } else {
                serde_json::json!({"behavior":"deny","message":reason.unwrap_or_else(|| "Denied by user".to_string())})
            }}
        }),
        DirectControl::Input { .. } => {
            return Err(serde_json::Error::io(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "Claude Code does not expose a separate host input response",
            )));
        }
        DirectControl::Steer { text } => {
            serde_json::json!({"type":"user","message":{"role":"user","content":text}})
        }
        DirectControl::UpdatePlanGoalDraft { .. }
        | DirectControl::GoalUpdate { .. }
        | DirectControl::GoalClear => {
            return Err(serde_json::Error::io(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "Claude Code does not expose Codex Goal controls",
            )));
        }
    };
    encode_stdio_rpc(&request)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::executors::provider_adapter::DirectControl;

    #[test]
    fn owns_claude_runtime_and_probe_shapes() {
        let agent: ClaudeCode = serde_json::from_value(serde_json::json!({
            "model": "sonnet",
            "effort": "max"
        }))
        .unwrap();
        let adapter = ClaudeCodeCommandAdapter::new(&agent);
        let runtime = adapter.build_runtime(true).unwrap();
        let probe = adapter.build_discovery_probe(&[]).unwrap();

        assert_eq!(runtime.base, "claude");
        let runtime_params = runtime.params.unwrap();
        assert_eq!(
            runtime_params
                .iter()
                .position(|param| param == "--effort")
                .and_then(|index| runtime_params.get(index + 1))
                .map(String::as_str),
            Some("max")
        );
        assert_eq!(probe.params.unwrap().last().map(String::as_str), Some("/"));
    }

    #[test]
    fn subscription_authentication_matches_across_all_four_lanes() {
        let agent: ClaudeCode = serde_json::from_value(serde_json::json!({
            "require_subscription_login": true
        }))
        .unwrap();
        let adapter = ClaudeCodeCommandAdapter::new(&agent);
        for builder in [
            adapter.build_auth_probe(),
            adapter.build_runtime(false).unwrap(),
            adapter.build_discovery_probe(&[]).unwrap(),
            adapter.build_passive_usage().unwrap(),
        ] {
            let params = builder.params.unwrap();
            let settings = params.iter().position(|arg| arg == "--settings").unwrap();
            let mut parsed: serde_json::Value =
                serde_json::from_str(&params[settings + 1]).unwrap();
            if params.iter().any(|arg| arg == "--no-session-persistence") {
                assert_eq!(
                    parsed.as_object_mut().unwrap().remove("disableAllHooks"),
                    Some(true.into())
                );
                for (flag, value) in [
                    ("--setting-sources", "user"),
                    ("--mcp-config", "{\"mcpServers\":{}}"),
                    ("--tools", ""),
                ] {
                    let index = params.iter().position(|arg| arg == flag).unwrap();
                    assert_eq!(params[index + 1], value);
                }
                assert!(params.iter().any(|arg| arg == "--strict-mcp-config"));
            }
            assert_eq!(
                parsed,
                serde_json::from_str::<serde_json::Value>(SUBSCRIPTION_SETTINGS).unwrap()
            );
            for key in [
                "ANTHROPIC_API_KEY",
                "ANTHROPIC_AUTH_TOKEN",
                "ANTHROPIC_CUSTOM_HEADERS",
            ] {
                assert_eq!(parsed["env"][key], "");
            }
            if let Some(prompt_start) = params.iter().position(|arg| arg == "--") {
                assert!(settings < prompt_start);
                assert_eq!(params.last().unwrap(), "/");
            }
            assert!(!params.iter().any(|arg| arg == "--bare"));
        }
        let legacy: ClaudeCode = serde_json::from_value(serde_json::json!({})).unwrap();
        let passive = ClaudeCodeCommandAdapter::new(&legacy)
            .build_passive_usage()
            .unwrap()
            .params
            .unwrap();
        let index = passive.iter().position(|arg| arg == "--settings").unwrap();
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&passive[index + 1]).unwrap(),
            serde_json::json!({"disableAllHooks":true})
        );
        assert!(
            !ClaudeCodeCommandAdapter::new(&legacy)
                .build_runtime(false)
                .unwrap()
                .params
                .unwrap()
                .iter()
                .any(|arg| arg == "--settings")
        );
    }

    #[test]
    fn owns_claude_control_encoding() {
        let value: serde_json::Value =
            serde_json::from_slice(&encode_control(DirectControl::Cancel).unwrap()).unwrap();

        assert_eq!(value["type"], "control_request");
        assert!(value["request_id"].is_string());
        assert_eq!(value["request"]["subtype"], "interrupt");
    }

    #[test]
    fn applies_claude_command_overrides_after_native_args() {
        let agent: ClaudeCode = serde_json::from_value(serde_json::json!({
            "additional_params": ["--profile-flag"]
        }))
        .unwrap();
        let params = ClaudeCodeCommandAdapter::new(&agent)
            .build_runtime(false)
            .unwrap()
            .params
            .unwrap();

        assert_eq!(params.last().map(String::as_str), Some("--profile-flag"));
    }

    #[test]
    fn discovery_keeps_repository_roots_before_prompt() {
        let agent: ClaudeCode = serde_json::from_value(serde_json::json!({})).unwrap();
        let adapter = ClaudeCodeCommandAdapter::new(&agent);
        for roots in [
            vec![],
            vec![PathBuf::from("/workspace/repo space")],
            vec![
                PathBuf::from("/workspace/repo space"),
                PathBuf::from("/workspace/repo space--b2222222"),
            ],
        ] {
            let builder = adapter.build_discovery_probe(&roots).unwrap();
            assert_eq!(builder.base, "claude");
            let params = builder.params.unwrap();
            let prompt_start = params.iter().position(|arg| arg == "--").unwrap();
            assert_eq!(&params[prompt_start..], &["--", "/"]);
            let actual_roots: Vec<_> = params[..prompt_start]
                .windows(2)
                .filter(|pair| pair[0] == "--add-dir")
                .map(|pair| PathBuf::from(&pair[1]))
                .collect();
            assert_eq!(actual_roots, roots);
        }
    }

    #[test]
    fn discovery_preserves_profile_options_before_prompt() {
        let agent: ClaudeCode = serde_json::from_value(serde_json::json!({
            "base_command_override": "custom-claude --base-flag",
            "additional_params": ["--profile-flag", "value with spaces"],
            "require_subscription_login": true
        }))
        .unwrap();
        let roots = [PathBuf::from("/workspace/repo space")];
        let builder = ClaudeCodeCommandAdapter::new(&agent)
            .build_discovery_probe(&roots)
            .unwrap();
        assert_eq!(builder.base, "custom-claude --base-flag");
        let params = builder.params.unwrap();
        assert_eq!(
            params,
            [
                "-p",
                "--verbose",
                "--output-format=stream-json",
                "--max-turns",
                "1",
                "--add-dir",
                "/workspace/repo space",
                "--settings",
                SUBSCRIPTION_SETTINGS,
                "--profile-flag",
                "value with spaces",
                "--",
                "/",
            ]
        );
    }
}
