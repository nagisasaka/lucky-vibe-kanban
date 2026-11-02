use std::{
    collections::HashMap,
    io::{Read, Write},
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread,
};

use portable_pty::{CommandBuilder, NativePtySystem, PtySize, PtySystem};
use thiserror::Error;
use tokio::sync::{mpsc, oneshot, watch};
use utils::shell::get_interactive_shell;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum PtyError {
    #[error("Failed to create PTY: {0}")]
    CreateFailed(String),
    #[error("Session not found: {0}")]
    SessionNotFound(Uuid),
    #[error("Failed to write to PTY: {0}")]
    WriteFailed(String),
    #[error("Failed to resize PTY: {0}")]
    ResizeFailed(String),
    #[error("PTY cleanup failed: {0}")]
    CleanupFailed(String),
    #[error("Session already closed")]
    SessionClosed,
}

struct PtySession {
    writer: Box<dyn Write + Send>,
    master: Box<dyn portable_pty::MasterPty + Send>,
    stop: Option<oneshot::Sender<()>>,
    settled: watch::Receiver<Option<Result<(), String>>>,
    closed: bool,
}

#[derive(Clone)]
pub struct PtyService {
    sessions: Arc<Mutex<HashMap<Uuid, PtySession>>>,
}

impl PtyService {
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn create_session(
        &self,
        working_dir: PathBuf,
        cols: u16,
        rows: u16,
    ) -> Result<(Uuid, mpsc::UnboundedReceiver<Vec<u8>>), PtyError> {
        let session_id = Uuid::new_v4();
        let (output_tx, output_rx) = mpsc::unbounded_channel();
        let shell = get_interactive_shell().await;

        let (stop_tx, stop_rx) = oneshot::channel();
        let (registered_tx, registered_rx) = oneshot::channel();
        let (settled_tx, settled_rx) = watch::channel(None);
        let runtime = tokio::runtime::Handle::current();
        let sessions = Arc::downgrade(&self.sessions);
        let result = tokio::task::spawn_blocking(move || {
            let pty_system = NativePtySystem::default();

            let pty_pair = pty_system
                .openpty(PtySize {
                    rows,
                    cols,
                    pixel_width: 0,
                    pixel_height: 0,
                })
                .map_err(|e| PtyError::CreateFailed(e.to_string()))?;

            let mut cmd = CommandBuilder::new(&shell);
            cmd.cwd(&working_dir);

            // Configure shell-specific options
            let shell_name = shell.file_name().and_then(|n| n.to_str()).unwrap_or("");

            if shell_name == "powershell.exe" || shell_name == "pwsh.exe" {
                // PowerShell: use -NoLogo for cleaner startup
                cmd.arg("-NoLogo");
            } else if shell_name == "cmd.exe" {
                // cmd.exe: no special args needed
            } else {
                // Unix shells
                cmd.env("VIBE_KANBAN_TERMINAL", "1");

                if shell_name == "bash" {
                    cmd.env("PROMPT_COMMAND", r#"PS1='$ '; unset PROMPT_COMMAND"#);
                } else if shell_name == "zsh" {
                    // PROMPT is queued on the PTY before exposing it
                } else {
                    cmd.env("PS1", "$ ");
                }
            }

            cmd.env("TERM", "xterm-256color");
            cmd.env("COLORTERM", "truecolor");

            let mut writer = pty_pair
                .master
                .take_writer()
                .map_err(|e| PtyError::CreateFailed(e.to_string()))?;

            if shell_name == "zsh" {
                let _ = writer.write_all(b" PROMPT='$ '; RPROMPT=''\n");
                let _ = writer.flush();
                let _ = writer.write_all(b"\x0c");
                let _ = writer.flush();
            }

            let mut reader = pty_pair
                .master
                .try_clone_reader()
                .map_err(|e| PtyError::CreateFailed(e.to_string()))?;

            let reader_stop = Arc::new(AtomicBool::new(false));
            let thread_stop = reader_stop.clone();
            #[cfg(target_os = "linux")]
            let poll_fd = {
                use std::os::fd::{FromRawFd, OwnedFd};
                let fd = pty_pair
                    .master
                    .as_raw_fd()
                    .ok_or_else(|| PtyError::CreateFailed("PTY fd unavailable".into()))?;
                // Own the polling descriptor independently of layout/connection handles.
                let fd = unsafe { nix::libc::dup(fd) };
                if fd < 0 {
                    return Err(PtyError::CreateFailed(
                        std::io::Error::last_os_error().to_string(),
                    ));
                }
                unsafe { OwnedFd::from_raw_fd(fd) }
            };
            let mut child = pty_pair
                .slave
                .spawn_command(cmd)
                .map_err(|e| PtyError::CreateFailed(e.to_string()))?;

            #[cfg(target_os = "linux")]
            let mut owner = match utils::process::OwnedProcessSession::capture(
                child
                    .process_id()
                    .ok_or_else(|| PtyError::CreateFailed("Shell PID unavailable".into()))?,
            ) {
                Ok(owner) => owner,
                Err(error) => {
                    // This child has never been reaped, so its PID cannot recycle.
                    if let Some(pid) = child.process_id() {
                        let _ = nix::sys::signal::kill(
                            nix::unistd::Pid::from_raw(pid as i32),
                            nix::sys::signal::Signal::SIGKILL,
                        );
                    }
                    let _ = child.wait();
                    return Err(PtyError::CreateFailed(error.to_string()));
                }
            };
            let output_handle = thread::spawn(move || {
                let mut buf = [0u8; 4096];
                while !thread_stop.load(Ordering::Acquire) && !output_tx.is_closed() {
                    #[cfg(target_os = "linux")]
                    {
                        use std::os::fd::AsRawFd;
                        let mut poll = nix::libc::pollfd {
                            fd: poll_fd.as_raw_fd(),
                            events: nix::libc::POLLIN,
                            revents: 0,
                        };
                        // Bounded wait lets cleanup join an idle PTY reader too.
                        let ready = unsafe { nix::libc::poll(&mut poll, 1, 100) };
                        if ready == 0 {
                            continue;
                        }
                        if ready < 0 {
                            break;
                        }
                    }
                    match reader.read(&mut buf) {
                        Ok(0) => break,
                        Ok(n) => {
                            if output_tx.send(buf[..n].to_vec()).is_err() {
                                break;
                            }
                        }
                        Err(_) => break,
                    }
                }
            });

            runtime.spawn(async move {
                let _ = registered_rx.await;
                #[cfg(target_os = "linux")]
                let result = async {
                    let waited = tokio::select! {
                        _ = stop_rx => Ok(()),
                        exited = owner.wait() => exited.map(|_| ()),
                    };
                    let cleanup = owner.terminate().await;
                    waited.and(cleanup)
                }
                .await;
                #[cfg(not(target_os = "linux"))]
                let result = {
                    let _ = stop_rx.await;
                    child.kill()
                };
                reader_stop.store(true, Ordering::Release);
                let reaped = tokio::time::timeout(
                    std::time::Duration::from_secs(5),
                    tokio::task::spawn_blocking(move || {
                        let wait = child.wait();
                        let reader = output_handle.join();
                        wait?;
                        reader.map_err(|_| std::io::Error::other("PTY reader panicked"))?;
                        Ok::<_, std::io::Error>(())
                    }),
                )
                .await;
                let result = result
                    .and_then(|_| {
                        reaped
                            .map_err(std::io::Error::other)?
                            .map_err(std::io::Error::other)?
                    })
                    .map_err(|e| e.to_string());
                if let Err(error) = &result {
                    tracing::error!(%session_id, %error, "PTY cleanup unconfirmed");
                }
                let _ = settled_tx.send(Some(result.clone()));
                if result.is_ok()
                    && let Some(sessions) = sessions.upgrade()
                    && let Ok(mut sessions) = sessions.lock()
                {
                    sessions.remove(&session_id);
                }
            });
            Ok::<_, PtyError>((pty_pair.master, writer))
        })
        .await
        .map_err(|e| PtyError::CreateFailed(e.to_string()))??;

        let (master, writer) = result;
        let session = PtySession {
            writer,
            master,
            stop: Some(stop_tx),
            settled: settled_rx,
            closed: false,
        };

        self.sessions
            .lock()
            .map_err(|e| PtyError::CreateFailed(e.to_string()))?
            .insert(session_id, session);
        let _ = registered_tx.send(());

        Ok((session_id, output_rx))
    }

    pub async fn write(&self, session_id: Uuid, data: &[u8]) -> Result<(), PtyError> {
        let mut sessions = self
            .sessions
            .lock()
            .map_err(|e| PtyError::WriteFailed(e.to_string()))?;
        let session = sessions
            .get_mut(&session_id)
            .ok_or(PtyError::SessionNotFound(session_id))?;

        if session.closed {
            return Err(PtyError::SessionClosed);
        }

        session
            .writer
            .write_all(data)
            .map_err(|e| PtyError::WriteFailed(e.to_string()))?;

        session
            .writer
            .flush()
            .map_err(|e| PtyError::WriteFailed(e.to_string()))?;

        Ok(())
    }

    pub async fn resize(&self, session_id: Uuid, cols: u16, rows: u16) -> Result<(), PtyError> {
        let sessions = self
            .sessions
            .lock()
            .map_err(|e| PtyError::ResizeFailed(e.to_string()))?;
        let session = sessions
            .get(&session_id)
            .ok_or(PtyError::SessionNotFound(session_id))?;

        if session.closed {
            return Err(PtyError::SessionClosed);
        }

        session
            .master
            .resize(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| PtyError::ResizeFailed(e.to_string()))?;

        Ok(())
    }

    /// Safe from Drop/cancellation: the owner task performs kill, wait and reader join.
    pub fn request_close(&self, session_id: Uuid) {
        if let Ok(mut sessions) = self.sessions.lock()
            && let Some(session) = sessions.get_mut(&session_id)
        {
            session.closed = true;
            if let Some(stop) = session.stop.take() {
                let _ = stop.send(());
            }
        }
    }

    pub async fn close_session(&self, session_id: Uuid) -> Result<(), PtyError> {
        let settled = self
            .sessions
            .lock()
            .map_err(|_| PtyError::SessionClosed)?
            .get(&session_id)
            .map(|s| s.settled.clone());
        self.request_close(session_id);
        if let Some(mut settled) = settled {
            loop {
                if let Some(result) = settled.borrow().clone() {
                    return result.map_err(PtyError::CleanupFailed);
                }
                settled
                    .changed()
                    .await
                    .map_err(|e| PtyError::CleanupFailed(e.to_string()))?;
            }
        }
        Ok(())
    }
}

impl Default for PtyService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use std::time::Duration;

    use super::*;

    async fn pids(path: &std::path::Path, count: usize) -> Vec<u32> {
        tokio::time::timeout(Duration::from_secs(10), async {
            loop {
                let ids: Vec<u32> = std::fs::read_to_string(path)
                    .unwrap_or_default()
                    .lines()
                    .filter_map(|s| s.parse().ok())
                    .collect();
                if ids.len() == count {
                    return ids;
                }
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await
        .unwrap()
    }

    fn running(pid: u32) -> bool {
        std::fs::read_to_string(format!("/proc/{pid}/stat"))
            .ok()
            .and_then(|s| {
                s.rsplit_once(')')
                    .map(|(_, rest)| !rest.trim_start().starts_with(['Z', 'X']))
            })
            .unwrap_or(false)
    }

    #[tokio::test]
    async fn close_reaps_shell_and_stops_foreground_children_without_touching_other_execution() {
        let dir = tempfile::tempdir().unwrap();
        let service = PtyService::new();
        let (id, _output) = service
            .create_session(dir.path().into(), 80, 24)
            .await
            .unwrap();
        let (other, mut other_output) = service
            .create_session(dir.path().into(), 80, 24)
            .await
            .unwrap();
        let mut unrelated = tokio::process::Command::new("sleep")
            .arg("300")
            .kill_on_drop(true)
            .spawn()
            .unwrap();
        let record = dir.path().join("pids");
        let command = format!(
            "echo $$ > '{}'; python3 -c \"import os,subprocess,time; p=subprocess.Popen(['sleep','300']); f=open('{}','a'); f.write(str(os.getpid())+'\\\\n'+str(p.pid)+'\\\\n'); f.close(); time.sleep(300)\"\n",
            record.display(),
            record.display()
        );
        service.write(id, command.as_bytes()).await.unwrap();
        let ids = pids(&record, 3).await;
        assert!(ids.iter().all(|pid| running(*pid)));
        service.close_session(id).await.unwrap();
        assert!(ids.iter().all(|pid| !running(*pid)));
        assert!(
            !std::path::Path::new(&format!("/proc/{}", ids[0])).exists(),
            "shell must be wait-reaped"
        );
        assert!(!service.sessions.lock().unwrap().contains_key(&id));
        assert!(unrelated.try_wait().unwrap().is_none());
        service
            .write(other, b"printf 'OTHER_ALIVE\\n'\n")
            .await
            .unwrap();
        tokio::time::timeout(Duration::from_secs(5), async {
            let mut output = String::new();
            while !output.contains("OTHER_ALIVE") {
                output.push_str(&String::from_utf8_lossy(
                    &other_output.recv().await.unwrap(),
                ));
            }
        })
        .await
        .unwrap();
        service.close_session(other).await.unwrap();
        unrelated.kill().await.unwrap();
        unrelated.wait().await.unwrap();
    }

    #[tokio::test]
    async fn natural_exit_cleans_background_job_and_idle_request_close_joins_reader() {
        let dir = tempfile::tempdir().unwrap();
        let service = PtyService::new();
        let (id, _output) = service
            .create_session(dir.path().into(), 80, 24)
            .await
            .unwrap();
        let record = dir.path().join("exit-pids");
        service.write(id, format!("echo $$ > '{}'; sh -c 'trap \"\" HUP; exec sleep 300' & echo $! >> '{}'; exit\n", record.display(), record.display()).as_bytes()).await.unwrap();
        let ids = pids(&record, 2).await;
        tokio::time::timeout(Duration::from_secs(10), async {
            while service.sessions.lock().unwrap().contains_key(&id) {
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await
        .unwrap();
        assert!(ids.iter().all(|pid| !running(*pid)));
        let (idle, _output) = service
            .create_session(dir.path().into(), 80, 24)
            .await
            .unwrap();
        service.request_close(idle);
        tokio::time::timeout(Duration::from_secs(10), service.close_session(idle))
            .await
            .unwrap()
            .unwrap();
        assert!(!service.sessions.lock().unwrap().contains_key(&idle));
    }
}
