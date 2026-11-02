//! Only Codex's empty-form, single MCP tool-call approval is supported here.
//! General server form/URL elicitation is deliberately cancelled. No allowlist,
//! auto_approve, credentials or persistent grants participate in this path.
use std::collections::{HashMap, HashSet};

use codex_app_server_protocol::{
    JSONRPCNotification, McpServerElicitationAction as Action, McpServerElicitationRequest,
    McpServerElicitationRequestParams, McpServerElicitationRequestResponse, RequestId,
};
use serde_json::{Value, json};

use super::{AppServerClient, ApprovalStatus, ExecutorError, JsonRpcPeer};

const MAX_REQUESTS: usize = 1024;

#[derive(Clone, PartialEq)]
struct Call {
    id: String,
    thread: String,
    turn: String,
    server: String,
    tool: String,
    arguments: Value,
}
#[derive(Default)]
pub(super) struct State {
    calls: HashMap<String, Call>,
    seen_requests: HashSet<String>,
    used_calls: HashSet<String>,
}

impl State {
    fn bind(&mut self, params: &McpServerElicitationRequestParams) -> Option<Call> {
        let McpServerElicitationRequest::Form {
            meta: Some(meta),
            requested_schema,
            ..
        } = &params.request
        else {
            return None;
        };
        // Exact empty schema only: credentials, extra form fields and URLs are
        // not representable by the existing yes/no permission UI.
        if serde_json::to_value(requested_schema).ok()? != json!({"type":"object","properties":{}})
            || meta.get("codex_approval_kind")?.as_str()? != "mcp_tool_call"
            || !meta.get("tool_params")?.is_object()
        {
            return None;
        }
        let turn = params.turn_id.as_ref()?;
        let mut matches = self.calls.values().filter(|call| {
            call.thread == params.thread_id
                && call.turn == *turn
                && call.server == params.server_name
                && call.arguments == meta["tool_params"]
        });
        let call = matches.next()?.clone();
        if matches.next().is_some() || self.used_calls.contains(&call.id) {
            return None; // Core provides no tool item ID: ambiguity must deny.
        }
        if meta
            .get("tool_name")
            .is_some_and(|name| name.as_str() != Some(&call.tool))
        {
            return None;
        }
        self.used_calls.insert(call.id.clone());
        Some(call)
    }
}

impl AppServerClient {
    pub(super) async fn observe_mcp_approval_item(&self, notification: &JSONRPCNotification) {
        let Some(params) = &notification.params else {
            return;
        };
        let Some(root) = self.thread_id.lock().await.clone() else {
            return;
        };
        if params.get("threadId").and_then(Value::as_str) != Some(&root) {
            return;
        }
        let mut state = self.mcp_approvals.lock().unwrap();
        if notification.method == "turn/completed" {
            if let Some(turn) = params.pointer("/turn/id").and_then(Value::as_str) {
                state.calls.retain(|_, call| call.turn != turn);
            }
            return;
        }
        let Some(item) = params.get("item") else {
            return;
        };
        let Some(id) = item.get("id").and_then(Value::as_str) else {
            return;
        };
        if notification.method == "item/completed" {
            state.calls.remove(id);
        } else if notification.method == "item/started"
            && item["type"] == "mcpToolCall"
            && item["status"] == "inProgress"
            && state.calls.len() < MAX_REQUESTS
        {
            let (Some(turn), Some(server), Some(tool), Some(arguments)) = (
                params.get("turnId").and_then(Value::as_str),
                item.get("server").and_then(Value::as_str),
                item.get("tool").and_then(Value::as_str),
                item.get("arguments").filter(|args| args.is_object()),
            ) else {
                return;
            };
            let safe_name = |s: &str| {
                !s.is_empty()
                    && s.len() <= 128
                    && s.bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b"_-.:/".contains(&b))
            };
            if !safe_name(server) || !safe_name(tool) {
                return;
            }
            state.calls.insert(
                id.into(),
                Call {
                    id: id.into(),
                    thread: root,
                    turn: turn.into(),
                    server: server.into(),
                    tool: tool.into(),
                    arguments: arguments.clone(),
                },
            );
        }
    }

    pub(super) async fn handle_mcp_approval(
        &self,
        peer: &JsonRpcPeer,
        id: RequestId,
        params: McpServerElicitationRequestParams,
    ) -> Result<(), ExecutorError> {
        // One answer per native request, including malformed/replayed requests.
        // Ignore duplicate IDs while the original explicit decision is pending.
        let capacity = {
            let mut state = self.mcp_approvals.lock().unwrap();
            let key = serde_json::to_string(&id)?;
            if state.seen_requests.contains(&key) {
                return Ok(());
            }
            let capacity = state.seen_requests.len() < MAX_REQUESTS;
            if capacity {
                state.seen_requests.insert(key);
            }
            capacity
        };
        if !capacity {
            return self
                .send_tool_approval_response(
                    peer,
                    id,
                    McpServerElicitationRequestResponse {
                        action: Action::Cancel,
                        content: None,
                        meta: None,
                    },
                )
                .await;
        }
        let target_matches = self.thread_id.lock().await.as_deref() == Some(&params.thread_id)
            && self.turn_id.lock().await.as_deref() == params.turn_id.as_deref()
            && params.turn_id.is_some();
        let call = if target_matches {
            self.mcp_approvals.lock().unwrap().bind(&params)
        } else {
            None
        };
        let action = if let Some(call) = call {
            // Name is taken from the exact active provider item, never the
            // elicitation's free-form message/description/display metadata.
            // The canonical UI joins the same item ID and preserves its existing
            // argument rendering/redaction. Do not duplicate raw args in labels.
            let label = format!(
                "MCP third-party server={} tool={} (one call only)",
                json!(call.server),
                json!(call.tool)
            );
            self.wait_mcp_approval(&call, &params, &label).await
        } else {
            tracing::warn!(
                "Unsupported, mismatched or replayed MCP elicitation; cancelling without permission"
            );
            Action::Cancel
        };
        self.send_tool_approval_response(
            peer,
            id,
            McpServerElicitationRequestResponse {
                action,
                content: None,
                meta: None,
            },
        )
        .await
    }

    pub(super) async fn cancel_invalid_mcp(
        &self,
        peer: &JsonRpcPeer,
        id: RequestId,
    ) -> Result<(), ExecutorError> {
        {
            let mut state = self.mcp_approvals.lock().unwrap();
            let key = serde_json::to_string(&id)?;
            if state.seen_requests.contains(&key) {
                return Ok(());
            }
            if state.seen_requests.len() < MAX_REQUESTS {
                state.seen_requests.insert(key);
            }
        }
        self.send_tool_approval_response(
            peer,
            id,
            McpServerElicitationRequestResponse {
                action: Action::Cancel,
                content: None,
                meta: None,
            },
        )
        .await
    }

    async fn wait_mcp_approval(
        &self,
        call: &Call,
        params: &McpServerElicitationRequestParams,
        label: &str,
    ) -> Action {
        let call_id = &call.id;
        let Some(service) = &self.approvals else {
            return Action::Cancel;
        };
        // Deliberately bypass request_tool_approval's legacy auto_approve branch.
        let Ok(approval_id) = service.create_tool_approval(label).await else {
            return Action::Cancel;
        };
        if approval_id == "noop" {
            return Action::Cancel;
        }
        if self
            .log_writer
            .log_raw(
                &json!({
                    "type":"approval_requested", "approval_id":approval_id,
                    "tool_call_id":call_id,"tool_name":label,
                })
                .to_string(),
            )
            .await
            .is_err()
        {
            return Action::Cancel;
        }
        {
            let mut pending = self.pending_tool_approvals.lock().await;
            *pending += 1;
            if self.log_tool_approval_count(*pending).await.is_err() {
                *pending -= 1;
                return Action::Cancel;
            }
        }
        let result = service
            .wait_tool_approval(&approval_id, self.cancel.clone())
            .await;
        let still_current = self.thread_id.lock().await.as_deref() == Some(&params.thread_id)
            && self.turn_id.lock().await.as_deref() == params.turn_id.as_deref()
            && self.mcp_approvals.lock().unwrap().calls.get(&call.id) == Some(call);
        let (action, reason) = match result {
            Ok(ApprovalStatus::Approved) if !self.cancel.is_cancelled() && still_current => {
                (Action::Accept, None)
            }
            Ok(ApprovalStatus::Denied { reason }) => (Action::Decline, reason),
            _ => (
                Action::Cancel,
                Some("MCP approval cancelled, timed out or unavailable".into()),
            ),
        };
        let resolution_logged = self
            .log_writer
            .log_raw(
                &json!({
                    "type":"approval_resolved", "approval_id":approval_id,
                    "approved":action == Action::Accept,"reason":reason,
                })
                .to_string(),
            )
            .await
            .is_ok();
        {
            let mut pending = self.pending_tool_approvals.lock().await;
            *pending -= 1;
            if self.log_tool_approval_count(*pending).await.is_err() || !resolution_logged {
                return Action::Cancel;
            }
        }
        action
    }
}

#[cfg(test)]
mod tests;
