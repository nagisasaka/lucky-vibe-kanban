use std::path::PathBuf;

use axum::{
    Router,
    extract::{Query, State, ws::Message},
    response::{IntoResponse, Response},
    routing::get,
};
use base64::{Engine, engine::general_purpose::STANDARD as BASE64};
use db::models::{workspace::Workspace, workspace_repo::WorkspaceRepo};
use deployment::Deployment;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    DeploymentImpl,
    error::ApiError,
    middleware::websocket::{LocalWebSocket, LocalWsUpgrade},
};

#[derive(Debug, Deserialize)]
struct TerminalQuery {
    pub workspace_id: Option<Uuid>,
    pub project_id: Option<Uuid>,
    pub repo_id: Option<Uuid>,
    #[serde(default = "default_cols")]
    pub cols: u16,
    #[serde(default = "default_rows")]
    pub rows: u16,
}

fn default_cols() -> u16 {
    80
}

fn default_rows() -> u16 {
    24
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum TerminalCommand {
    Input { data: String },
    Resize { cols: u16, rows: u16 },
    Close,
}

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum TerminalMessage {
    Output { data: String },
    Heartbeat,
    Exit,
    Error { message: String },
}

async fn terminal_ws(
    ws: LocalWsUpgrade,
    State(deployment): State<DeploymentImpl>,
    Query(query): Query<TerminalQuery>,
) -> Result<Response, ApiError> {
    if let (None, Some(project_id), Some(repo_id)) =
        (query.workspace_id, query.project_id, query.repo_id)
    {
        let repo = super::project_utilities::project_repo(&deployment, project_id, repo_id).await?;
        let working_dir = tokio::fs::canonicalize(repo.path).await?;
        return Ok(ws
            .on_upgrade(move |socket| {
                handle_terminal_ws(socket, deployment, working_dir, query.cols, query.rows)
            })
            .into_response());
    }
    if query.project_id.is_some() || query.repo_id.is_some() {
        return Err(ApiError::BadRequest(
            "Choose exactly one terminal context".into(),
        ));
    }
    let workspace_id = query
        .workspace_id
        .ok_or_else(|| ApiError::BadRequest("Terminal context required".into()))?;
    let attempt = Workspace::find_by_id(&deployment.db().pool, workspace_id)
        .await?
        .ok_or_else(|| ApiError::BadRequest("Attempt not found".to_string()))?;
    attempt.require_interactive()?;
    if attempt.archived || attempt.worktree_deleted {
        return Err(ApiError::Conflict(
            "Archived or deleted Workspace cannot start a terminal".into(),
        ));
    }

    let container_ref = attempt
        .container_ref
        .ok_or_else(|| ApiError::BadRequest("Attempt has no workspace directory".to_string()))?;

    let base_dir = PathBuf::from(&container_ref);
    if !base_dir.exists() {
        return Err(ApiError::BadRequest(
            "Workspace directory does not exist".to_string(),
        ));
    }

    let mut working_dir = base_dir.clone();
    match WorkspaceRepo::find_repos_for_workspace(&deployment.db().pool, workspace_id).await {
        Ok(repos) if repos.len() == 1 => {
            let repo_dir = repos[0].worktree_dir(&base_dir);
            if repo_dir.exists() {
                working_dir = repo_dir;
            }
        }
        Ok(_) => {}
        Err(e) => {
            tracing::warn!(
                "Failed to resolve repos for workspace {}: {}",
                attempt.id,
                e
            );
        }
    }

    Ok(ws
        .on_upgrade(move |socket| {
            handle_terminal_ws(socket, deployment, working_dir, query.cols, query.rows)
        })
        .into_response())
}

struct CloseOnDrop(local_deployment::pty::PtyService, Uuid);
impl Drop for CloseOnDrop {
    fn drop(&mut self) {
        self.0.request_close(self.1);
    }
}

const HEARTBEAT_INTERVAL: std::time::Duration = std::time::Duration::from_secs(10);
const HEARTBEAT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(45);

async fn send_terminal(socket: &mut LocalWebSocket, message: Message) -> bool {
    matches!(
        tokio::time::timeout(std::time::Duration::from_secs(5), socket.send(message)).await,
        Ok(Ok(()))
    )
}

async fn handle_terminal_ws(
    mut socket: LocalWebSocket,
    deployment: DeploymentImpl,
    working_dir: PathBuf,
    cols: u16,
    rows: u16,
) {
    let (session_id, mut output_rx) = match deployment
        .pty()
        .create_session(working_dir, cols, rows)
        .await
    {
        Ok(result) => result,
        Err(e) => {
            tracing::error!("Failed to create PTY session: {}", e);
            let _ = send_error(&mut socket, &e.to_string()).await;
            return;
        }
    };

    let pty_service = deployment.pty().clone();
    // Access lease cancellation also drops this guard; cleanup is not tied to
    // the successful completion of the WebSocket handler's future.
    let _owner = CloseOnDrop(pty_service.clone(), session_id);
    let mut heartbeat = tokio::time::interval(HEARTBEAT_INTERVAL);
    heartbeat.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let mut last_response = tokio::time::Instant::now();
    let session_id_for_input = session_id;

    loop {
        tokio::select! {
            maybe_output = output_rx.recv() => {
                let Some(data) = maybe_output else {
                    break;
                };

                let msg = TerminalMessage::Output {
                    data: BASE64.encode(&data),
                };
                let json = match serde_json::to_string(&msg) {
                    Ok(j) => j,
                    Err(_) => continue,
                };

                if !send_terminal(&mut socket, Message::Text(json.into())).await {
                    break;
                }
            }
            _ = heartbeat.tick() => {
                if last_response.elapsed() >= HEARTBEAT_TIMEOUT { break; }
                let json = serde_json::to_string(&TerminalMessage::Heartbeat).unwrap();
                if !send_terminal(&mut socket, Message::Ping(Vec::new().into())).await
                    || !send_terminal(&mut socket, Message::Text(json.into())).await { break; }
            }
            inbound = socket.recv() => {
                match inbound {
                    Ok(Some(Message::Text(text))) => {
                        last_response = tokio::time::Instant::now();
                        if let Ok(cmd) = serde_json::from_str::<TerminalCommand>(text.as_str()) {
                            match cmd {
                                TerminalCommand::Input { data } => {
                                    if let Ok(bytes) = BASE64.decode(&data) {
                                        let _ = pty_service.write(session_id_for_input, &bytes).await;
                                    }
                                }
                                TerminalCommand::Close => break,
                                TerminalCommand::Resize { cols, rows } => {
                                    let _ = pty_service.resize(session_id_for_input, cols, rows).await;
                                }
                            }
                        }
                    }
                    Ok(Some(Message::Close(_))) => break,
                    Ok(Some(Message::Pong(_))) | Ok(Some(Message::Ping(_))) => { last_response = tokio::time::Instant::now(); }
                    Ok(Some(_)) => {}
                    Ok(None) => break,
                    Err(error) => {
                        tracing::warn!("terminal WS receive error: {}", error);
                        break;
                    }
                }
            }
        }
    }

    match deployment.pty().close_session(session_id).await {
        Ok(()) => {
            let json = serde_json::to_string(&TerminalMessage::Exit).unwrap();
            let _ = send_terminal(&mut socket, Message::Text(json.into())).await;
        }
        Err(error) => {
            tracing::error!(%session_id, %error, "Terminal stopped without confirmed cleanup")
        }
    }
    let _ = tokio::time::timeout(std::time::Duration::from_secs(5), socket.close()).await;
}

async fn send_error(socket: &mut LocalWebSocket, message: &str) -> anyhow::Result<()> {
    let msg = TerminalMessage::Error {
        message: message.to_string(),
    };
    let json = serde_json::to_string(&msg).unwrap_or_default();
    socket.send(Message::Text(json.into())).await?;
    socket.close().await?;
    Ok(())
}

pub(super) fn router() -> Router<DeploymentImpl> {
    Router::new().route("/terminal/ws", get(terminal_ws))
}
