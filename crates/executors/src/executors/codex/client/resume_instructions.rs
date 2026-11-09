//! Conservative history evidence for reducing host-owned resume appends.
//! No sidecar acknowledgement: compaction/rollback can invalidate an old receipt.
use std::path::Path;

use serde_json::Value;
#[cfg(unix)]
use tokio::io::AsyncReadExt;

#[cfg(unix)]
const MAX_ROLLOUT_BYTES: u64 = 16 * 1024 * 1024;
const MANIFEST_IDENTITY: &str = "LVK Change Manifest (CURRENT host identity JSON): ";
const PARALLEL_IDENTITY: &str =
    "LVK parallel context v1 (CURRENT identity and read-only discovery): ";

// Only these host JSON identity lines are separable. All other text, including
// caller instructions, skill references and safety policy, stays together and
// must match exactly. Unknown/malformed formats cannot enable the optimisation.
fn split(instructions: &str) -> Option<(String, String)> {
    let mut fixed = String::new();
    let mut dynamic = String::new();
    let mut memory = false;
    for line in instructions.split_inclusive('\n') {
        let text = line.trim_end_matches('\n');
        if let Some((prefix, json)) = [MANIFEST_IDENTITY, PARALLEL_IDENTITY]
            .into_iter()
            .find_map(|prefix| text.strip_prefix(prefix).map(|json| (prefix, json)))
        {
            let identity: Value = serde_json::from_str(json).ok()?;
            if !identity.is_object() {
                return None;
            }
            if prefix == MANIFEST_IDENTITY {
                if !identity["run_id"].is_string() || !identity["draft_path"].is_string() {
                    return None;
                }
                memory = true;
            }
            dynamic.push_str(text);
            dynamic.push('\n');
        } else {
            fixed.push_str(line);
        }
    }
    memory.then_some((fixed, dynamic))
}

fn retained(instructions: &str, thread: &str, rollout: &str) -> bool {
    let Some((fixed, _)) = split(instructions) else {
        return false;
    };
    if fixed.is_empty() || !rollout.ends_with('\n') {
        return false;
    }
    let mut identified = false;
    let mut matches = false;
    for line in rollout.lines() {
        let Ok(item) = serde_json::from_str::<Value>(line) else {
            return false;
        };
        if !identified && item["type"] != "session_meta" {
            return false;
        }
        let payload = &item["payload"];
        match item["type"].as_str() {
            Some("session_meta") => {
                // Fork/history-base reconstruction is deliberately unsupported.
                if identified
                    || payload["id"].as_str() != Some(thread)
                    || !payload["history_base"].is_null()
                    || !payload["forked_from_id"].is_null()
                    || !payload["forked_from_ordinal_exclusive"].is_null()
                    || !matches!(payload["cli_version"].as_str(), Some("0.144.1" | "0.163.0"))
                {
                    return false;
                }
                identified = true;
            }
            // Do not partially reimplement reverse replay. Any replacement or
            // rollback makes this entire rollout ineligible, even after a later
            // full append. Compacted/forked threads keep the safe old behaviour.
            Some("compacted") => return false,
            Some("response_item") if payload["role"] == "developer" => {
                if payload["type"] != "message" {
                    return false;
                }
                let Some(content) = payload["content"].as_array() else {
                    return false;
                };
                let mut text = String::new();
                for part in content {
                    if part["type"] != "input_text" {
                        return false;
                    }
                    let Some(part) = part["text"].as_str() else {
                        return false;
                    };
                    text.push_str(part);
                }
                match split(&text) {
                    // A previous reduced append refreshes identity only.
                    Some((previous, _)) if previous.is_empty() => {}
                    Some((previous, _)) => matches = previous == fixed,
                    None => matches = false,
                }
            }
            // Added in the reviewed 0.163 replay: display/settings metadata,
            // explicitly ignored by reconstruct_history_from_rollout.
            Some("event_msg") if payload["type"] == "thread_settings_applied" => {}
            Some("event_msg") => {
                use codex_protocol::protocol::EventMsg;
                match serde_json::from_value::<EventMsg>(payload.clone()) {
                    Ok(EventMsg::ThreadRolledBack(_) | EventMsg::ContextCompacted(_)) | Err(_) => {
                        return false;
                    }
                    Ok(_) => {}
                }
            }
            Some("response_item") => match payload["type"].as_str() {
                Some(
                    "message"
                    | "reasoning"
                    | "function_call"
                    | "function_call_output"
                    | "custom_tool_call"
                    | "custom_tool_call_output"
                    | "local_shell_call"
                    | "web_search_call"
                    | "image_generation_call",
                ) => {}
                _ => return false,
            },
            // These records do not replace response history in the reviewed
            // Codex replay paths. RetainedContext and other new forms fall back.
            Some(
                "turn_context"
                | "world_state"
                | "token_usage_record"
                | "security_risk_score"
                | "realtime_item",
            ) => {}
            // New storage/reconstruction formats must be reviewed explicitly.
            _ => return false,
        }
    }
    identified && matches
}

pub(super) fn eligible(instructions: &str, user_agent: Option<&str>) -> bool {
    // Only enable the optimisation for reviewed public replay implementations.
    // This does not gate ordinary Codex compatibility, and version metadata is
    // not a claim that a locally installed binary has a verified Git revision.
    let reviewed = user_agent
        .and_then(|ua| ua.split_whitespace().next())
        .and_then(|product| product.rsplit_once('/'))
        .is_some_and(|(_, version)| matches!(version, "0.144.1" | "0.163.0"));
    cfg!(unix) && reviewed && split(instructions).is_some()
}

pub(super) struct Evidence {
    path: std::path::PathBuf,
    metadata: std::fs::Metadata,
    dynamic: String,
    rollout: String,
}

#[cfg(unix)]
async fn read_stable(path: &Path) -> Option<(String, std::fs::Metadata)> {
    // The local app-server is the path authority. No directory discovery,
    // archives, history-base traversal, contents in logs, or sidecar writes.
    // Reject symlink components and nonregular files before opening; final
    // O_NOFOLLOW/NONBLOCK also closes the FIFO/final-symlink swap window.
    if !path.is_absolute()
        || path.extension()? != "jsonl"
        || tokio::fs::canonicalize(path).await.ok()? != path
    {
        return None;
    }
    let before = tokio::fs::symlink_metadata(path).await.ok()?;
    if !before.is_file() || before.len() > MAX_ROLLOUT_BYTES {
        return None;
    }
    let mut options = tokio::fs::OpenOptions::new();
    options
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    let file = options.open(path).await.ok()?;
    let opened = file.metadata().await.ok()?;
    if !same_file(&before, &opened) {
        return None;
    }
    let mut bytes = Vec::new();
    let mut reader = file.take(MAX_ROLLOUT_BYTES + 1);
    reader.read_to_end(&mut bytes).await.ok()?;
    if bytes.len() as u64 > MAX_ROLLOUT_BYTES {
        return None;
    }
    let after = reader.get_ref().metadata().await.ok()?;
    let path_after = tokio::fs::symlink_metadata(path).await.ok()?;
    if bytes.len() as u64 != before.len()
        || !same_file(&before, &after)
        || !same_file(&before, &path_after)
        || tokio::fs::canonicalize(path).await.ok()? != path
    {
        return None;
    }
    Some((String::from_utf8(bytes).ok()?, before))
}
#[cfg(not(unix))]
async fn read_stable(_path: &Path) -> Option<(String, std::fs::Metadata)> {
    None
}
pub(super) async fn prepare(instructions: &str, thread: &str, path: &Path) -> Option<Evidence> {
    let (rollout, metadata) =
        tokio::time::timeout(std::time::Duration::from_secs(2), read_stable(path))
            .await
            .ok()??;
    if !retained(instructions, thread, &rollout) {
        return None;
    }
    Some(Evidence {
        path: path.to_owned(),
        metadata,
        dynamic: split(instructions)?.1,
        rollout,
    })
}

impl Evidence {
    // The pre-resume prefix must span native hydration unchanged. 0.163
    // appends ThreadSettingsApplied while loading; permit only that known
    // model-invisible suffix (and a model-switch Warning), never arbitrary
    // changes or newly appended response history that hydration may not include.
    pub(super) async fn after_resume(self, path: Option<&Path>) -> Option<String> {
        #[cfg(unix)]
        if path == Some(self.path.as_path())
            && let Ok(Some((rollout, metadata))) =
                tokio::time::timeout(std::time::Duration::from_secs(2), read_stable(&self.path))
                    .await
        {
            use std::os::unix::fs::MetadataExt;
            if self.metadata.dev() != metadata.dev() || self.metadata.ino() != metadata.ino() {
                return None;
            }
            let suffix = rollout.strip_prefix(&self.rollout)?;
            if suffix.is_empty() && !same_file(&self.metadata, &metadata) {
                return None;
            }
            if !suffix.lines().all(|line| {
                serde_json::from_str::<Value>(line).is_ok_and(|item| {
                    item["type"] == "event_msg"
                        && matches!(
                            item["payload"]["type"].as_str(),
                            Some("thread_settings_applied" | "warning")
                        )
                })
            }) {
                return None;
            }
            return Some(self.dynamic);
        }
        #[cfg(not(unix))]
        let _ = (self.path, self.metadata, self.dynamic, self.rollout, path);
        None
    }
}

#[cfg(test)]
async fn for_resume(
    instructions: &str,
    thread: &str,
    path: Option<&Path>,
    user_agent: Option<&str>,
) -> String {
    if eligible(instructions, user_agent)
        && let Some(path) = path
        && let Some(evidence) = prepare(instructions, thread, path).await
        && let Some(dynamic) = evidence.after_resume(Some(path)).await
    {
        return dynamic;
    }
    instructions.to_owned()
}

#[cfg(unix)]
fn same_file(a: &std::fs::Metadata, b: &std::fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt;
    a.is_file()
        && b.is_file()
        && a.dev() == b.dev()
        && a.ino() == b.ino()
        && a.len() == b.len()
        && a.mtime() == b.mtime()
        && a.mtime_nsec() == b.mtime_nsec()
        && a.ctime() == b.ctime()
        && a.ctime_nsec() == b.ctime_nsec()
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn instructions(run: &str, policy: &str) -> String {
        format!(
            "{MANIFEST_IDENTITY}{{\"run_id\":\"{run}\",\"draft_path\":\"drafts/{run}.json\"}}\n{policy}"
        )
    }

    fn record(kind: &str, payload: Value) -> String {
        format!("{}\n", json!({"type":kind,"payload":payload}))
    }

    fn message(text: &str) -> String {
        record(
            "response_item",
            json!({"type":"message","role":"developer","content":[{"type":"input_text","text":text}]}),
        )
    }

    fn history(text: &str) -> String {
        record(
            "session_meta",
            json!({"id":"thread","cli_version":"0.163.0"}),
        ) + &message(text)
    }

    #[test]
    fn initial_new_run_same_run_and_repeated_resumes() {
        let initial = instructions("old", "Fixed policy: 日本語 safety and draft rules.");
        let current = instructions("current", "Fixed policy: 日本語 safety and draft rules.");
        let mut rollout = history(&initial);
        assert!(!retained(&current, "thread", "")); // fresh/reset
        let (_, dynamic) = split(&current).unwrap();
        println!(
            "initial text: {} chars, {} UTF-8 bytes; fixed occurrences: 1",
            initial.chars().count(),
            initial.len()
        );
        for resume in 1..=3 {
            assert!(retained(&current, "thread", &rollout));
            assert!(dynamic.contains("drafts/current.json"));
            assert!(!dynamic.contains("old"));
            assert!(!dynamic.contains("Fixed policy"));
            rollout.push_str(&message(&dynamic));
            println!(
                "resume {resume}: config {} chars/{} bytes; inject {} chars/{} bytes; retained fixed occurrences: {}",
                current.chars().count(),
                current.len(),
                dynamic.chars().count(),
                dynamic.len(),
                rollout.matches("Fixed policy").count()
            );
        }
        assert_eq!(rollout.matches("Fixed policy").count(), 1);
    }

    #[test]
    fn multiple_repository_identities_refresh_together_with_one_retained_contract() {
        let policy = workspace_utils::repository_memory::CODING_RUN_INSTRUCTIONS;
        let initial = format!(
            "{}\n\n{}\n\n{policy}",
            instructions("repo-a-old", ""),
            instructions("repo-b-old", "")
        );
        let current = initial
            .replace("repo-a-old", "repo-a-current")
            .replace("repo-b-old", "repo-b-current");
        // A pre-refactor/full-history mismatch cannot enable reduced injection.
        assert!(!retained(
            &current,
            "thread",
            &history(&instructions("old", "old contract"))
        ));
        assert!(retained(&current, "thread", &history(&initial)));
        let (_, dynamic) = split(&current).unwrap();
        assert!(dynamic.contains("drafts/repo-a-current.json"));
        assert!(dynamic.contains("drafts/repo-b-current.json"));
        assert!(!dynamic.contains("-old"));
        assert!(!dynamic.contains(policy));
    }

    #[test]
    fn changed_policy_and_reversions_require_latest_full_policy() {
        let old = instructions("old", "policy v1");
        let new = instructions("current", "policy v2");
        let rollout = history(&old);
        assert!(!retained(&new, "thread", &rollout));
        let rollout = rollout + &message(&new);
        assert!(retained(&new, "thread", &rollout));
        assert!(!retained(&old, "thread", &rollout));
        assert!(!retained(&(new.clone() + "\n"), "thread", &rollout));
        assert!(!retained(
            &new.replace("policy v2", "    policy v2"),
            "thread",
            &rollout
        ));
    }

    #[test]
    fn compact_rollback_unknown_or_incomplete_history_falls_back() {
        let current = instructions("current", "policy");
        let rollout = history(&current);
        for invalidation in [
            record("compacted", json!({"replacement_history":[]})),
            record(
                "event_msg",
                json!({"type":"thread_rolled_back","num_turns":1}),
            ),
        ] {
            let invalidated = rollout.clone() + &invalidation;
            assert!(!retained(&current, "thread", &invalidated));
            assert!(!retained(
                &current,
                "thread",
                &(invalidated + &message(&current))
            ));
        }
        assert!(!retained(&current, "other", &rollout));
        assert!(!retained(&current, "thread", rollout.trim_end()));
        assert!(!retained(
            &current,
            "thread",
            &(rollout.clone() + "invalid\n")
        ));
        assert!(!retained(
            &current,
            "thread",
            &(rollout.clone() + &record("new_format", json!({})))
        ));
        assert!(!retained(
            &current,
            "thread",
            &(rollout + &message("other developer update"))
        ));
        assert!(!retained(
            "custom instructions only",
            "thread",
            &history("custom instructions only")
        ));
        let fork = record(
            "session_meta",
            json!({"id":"thread","cli_version":"0.163.0","forked_from_id":"parent"}),
        ) + &message(&current);
        assert!(!retained(&current, "thread", &fork));
    }

    #[test]
    fn parallel_identity_refreshes_but_every_other_byte_is_fixed() {
        let old = instructions(
            "old",
            &format!("policy\n{PARALLEL_IDENTITY}{{\"workspace_id\":\"old\"}}\npeer safety"),
        );
        let new = instructions(
            "new",
            &format!("policy\n{PARALLEL_IDENTITY}{{\"workspace_id\":\"new\"}}\npeer safety"),
        );
        assert!(retained(&new, "thread", &history(&old)));
        let (fixed, dynamic) = split(&new).unwrap();
        assert!(fixed.contains("peer safety"));
        assert!(dynamic.contains("\"workspace_id\":\"new\""));
        assert!(!retained(
            &new.replace("peer safety", "changed safety"),
            "thread",
            &history(&old)
        ));
        assert!(split(&new.replace("\"run_id\":\"new\"", "\"run_id\":null")).is_none());
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn bounded_file_evidence_or_full_fallback() {
        let current = instructions("current", "policy");
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().canonicalize().unwrap().join("fixture.jsonl");
        assert_eq!(
            for_resume(&current, "thread", None, Some("codex/0.163.0")).await,
            current
        );
        assert_eq!(
            for_resume(&current, "thread", Some(&path), Some("codex/0.163.0")).await,
            current
        );
        tokio::fs::write(&path, history(&current)).await.unwrap();
        let evidence = prepare(&current, "thread", &path).await.unwrap();
        let settings = record("event_msg", json!({"type":"thread_settings_applied"}));
        tokio::fs::write(&path, history(&current) + &settings)
            .await
            .unwrap();
        assert_eq!(
            evidence.after_resume(Some(&path)).await.unwrap(),
            split(&current).unwrap().1
        );
        assert!(retained(
            &current,
            "thread",
            &(history(&current) + &settings)
        ));
        tokio::fs::write(&path, history(&current)).await.unwrap();
        for ua in [None, Some("codex/0.999.0"), Some("codex/0.163.0-alpha")] {
            assert_eq!(
                for_resume(&current, "thread", Some(&path), ua).await,
                current
            );
        }
        assert_eq!(
            for_resume(&current, "thread", Some(&path), Some("codex/0.163.0")).await,
            split(&current).unwrap().1
        );
        assert_eq!(
            for_resume(&current, "wrong", Some(&path), Some("codex/0.163.0")).await,
            current
        );
        // Mutation between the pre-resume read and native hydration invalidates
        // evidence, as does replacing the inode or changing the returned path.
        let evidence = prepare(&current, "thread", &path).await.unwrap();
        tokio::fs::write(&path, history(&current) + "changed\n")
            .await
            .unwrap();
        assert!(evidence.after_resume(Some(&path)).await.is_none());
        tokio::fs::write(&path, history(&current)).await.unwrap();
        let evidence = prepare(&current, "thread", &path).await.unwrap();
        tokio::fs::write(&path, history(&current) + &message(&current))
            .await
            .unwrap();
        assert!(evidence.after_resume(Some(&path)).await.is_none());
        tokio::fs::write(&path, history(&current)).await.unwrap();
        let evidence = prepare(&current, "thread", &path).await.unwrap();
        let replacement = dir.path().join("replacement.jsonl");
        tokio::fs::write(&replacement, history(&current))
            .await
            .unwrap();
        tokio::fs::rename(&replacement, &path).await.unwrap();
        assert!(evidence.after_resume(Some(&path)).await.is_none());
        let evidence = prepare(&current, "thread", &path).await.unwrap();
        assert!(evidence.after_resume(Some(&replacement)).await.is_none());
        let link = dir.path().join("link.jsonl");
        std::os::unix::fs::symlink(&path, &link).unwrap();
        assert!(prepare(&current, "thread", &link).await.is_none());
        let socket_path = dir.path().join("socket.jsonl");
        let _socket = std::os::unix::net::UnixListener::bind(&socket_path).unwrap();
        assert!(prepare(&current, "thread", &socket_path).await.is_none());
        assert!(prepare(&current, "thread", dir.path()).await.is_none());
        let file = tokio::fs::File::create(&path).await.unwrap();
        file.set_len(MAX_ROLLOUT_BYTES + 1).await.unwrap();
        drop(file);
        assert_eq!(
            for_resume(&current, "thread", Some(&path), Some("codex/0.163.0")).await,
            current
        );
    }

    #[test]
    fn unknown_events_replacements_history_bases_and_reset_never_prove_retention() {
        let current = instructions("current", "policy");
        for record in [
            record("event_msg", json!({"type":"future_history_trim"})),
            record("event_msg", json!({"type":"context_compacted"})),
            record("retained_context", json!({})),
            record("response_item", json!({"type":"context_compaction"})),
            record("response_item", json!({"type":"configuration_update"})),
        ] {
            assert!(!retained(
                &current,
                "thread",
                &(history(&current) + &record)
            ));
        }
        for meta in [
            json!({"id":"thread","cli_version":"0.163.0","history_base":{"thread_id":"base"}}),
            json!({"id":"thread","cli_version":"unknown"}),
            json!({"id":"thread","cli_version":"0.163.0","forked_from_ordinal_exclusive":1}),
        ] {
            assert!(!retained(
                &current,
                "thread",
                &(record("session_meta", meta) + &message(&current))
            ));
        }
        assert!(!retained(
            &current,
            "thread",
            &record(
                "session_meta",
                json!({"id":"thread","cli_version":"0.163.0"})
            )
        ));
        assert!(!retained(
            &current,
            "thread",
            &(message(&current) + &history(&current))
        ));
    }
}
