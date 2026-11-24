//! Read the official CLI's next-Run settings without sending a user/model turn.
//! `initialize` and `get_settings` are Agent SDK 0.3.308 control methods.
use std::{path::Path, time::Duration};

use serde_json::Value;
use tokio::process::Command;
use tokio_util::sync::CancellationToken;
use workspace_utils::command_ext::GroupSpawnNoWindowExt;

use super::{ClaudeCode, command_adapter::ClaudeCodeCommandAdapter, protocol::ProtocolPeer};
use crate::{
    env::{ExecutionEnv, RepoContext},
    executors::ExecutorError,
    model_selector::{ModelInfo, ReasoningOption, RunDefaults},
};

#[derive(Default)]
pub(super) struct Selection {
    pub models: Vec<ModelInfo>,
    pub defaults: RunDefaults,
    pub unsupported: Vec<String>,
}

fn selection_from_responses(
    initial: Value,
    settings: Value,
    requested_model: Option<&str>,
) -> Selection {
    // Only the CLI's applied projection proves provider defaults. Older CLIs
    // may return effective configuration alone; retain each available field.
    let effective = &settings["effective"];
    let applied = &settings["applied"];
    let string = |v: &Value| v.as_str().filter(|v| !v.is_empty()).map(str::to_owned);
    let model = string(&applied["model"]).or_else(|| string(&effective["model"]));
    let reasoning = string(&applied["effort"]).or_else(|| string(&effective["effortLevel"]));
    let mut selection = Selection::default();
    for m in initial["models"].as_array().into_iter().flatten() {
        let Some(id) = string(&m["resolvedModel"]).or_else(|| string(&m["value"])) else {
            continue;
        };
        if m["value"] == "default"
            && initial["models"]
                .as_array()
                .into_iter()
                .flatten()
                .any(|other| {
                    other["value"] != "default" && other["resolvedModel"].as_str() == Some(&id)
                })
        {
            continue;
        }
        if selection.models.iter().any(|m| m.id == id) {
            continue;
        }
        if m["supportsEffort"] == false {
            selection.unsupported.push(id.clone());
        }
        let names = m["supportedEffortLevels"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str);
        let default = (Some(&id) == model.as_ref())
            .then_some(reasoning.as_deref())
            .flatten();
        selection.models.push(ModelInfo {
            id,
            name: string(&m["displayName"])
                .filter(|_| m["value"] != "default")
                .unwrap_or_else(|| string(&m["resolvedModel"]).unwrap_or_default()),
            provider_id: None,
            reasoning_options: ReasoningOption::from_names_with_default(names, default),
        });
    }
    // Keep explicitly configured aliases matchable, without guessing their target.
    if let Some(alias) = requested_model
        .map(str::to_owned)
        .or_else(|| string(&effective["model"]))
        && let Some(native) = initial["models"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|m| m["value"].as_str() == Some(&alias))
        && let Some(target) = native["resolvedModel"].as_str()
        && let Some(mut row) = selection.models.iter().find(|m| m.id == target).cloned()
        && alias != target
    {
        row.id = alias.clone();
        if selection.unsupported.iter().any(|m| m == target) {
            selection.unsupported.push(alias);
        }
        selection.models.push(row);
    }
    selection.defaults = RunDefaults {
        model_source: model.as_ref().map(|_| {
            if effective["model"].is_string() {
                "configured"
            } else {
                "provider"
            }
            .into()
        }),
        reasoning_source: reasoning.as_ref().map(|_| {
            if effective["effortLevel"].is_string() {
                "configured"
            } else {
                "provider"
            }
            .into()
        }),
        model,
        reasoning,
        approval: None,
    };
    selection
}

impl ClaudeCode {
    pub(super) fn resolved_run_arguments(&self, selection: &Selection) -> Vec<String> {
        let mut args = Vec::new();
        if self.model.is_none()
            && let Some(model) = &selection.defaults.model
        {
            args.extend(["--model".into(), model.clone()]);
        }
        let unsupported = selection
            .defaults
            .model
            .as_ref()
            .is_some_and(|m| selection.unsupported.contains(m));
        if self.effort.is_none()
            && !unsupported
            && let Some(effort) = &selection.defaults.reasoning
        {
            args.extend(["--effort".into(), effort.clone()]);
        }
        args
    }

    pub(super) async fn read_run_selection(&self, cwd: &Path) -> Result<Selection, ExecutorError> {
        if self.claude_code_router == Some(true)
            || !crate::command::supports_passive_launcher(&self.cmd)
        {
            return Err(ExecutorError::Io(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "Run settings discovery requires the official CLI without custom arguments",
            )));
        }
        let parts = ClaudeCodeCommandAdapter::new(self)
            .build_settings_probe()?
            .build_initial()?;
        let (program, args) = parts.into_resolved_passive().await?;
        let mut command = Command::new(program);
        command
            .args(args)
            .current_dir(cwd)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .kill_on_drop(true);
        ExecutionEnv::new(RepoContext::default())
            .with_profile(&self.cmd)
            .apply_to_command(&mut command);
        let requested_model = self.model.clone();
        let cancel = CancellationToken::new();
        let _on_drop = cancel.clone().drop_guard();
        // The owner task always reaps the probe, even if the WebSocket disappears.
        tokio::spawn(async move {
            let mut child = command.group_spawn_no_window()?;
            let mut owner = match workspace_utils::process::OwnedProcessGroup::capture(&mut child, "claude-run-settings") {
                Ok(owner) => owner,
                Err(e) => { let _ = workspace_utils::process::kill_process_group(&mut child).await; return Err(ExecutorError::Io(e)); }
            };
            let stdin = child.inner().stdin.take().expect("piped stdin");
            let stdout = child.inner().stdout.take().expect("piped stdout");
            let reader_cancel = CancellationToken::new();
            let (peer, mut reader) = ProtocolPeer::spawn_passive(stdin, stdout, reader_cancel.clone());
            let result = tokio::select! {
                _ = cancel.cancelled() => Err(ExecutorError::Io(std::io::Error::new(std::io::ErrorKind::Interrupted,"Settings read cancelled"))),
                result = peer.read_run_settings() => result.map(|(init, settings)| selection_from_responses(init, settings, requested_model.as_deref())),
            };
            reader_cancel.cancel();
            let cleanup = owner.terminate(&mut child).await;
            if tokio::time::timeout(Duration::from_secs(2), &mut reader).await.is_err() { reader.abort(); let _ = reader.await; }
            cleanup?;
            result
        }).await.map_err(|e| ExecutorError::Io(std::io::Error::other(e)))?
    }

    /// Only resolve the requested model; never enumerate candidate defaults.
    pub async fn discover_selected_model(
        &self,
        cwd: &Path,
        model: &str,
    ) -> Result<json_patch::Patch, ExecutorError> {
        let mut probe = self.clone();
        probe.model = Some(model.to_owned());
        probe.effort = None;
        let selection = probe.read_run_selection(cwd).await.ok();
        let mut options = crate::executor_discovery::ExecutorDiscoveredOptions::default();
        super::apply_run_selection(&mut options, selection.as_ref());
        Ok(super::patch::executor_discovered_options(options))
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    #[cfg(unix)]
    #[tokio::test]
    async fn next_run_control_probe_and_runtime_arguments_preserve_independent_fields() {
        use std::os::unix::fs::PermissionsExt;

        use crate::executors::provider_adapter::DirectIntent;
        let root = tempfile::tempdir().unwrap();
        let executable = root.path().join("claude-fixture");
        std::fs::write(
            &executable,
            include_str!("../../../../../tests/run-selection/fake-claude.py"),
        )
        .unwrap();
        std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o700)).unwrap();
        for case in [
            "empty",
            "models-fail",
            "settings-fail",
            "model-missing",
            "effort-missing",
        ] {
            let record = root.path().join(format!("{case}.jsonl"));
            let agent: ClaudeCode =
                serde_json::from_value(json!({"base_command_override":executable,
                "env":{"LVK480_RECORD":record,"LVK480_CASE":case}}))
                .unwrap();
            let before = serde_json::to_value(&agent).unwrap();
            let selection = agent.read_run_selection(root.path()).await.unwrap();
            assert_eq!(
                selection.defaults.model.as_deref(),
                if ["model-missing", "settings-fail"].contains(&case) {
                    None
                } else {
                    Some("astra")
                }
            );
            assert_eq!(
                selection.defaults.reasoning.as_deref(),
                if ["effort-missing", "settings-fail"].contains(&case) {
                    None
                } else {
                    Some("medium")
                }
            );
            if case == "models-fail" {
                assert!(selection.unsupported.is_empty());
            }
            for intent in [DirectIntent::Initial, DirectIntent::Resume] {
                let parts = ClaudeCodeCommandAdapter::new(&agent)
                    .build_for_intent(true, intent, Some("same-conversation"), None)
                    .unwrap();
                let (_, mut args) = parts.into_resolved_passive().await.unwrap();
                args.extend(agent.resolved_run_arguments(&selection));
                assert_eq!(
                    args.iter().filter(|a| a.as_str() == "--model").count(),
                    usize::from(selection.defaults.model.is_some())
                );
                assert_eq!(
                    args.iter().filter(|a| a.as_str() == "--effort").count(),
                    usize::from(selection.defaults.reasoning.is_some())
                );
                if args.iter().any(|a| a == "--resume") {
                    assert!(args.iter().any(|a| a == "same-conversation"));
                }
            }
            assert_eq!(serde_json::to_value(&agent).unwrap(), before);
            for line in std::fs::read_to_string(record).unwrap().lines() {
                let value: Value = serde_json::from_str(line).unwrap();
                let args = value["args"].as_array().unwrap();
                assert!(args.contains(&json!("--no-session-persistence")));
                assert!(args.contains(&json!("--strict-mcp-config")));
                assert!(!args.contains(&json!("--resume")));
            }
        }
        let agent: ClaudeCode = serde_json::from_value(json!({"base_command_override":executable,
            "env":{"LVK480_RECORD":root.path().join("models.jsonl")}}))
        .unwrap();
        let selection = agent.read_run_selection(root.path()).await.unwrap();
        let beta = selection.models.iter().find(|m| m.id == "beta").unwrap();
        assert!(!beta.reasoning_options.iter().any(|r| r.is_default));
        let explicit: ClaudeCode =
            serde_json::from_value(json!({"model":"custom","effort":"high"})).unwrap();
        assert!(explicit.resolved_run_arguments(&selection).is_empty());
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn next_run_discovery_never_probes_unselected_candidates_even_with_cache() {
        use std::os::unix::fs::PermissionsExt;

        use futures::StreamExt;

        use crate::{
            executor_discovery::ExecutorConfigCacheKey,
            executors::{BaseCodingAgent, StandardCodingAgentExecutor},
        };
        let root = tempfile::tempdir().unwrap();
        let executable = root.path().join("claude");
        std::fs::write(
            &executable,
            include_str!("../../../../../tests/run-selection/fake-claude.py"),
        )
        .unwrap();
        std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o700)).unwrap();
        let record = root.path().join("calls.jsonl");
        let agent: ClaudeCode = serde_json::from_value(json!({"base_command_override":executable,
            "env":{"LVK480_RECORD":record,"LVK480_BETA_DELAY":"2"}}))
        .unwrap();
        for cached in [false, true] {
            if cached {
                let key = ExecutorConfigCacheKey::new(
                    Some(&root.path().to_path_buf()),
                    crate::repo_skills::discovery_cache_key(agent.compute_cmd_key(), &[]),
                    BaseCodingAgent::ClaudeCode,
                );
                crate::executors::utils::executor_options_cache()
                    .put(key, super::super::default_discovered_options(true));
            }
            let started = std::time::Instant::now();
            let mut stream = agent
                .discover_options(Some(root.path()), None, &[])
                .await
                .unwrap();
            let patch = stream.next().await.unwrap();
            let elapsed = started.elapsed();
            let mut data = json!({"options":null});
            json_patch::patch(&mut data, &patch).unwrap();
            assert_eq!(
                data["options"]["model_selector"]["run_defaults"]["model"],
                "astra"
            );
            assert_eq!(
                data["options"]["model_selector"]["run_defaults"]["reasoning"],
                "medium"
            );
            assert!(elapsed < Duration::from_secs(2));
            let calls = std::fs::read_to_string(&record).unwrap();
            assert_eq!(calls.lines().count(), if cached { 2 } else { 1 });
            assert!(!calls.contains("beta"));
            eprintln!(
                "first usable cached={cached}: {}ms, total children={}",
                elapsed.as_millis(),
                calls.lines().count()
            );
        }
        let started = std::time::Instant::now();
        let patch = agent
            .discover_selected_model(root.path(), "beta")
            .await
            .unwrap();
        let mut data = json!({"options":null});
        json_patch::patch(&mut data, &patch).unwrap();
        assert_eq!(
            data["options"]["model_selector"]["run_defaults"]["reasoning"],
            "low"
        );
        assert_eq!(std::fs::read_to_string(record).unwrap().lines().count(), 3);
        eprintln!(
            "selected beta: {}ms, total children=3",
            started.elapsed().as_millis()
        );
        for (case, expected) in [
            ("models-fail", Some("low")),
            ("effort-missing", None),
            ("settings-fail", None),
        ] {
            let probe: ClaudeCode =
                serde_json::from_value(json!({"base_command_override":executable,
                "env":{"LVK480_RECORD":root.path().join("partial.jsonl"),"LVK480_CASE":case}}))
                .unwrap();
            let patch = probe
                .discover_selected_model(root.path(), "beta")
                .await
                .unwrap();
            let mut data = json!({"options":null});
            json_patch::patch(&mut data, &patch).unwrap();
            assert_eq!(
                data["options"]["model_selector"]["run_defaults"]["reasoning"].as_str(),
                expected
            );
            if case == "models-fail" {
                assert_eq!(data["options"]["model_selector"]["models"], json!([]));
            }
        }
    }

    #[test]
    fn next_run_settings_are_resolved_per_field_without_inventing_capabilities() {
        let init = json!({"models":[
            {"value":"opus","resolvedModel":"opus-real","supportsEffort":true,"supportedEffortLevels":["low","medium"]},
            {"value":"unsupported","supportsEffort":false}, {"value":"unknown"}
        ]});
        let s = selection_from_responses(
            init.clone(),
            json!({"applied":{"model":"opus-real","effort":"medium"}}),
            None,
        );
        assert_eq!(s.defaults.model.as_deref(), Some("opus-real"));
        assert_eq!(
            s.models[0]
                .reasoning_options
                .iter()
                .find(|r| r.is_default)
                .unwrap()
                .id,
            "medium"
        );
        assert_eq!(s.unsupported, vec!["unsupported"]);
        let s = selection_from_responses(
            init,
            json!({"effective":{"effortLevel":"high"},"applied":{"model":"custom"}}),
            None,
        );
        assert_eq!(s.defaults.reasoning.as_deref(), Some("high"));
        assert_eq!(s.defaults.model.as_deref(), Some("custom"));
        let s = selection_from_responses(json!({}), json!({}), None);
        assert!(
            s.defaults.model.is_none()
                && s.defaults.reasoning.is_none()
                && s.unsupported.is_empty()
        );
    }
}
