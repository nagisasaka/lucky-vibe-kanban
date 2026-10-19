//! Linux runtime processes. No AgentRun is created; receipts are owned by the MCP layer.
//! Stop and timeout target exactly one PID through a pidfd, never a process group.
use std::{
    collections::{HashMap, VecDeque},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Duration,
};

use anyhow::{Result, anyhow, bail};
use serde_json::{Value, json};
use tokio::{
    io::{AsyncRead, AsyncReadExt},
    process::Command,
};
use uuid::Uuid;

const OUTPUT_CAP: usize = 1024 * 1024;
const MAX_RECORDS: usize = 128;
#[derive(Default)]
pub struct ProcessRegistry {
    records: Mutex<HashMap<String, Arc<Mutex<Record>>>>,
}
struct Record {
    owner: String,
    root: PathBuf,
    identity: String,
    stdout: Buffer,
    stderr: Buffer,
    state: String,
    exit_code: Option<i32>,
    signal: Option<i32>,
}
#[derive(Default)]
struct Buffer {
    bytes: VecDeque<u8>,
    total: u64,
    capture_state: Option<&'static str>,
}
impl Buffer {
    fn append(&mut self, data: &[u8]) {
        self.total += data.len() as u64;
        self.bytes.extend(data);
        while self.bytes.len() > OUTPUT_CAP {
            self.bytes.pop_front();
        }
    }
    fn page(&self, offset: u64, limit: usize) -> Value {
        let start = self.total - self.bytes.len() as u64;
        let effective = offset.max(start).min(self.total);
        let bytes: Vec<u8> = self
            .bytes
            .iter()
            .skip((effective - start) as usize)
            .take(limit.clamp(1, 65536))
            .copied()
            .collect();
        json!({"capture_state":self.capture_state.unwrap_or("capturing"),"text":String::from_utf8_lossy(&bytes),"encoding":"utf-8-lossy","invalid_utf8":std::str::from_utf8(&bytes).is_err(),"offset":effective,"next_offset":effective+bytes.len() as u64,"total_bytes":self.total,"dropped_before":start,"cursor_lost":offset<start,"cursor_ahead":offset>self.total})
    }
}

#[derive(Debug)]
struct Observation {
    pid: u32,
    ppid: u32,
    ticks: u64,
    state: String,
    cwd: PathBuf,
    executable: PathBuf,
    identity: String,
}
fn stat(pid: u32) -> Result<(u32, u64, String)> {
    let text = std::fs::read_to_string(format!("/proc/{pid}/stat"))
        .map_err(|_| anyhow!("process_not_found_or_unreadable"))?;
    let tail = text
        .rsplit_once(')')
        .ok_or_else(|| anyhow!("process_stat_invalid"))?
        .1;
    let fields: Vec<_> = tail.split_whitespace().collect();
    Ok((
        fields
            .get(1)
            .ok_or_else(|| anyhow!("process_stat_invalid"))?
            .parse()?,
        fields
            .get(19)
            .ok_or_else(|| anyhow!("process_stat_invalid"))?
            .parse()?,
        fields
            .first()
            .ok_or_else(|| anyhow!("process_stat_invalid"))?
            .to_string(),
    ))
}
fn observe(pid: u32) -> Result<Observation> {
    use std::os::unix::fs::MetadataExt;
    let base = PathBuf::from(format!("/proc/{pid}"));
    if std::fs::metadata(&base)?.uid() != unsafe { libc::geteuid() } {
        bail!("process_owner_denied");
    }
    let (ppid, ticks, state) = stat(pid)?;
    let cwd = std::fs::read_link(base.join("cwd"))?;
    let executable = std::fs::read_link(base.join("exe"))?;
    let boot = std::fs::read_to_string("/proc/sys/kernel/random/boot_id")?;
    if stat(pid)?.1 != ticks {
        bail!("process_identity_changed");
    }
    Ok(Observation {
        pid,
        ppid,
        ticks,
        state,
        cwd,
        executable,
        identity: format!("{}:{pid}:{ticks}", boot.trim()),
    })
}
fn protected(pid: u32) -> bool {
    let mut next = std::process::id();
    for _ in 0..256 {
        if pid == next {
            return true;
        }
        match stat(next) {
            Ok((parent, _, _)) if parent > 0 && parent != next => next = parent,
            _ => break,
        }
    }
    pid <= 1
}
fn scoped(root: &Path, allowed: &[PathBuf], pid: u32) -> Result<Observation> {
    let observation = observe(pid)?;
    if protected(pid)
        || !observation.cwd.starts_with(root)
        || !allowed.contains(&observation.executable)
    {
        bail!("process_scope_denied");
    }
    Ok(observation)
}
fn pid_from(identity: &str) -> Result<u32> {
    identity
        .split(':')
        .nth(1)
        .ok_or_else(|| anyhow!("invalid_process_identity"))?
        .parse()
        .map_err(|_| anyhow!("invalid_process_identity"))
}
fn metadata(o: &Observation) -> Value {
    json!({"observed_at":chrono::Utc::now(),"identity":o.identity,"pid":o.pid,"parent_pid":o.ppid,"start_ticks":o.ticks,"state":o.state,"cwd":o.cwd,"executable":o.executable,"argv":"withheld","environment":"withheld","project_id":null,"workspace_id":null,"build_commit":"unknown","stdout_availability":"managed runs only; otherwise read configured log files"})
}

pub fn list(root: &Path, allowed: &[PathBuf], after_pid: u32, limit: usize) -> Result<Value> {
    let mut ids: Vec<u32> = std::fs::read_dir("/proc")?
        .filter_map(|e| e.ok()?.file_name().to_str()?.parse().ok())
        .filter(|p| *p > after_pid)
        .collect();
    ids.sort_unstable();
    let limit = limit.clamp(1, 200);
    let mut items = Vec::new();
    let mut next = None;
    for pid in ids {
        if let Ok(o) = scoped(root, allowed, pid) {
            if items.len() == limit {
                next = items.last().and_then(|v: &Value| v["pid"].as_u64());
                break;
            }
            items.push(metadata(&o));
        }
    }
    Ok(json!({"processes":items,"next_after_pid":next,"snapshot":false}))
}
pub fn inspect(root: &Path, allowed: &[PathBuf], identity: &str) -> Result<Value> {
    let o = scoped(root, allowed, pid_from(identity)?)?;
    if o.identity != identity {
        bail!("process_identity_changed");
    }
    Ok(metadata(&o))
}

fn process_io_error(error: std::io::Error) -> anyhow::Error {
    match error.raw_os_error() {
        Some(libc::ESRCH) => anyhow!("process_already_exited"),
        Some(libc::ENOENT) => anyhow!("process_or_executable_not_found"),
        Some(libc::EACCES | libc::EPERM) => anyhow!("process_permission_denied"),
        Some(libc::ENOSYS) => anyhow!("pidfd_not_supported_by_kernel"),
        _ => anyhow!("process_operation_unavailable"),
    }
}

/// Open the kernel handle before re-observing identity; no kill-by-reused-PID window.
fn signal_identity(identity: &str, signal: i32) -> Result<()> {
    use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
    let pid = pid_from(identity)?;
    let raw = unsafe { libc::syscall(libc::SYS_pidfd_open, pid, 0) as i32 };
    if raw < 0 {
        return Err(process_io_error(std::io::Error::last_os_error()));
    }
    let fd = unsafe { OwnedFd::from_raw_fd(raw) };
    if observe(pid)?.identity != identity {
        bail!("process_identity_changed");
    }
    if unsafe {
        libc::syscall(
            libc::SYS_pidfd_send_signal,
            fd.as_raw_fd(),
            signal,
            std::ptr::null::<libc::siginfo_t>(),
            0,
        )
    } < 0
    {
        return Err(process_io_error(std::io::Error::last_os_error()));
    }
    Ok(())
}
async fn capture<R: AsyncRead + Unpin>(mut reader: R, record: Arc<Mutex<Record>>, stderr: bool) {
    let mut bytes = [0; 8192];
    loop {
        match reader.read(&mut bytes).await {
            result @ (Ok(0) | Err(_)) => {
                let mut r = record.lock().unwrap();
                let buffer = if stderr { &mut r.stderr } else { &mut r.stdout };
                buffer.capture_state = Some(if result.is_err() { "read_error" } else { "eof" });
                break;
            }
            Ok(count) => {
                let mut r = record.lock().unwrap();
                if stderr {
                    r.stderr.append(&bytes[..count])
                } else {
                    r.stdout.append(&bytes[..count])
                }
            }
        }
    }
}
impl ProcessRegistry {
    #[allow(clippy::too_many_arguments)]
    #[cfg(test)]
    pub async fn run(
        &self,
        owner: &str,
        root: &Path,
        executable: &Path,
        args: &[String],
        cwd: &Path,
        lifetime_seconds: u64,
        service: bool,
    ) -> Result<Value> {
        self.run_audited(
            owner,
            root,
            executable,
            args,
            cwd,
            lifetime_seconds,
            service,
            None,
        )
        .await
    }
    #[allow(clippy::too_many_arguments)]
    pub async fn run_audited(
        &self,
        owner: &str,
        root: &Path,
        executable: &Path,
        args: &[String],
        cwd: &Path,
        lifetime_seconds: u64,
        service: bool,
        audit: Option<(sqlx::SqlitePool, String, Uuid)>,
    ) -> Result<Value> {
        if !cwd.starts_with(root)
            || lifetime_seconds == 0
            || lifetime_seconds > 604800
            || (!service && lifetime_seconds > 3600)
            || args.len() > 256
            || args.iter().map(String::len).sum::<usize>() > 65536
        {
            bail!("invalid_process_parameters");
        }
        // Reserve capacity across spawn: this brief synchronous section has no await.
        let mut records = self.records.lock().unwrap();
        if records.len() >= MAX_RECORDS {
            if let Some(id) = records
                .iter()
                .find(|(_, r)| {
                    matches!(
                        r.lock().unwrap().state.as_str(),
                        "exited" | "timed_out" | "lifetime_expired"
                    )
                })
                .map(|(id, _)| id.clone())
            {
                records.remove(&id);
            } else {
                bail!("runtime_process_capacity");
            }
        }
        use std::os::fd::AsRawFd;
        let relative = cwd
            .strip_prefix(root)
            .map_err(|_| anyhow!("process_cwd_outside_region"))?;
        let directory = super::environment_files::open_directory(
            root,
            relative
                .to_str()
                .ok_or_else(|| anyhow!("process_cwd_not_utf8"))?,
        )?;
        let mut command = Command::new(executable);
        command
            .args(args)
            .current_dir(format!("/proc/self/fd/{}", directory.as_raw_fd()))
            .env_clear()
            .env("PATH", "/usr/local/bin:/usr/bin:/bin")
            .env("LANG", "C.UTF-8")
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());
        let mut child = command.spawn().map_err(process_io_error)?;
        let pid = child
            .id()
            .ok_or_else(|| anyhow!("process_pid_unavailable"))?;
        let identity = match stat(pid).and_then(|(_, ticks, _)| {
            Ok(format!(
                "{}:{pid}:{ticks}",
                std::fs::read_to_string("/proc/sys/kernel/random/boot_id")?.trim()
            ))
        }) {
            Ok(identity) => identity,
            Err(e) => {
                let _ = child.start_kill();
                tokio::spawn(async move {
                    let _ = child.wait().await;
                });
                return Err(e);
            }
        };
        let id = Uuid::new_v4().to_string();
        let record = Arc::new(Mutex::new(Record {
            owner: owner.into(),
            root: root.into(),
            identity: identity.clone(),
            stdout: Buffer::default(),
            stderr: Buffer::default(),
            state: "running".into(),
            exit_code: None,
            signal: None,
        }));
        records.insert(id.clone(), record.clone());
        drop(records);
        let stdout = child.stdout.take().unwrap();
        let stderr = child.stderr.take().unwrap();
        let out = tokio::spawn(capture(stdout, record.clone(), false));
        let err = tokio::spawn(capture(stderr, record.clone(), true));
        let monitor_identity = identity.clone();
        tokio::spawn(async move {
            use std::os::unix::process::ExitStatusExt;
            let result =
                tokio::time::timeout(Duration::from_secs(lifetime_seconds), child.wait()).await;
            let (status, timed_out) = match result {
                Ok(result) => (result, false),
                Err(_) => {
                    let _ = signal_identity(&monitor_identity, libc::SIGTERM);
                    let result = tokio::time::timeout(Duration::from_secs(5), child.wait()).await;
                    match result {
                        Ok(result) => (result, true),
                        Err(_) => {
                            let _ = signal_identity(&monitor_identity, libc::SIGKILL);
                            (
                                tokio::time::timeout(Duration::from_secs(5), child.wait())
                                    .await
                                    .unwrap_or_else(|_| {
                                        Err(std::io::Error::other("termination_unconfirmed"))
                                    }),
                                true,
                            )
                        }
                    }
                }
            };
            // A detached descendant may keep pipes open. Capturing has a bounded drain period.
            let mut out = out;
            let mut err = err;
            let _ = tokio::time::timeout(Duration::from_millis(500), async {
                let _ = (&mut out).await;
                let _ = (&mut err).await;
            })
            .await;
            out.abort();
            err.abort();
            let outcome = {
                let mut r = record.lock().unwrap();
                if r.stdout.capture_state.is_none() {
                    r.stdout.capture_state = Some("drain_deadline");
                }
                if r.stderr.capture_state.is_none() {
                    r.stderr.capture_state = Some("drain_deadline");
                }
                match status {
                    Ok(status) => {
                        r.exit_code = status.code();
                        r.signal = status.signal();
                        r.state = if timed_out {
                            if service {
                                "lifetime_expired"
                            } else {
                                "timed_out"
                            }
                        } else {
                            "exited"
                        }
                        .into();
                    }
                    Err(_) => r.state = "unknown".into(),
                }
                json!({"state":r.state,"identity":r.identity,"exit_code":r.exit_code,"signal":r.signal})
            };
            if let Some((pool, actor, operation)) = audit
                && sqlx::query("INSERT INTO access_audit(at,actor,channel,action,target,outcome) VALUES(?,?,'mcp','runtime_process_exit',?,?)")
                    .bind(chrono::Utc::now().timestamp()).bind(actor).bind(operation.to_string()).bind(outcome.to_string())
                    .execute(&pool).await.is_err() {
                    tracing::error!(operation_id=%operation, "Runtime exit audit unavailable; termination evidence is volatile");
            }
        });
        Ok(
            json!({"observed_at":chrono::Utc::now(),"evidence":"startup observation only; recheck output/process for current state","cwd":cwd,"executable":executable,"run_id":id,"identity":identity,"pid":pid,"state":"running","accepted":true,"completed":false,"mode":if service {"service"}else{"finite"},"lifetime_seconds":lifetime_seconds,"stop_scope":"exact_pid_only","descendants":"not managed; script exit is not service health","build_commit":"unknown"}),
        )
    }
    pub fn output(
        &self,
        owner: &str,
        root: &Path,
        id: &str,
        stream: &str,
        offset: u64,
        limit: usize,
    ) -> Result<Value> {
        let records = self.records.lock().unwrap();
        let r = records
            .get(id)
            .ok_or_else(|| anyhow!("process_output_unavailable_after_restart_or_eviction"))?
            .lock()
            .unwrap();
        if r.owner != owner || r.root != root {
            bail!("process_owner_denied");
        }
        let buffer = match stream {
            "stdout" => &r.stdout,
            "stderr" => &r.stderr,
            _ => bail!("invalid_output_stream"),
        };
        Ok(
            json!({"observed_at":chrono::Utc::now(),"run_id":id,"identity":r.identity,"state":r.state,"exit_code":r.exit_code,"signal":r.signal,"stream":stream,"output":buffer.page(offset,limit)}),
        )
    }
    pub fn stop(
        &self,
        owner: &str,
        root: &Path,
        allowed: &[PathBuf],
        identity: &str,
        force: bool,
    ) -> Result<Value> {
        let mut selected_identity = identity.to_owned();
        for (id, record) in self.records.lock().unwrap().iter() {
            let r = record.lock().unwrap();
            if r.identity == identity || id == identity {
                selected_identity = r.identity.clone();
                if r.owner != owner || r.root != root {
                    bail!("process_owner_denied");
                }
                if r.state == "unknown" && id == identity {
                    bail!(
                        "process_termination_unknown: reobserve and supply the exact observed identity"
                    );
                }
                if !matches!(r.state.as_str(), "running" | "unknown") {
                    return Ok(
                        json!({"identity":identity,"stop_requested":false,"stopped":true,"state":r.state}),
                    );
                }
            }
        }
        let identity = selected_identity.as_str();
        let o = scoped(root, allowed, pid_from(identity)?)?;
        if o.identity != identity {
            bail!("process_identity_changed");
        }
        signal_identity(identity, if force { libc::SIGKILL } else { libc::SIGTERM })?;
        Ok(
            json!({"identity":identity,"stop_requested":true,"stopped":false,"signal":if force {"SIGKILL"}else{"SIGTERM"},"scope":"exact_pid_only","confirmation":"inspect process identity or managed output; acknowledgement is not termination"}),
        )
    }
}

/// Test-only panic cleanup: retain the kernel process handle, never signal a reused PID.
#[cfg(test)]
pub(super) struct TestProcessCleanup(std::os::fd::OwnedFd);
#[cfg(test)]
impl TestProcessCleanup {
    pub(super) fn new(identity: &str) -> Result<Self> {
        use std::os::fd::FromRawFd;
        let pid = pid_from(identity)?;
        let raw = unsafe { libc::syscall(libc::SYS_pidfd_open, pid, 0) as i32 };
        if raw < 0 {
            bail!(
                "test cleanup pidfd unavailable: {}",
                std::io::Error::last_os_error()
            );
        }
        let fd = unsafe { std::os::fd::OwnedFd::from_raw_fd(raw) };
        let (_, ticks, _) = stat(pid)?;
        let observed = format!(
            "{}:{pid}:{ticks}",
            std::fs::read_to_string("/proc/sys/kernel/random/boot_id")?.trim()
        );
        if identity != observed {
            bail!("test cleanup process identity changed");
        }
        Ok(Self(fd))
    }
}
#[cfg(test)]
impl Drop for TestProcessCleanup {
    fn drop(&mut self) {
        use std::os::fd::AsRawFd;
        let raw = self.0.as_raw_fd();
        let mut pollfd = libc::pollfd {
            fd: raw,
            events: libc::POLLIN,
            revents: 0,
        };
        // Exit-ready handles need no signal. All operations are bound to the original pidfd.
        unsafe {
            if libc::poll(&mut pollfd, 1, 0) == 0 {
                libc::syscall(
                    libc::SYS_pidfd_send_signal,
                    raw,
                    libc::SIGKILL,
                    std::ptr::null::<libc::siginfo_t>(),
                    0,
                );
                libc::poll(&mut pollfd, 1, 1000);
            }
            // Tokio may already have reaped the child; ECHILD then simply means cleanup is done.
            // WNOHANG keeps panic cleanup bounded even when termination is not confirmed.
            let mut info: libc::siginfo_t = std::mem::zeroed();
            libc::waitid(
                libc::P_PIDFD,
                raw as libc::id_t,
                &mut info,
                libc::WEXITED | libc::WNOHANG,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn output_is_bounded_and_reports_lost_cursor() {
        let mut b = Buffer::default();
        b.append(&vec![b'x'; OUTPUT_CAP + 100]);
        let p = b.page(0, 10);
        assert_eq!(p["dropped_before"], 100);
        assert_eq!(p["cursor_lost"], true);
        assert_eq!(p["next_offset"], 110);
    }
    #[test]
    fn self_is_protected_and_identity_mismatch_is_rejected() {
        let o = observe(std::process::id()).unwrap();
        assert!(protected(o.pid));
        assert!(
            inspect(
                Path::new("/"),
                std::slice::from_ref(&o.executable),
                &o.identity
            )
            .is_err()
        );
        assert!(signal_identity(&format!("wrong:{}:{}", o.pid, o.ticks), 0).is_err());
    }
    #[tokio::test]
    async fn managed_run_output_owner_and_stop() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().canonicalize().unwrap();
        let shell = Path::new("/bin/sh").canonicalize().unwrap();
        let script = root.join("server.sh");
        std::fs::write(
            &script,
            "printf 'ready'; printf 'error' >&2; exec sleep 30\n",
        )
        .unwrap();
        let registry = ProcessRegistry::default();
        let run = registry
            .run(
                "alice",
                &root,
                &shell,
                &[script.to_string_lossy().into()],
                &root,
                60,
                false,
            )
            .await
            .unwrap();
        let id = run["run_id"].as_str().unwrap();
        let identity = run["identity"].as_str().unwrap();
        let _cleanup = TestProcessCleanup::new(identity).unwrap();
        assert!(registry.output("bob", &root, id, "stdout", 0, 100).is_err());
        let allowed = vec![shell, Path::new("/bin/sleep").canonicalize().unwrap()];
        for _ in 0..100 {
            if registry
                .output("alice", &root, id, "stdout", 0, 100)
                .unwrap()["output"]["text"]
                == "ready"
                && registry
                    .output("alice", &root, id, "stderr", 0, 100)
                    .unwrap()["output"]["text"]
                    == "error"
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        assert_eq!(
            registry
                .output("alice", &root, id, "stderr", 0, 100)
                .unwrap()["output"]["text"],
            "error"
        );
        assert!(
            registry
                .stop("bob", &root, &allowed, identity, false)
                .is_err()
        );
        registry
            .stop("alice", &root, &allowed, identity, false)
            .unwrap();
        for _ in 0..100 {
            if registry
                .output("alice", &root, id, "stdout", 0, 100)
                .unwrap()["state"]
                != "running"
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        assert_eq!(
            registry
                .stop("alice", &root, &allowed, identity, false)
                .unwrap()["stopped"],
            true
        );
        assert!(
            ProcessRegistry::default()
                .output("alice", &root, id, "stdout", 0, 100)
                .is_err()
        );
    }
    #[tokio::test]
    async fn finite_timeout_and_fast_exit_are_observed() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().canonicalize().unwrap();
        let registry = ProcessRegistry::default();
        let sleep = Path::new("/bin/sleep").canonicalize().unwrap();
        let run = registry
            .run("alice", &root, &sleep, &["30".into()], &root, 1, false)
            .await
            .unwrap();
        let _cleanup = TestProcessCleanup::new(run["identity"].as_str().unwrap()).unwrap();
        let id = run["run_id"].as_str().unwrap();
        assert!(inspect(&root, &[], run["identity"].as_str().unwrap()).is_err());
        assert!(
            inspect(
                Path::new("/unrelated"),
                &[sleep],
                run["identity"].as_str().unwrap()
            )
            .is_err()
        );
        tokio::time::sleep(Duration::from_millis(1300)).await;
        assert_eq!(
            registry
                .output("alice", &root, id, "stdout", 0, 100)
                .unwrap()["state"],
            "timed_out"
        );
        let echo = Path::new("/bin/echo").canonicalize().unwrap();
        let run = registry
            .run("alice", &root, &echo, &["done".into()], &root, 10, false)
            .await
            .unwrap();
        let _echo_cleanup = TestProcessCleanup::new(run["identity"].as_str().unwrap()).unwrap();
        let id = run["run_id"].as_str().unwrap();
        for _ in 0..100 {
            if registry
                .output("alice", &root, id, "stdout", 0, 100)
                .unwrap()["state"]
                == "exited"
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        let output = registry
            .output("alice", &root, id, "stdout", 0, 100)
            .unwrap();
        assert_eq!(output["exit_code"], 0);
        assert_eq!(output["output"]["text"], "done\n");
        assert!(
            registry
                .run("alice", &root, &echo, &[], &root, 3601, false)
                .await
                .is_err()
        );
    }
}
