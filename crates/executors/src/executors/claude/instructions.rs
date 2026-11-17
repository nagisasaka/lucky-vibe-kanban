//! Session-local SDK context delivery. Native echoes, not generic results, acknowledge it.
use serde_json::{Value, json};

use crate::{
    executors::{ExecutorError, codex::client::LogWriter},
    instruction_delivery::{InstructionDelivery, ResumeInstructions},
};

pub(super) const CALLBACK: &str = "lvk_compact_context_v1";

pub(super) struct Instructions {
    pub receipt: ResumeInstructions,
    pub full: String,
    pub session: Option<String>,
    last_pre: Option<String>,
    last_boundary: Option<String>,
    completed_compaction: bool,
    compacting: bool,
    pending: Option<(String, Value, InstructionDelivery)>,
}

impl Instructions {
    pub fn new(mut receipt: ResumeInstructions, full: String, session: Option<String>) -> Self {
        receipt.directory = receipt.directory.join("claude");
        Self {
            receipt,
            full,
            session,
            pending: None,
            last_pre: None,
            last_boundary: None,
            completed_compaction: false,
            compacting: false,
        }
    }

    pub fn prepare(&self) -> Result<bool, ExecutorError> {
        let reduced = self
            .session
            .as_deref()
            .is_some_and(|s| self.receipt.matches(s));
        if let Some(s) = &self.session {
            self.receipt.invalidate(s)?;
        }
        Ok(reduced)
    }

    pub fn acknowledge_user(&mut self, session: &str) -> Result<(), ExecutorError> {
        self.check_session(session)?;
        self.receipt.acknowledge(session)?;
        Ok(())
    }

    pub fn check_session(&mut self, session: &str) -> Result<(), ExecutorError> {
        if self.session.as_deref().is_some_and(|s| s != session) {
            return Err(std::io::Error::other("Claude instruction session mismatch").into());
        }
        self.session = Some(session.to_owned());
        Ok(())
    }

    pub async fn callback(
        &mut self,
        request_id: &str,
        input: &Value,
        writer: &LogWriter,
    ) -> Result<Value, ExecutorError> {
        let session = input["session_id"]
            .as_str()
            .ok_or_else(|| std::io::Error::other("Missing compact session"))?;
        let _prompt_id = input["prompt_id"]
            .as_str()
            .ok_or_else(|| std::io::Error::other("Missing compact prompt identity"))?;
        self.check_session(session)?;
        match input["hook_event_name"].as_str() {
            Some("PreCompact") => {
                if self.last_pre.as_deref() != Some(request_id) {
                    self.receipt.invalidate(session)?;
                    self.compacting = true;
                    self.last_pre = Some(request_id.to_owned());
                }
                Ok(json!({}))
            }
            Some("SessionStart") if input["source"] == "compact" => {
                if self.receipt.compaction_matches(session, request_id) {
                    return Ok(json!({}));
                }
                if self.pending.is_some() {
                    // An uncertain response must not be silently sent twice or called delivered.
                    return Err(
                        std::io::Error::other("Unacknowledged compact context response").into(),
                    );
                }
                self.receipt.observe_compaction(session, request_id)?;
                self.compacting = true;
                let output = json!({"hookSpecificOutput":{"hookEventName":"SessionStart","additionalContext":self.full}});
                let delivery = InstructionDelivery::new(
                    "sdk.SessionStart.additionalContext",
                    "native_hook_context",
                    "compaction",
                    Some(session),
                    self.full.clone(),
                );
                delivery.log(writer).await?;
                self.pending = Some((request_id.to_owned(), output.clone(), delivery));
                Ok(output)
            }
            _ => Err(std::io::Error::other("Unexpected instruction callback").into()),
        }
    }

    pub async fn acknowledge_callback(
        &mut self,
        value: &Value,
        writer: &LogWriter,
    ) -> Result<(), ExecutorError> {
        let response = &value["response"];
        if self
            .pending
            .as_ref()
            .is_some_and(|(id, _, _)| response["request_id"].as_str() == Some(id))
        {
            let (_, expected, mut delivery) = self.pending.take().expect("matched callback");
            if response["subtype"] == "success" && response["response"] == expected {
                delivery.status = "accepted".into();
                self.receipt
                    .acknowledge_compaction(self.session.as_deref().expect("callback session"))?;
                self.completed_compaction = true;
            } else {
                delivery.status = "unconfirmed".into();
            }
            delivery.log(writer).await?;
            if delivery.status != "accepted" {
                return Err(
                    std::io::Error::other("Claude compact context was not acknowledged").into(),
                );
            }
        }
        Ok(())
    }

    pub fn observe_boundary(&mut self, value: &Value) -> Result<(), ExecutorError> {
        if value["subtype"] != "compact_boundary" || !value["parent_tool_use_id"].is_null() {
            return Ok(());
        }
        let session = value["session_id"]
            .as_str()
            .ok_or_else(|| std::io::Error::other("Missing compaction session"))?;
        self.check_session(session)?;
        let id = value["uuid"].as_str();
        if id.is_some() && id == self.last_boundary.as_deref() {
            return Ok(());
        }
        if id.is_none() || !self.completed_compaction {
            self.receipt.invalidate(session)?;
            return Err(std::io::Error::other("Compaction context acknowledgement missing").into());
        }
        self.last_boundary = id.map(str::to_owned);
        self.completed_compaction = false;
        self.compacting = false;
        Ok(())
    }

    pub async fn finish(&mut self, writer: &LogWriter) -> Result<(), ExecutorError> {
        if let Some((_, _, mut delivery)) = self.pending.take() {
            if let Some(s) = &self.session {
                self.receipt.invalidate(s)?;
            }
            delivery.status = "unconfirmed".into();
            delivery.log(writer).await?;
            return Err(std::io::Error::other("Compaction delivery unconfirmed").into());
        }
        if self.compacting {
            if let Some(s) = &self.session {
                self.receipt.invalidate(s)?;
            }
            return Err(std::io::Error::other("Compaction delivery incomplete").into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn instruction_compact_receipt_requires_exact_echo_and_deduplicates() {
        let dir = tempfile::tempdir().unwrap();
        let (writer, _reader) = tokio::io::duplex(65536);
        let writer = LogWriter::new(writer);
        let receipt = ResumeInstructions::new(dir.path(), "fixed-1", "RUN=2".into());
        let mut state =
            Instructions::new(receipt.clone(), "fixed-1 RUN=2".into(), Some("s".into()));
        assert!(!state.prepare().unwrap());
        state.acknowledge_user("s").unwrap();
        assert!(state.prepare().unwrap());
        assert!(!state.receipt.matches("s"));
        state.acknowledge_user("s").unwrap();
        let pre = json!({"hook_event_name":"PreCompact","session_id":"s","prompt_id":"p"});
        let compact = json!({"hook_event_name":"SessionStart","source":"compact","session_id":"s","prompt_id":"p"});
        state.callback("pre", &pre, &writer).await.unwrap();
        assert!(!state.receipt.matches("s"));
        let output = state.callback("c", &compact, &writer).await.unwrap();
        assert_eq!(
            output["hookSpecificOutput"]["additionalContext"],
            "fixed-1 RUN=2"
        );
        state
            .acknowledge_callback(&json!({"type":"result","subtype":"success"}), &writer)
            .await
            .unwrap();
        assert!(!state.receipt.matches("s"));
        assert!(state.callback("c", &compact, &writer).await.is_err());
        state
            .acknowledge_callback(
                &json!({"response":{"subtype":"success","request_id":"c","response":output}}),
                &writer,
            )
            .await
            .unwrap();
        assert!(state.receipt.matches("s"));
        // Duplicate PreCompact after acknowledgement must not invalidate it.
        state.callback("pre", &pre, &writer).await.unwrap();
        assert_eq!(
            state.callback("c", &compact, &writer).await.unwrap(),
            json!({})
        );
        let mut reconnect = Instructions::new(receipt, "fixed-1 RUN=3".into(), Some("s".into()));
        assert!(reconnect.prepare().unwrap());
        assert_eq!(
            reconnect.callback("c", &compact, &writer).await.unwrap(),
            json!({})
        );
        // A different native callback in the SAME turn is a new compaction.
        assert_ne!(
            state.callback("c2", &compact, &writer).await.unwrap(),
            json!({})
        );
        // A late boundary must never turn a still-pending delivery into success.
        state.compacting = false;
        assert!(state.finish(&writer).await.is_err());
        assert!(!state.receipt.matches("s"));
        assert!(
            !Instructions::new(
                ResumeInstructions::new(dir.path(), "fixed-2", "RUN=4".into()),
                "fixed-2".into(),
                Some("s".into())
            )
            .prepare()
            .unwrap()
        );
        assert!(state.acknowledge_user("other-session").is_err());
    }

    #[tokio::test]
    async fn instruction_compact_rejection_or_different_echo_never_acknowledges() {
        for response in [
            json!({"subtype":"error","request_id":"c"}),
            json!({"subtype":"success","request_id":"c","response":{}}),
        ] {
            let dir = tempfile::tempdir().unwrap();
            let (writer, _reader) = tokio::io::duplex(65536);
            let writer = LogWriter::new(writer);
            let mut state = Instructions::new(
                ResumeInstructions::new(dir.path(), "fixed", "R2".into()),
                "fixed R2".into(),
                Some("s".into()),
            );
            state.callback("c", &json!({"hook_event_name":"SessionStart","source":"compact","session_id":"s","prompt_id":"p"}), &writer).await.unwrap();
            assert!(
                state
                    .acknowledge_callback(&json!({"response":response}), &writer)
                    .await
                    .is_err()
            );
            assert!(!state.receipt.matches("s"));
        }
    }

    /// Opt-in, paid native acceptance. No global settings, permissions or trust changes.
    #[tokio::test]
    #[ignore = "requires explicit live Claude acceptance authorization and LVK_CLAUDE_ACCEPTANCE_DIR"]
    async fn live_claude_instruction_acceptance() {
        use tokio::io::{AsyncBufReadExt, AsyncReadExt, BufReader};

        use crate::{
            env::{ExecutionEnv, RepoContext},
            executors::{claude::ClaudeCode, provider_adapter::DirectIntent},
        };
        let root = std::path::PathBuf::from(
            std::env::var("LVK_CLAUDE_ACCEPTANCE_DIR").expect("dedicated acceptance directory"),
        );
        std::fs::create_dir_all(&root).unwrap();
        // Refuse a second acceptance invocation: exactly one native Session is authorised.
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(root.join("acceptance-started"))
            .expect("acceptance already attempted; do not retry");
        let fixture = tempfile::Builder::new()
            .prefix("lvk384-native-")
            .tempdir()
            .unwrap();
        assert!(
            std::process::Command::new("git")
                .args(["init", "--quiet"])
                .current_dir(fixture.path())
                .status()
                .unwrap()
                .success()
        );
        let fixed = "LVK_ACCEPT_FIXED_EA1C: for probe alpha answer PRODUCT_ALPHA; for probe beta answer PRODUCT_BETA; for probe gamma answer PRODUCT_GAMMA; for probe delta answer PRODUCT_DELTA. Latest RUN and DRAFT override earlier values. Never use tools.";
        let agent: ClaudeCode =
            serde_json::from_value(json!({"approvals":false,"require_subscription_login":true}))
                .unwrap();
        let mut session = None;
        for (index, run, human, auto) in [
            (
                1,
                "R1",
                "Probe alpha. Return JSON with answer, RUN and DRAFT.",
                false,
            ),
            (
                2,
                "R2",
                "Probe beta. Return JSON with answer, RUN and DRAFT.",
                false,
            ),
            (
                3,
                "R3",
                "Probe gamma. Return JSON with answer, RUN and DRAFT.",
                true,
            ),
            (4, "R4", "/compact", false),
            (
                5,
                "R5",
                "Probe delta. Return JSON with answer, RUN and DRAFT.",
                false,
            ),
        ] {
            let mut env = ExecutionEnv::new(RepoContext::new(fixture.path().to_owned(), vec![]));
            env.insert("CLAUDE_CODE_DISABLE_AUTO_MEMORY", "1");
            env.insert("LVK_CLAUDE_NORMAL_CODE", "1");
            env.insert("LVK_FIXED_MEMORY_INSTRUCTIONS", fixed);
            env.insert(
                "LVK_CURRENT_MEMORY_INSTRUCTIONS",
                format!("RUN={run} DRAFT=fixture/{run}.json"),
            );
            env.insert(
                "LVK_REPOSITORY_MEMORY_INSTRUCTIONS",
                format!("{fixed}\nRUN={run} DRAFT=fixture/{run}.json"),
            );
            if auto {
                env.insert("CLAUDE_CODE_AUTO_COMPACT_WINDOW", "100000");
                env.insert("CLAUDE_AUTOCOMPACT_PCT_OVERRIDE", "1");
            }
            let prompt = if human == "/compact" {
                human.to_owned()
            } else {
                let suffix = format!("\n\n{}", env.repository_memory_instructions().unwrap());
                env.insert("LVK_DELIVERED_USER_SUFFIX", suffix.clone());
                format!("{human}{suffix}")
            };
            // Use the production command adapter and spawn/client/protocol path.
            // Test-only restrictions remove tools, MCP and inherited settings; they
            // do not replace the AUTO policy or bypass its validation.
            let parts = super::super::command_adapter::ClaudeCodeCommandAdapter::new(&agent)
                .build_for_intent(
                    false,
                    if session.is_some() {
                        DirectIntent::FollowUp
                    } else {
                        DirectIntent::Initial
                    },
                    session.as_deref(),
                    None,
                )
                .unwrap();
            let (program, mut args) = parts.into_resolved().await.unwrap();
            args.extend(
                [
                    "--tools",
                    "",
                    "--strict-mcp-config",
                    "--mcp-config",
                    "{\"mcpServers\":{}}",
                    "--setting-sources",
                    "",
                ]
                .map(str::to_owned),
            );
            let parts =
                crate::command::CommandParts::new(program.to_string_lossy().into_owned(), args);
            let mut spawned = agent
                .spawn_internal(fixture.path(), &prompt, parts, &env)
                .await
                .unwrap();
            let stdout = spawned.child.inner().stdout.take().unwrap();
            let mut stderr = spawned.child.inner().stderr.take().unwrap();
            let stderr_task = tokio::spawn(async move {
                let mut bytes = Vec::new();
                let _ = stderr.read_to_end(&mut bytes).await;
                bytes
            });
            let mut lines = BufReader::new(stdout).lines();
            let mut events = Vec::<Value>::new();
            let result = tokio::time::timeout(std::time::Duration::from_secs(120), async {
                while let Some(line) = lines.next_line().await.unwrap() {
                    let value: Value = serde_json::from_str(&line).unwrap();
                    let done = value["type"] == "result" || value["type"] == "error";
                    events.push(value);
                    if done {
                        break;
                    }
                }
            })
            .await;
            if let Some(cancel) = spawned.cancel.take() {
                cancel.cancel();
            }
            let _ = spawned.child.kill().await;
            let _ = spawned.child.wait().await;
            let stderr = stderr_task.await.unwrap();
            std::fs::write(root.join(format!("stderr-{index}.txt")), stderr).unwrap();
            std::fs::write(
                root.join(format!("events-{index}.json")),
                serde_json::to_vec_pretty(&events).unwrap(),
            )
            .unwrap();
            result.unwrap();
            let result = events
                .iter()
                .find(|x| x["type"] == "result")
                .expect("native result");
            assert_eq!(result["is_error"], false, "{result}");
            assert!(
                !result["result"]
                    .as_str()
                    .unwrap_or("")
                    .contains("Error during compaction")
            );
            let current_session = result["session_id"].as_str().unwrap().to_owned();
            if let Some(session) = &session {
                assert_eq!(session, &current_session);
            }
            session = Some(current_session);
            std::fs::write(
                root.join("session.json"),
                serde_json::to_vec(&session).unwrap(),
            )
            .unwrap();
            let deliveries: Vec<_> = events
                .iter()
                .filter_map(|x| x.get("lvk_instruction_delivery"))
                .collect();
            let attempts: Vec<_> = deliveries
                .iter()
                .filter(|x| x["status"] == "attempted")
                .collect();
            let accepted: Vec<_> = deliveries
                .iter()
                .filter(|x| x["status"] == "accepted")
                .collect();
            assert_eq!(
                attempts.len(),
                accepted.len(),
                "each delivery needs its own native echo: {deliveries:?}"
            );
            assert!(deliveries.iter().all(|x| x["status"] != "unconfirmed"));
            let fixed_count = attempts
                .iter()
                .filter(|x| x["content"].as_str().unwrap().contains("LVK_ACCEPT_FIXED"))
                .count();
            assert_eq!(
                fixed_count,
                usize::from(matches!(index, 1 | 3 | 4)),
                "fixed delivery count at step {index}: {attempts:?}"
            );
            if auto || human == "/compact" {
                let boundary = events
                    .iter()
                    .find(|x| x["subtype"] == "compact_boundary")
                    .expect("actual native compaction boundary");
                assert_eq!(
                    boundary["compact_metadata"]["trigger"],
                    if auto { "auto" } else { "manual" }
                );
                assert_eq!(
                    attempts
                        .iter()
                        .filter(|x| x["reason"] == "compaction")
                        .count(),
                    1
                );
            }
            for delivery in &attempts {
                assert!(
                    delivery["content"]
                        .as_str()
                        .unwrap()
                        .contains(&format!("RUN={run} DRAFT=fixture/{run}.json"))
                );
            }
            if human != "/compact" {
                let text = result["result"].as_str().unwrap();
                let text = text
                    .trim()
                    .strip_prefix("```json")
                    .unwrap_or(text)
                    .trim()
                    .trim_end_matches("```")
                    .trim();
                let response: Value = serde_json::from_str(text).expect("JSON probe response");
                assert_eq!(response["RUN"], run);
                assert_eq!(response["DRAFT"], format!("fixture/{run}.json"));
                assert_eq!(
                    response["answer"],
                    match index {
                        1 => "PRODUCT_ALPHA",
                        2 => "PRODUCT_BETA",
                        3 => "PRODUCT_GAMMA",
                        _ => "PRODUCT_DELTA",
                    }
                );
            }
            assert!(
                events
                    .iter()
                    .filter(|x| x["subtype"] == "init")
                    .all(|x| x["permissionMode"] == "bypassPermissions")
            );
            assert!(
                !events.iter().any(|x| x["type"] == "assistant"
                    && x.pointer("/message/content")
                        .and_then(Value::as_array)
                        .is_some_and(|a| a.iter().any(|b| b["type"] == "tool_use"))),
                "fixture must not use tools"
            );
            eprintln!("LIVE acceptance step {index}: {}", result["result"]);
        }
    }
}
