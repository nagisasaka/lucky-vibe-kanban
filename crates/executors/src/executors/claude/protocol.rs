use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex as StdMutex,
        atomic::{AtomicBool, Ordering},
    },
};

use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    process::{ChildStdin, ChildStdout},
    sync::{Mutex, oneshot},
};
use tokio_util::sync::CancellationToken;

use super::types::{CLIMessage, ControlRequestType, ControlResponseMessage, ControlResponseType};
use crate::{
    approvals::ExecutorApprovalError,
    executors::{
        ExecutorError,
        claude::{
            client::ClaudeAgentClient,
            types::{Message, PermissionMode, SDKControlRequest, SDKControlRequestType},
        },
    },
};

type ControlWaiters =
    Arc<StdMutex<HashMap<String, oneshot::Sender<Result<serde_json::Value, ()>>>>>;

/// Handles bidirectional control protocol communication
#[derive(Clone, Debug)]
pub struct ProtocolPeer {
    stdin: Arc<Mutex<ChildStdin>>,
    interrupt_sent: Arc<AtomicBool>,
    pending: ControlWaiters,
    closed: Arc<AtomicBool>,
}

#[async_trait::async_trait]
impl crate::executors::ExecutorControl for ProtocolPeer {
    async fn send(
        &self,
        control: crate::executors::provider_adapter::DirectControl,
    ) -> Result<Vec<u8>, ExecutorError> {
        use crate::executors::provider_adapter::DirectControl;

        match control {
            DirectControl::Cancel => self.send_interrupt_once().await,
            DirectControl::Steer { text } => self.send_user_message_bytes(text).await,
            DirectControl::Approve {
                request_id,
                approved,
                reason,
            } => {
                let response = if approved {
                    serde_json::json!({
                        "behavior": "allow",
                        "updatedInput": {}
                    })
                } else {
                    serde_json::json!({
                        "behavior": "deny",
                        "message": reason.unwrap_or_else(|| "Denied by user".to_string())
                    })
                };
                self.send_json_bytes(&ControlResponseMessage::new(ControlResponseType::Success {
                    request_id,
                    response: Some(response),
                }))
                .await
            }
            DirectControl::Input { .. } => Err(ExecutorError::Io(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "Claude Code does not expose a separate host input response",
            ))),
            DirectControl::UpdatePlanGoalDraft { .. }
            | DirectControl::GoalUpdate { .. }
            | DirectControl::GoalClear => Err(ExecutorError::Io(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "Claude Code does not expose Codex Goal controls",
            ))),
        }
    }
}

impl ProtocolPeer {
    pub fn spawn(
        stdin: ChildStdin,
        stdout: ChildStdout,
        client: Arc<ClaudeAgentClient>,
        cancel: CancellationToken,
    ) -> Self {
        let peer = Self {
            stdin: Arc::new(Mutex::new(stdin)),
            interrupt_sent: Arc::new(AtomicBool::new(false)),
            pending: Default::default(),
            closed: Arc::new(AtomicBool::new(false)),
        };

        let reader_peer = peer.clone();
        tokio::spawn(async move {
            if let Err(e) = reader_peer.read_loop(stdout, Some(client), cancel).await {
                tracing::error!("Protocol reader loop error: {}", e);
            }
            reader_peer.closed.store(true, Ordering::Release);
            reader_peer.pending.lock().unwrap().clear();
        });

        peer
    }

    /// Passive transport uses the same correlation/control framing without
    /// execution callbacks, user messages, interrupts or raw logging.
    pub(super) fn spawn_passive(
        stdin: ChildStdin,
        stdout: ChildStdout,
        cancel: CancellationToken,
    ) -> (Self, tokio::task::JoinHandle<()>) {
        let peer = Self {
            stdin: Arc::new(Mutex::new(stdin)),
            interrupt_sent: Arc::new(AtomicBool::new(false)),
            pending: Default::default(),
            closed: Arc::new(AtomicBool::new(false)),
        };
        let reader = peer.clone();
        let task = tokio::spawn(async move {
            let _ = reader.read_loop(stdout, None, cancel).await;
            reader.closed.store(true, Ordering::Release);
            reader.pending.lock().unwrap().clear();
        });
        (peer, task)
    }

    async fn request_control(
        &self,
        request: SDKControlRequestType,
    ) -> Result<serde_json::Value, ExecutorError> {
        if self.closed.load(Ordering::Acquire) {
            return Err(ExecutorError::Io(std::io::Error::other(
                "Claude control connection closed",
            )));
        }
        let request = SDKControlRequest::new(request);
        let (tx, rx) = oneshot::channel();
        {
            let mut pending = self.pending.lock().unwrap();
            if pending.len() >= 4 {
                return Err(ExecutorError::Io(std::io::Error::other(
                    "Too many Claude control reads",
                )));
            }
            pending.insert(request.request_id.clone(), tx);
        }
        struct RemovePending<'a>(&'a ProtocolPeer, String);
        impl Drop for RemovePending<'_> {
            fn drop(&mut self) {
                self.0.pending.lock().unwrap().remove(&self.1);
            }
        }
        let _remove = RemovePending(self, request.request_id.clone());
        tokio::time::timeout(std::time::Duration::from_secs(5), async {
            self.send_json(&request).await?;
            rx.await.ok().and_then(Result::ok).ok_or_else(|| {
                ExecutorError::Io(std::io::Error::other(
                    "Claude control RPC unsupported or unavailable",
                ))
            })
        })
        .await
        .map_err(|_| {
            ExecutorError::Io(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "Claude control RPC timed out",
            ))
        })?
    }

    pub(super) async fn initialize_passive(&self) -> Result<(), ExecutorError> {
        self.request_control(SDKControlRequestType::Initialize { hooks: None })
            .await
            .map(|_| ())
    }

    pub async fn get_usage(&self) -> Result<super::usage::Usage, ExecutorError> {
        let value = self
            .request_control(SDKControlRequestType::GetUsage {
                skip_behaviors: true,
            })
            .await?;
        if value.get("behaviors") != Some(&serde_json::Value::Null) {
            return Err(ExecutorError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "CLI did not confirm skipped usage behaviors",
            )));
        }
        let usage: super::usage::Usage = serde_json::from_value(value)?;
        if !usage.validate() {
            return Err(ExecutorError::Io(std::io::Error::other(
                "Incompatible usage response",
            )));
        }
        Ok(usage)
    }

    async fn read_loop(
        &self,
        stdout: ChildStdout,
        client: Option<Arc<ClaudeAgentClient>>,
        cancel: CancellationToken,
    ) -> Result<(), ExecutorError> {
        let max_bytes = if client.is_none() {
            2 * 1024 * 1024
        } else {
            u64::MAX
        };
        let mut reader = BufReader::new(stdout.take(max_bytes));
        let mut buffer = String::new();
        let mut cancellation_observed = false;

        loop {
            buffer.clear();
            tokio::select! {
                biased;
                _ = cancel.cancelled(), if !cancellation_observed => {
                    cancellation_observed = true;
                    if client.is_none() { break; }
                    tracing::info!("Cancellation received in read_loop, sending interrupt to Claude");
                    if let Err(e) = self.send_interrupt_once().await {
                        tracing::warn!("Failed to send interrupt to Claude: {e}");
                    }
                    // Continue the loop to read Claude's response (it should send a result)
                }
                line_result = reader.read_line(&mut buffer) => {
                    match line_result {
                        Ok(0) => break, // EOF
                        Ok(_) => {
                            let line = buffer.trim();
                            if line.is_empty() {
                                continue;
                            }
                            // Correlate before execution logging: usage snapshots
                            // never become transcript/log messages.
                            if let Ok(CLIMessage::ControlResponse { response }) = serde_json::from_str::<CLIMessage>(line) {
                                let (id, result) = match response {
                                    ControlResponseType::Success { request_id, response } => (request_id, Ok(response.unwrap_or(serde_json::Value::Null))),
                                    ControlResponseType::Error { request_id, .. } => (request_id, Err(())),
                                };
                                if let Some(sender) = self.pending.lock().unwrap().remove(&id) { let _ = sender.send(result); }
                                continue;
                            }
                            if serde_json::from_str::<serde_json::Value>(line).ok().is_some_and(|v|v["type"]=="control_response") {
                                // Unknown experimental response shapes must not
                                // spill raw usage or account fields into logs.
                                continue;
                            }
                            let Some(client) = &client else {
                                if serde_json::from_str::<CLIMessage>(line).is_ok_and(|m| matches!(m, CLIMessage::ControlRequest { .. } | CLIMessage::Result(_))) {
                                    break;
                                }
                                continue;
                            };
                            client.log_message(line).await?;

                            // Parse and handle control messages
                            match serde_json::from_str::<CLIMessage>(line) {
                                Ok(CLIMessage::ControlRequest {
                                    request_id,
                                    request,
                                }) => {
                                    self.handle_control_request(client, request_id, request)
                                        .await;
                                }
                                Ok(CLIMessage::Result(_)) => {
                                    break;
                                }
                                _ => {}
                            }
                        }
                        Err(e) => {
                            tracing::error!("Error reading stdout: {}", e);
                            break;
                        }
                    }
                }
            }
        }
        self.closed.store(true, Ordering::Release);
        self.pending.lock().unwrap().clear();
        Ok(())
    }

    async fn handle_control_request(
        &self,
        client: &Arc<ClaudeAgentClient>,
        request_id: String,
        request: ControlRequestType,
    ) {
        match request {
            ControlRequestType::CanUseTool {
                tool_name,
                input,
                permission_suggestions,
                blocked_paths: _,
                tool_use_id,
            } => {
                match client
                    .on_can_use_tool(tool_name, input, permission_suggestions, tool_use_id)
                    .await
                {
                    Ok(result) => {
                        if let Err(e) = self
                            .send_hook_response(request_id, serde_json::to_value(result).unwrap())
                            .await
                        {
                            tracing::error!("Failed to send permission result: {e}");
                        }
                    }
                    Err(ExecutorError::ExecutorApprovalError(ExecutorApprovalError::Cancelled)) => {
                    }
                    Err(e) => {
                        tracing::error!("Error in on_can_use_tool: {e}");
                        if let Err(e2) = self.send_error(request_id, e.to_string()).await {
                            tracing::error!("Failed to send error response: {e2}");
                        }
                    }
                }
            }
            ControlRequestType::HookCallback {
                callback_id,
                input,
                tool_use_id,
            } => {
                match client
                    .on_hook_callback(callback_id, input, tool_use_id)
                    .await
                {
                    Ok(hook_output) => {
                        if let Err(e) = self.send_hook_response(request_id, hook_output).await {
                            tracing::error!("Failed to send hook callback result: {e}");
                        }
                    }
                    Err(e) => {
                        tracing::error!("Error in on_hook_callback: {e}");
                        if let Err(e2) = self.send_error(request_id, e.to_string()).await {
                            tracing::error!("Failed to send error response: {e2}");
                        }
                    }
                }
            }
        }
    }

    pub async fn send_hook_response(
        &self,
        request_id: String,
        hook_output: serde_json::Value,
    ) -> Result<(), ExecutorError> {
        self.send_json(&ControlResponseMessage::new(ControlResponseType::Success {
            request_id,
            response: Some(hook_output),
        }))
        .await
    }

    /// Send error response to CLI
    async fn send_error(&self, request_id: String, error: String) -> Result<(), ExecutorError> {
        self.send_json(&ControlResponseMessage::new(ControlResponseType::Error {
            request_id,
            error: Some(error),
        }))
        .await
    }

    async fn send_json<T: serde::Serialize>(&self, message: &T) -> Result<(), ExecutorError> {
        self.send_json_bytes(message).await.map(|_| ())
    }

    async fn send_json_bytes<T: serde::Serialize>(
        &self,
        message: &T,
    ) -> Result<Vec<u8>, ExecutorError> {
        let json = serde_json::to_string(message)?;
        let mut bytes = json.into_bytes();
        bytes.push(b'\n');
        let mut stdin = self.stdin.lock().await;
        stdin.write_all(&bytes).await?;
        stdin.flush().await?;
        Ok(bytes)
    }

    async fn send_interrupt_once(&self) -> Result<Vec<u8>, ExecutorError> {
        if self
            .interrupt_sent
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return Ok(Vec::new());
        }

        let result = self
            .send_json_bytes(&SDKControlRequest::new(SDKControlRequestType::Interrupt {}))
            .await;
        if result.is_err() {
            self.interrupt_sent.store(false, Ordering::Release);
        }
        result
    }

    pub async fn send_user_message(&self, content: String) -> Result<(), ExecutorError> {
        self.send_user_message_bytes(content).await.map(|_| ())
    }

    async fn send_user_message_bytes(&self, content: String) -> Result<Vec<u8>, ExecutorError> {
        let message = Message::new_user(content);
        self.send_json_bytes(&message).await
    }

    pub async fn initialize(&self, hooks: Option<serde_json::Value>) -> Result<(), ExecutorError> {
        self.send_json(&SDKControlRequest::new(SDKControlRequestType::Initialize {
            hooks,
        }))
        .await
    }
    pub async fn interrupt(&self) -> Result<(), ExecutorError> {
        self.send_interrupt_once().await.map(|_| ())
    }

    pub async fn set_permission_mode(&self, mode: PermissionMode) -> Result<(), ExecutorError> {
        self.send_json(&SDKControlRequest::new(
            SDKControlRequestType::SetPermissionMode { mode },
        ))
        .await
    }
}
