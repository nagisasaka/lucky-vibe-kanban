//! Scope raw multiplexed app-server frames before canonical projection.
//! Audit retains the original frames, including child deltas. Only completed
//! child observations become durable activity, never parent output or control.
use std::collections::{BTreeSet, HashMap};

use super::super::provider_adapter::{DecodedProviderEvent, TypedProviderEvent};
use crate::runtime::AgentActivity;

#[derive(Debug, Default)]
pub struct AgentScope {
    root: Option<String>,
    agents: HashMap<String, (Option<String>, Option<String>)>,
    child_turns: HashMap<String, String>,
}

impl AgentScope {
    pub fn apply(&mut self, event: &mut DecodedProviderEvent) {
        self.apply_version(event, true);
    }

    pub fn apply_legacy(&mut self, event: &mut DecodedProviderEvent) {
        self.apply_version(event, false);
    }

    fn apply_version(&mut self, event: &mut DecodedProviderEvent, current: bool) {
        let Some(raw) = event.raw.payload_json.as_ref() else {
            return;
        };
        if self.root.is_none() {
            self.root = raw
                .pointer("/result/thread/id")
                .and_then(|v| v.as_str())
                .map(str::to_owned);
            if let Some(root) = &self.root {
                // The control response establishes the primary identity even
                // if thread/started arrived before it. Never adopt a child
                // notification as the Run's resumable provider session.
                event.typed = TypedProviderEvent::SessionObserved(root.clone());
            }
        }
        let Some(method) = raw.get("method").and_then(|v| v.as_str()) else {
            return;
        };
        let Some(params) = raw.get("params") else {
            return;
        };
        if matches!(method, "thread/started" | "thread.started") {
            event.typed = TypedProviderEvent::AuditOnly {
                event_type: method.to_owned(),
            };
            return;
        }
        let thread = params.get("threadId").and_then(|v| v.as_str());
        let item = params.get("item");

        if current
            && matches!(method, "item/started" | "item/completed")
            && item.and_then(|v| v.get("type")).and_then(|v| v.as_str())
                == Some("collabAgentToolCall")
        {
            let item = item.unwrap();
            let tool = item.get("tool").and_then(|v| v.as_str());
            let spawn = tool == Some("spawnAgent");
            let states = item.get("agentsStates").and_then(|v| v.as_object());
            let mut ids = BTreeSet::new();
            if let Some(receivers) = item.get("receiverThreadIds").and_then(|v| v.as_array()) {
                ids.extend(receivers.iter().filter_map(|v| v.as_str()));
            }
            if let Some(states) = states {
                ids.extend(states.keys().map(String::as_str));
            }
            let activities = ids
                .into_iter()
                .filter(|id| self.root.as_deref() != Some(*id))
                .map(|id| {
                    let record = self.agents.entry(id.to_owned()).or_default();
                    // Sending input, waiting or closing a peer is not parenting it.
                    if spawn
                        && let Some(sender) = item
                            .get("senderThreadId")
                            .and_then(|v| v.as_str())
                            .filter(|sender| *sender != id)
                    {
                        record.0 = Some(sender.to_owned());
                    }
                    let state = states.and_then(|states| states.get(id));
                    let kind = match state.and_then(|v| v.get("status")).and_then(|v| v.as_str()) {
                        Some("pendingInit") => "pendingInit",
                        Some("running") => "running",
                        Some("completed") => "completed",
                        Some("interrupted") => "interrupted",
                        Some("errored") => "failed",
                        Some("shutdown") => "shutdown",
                        Some(_) => "unknown",
                        // Input/resume may start another turn, invalidating any
                        // earlier completion without proving that it is running.
                        None if matches!(tool, Some("sendInput" | "resumeAgent")) => "unknown",
                        // Waiting does not itself change the child's lifecycle.
                        None => "observed",
                    };
                    let mut content = Vec::new();
                    if spawn
                        && method == "item/completed"
                        && let Some(prompt) = item.get("prompt").and_then(|v| v.as_str())
                    {
                        content.push(format!("Task: {prompt}"));
                    }
                    if let Some(message) = state
                        .and_then(|v| v.get("message"))
                        .and_then(|v| v.as_str())
                    {
                        content.push(message.to_owned());
                    }
                    AgentActivity {
                        thread_id: id.to_owned(),
                        parent_thread_id: record.0.clone(),
                        agent_path: record.1.clone(),
                        kind: kind.to_owned(),
                        content: (!content.is_empty()).then(|| content.join("\n\n")),
                    }
                })
                .collect::<Vec<_>>();
            event.typed = match activities.len() {
                0 => TypedProviderEvent::AuditOnly {
                    event_type: method.to_owned(),
                },
                1 => TypedProviderEvent::AgentActivity(activities.into_iter().next().unwrap()),
                _ => TypedProviderEvent::AgentActivities(activities),
            };
            return;
        }

        if method == "item/started"
            && item.and_then(|v| v.get("type")).and_then(|v| v.as_str()) == Some("subAgentActivity")
        {
            event.typed = TypedProviderEvent::AuditOnly {
                event_type: method.to_owned(),
            };
            return;
        }

        if method == "item/completed"
            && item.and_then(|v| v.get("type")).and_then(|v| v.as_str()) == Some("subAgentActivity")
        {
            let item = item.unwrap();
            if let Some(id) = item.get("agentThreadId").and_then(|v| v.as_str()) {
                let kind = item
                    .get("kind")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");
                // Native collaboration interruption is a request. Only the child's
                // turn/completed interrupted status proves termination.
                let kind = if current && kind == "interrupted" {
                    "interrupt_requested"
                } else if current && kind == "interacted" {
                    // QueueOnly and TriggerTurn share this event. Neither lets
                    // us retain an earlier completion as the current state.
                    "unknown"
                } else {
                    kind
                };
                let record = self.agents.entry(id.to_owned()).or_default();
                if let Some(path) = item.get("agentPath").and_then(|v| v.as_str()) {
                    record.1 = Some(path.to_owned());
                }
                if kind == "started" && thread.is_some_and(|parent| parent != id) {
                    record.0 = thread.map(str::to_owned);
                }
                // Interactions with the primary agent are not child creation.
                if self.root.as_deref() != Some(id) {
                    event.typed = TypedProviderEvent::AgentActivity(AgentActivity {
                        thread_id: id.to_owned(),
                        parent_thread_id: record.0.clone(),
                        agent_path: record.1.clone(),
                        kind: kind.to_owned(),
                        content: None,
                    });
                    return;
                }
                event.typed = TypedProviderEvent::AuditOnly {
                    event_type: method.to_owned(),
                };
                return;
            }
        }

        let Some(thread) = thread else {
            if method.starts_with("item/")
                || method.starts_with("turn/")
                || method.starts_with("thread/goal/")
                || (current && matches!(event.typed, TypedProviderEvent::TokenUsage { .. }))
            {
                event.typed = TypedProviderEvent::AuditOnly {
                    event_type: method.to_owned(),
                };
            }
            return;
        };
        if self.root.as_deref() == Some(thread) {
            return;
        }
        if current {
            let turn = params.pointer("/turn/id").and_then(|v| v.as_str());
            if method == "turn/started" {
                if let Some(turn) = turn {
                    self.child_turns.insert(thread.to_owned(), turn.to_owned());
                } else {
                    self.child_turns.remove(thread);
                }
            } else if method == "turn/completed"
                && self
                    .child_turns
                    .get(thread)
                    .is_some_and(|current_turn| Some(current_turn.as_str()) != turn)
            {
                // A late completion for a previous turn must not close the
                // currently observed child turn, including after replay.
                event.typed = TypedProviderEvent::AuditOnly {
                    event_type: method.to_owned(),
                };
                return;
            }
        }
        let (parent, path) = self.agents.get(thread).cloned().unwrap_or_default();
        let observation = match (&event.typed, method) {
            (
                TypedProviderEvent::Message {
                    content,
                    final_output,
                    ..
                },
                _,
            ) => Some((
                if *final_output {
                    "answer"
                } else {
                    "commentary"
                },
                Some(content.clone()),
            )),
            (
                TypedProviderEvent::ToolCall {
                    name,
                    arguments,
                    result,
                    ..
                },
                "item/completed",
            ) => Some((
                "tool",
                Some(format!(
                    "{name}\nArguments: {}\nResult: {}",
                    arguments.as_ref().unwrap_or(&serde_json::Value::Null),
                    result.as_ref().unwrap_or(&serde_json::Value::Null)
                )),
            )),
            (_, "turn/started") => Some(("running", None)),
            (_, "turn/completed") => Some((
                match params.pointer("/turn/status").and_then(|v| v.as_str()) {
                    Some("completed") => "completed",
                    Some("failed") => "failed",
                    Some("interrupted") => "interrupted",
                    _ => "unknown",
                },
                None,
            )),
            _ => None,
        };
        event.typed = match observation {
            Some((kind, content)) => TypedProviderEvent::AgentActivity(AgentActivity {
                thread_id: thread.to_owned(),
                parent_thread_id: parent,
                agent_path: path,
                kind: kind.to_owned(),
                content,
            }),
            None => TypedProviderEvent::AuditOnly {
                event_type: method.to_owned(),
            },
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        executors::provider_adapter::DirectProvider,
        runtime::{NativeAuditChannel, NativeAuditDirection, NativeAuditFrame},
    };

    fn event(value: serde_json::Value, sequence: u64) -> DecodedProviderEvent {
        let frame = NativeAuditFrame::from_bytes(
            sequence,
            chrono::Utc::now(),
            NativeAuditDirection::Output,
            NativeAuditChannel::Stdout,
            "application/json",
            uuid::Uuid::nil(),
            &serde_json::to_vec(&value).unwrap(),
        );
        DirectProvider::Codex.decode_native_frame(&frame).unwrap()
    }

    #[test]
    fn children_and_unknown_threads_never_become_parent_output() {
        let mut scope = AgentScope::default();
        scope.apply(&mut event(
            serde_json::json!({"result":{"thread":{"id":"root"}}}),
            1,
        ));
        let mut spawn = event(
            serde_json::json!({"method":"item/completed","params":{"threadId":"root","item":{"type":"subAgentActivity","id":"spawn","kind":"started","agentThreadId":"child","agentPath":"/root/research"}}}),
            2,
        );
        scope.apply(&mut spawn);
        assert!(
            matches!(spawn.typed, TypedProviderEvent::AgentActivity(ref a) if a.parent_thread_id.as_deref()==Some("root"))
        );
        for thread in ["child", "unknown"] {
            let mut child = event(
                serde_json::json!({"method":"item/completed","params":{"threadId":thread,"item":{"type":"agentMessage","id":"answer","text":"child answer","phase":"final_answer"}}}),
                3,
            );
            scope.apply(&mut child);
            assert!(
                matches!(child.typed, TypedProviderEvent::AgentActivity(ref a) if a.kind=="answer")
            );
            let mut delta = event(
                serde_json::json!({"method":"item/agentMessage/delta","params":{"threadId":thread,"itemId":"answer","delta":"abc"}}),
                4,
            );
            scope.apply(&mut delta);
            assert!(matches!(delta.typed, TypedProviderEvent::AuditOnly { .. }));
        }
        let mut parent = event(
            serde_json::json!({"method":"item/completed","params":{"threadId":"root","item":{"type":"agentMessage","id":"parent-answer","text":"parent answer"}}}),
            5,
        );
        scope.apply(&mut parent);
        assert!(matches!(
            parent.typed,
            TypedProviderEvent::Message {
                final_output: true,
                ..
            }
        ));
    }

    #[test]
    fn nested_agents_keep_immediate_parent_and_child_goal_is_not_parent_control() {
        let mut scope = AgentScope::default();
        scope.apply(&mut event(
            serde_json::json!({"result":{"thread":{"id":"root"}}}),
            1,
        ));
        let mut spawn = event(
            serde_json::json!({"method":"item/completed","params":{"threadId":"child","item":{"type":"subAgentActivity","id":"spawn","kind":"started","agentThreadId":"grandchild","agentPath":"/root/research/review"}}}),
            2,
        );
        scope.apply(&mut spawn);
        assert!(
            matches!(spawn.typed, TypedProviderEvent::AgentActivity(ref a) if a.parent_thread_id.as_deref()==Some("child"))
        );
        let mut goal = event(
            serde_json::json!({"method":"thread/goal/cleared","params":{"threadId":"grandchild"}}),
            3,
        );
        scope.apply(&mut goal);
        assert!(matches!(goal.typed, TypedProviderEvent::AuditOnly { .. }));
        let mut anonymous = event(
            serde_json::json!({"method":"item/completed","params":{"item":{"type":"agentMessage","id":"m","text":"unknown origin"}}}),
            4,
        );
        scope.apply(&mut anonymous);
        assert!(matches!(
            anonymous.typed,
            TypedProviderEvent::AuditOnly { .. }
        ));
    }

    #[test]
    fn collaboration_projects_each_child_without_inventing_lifecycle_or_parentage() {
        let mut scope = AgentScope::default();
        scope.apply(&mut event(
            serde_json::json!({"result":{"thread":{"id":"root"}}}),
            1,
        ));
        let spawn = serde_json::json!({"method":"item/completed","params":{"threadId":"root","item":{
            "type":"collabAgentToolCall","id":"spawn","tool":"spawnAgent","status":"completed",
            "senderThreadId":"root","receiverThreadIds":["a","b","root"],"prompt":"Review changes",
            "agentsStates":{"a":{"status":"running"},"b":{"status":"errored","message":"Could not start"},"root":{"status":"running"}}
        }}});
        let mut observed = event(spawn, 2);
        scope.apply(&mut observed);
        let TypedProviderEvent::AgentActivities(children) = observed.typed else {
            panic!("expected children");
        };
        assert_eq!(children.len(), 2);
        assert_eq!(children[0].thread_id, "a");
        assert_eq!(children[0].parent_thread_id.as_deref(), Some("root"));
        assert_eq!(children[0].kind, "running");
        assert_eq!(children[0].content.as_deref(), Some("Task: Review changes"));
        assert_eq!(children[1].kind, "failed");
        assert!(
            children[1]
                .content
                .as_ref()
                .unwrap()
                .contains("Could not start")
        );

        for method in ["item/started", "item/completed"] {
            let mut wait = event(
                serde_json::json!({"method":method,"params":{"threadId":"peer","item":{
                    "type":"collabAgentToolCall","id":"wait","tool":"wait","status":"completed",
                    "senderThreadId":"peer","receiverThreadIds":["a"],"prompt":"not a new task","agentsStates":{}
                }}}),
                3,
            );
            scope.apply(&mut wait);
            assert!(
                matches!(wait.typed, TypedProviderEvent::AgentActivity(ref a)
                if a.parent_thread_id.as_deref() == Some("root") && a.kind == "observed" && a.content.is_none())
            );
        }
        for (native, expected) in [
            ("pendingInit", "pendingInit"),
            ("completed", "completed"),
            ("interrupted", "interrupted"),
            ("shutdown", "shutdown"),
            ("notFound", "unknown"),
            ("future", "unknown"),
        ] {
            let mut update = event(
                serde_json::json!({"method":"item/completed","params":{"threadId":"root","item":{
                    "type":"collabAgentToolCall","id":"wait","tool":"wait","status":"completed",
                    "receiverThreadIds":[],"agentsStates":{"a":{"status":native}}
                }}}),
                4,
            );
            scope.apply(&mut update);
            assert!(
                matches!(update.typed, TypedProviderEvent::AgentActivity(ref a) if a.kind == expected)
            );
        }
        let mut root_only = event(
            serde_json::json!({"method":"item/completed","params":{"threadId":"peer","item":{
                "type":"collabAgentToolCall","id":"send","tool":"sendInput","receiverThreadIds":["root"],"agentsStates":{}
            }}}),
            5,
        );
        scope.apply(&mut root_only);
        assert!(matches!(
            root_only.typed,
            TypedProviderEvent::AuditOnly { .. }
        ));
    }

    #[test]
    fn child_restart_requests_invalidate_completion_without_inventing_running() {
        for tool in ["sendInput", "resumeAgent"] {
            for method in ["item/started", "item/completed"] {
                let mut scope = AgentScope::default();
                scope.apply(&mut event(
                    serde_json::json!({"result":{"thread":{"id":"root"}}}),
                    1,
                ));
                let mut update = event(
                    serde_json::json!({"method":method,"params":{"threadId":"root","item":{
                        "type":"collabAgentToolCall","id":"restart","tool":tool,"status":"completed",
                        "receiverThreadIds":["child"],"agentsStates":{}
                    }}}),
                    2,
                );
                scope.apply(&mut update);
                assert!(
                    matches!(update.typed, TypedProviderEvent::AgentActivity(ref a) if a.kind == "unknown")
                );
                let mut explicit = event(
                    serde_json::json!({"method":method,"params":{"threadId":"root","item":{
                        "type":"collabAgentToolCall","id":"restart","tool":tool,"status":"completed",
                        "receiverThreadIds":["child"],"agentsStates":{"child":{"status":"running"}}
                    }}}),
                    3,
                );
                scope.apply(&mut explicit);
                assert!(
                    matches!(explicit.typed, TypedProviderEvent::AgentActivity(ref a) if a.kind == "running")
                );
            }
        }
        for current in [false, true] {
            let mut scope = AgentScope::default();
            let mut interaction = event(
                serde_json::json!({"method":"item/completed","params":{"threadId":"root","item":{
                    "type":"subAgentActivity","id":"interaction","kind":"interacted","agentThreadId":"child"
                }}}),
                1,
            );
            scope.apply_version(&mut interaction, current);
            let expected = if current { "unknown" } else { "interacted" };
            assert!(
                matches!(interaction.typed, TypedProviderEvent::AgentActivity(ref a) if a.kind == expected)
            );
        }
    }

    #[test]
    fn current_projection_ignores_stale_child_turn_completion() {
        for current in [false, true] {
            let mut scope = AgentScope::default();
            scope.apply_version(
                &mut event(serde_json::json!({"result":{"thread":{"id":"root"}}}), 1),
                current,
            );
            for (sequence, turn) in [(2, "old"), (3, "new")] {
                let mut started = event(
                    serde_json::json!({"method":"turn/started","params":{"threadId":"child","turn":{"id":turn}}}),
                    sequence,
                );
                scope.apply_version(&mut started, current);
                assert!(
                    matches!(started.typed, TypedProviderEvent::AgentActivity(ref a) if a.kind == "running")
                );
            }
            for turn in [Some("old"), None] {
                let mut completed = event(
                    serde_json::json!({"method":"turn/completed","params":{"threadId":"child","turn":{"id":turn,"status":"completed"}}}),
                    4,
                );
                scope.apply_version(&mut completed, current);
                if current {
                    assert!(matches!(
                        completed.typed,
                        TypedProviderEvent::AuditOnly { .. }
                    ));
                } else {
                    assert!(
                        matches!(completed.typed, TypedProviderEvent::AgentActivity(ref a) if a.kind == "completed")
                    );
                }
            }
            let mut completed = event(
                serde_json::json!({"method":"turn/completed","params":{"threadId":"child","turn":{"id":"new","status":"completed"}}}),
                5,
            );
            scope.apply_version(&mut completed, current);
            assert!(
                matches!(completed.typed, TypedProviderEvent::AgentActivity(ref a) if a.kind == "completed")
            );
            let mut unseen = event(
                serde_json::json!({"method":"turn/completed","params":{"threadId":"other-child","turn":{"id":"other","status":"completed"}}}),
                6,
            );
            scope.apply_version(&mut unseen, current);
            assert!(
                matches!(unseen.typed, TypedProviderEvent::AgentActivity(ref a) if a.kind == "completed")
            );
        }
    }

    #[test]
    fn anonymous_and_child_usage_cannot_replace_parent_usage() {
        let mut scope = AgentScope::default();
        scope.apply(&mut event(
            serde_json::json!({"result":{"thread":{"id":"root"}}}),
            1,
        ));
        for thread in [None, Some("child"), Some("root")] {
            let mut params = serde_json::json!({"tokenUsage":{"total":{"inputTokens":100,"outputTokens":20},"last":{"inputTokens":10,"outputTokens":2},"modelContextWindow":1000}});
            if let Some(thread) = thread {
                params["threadId"] = serde_json::json!(thread);
            }
            let mut usage = event(
                serde_json::json!({"method":"thread/tokenUsage/updated","params":params}),
                2,
            );
            assert!(matches!(usage.typed, TypedProviderEvent::TokenUsage { .. }));
            scope.apply(&mut usage);
            if thread == Some("root") {
                assert!(matches!(usage.typed, TypedProviderEvent::TokenUsage { .. }));
            } else {
                assert!(matches!(usage.typed, TypedProviderEvent::AuditOnly { .. }));
            }
        }
    }

    #[test]
    fn interruption_request_is_not_child_termination() {
        let mut scope = AgentScope::default();
        scope.apply(&mut event(
            serde_json::json!({"result":{"thread":{"id":"root"}}}),
            1,
        ));
        let mut request = event(
            serde_json::json!({"method":"item/completed","params":{"threadId":"root","item":{
                "type":"subAgentActivity","id":"interrupt","kind":"interrupted","agentThreadId":"child"
            }}}),
            2,
        );
        scope.apply(&mut request);
        assert!(
            matches!(request.typed, TypedProviderEvent::AgentActivity(ref a) if a.kind == "interrupt_requested")
        );
        let mut settled = event(
            serde_json::json!({"method":"turn/completed","params":{"threadId":"child","turn":{"id":"child-turn","status":"interrupted"}}}),
            3,
        );
        scope.apply(&mut settled);
        assert!(
            matches!(settled.typed, TypedProviderEvent::AgentActivity(ref a) if a.kind == "interrupted")
        );
    }

    #[test]
    fn only_the_control_response_can_bind_the_primary_session() {
        let mut scope = AgentScope::default();
        let mut early = event(
            serde_json::json!({"method":"thread/started","params":{"thread":{"id":"parent"}}}),
            1,
        );
        scope.apply(&mut early);
        assert!(matches!(early.typed, TypedProviderEvent::AuditOnly { .. }));
        let mut response = event(serde_json::json!({"result":{"thread":{"id":"parent"}}}), 2);
        scope.apply(&mut response);
        assert!(
            matches!(response.typed, TypedProviderEvent::SessionObserved(ref id) if id == "parent")
        );
        let mut child = event(
            serde_json::json!({"method":"thread/started","params":{"thread":{"id":"child"}}}),
            3,
        );
        scope.apply(&mut child);
        assert!(matches!(child.typed, TypedProviderEvent::AuditOnly { .. }));
        let mut parent = event(
            serde_json::json!({"method":"item/agentMessage/delta","params":{"threadId":"parent","itemId":"m","delta":"text"}}),
            4,
        );
        scope.apply(&mut parent);
        assert!(matches!(
            parent.typed,
            TypedProviderEvent::MessageDelta { .. }
        ));
    }
}
