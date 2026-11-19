use std::{
    collections::{HashMap, HashSet},
    sync::{
        Arc, Mutex as StdMutex, Weak,
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
        ExecutorError, ExecutorExitResult, ExecutorExitSignal,
        claude::{
            client::ClaudeAgentClient,
            types::{Message, PermissionMode, SDKControlRequest, SDKControlRequestType},
        },
    },
};

/// Safe diagnostic categories only; never serialize native error bodies or credentials.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum ControlFailureKind {
    Rejected,
    Timeout,
    Closed,
    Io,
    Protocol,
}

impl ControlFailureKind {
    pub fn from_error(error: &ExecutorError) -> Self {
        match error {
            ExecutorError::Io(error) => match error.kind() {
                std::io::ErrorKind::Unsupported => Self::Rejected,
                std::io::ErrorKind::TimedOut => Self::Timeout,
                std::io::ErrorKind::UnexpectedEof | std::io::ErrorKind::BrokenPipe => Self::Closed,
                _ => Self::Io,
            },
            _ => Self::Protocol,
        }
    }
}

type ControlWaiters =
    Arc<StdMutex<HashMap<String, oneshot::Sender<Result<serde_json::Value, ()>>>>>;

type RuntimeFailure = Arc<StdMutex<Option<oneshot::Sender<ExecutorExitResult>>>>;

/// Handles bidirectional control protocol communication
#[derive(Clone)]
pub struct ProtocolPeer {
    stdin: Arc<Mutex<ChildStdin>>,
    interrupt_sent: Arc<AtomicBool>,
    pending: ControlWaiters,
    closed: Arc<AtomicBool>,
    failure: Option<RuntimeFailure>,
    client: Option<Weak<ClaudeAgentClient>>,
    callbacks: Arc<Mutex<Vec<tokio::task::JoinHandle<()>>>>,
}

impl std::fmt::Debug for ProtocolPeer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProtocolPeer")
            .field("closed", &self.closed)
            .finish_non_exhaustive()
    }
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
                if self.closed.load(Ordering::Acquire) {
                    return Err(ExecutorApprovalError::ServiceUnavailable.into());
                }
                self.client
                    .as_ref()
                    .and_then(Weak::upgrade)
                    .ok_or(ExecutorApprovalError::ServiceUnavailable)?
                    .resolve_approval(&request_id, approved, reason)
                    .await?;
                Ok(Vec::new())
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
    ) -> (Self, ExecutorExitSignal) {
        let (failure_tx, failure_rx) = oneshot::channel();
        let peer = Self {
            stdin: Arc::new(Mutex::new(stdin)),
            interrupt_sent: Arc::new(AtomicBool::new(false)),
            pending: Default::default(),
            closed: Arc::new(AtomicBool::new(false)),
            failure: Some(Arc::new(StdMutex::new(Some(failure_tx)))),
            client: Some(Arc::downgrade(&client)),
            callbacks: Default::default(),
        };

        let reader_peer = peer.clone();
        let reader_cancel = cancel.clone();
        tokio::spawn(async move {
            let outcome = reader_peer
                .read_loop(stdout, Some(client.clone()), cancel)
                .await;
            reader_peer.closed.store(true, Ordering::Release);
            reader_cancel.cancel();
            for callback in reader_peer.callbacks.lock().await.drain(..) {
                let _ = callback.await;
            }
            reader_peer.pending.lock().unwrap().clear();
            if let Err(e) = outcome {
                tracing::error!("Protocol reader loop error: {}", e);
                let _ = client.log_message(&serde_json::json!({"type":"error","message":"Claude control stream closed or failed before a result"}).to_string()).await;
                reader_peer.fail_runtime();
            }
            if client.finish_instruction_delivery().await.is_err() {
                reader_peer.fail_runtime();
            }
        });

        (peer, failure_rx)
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
            failure: None,
            client: None,
            callbacks: Default::default(),
        };
        let reader = peer.clone();
        let task = tokio::spawn(async move {
            let _ = reader.read_loop(stdout, None, cancel).await;
            reader.closed.store(true, Ordering::Release);
            reader.pending.lock().unwrap().clear();
        });
        (peer, task)
    }

    pub(super) fn fail_runtime(&self) {
        if let Some(failure) = &self.failure
            && let Some(sender) = failure.lock().unwrap().take()
        {
            let _ = sender.send(ExecutorExitResult::Failure);
        }
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
            match rx.await {
                Ok(Ok(value)) => Ok(value),
                Ok(Err(())) => Err(ExecutorError::Io(std::io::Error::new(
                    std::io::ErrorKind::Unsupported,
                    "Claude control RPC is unsupported or rejected by the current authentication",
                ))),
                Err(_) => Err(ExecutorError::Io(std::io::Error::new(
                    std::io::ErrorKind::UnexpectedEof,
                    "Claude control connection closed before its response",
                ))),
            }
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
        let mut requests = HashMap::<String, CancellationToken>::new();
        let mut tool_calls = HashSet::new();

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
                        Ok(0) => {
                            if client.is_some() {
                                return Err(ExecutorError::Io(std::io::Error::new(std::io::ErrorKind::UnexpectedEof, "Claude stream ended before result")));
                            }
                            break;
                        },
                        Ok(_) => {
                            let line = buffer.trim();
                            if line.is_empty() {
                                continue;
                            }
                            // Correlate before execution logging: usage snapshots
                            // never become transcript/log messages.
                            if let Some(client) = &client
                                && let Ok(value) = serde_json::from_str::<serde_json::Value>(line)
                                && value["type"] == "control_response" {
                                client.acknowledge_instruction_callback(&value).await?;
                            }
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
                            let message = serde_json::from_str::<CLIMessage>(line);
                            // Do not publish a native success result while a known
                            // compaction delivery is still unconfirmed/incomplete.
                            if matches!(&message, Ok(CLIMessage::Result(_))) {
                                client.finish_instruction_delivery().await?;
                            }
                            client.log_message(line).await?;

                            // Parse and handle control messages
                            match message {
                                Ok(CLIMessage::ControlRequest {
                                    request_id,
                                    request,
                                }) => {
                                    if requests.contains_key(&request_id) {
                                        // A retry is still the original pending request, never
                                        // an early error followed by a second native response.
                                        continue;
                                    }
                                    if let ControlRequestType::CanUseTool { tool_use_id: Some(id), .. } = &request
                                        && !tool_calls.insert(id.clone())
                                    {
                                        self.send_error(request_id, "Duplicate Claude tool request".into()).await?;
                                        continue;
                                    }
                                    let request_cancel = cancel.child_token();
                                    requests.insert(request_id.clone(), request_cancel.clone());
                                    let peer = self.clone();
                                    let client = client.with_cancel(request_cancel);
                                    self.callbacks.lock().await.push(tokio::spawn(async move {
                                        peer.handle_control_request(&client, request_id, request).await;
                                    }));
                                }
                                Ok(CLIMessage::ControlCancelRequest { request_id }) => {
                                    if let Some(request_cancel) = requests.get(&request_id) {
                                        request_cancel.cancel();
                                    }
                                }
                                Ok(CLIMessage::Result(_)) => {
                                    break;
                                }
                                _ => {
                                    // Optional notifications may be ignored, but an unknown
                                    // request needs a rejection so the CLI cannot wait forever.
                                    if let Ok(value) = serde_json::from_str::<serde_json::Value>(line)
                                        && value["type"] == "control_request"
                                        && let Some(id) = value["request_id"].as_str()
                                    {
                                        self.send_error(id.to_string(), "Unsupported Claude control request".into()).await?;
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            return Err(ExecutorError::Io(e));
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
                        if client.is_cancelled() {
                            return;
                        }
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
                if callback_id == super::instructions::CALLBACK {
                    let result = async {
                        let output = client.instruction_callback(&request_id, &input).await?;
                        self.send_hook_response(request_id.clone(), output).await
                    }
                    .await;
                    if let Err(error) = result {
                        let _ = client.finish_instruction_delivery().await;
                        let _ = self.send_error(request_id, error.to_string()).await;
                        self.fail_runtime();
                    }
                    return;
                }
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
        let response = ControlResponseMessage::new(ControlResponseType::Success {
            request_id,
            response: Some(hook_output),
        });
        self.send_json(&response).await?;
        if let Some(client) = self.client.as_ref().and_then(Weak::upgrade) {
            client
                .log_message(
                    &serde_json::json!({
                        "type":"approval_protocol_response", "response":response
                    })
                    .to_string(),
                )
                .await?;
        }
        Ok(())
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
        if self.closed.load(Ordering::Acquire) {
            return Err(ExecutorError::Io(std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                "Claude control connection closed",
            )));
        }
        tokio::time::timeout(std::time::Duration::from_secs(5), async {
            let mut stdin = self.stdin.lock().await;
            stdin.write_all(&bytes).await?;
            stdin.flush().await
        })
        .await
        .map_err(|_| {
            ExecutorError::Io(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "Claude control write timed out",
            ))
        })??;
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
        self.request_control(SDKControlRequestType::Initialize { hooks })
            .await
            .map(|_| ())
    }
    pub async fn interrupt(&self) -> Result<(), ExecutorError> {
        self.send_interrupt_once().await.map(|_| ())
    }

    pub async fn set_permission_mode(&self, mode: PermissionMode) -> Result<(), ExecutorError> {
        self.request_control(SDKControlRequestType::SetPermissionMode { mode })
            .await
            .map(|_| ())
    }
}

#[cfg(test)]
mod instruction_protocol_tests {
    use tokio::{io::AsyncReadExt, process::Command};

    use super::*;
    use crate::instruction_delivery::ResumeInstructions;

    // No network, credentials, settings or real provider. The mock only speaks stdio RPC.
    const MOCK: &str = r#"
import json,sys
mode=sys.argv[1]
turn=sys.argv[2] if len(sys.argv)>2 else "one"
def emit(x): print(json.dumps(x),flush=True)
def read(): return json.loads(sys.stdin.readline())
def hook(id,event,source=None):
    inp={'session_id':'mock-s','prompt_id':'turn-1','hook_event_name':event}
    if source: inp['source']=source
    emit({'type':'control_request','request_id':id,'request':{'subtype':'hook_callback','callback_id':'lvk_compact_context_v1','input':inp}})
    reply=read(); assert reply['response']['request_id']==id
    if not (mode=='unconfirmed' and event=='SessionStart'): emit(reply)
    return reply
while True:
    line=sys.stdin.readline()
    if not line: break
    q=json.loads(line)
    if q['type']=='control_request':
        subtype=q['request']['subtype']
        if subtype=='set_permission_mode' and mode=='reject':
            emit({'type':'control_response','response':{'subtype':'error','request_id':q['request_id'],'error':'SENSITIVE_MOCK_BODY'}})
        elif subtype=='set_permission_mode' and mode=='timeout': pass
        else: emit({'type':'control_response','response':{'subtype':'success','request_id':q['request_id'],'response':{}}})
    elif q['type']=='user':
        emit(dict(q,session_id='mock-s'))
        if mode in ['compact','duplicate','missing','unconfirmed']:
            if mode!='missing':
                hook('pre-'+turn,'PreCompact')
                reply=hook('compact-'+turn,'SessionStart','compact')
                assert 'FIXED' in reply['response']['response']['hookSpecificOutput']['additionalContext']
                if mode=='duplicate':
                    reply=hook('compact-'+turn,'SessionStart','compact')
                    assert reply['response']['response']=={}
            boundary={'type':'system','subtype':'compact_boundary','uuid':'boundary-'+turn,'session_id':'mock-s','compact_metadata':{'trigger':'auto'}}
            if mode!='unconfirmed': emit(boundary)
            if mode=='duplicate': emit(boundary)
        emit({'type':'result','subtype':'success','is_error':False,'session_id':'mock-s','result':'mock result, not model evidence'})
        break
"#;

    #[tokio::test]
    async fn instruction_mock_stdio_first_resume_compact_duplicates_and_missing_callback() {
        let dir = tempfile::tempdir().unwrap();
        for (index, mode, expected_reduced) in [
            (1, "normal", false),
            (2, "normal", true),
            (3, "compact", true),
            (4, "duplicate", true),
            (5, "unconfirmed", true),
            (6, "missing", false),
            (7, "normal", false),
        ] {
            let receipt = ResumeInstructions::new(
                dir.path(),
                "fixed-contract",
                format!("RUN={index} DRAFT=fixture/{index}.json"),
            );
            let state = super::super::instructions::Instructions::new(
                receipt,
                format!("FIXED RUN={index} DRAFT=fixture/{index}.json"),
                (index > 1).then(|| "mock-s".to_owned()),
            );
            let reduced = state.prepare().unwrap();
            assert_eq!(reduced, expected_reduced);
            let additional = if reduced {
                format!("\n\nRUN={index} DRAFT=fixture/{index}.json")
            } else {
                format!("FIXED RUN={index} DRAFT=fixture/{index}.json")
            };
            let prompt = format!("human{additional}");
            let mut child = Command::new("python3")
                .args(["-u", "-c", MOCK, mode, &index.to_string()])
                .stdin(std::process::Stdio::piped())
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::null())
                .kill_on_drop(true)
                .spawn()
                .unwrap();
            let (writer, mut reader) = tokio::io::duplex(65536);
            let cancel = CancellationToken::new();
            let client = ClaudeAgentClient::new(
                super::super::super::codex::client::LogWriter::new(writer),
                None,
                cancel.clone(),
            );
            *client.instructions.lock().await = Some(state);
            let (peer, signal) = ProtocolPeer::spawn(
                child.stdin.take().unwrap(),
                child.stdout.take().unwrap(),
                client.clone(),
                cancel.clone(),
            );
            peer.initialize(Some(serde_json::json!({"SessionStart":[{"matcher":"compact","hookCallbackIds":[super::super::instructions::CALLBACK]}]}))).await.unwrap();
            peer.set_permission_mode(PermissionMode::BypassPermissions)
                .await
                .unwrap();
            client
                .begin_instruction_delivery(prompt.clone(), additional)
                .await
                .unwrap();
            peer.send_user_message(prompt).await.unwrap();
            let status = tokio::time::timeout(std::time::Duration::from_secs(5), child.wait())
                .await
                .unwrap()
                .unwrap();
            assert!(
                status.success() || mode == "missing",
                "mock step {index} {mode}: {status}"
            );
            for _ in 0..1000 {
                if peer.closed.load(Ordering::Acquire) {
                    break;
                }
                tokio::task::yield_now().await;
            }
            assert!(peer.closed.load(Ordering::Acquire));
            let state = client.instructions.lock().await;
            assert_eq!(
                state.as_ref().unwrap().receipt.matches("mock-s"),
                !matches!(mode, "missing" | "unconfirmed")
            );
            drop(state);
            if matches!(mode, "missing" | "unconfirmed") {
                assert!(matches!(signal.await.unwrap(), ExecutorExitResult::Failure));
            }
            drop(peer);
            drop(client);
            cancel.cancel();
            let mut output = String::new();
            reader.read_to_string(&mut output).await.unwrap();
            let deliveries: Vec<serde_json::Value> = output
                .lines()
                .filter_map(|l| {
                    serde_json::from_str::<serde_json::Value>(l)
                        .ok()?
                        .get("lvk_instruction_delivery")
                        .cloned()
                })
                .collect();
            if matches!(mode, "missing" | "unconfirmed") {
                assert!(
                    !output
                        .lines()
                        .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
                        .any(|v| v["type"] == "result"),
                    "incomplete delivery must not publish native success"
                );
            }
            let compact: Vec<_> = deliveries
                .iter()
                .filter(|v| v["reason"] == "compaction")
                .collect();
            assert_eq!(
                compact.len(),
                if matches!(mode, "compact" | "duplicate" | "unconfirmed") {
                    2
                } else {
                    0
                }
            );
            if !compact.is_empty() {
                assert_eq!(compact[0]["status"], "attempted");
                assert_eq!(
                    compact[1]["status"],
                    if mode == "unconfirmed" {
                        "unconfirmed"
                    } else {
                        "accepted"
                    }
                );
                assert_eq!(
                    compact[0]["content"],
                    format!("FIXED RUN={index} DRAFT=fixture/{index}.json")
                );
            }
            assert!(!output.contains("SENSITIVE_MOCK_BODY"));
        }
    }

    #[tokio::test]
    async fn instruction_mock_startup_failure_exits_before_any_user_message() {
        use tokio::io::{AsyncBufReadExt, BufReader};
        let dir = tempfile::tempdir().unwrap();
        let agent: super::super::ClaudeCode = serde_json::from_value(
            serde_json::json!({"approvals":false,"base_command_override":"python3"}),
        )
        .unwrap();
        let parts = crate::command::CommandParts::new(
            "python3".into(),
            vec!["-u".into(), "-c".into(), MOCK.into(), "reject".into()],
        );
        let env = crate::env::ExecutionEnv::new(crate::env::RepoContext::new(
            dir.path().to_owned(),
            vec![],
        ));
        let mut spawned = agent
            .spawn_internal(dir.path(), "must not be sent", parts, &env)
            .await
            .unwrap();
        let mut lines = BufReader::new(spawned.child.inner().stdout.take().unwrap()).lines();
        let event: serde_json::Value = serde_json::from_str(
            &tokio::time::timeout(std::time::Duration::from_secs(10), lines.next_line())
                .await
                .unwrap()
                .unwrap()
                .unwrap(),
        )
        .unwrap();
        assert_eq!(event["type"], "error");
        assert_eq!(event["phase"], "set_permission_mode");
        assert_eq!(event["failure_kind"], "rejected");
        assert!(!event.to_string().contains("SENSITIVE_MOCK_BODY"));
        assert!(matches!(
            spawned.exit_signal.take().unwrap().await.unwrap(),
            ExecutorExitResult::Failure
        ));
        if let Some(cancel) = spawned.cancel.take() {
            cancel.cancel();
        }
        let _ = spawned.child.kill().await;
        let _ = spawned.child.wait().await;
    }

    #[tokio::test]
    async fn instruction_mock_control_rejection_and_timeout_are_distinct_safe_categories() {
        for (mode, expected) in [
            ("reject", ControlFailureKind::Rejected),
            ("timeout", ControlFailureKind::Timeout),
        ] {
            let mut child = Command::new("python3")
                .args(["-u", "-c", MOCK, mode])
                .stdin(std::process::Stdio::piped())
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::null())
                .kill_on_drop(true)
                .spawn()
                .unwrap();
            let cancel = CancellationToken::new();
            let (peer, task) = ProtocolPeer::spawn_passive(
                child.stdin.take().unwrap(),
                child.stdout.take().unwrap(),
                cancel.clone(),
            );
            peer.initialize_passive().await.unwrap();
            let error = peer
                .set_permission_mode(PermissionMode::BypassPermissions)
                .await
                .unwrap_err();
            assert_eq!(ControlFailureKind::from_error(&error), expected);
            assert!(!error.to_string().contains("SENSITIVE_MOCK_BODY"));
            cancel.cancel();
            let _ = child.kill().await;
            let _ = child.wait().await;
            task.await.unwrap();
        }
    }
}
