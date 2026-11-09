//! Cleanup while the unreaped group leader still anchors the group's identity.
//! Never signal a bare, potentially recycled PGID after `Child::wait`.
#[cfg(target_os = "linux")]
use std::time::Duration;
use std::{io, process::ExitStatus};

use command_group::AsyncGroupChild;

pub struct OwnedProcessGroup {
    purpose: &'static str,
    settled: bool,
    #[cfg(target_os = "linux")]
    root: linux::Process,
}

impl OwnedProcessGroup {
    pub fn capture(child: &mut AsyncGroupChild, purpose: &'static str) -> io::Result<Self> {
        #[cfg(not(target_os = "linux"))]
        let _ = &child;
        #[cfg(target_os = "linux")]
        let root = linux::Process::leader(
            child
                .inner()
                .id()
                .ok_or_else(|| io::Error::other("provider leader was already reaped"))?,
        )?;
        #[cfg(target_os = "linux")]
        tracing::info!(
            purpose,
            pid = root.pid,
            start = root.identity.start,
            pgid = root.identity.group,
            "owned provider group started"
        );
        Ok(Self {
            purpose,
            settled: false,
            #[cfg(target_os = "linux")]
            root,
        })
    }

    /// Linux waitid(WNOWAIT) keeps the leader unreaped, so its PID/PGID cannot
    /// be recycled while cleanup discovers orphaned members of the same group.
    pub async fn wait(&self, child: &mut AsyncGroupChild) -> io::Result<ExitStatus> {
        #[cfg(target_os = "linux")]
        {
            let _ = child;
            self.root.wait_without_reaping().await
        }
        #[cfg(not(target_os = "linux"))]
        child.wait().await
    }

    pub async fn terminate(&mut self, child: &mut AsyncGroupChild) -> io::Result<()> {
        #[cfg(target_os = "linux")]
        let result = self.terminate_linux(child).await;
        #[cfg(not(target_os = "linux"))]
        let result = super::kill_process_group(child).await;
        self.settled = result.is_ok();
        #[cfg(target_os = "linux")]
        tracing::info!(
            purpose = self.purpose,
            pid = self.root.pid,
            start = self.root.identity.start,
            pgid = self.root.identity.group,
            success = self.settled,
            "owned provider cleanup settled"
        );
        match &result {
            Ok(()) => tracing::info!(purpose = self.purpose, "owned provider cleanup complete"),
            Err(error) => {
                tracing::error!(purpose = self.purpose, %error, "owned provider cleanup failed")
            }
        }
        result
    }

    #[cfg(target_os = "linux")]
    async fn terminate_linux(&self, child: &mut AsyncGroupChild) -> io::Result<()> {
        // Capture session-escaping descendants before sending any signals.
        self.root.verify_anchor()?;
        let descendants = super::CancelledDescendants::capture(Some(self.root.pid));
        let group_cleanup = async {
            for signal in [nix::libc::SIGINT, nix::libc::SIGTERM] {
                let members = self.root.members()?;
                let mut alive = false;
                for member in &members {
                    if !member.exited()? {
                        member.signal(signal)?;
                        alive = true;
                    }
                }
                if !alive {
                    break;
                }
                // Grace is a deadline, not a mandatory delay after kernel exit.
                // Keep the anchor until the final rescan also confirms no late forks.
                let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
                loop {
                    let mut exited = true;
                    for member in &members {
                        exited &= member.exited()?;
                    }
                    if exited || tokio::time::Instant::now() >= deadline {
                        break;
                    }
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
            }
            tokio::time::timeout(Duration::from_secs(5), async {
                loop {
                    // Include children created during the grace period. Every
                    // signal targets a verified pidfd, never a numeric group.
                    let members = self.root.members()?;
                    for member in &members {
                        member.signal(nix::libc::SIGKILL)?;
                    }
                    let mut exited = true;
                    for member in &members {
                        exited &= member.exited()?;
                    }
                    if exited {
                        // A member may have forked immediately before its exit.
                        let mut remaining = false;
                        for member in self.root.members()? {
                            remaining |= !member.exited()?;
                        }
                        if !remaining {
                            return Ok::<_, io::Error>(());
                        }
                    }
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
            })
            .await
            .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "owned group did not exit"))?
        };
        // Both outcomes are observed, even when one cleanup fails.
        let (group, descendants) = tokio::join!(group_cleanup, async {
            match descendants {
                Ok(descendants) => descendants.terminate().await,
                Err(error) => Err(error),
            }
        });
        match (group, descendants) {
            (Ok(()), Ok(())) => {}
            (Err(group), Err(descendants)) => {
                return Err(io::Error::other(format!(
                    "group cleanup failed: {group}; descendant cleanup failed: {descendants}"
                )));
            }
            (Err(error), _) | (_, Err(error)) => return Err(error),
        }
        // Only now release the PID/PGID anchor. Do not call group wait again:
        // waitpid(ECHILD) alone is not proof that orphaned members have exited.
        tokio::time::timeout(Duration::from_secs(5), child.inner().wait())
            .await
            .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "leader reap timed out"))??;
        Ok(())
    }
}

/// A PTY shell owns a POSIX session, including foreground/background job groups.
/// Keep its leader unreaped until every observed member has exited.
#[cfg(target_os = "linux")]
pub struct OwnedProcessSession {
    root: linux::Process,
    settled: bool,
}

#[cfg(target_os = "linux")]
impl OwnedProcessSession {
    pub fn capture(pid: u32) -> io::Result<Self> {
        let root = linux::Process::leader(pid)?;
        if root.identity.session != pid {
            return Err(io::Error::other("PTY child is not our session leader"));
        }
        Ok(Self {
            root,
            settled: false,
        })
    }

    pub async fn wait(&self) -> io::Result<ExitStatus> {
        self.root.wait_without_reaping().await
    }

    pub async fn terminate(&mut self) -> io::Result<()> {
        self.root.verify_anchor()?;
        // Reuse pidfd descendant capture for jobs that escaped into a new session
        // but are still children at cancellation. This is not a daemon sandbox.
        let descendants = super::CancelledDescendants::capture(Some(self.root.pid))?;
        let cleanup = async {
            tokio::time::timeout(Duration::from_secs(5), async {
                loop {
                    let members = self.root.session_members()?;
                    // Signal job groups before the shell, including background jobs.
                    for member in members.iter().filter(|m| m.pid != self.root.pid) {
                        member.signal(nix::libc::SIGKILL)?;
                    }
                    self.root.signal(nix::libc::SIGKILL)?;
                    let mut exited = true;
                    for member in &members {
                        exited &= member.exited()?;
                    }
                    if exited
                        && self
                            .root
                            .session_members()?
                            .iter()
                            .all(|m| m.exited().unwrap_or(false))
                    {
                        return Ok::<_, io::Error>(());
                    }
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
            })
            .await
            .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "PTY session did not exit"))?
        };
        let (session, descendants) = tokio::join!(cleanup, descendants.terminate());
        let result = session.and(descendants);
        self.settled = result.is_ok();
        result
    }
}

#[cfg(target_os = "linux")]
impl Drop for OwnedProcessSession {
    fn drop(&mut self) {
        if !self.settled {
            // Last-resort cancellation safety. No numeric PID/PGID signalling.
            if let Ok(members) = self.root.session_members() {
                for member in members {
                    let _ = member.signal(nix::libc::SIGKILL);
                }
            }
            tracing::error!(
                pid = self.root.pid,
                "PTY owner dropped without confirmed wait/cleanup"
            );
        }
    }
}

impl Drop for OwnedProcessGroup {
    fn drop(&mut self) {
        #[cfg(target_os = "linux")]
        if !self.settled {
            tracing::error!(
                purpose = self.purpose,
                "provider ownership dropped without confirmed cleanup"
            );
        }
    }
}

#[cfg(target_os = "linux")]
mod linux {
    use std::{
        io,
        os::{
            fd::{AsRawFd, FromRawFd, OwnedFd},
            unix::process::ExitStatusExt,
        },
        process::ExitStatus,
        time::Duration,
    };

    use nix::libc;

    #[derive(Clone, Copy, PartialEq, Eq)]
    pub(super) struct Identity {
        parent: u32,
        pub group: u32,
        pub(super) session: u32,
        pub start: u64,
    }

    pub(super) fn identity(pid: u32) -> io::Result<Identity> {
        read_identity(|| std::fs::read_to_string(format!("/proc/{pid}/stat"))).map_err(|error| {
            io::Error::new(
                error.kind(),
                format!("process identity for pid {pid}: {error}"),
            )
        })
    }

    // procfs enumeration races with exit. Retry an incomplete/malformed sample,
    // never skip a still-unreadable process or relax the anchored pidfd checks.
    fn read_identity(mut read: impl FnMut() -> io::Result<String>) -> io::Result<Identity> {
        for attempt in 0..3 {
            let stat = read()?;
            match parse_identity(&stat) {
                Ok(identity) => return Ok(identity),
                Err(_) if attempt < 2 => std::thread::yield_now(),
                Err(error) => return Err(error),
            }
        }
        unreachable!()
    }

    fn parse_identity(stat: &str) -> io::Result<Identity> {
        let fields: Vec<_> = stat
            .rsplit_once(')')
            .ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidData, "invalid process stat delimiter")
            })?
            .1
            .split_whitespace()
            .collect();
        let number = |index: usize, name: &str| -> io::Result<u64> {
            fields
                .get(index)
                .and_then(|value| value.parse().ok())
                .ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("invalid process identity field: {name}"),
                    )
                })
        };
        let pid_number = |index, name| -> io::Result<u32> {
            u32::try_from(number(index, name)?).map_err(|_| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("invalid process identity field: {name}"),
                )
            })
        };
        Ok(Identity {
            parent: pid_number(1, "ppid")?,
            group: pid_number(2, "pgrp")?,
            session: pid_number(3, "session")?,
            start: number(19, "starttime")?,
        })
    }

    pub(super) struct Process {
        pub pid: u32,
        pub identity: Identity,
        fd: OwnedFd,
    }

    impl Process {
        pub(super) fn pin(pid: u32, expected: Identity) -> io::Result<Self> {
            // SAFETY: scalar pidfd_open arguments, descriptor owned on success.
            let fd = unsafe { libc::syscall(libc::SYS_pidfd_open, pid, 0) };
            if fd < 0 {
                return Err(io::Error::last_os_error());
            }
            let fd = unsafe { OwnedFd::from_raw_fd(fd as i32) };
            let actual = identity(pid)?;
            if actual.start != expected.start
                || actual.group != expected.group
                || actual.session != expected.session
            {
                return Err(io::Error::other("process identity changed"));
            }
            Ok(Self {
                pid,
                identity: expected,
                fd,
            })
        }

        pub fn leader(pid: u32) -> io::Result<Self> {
            let expected = identity(pid)?;
            if expected.parent != std::process::id() || expected.group != pid {
                return Err(io::Error::other("process is not our group leader"));
            }
            Self::pin(pid, expected)
        }

        pub fn verify_anchor(&self) -> io::Result<()> {
            if identity(self.pid)? != self.identity {
                return Err(io::Error::other("owned group leader identity changed"));
            }
            Ok(())
        }

        pub fn members(&self) -> io::Result<Vec<Self>> {
            self.find_members(true)
        }

        pub fn session_members(&self) -> io::Result<Vec<Self>> {
            self.find_members(false)
        }

        fn find_members(&self, same_group: bool) -> io::Result<Vec<Self>> {
            self.verify_anchor()?;
            let mut members = Vec::new();
            for entry in std::fs::read_dir("/proc")? {
                let entry = entry?;
                let Some(pid) = entry.file_name().to_str().and_then(|v| v.parse().ok()) else {
                    continue;
                };
                let observed = match identity(pid) {
                    Ok(value) => value,
                    Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
                    Err(error) => return Err(error),
                };
                if (same_group && observed.group != self.identity.group)
                    || observed.session != self.identity.session
                {
                    continue;
                }
                match Self::pin(pid, observed) {
                    Ok(member) => members.push(member),
                    Err(error)
                        if error.kind() == io::ErrorKind::NotFound
                            || error.raw_os_error() == Some(libc::ESRCH) => {}
                    Err(error) => return Err(error),
                }
            }
            self.verify_anchor()?;
            Ok(members)
        }

        pub fn signal(&self, signal: i32) -> io::Result<()> {
            // SAFETY: owned pidfd; recycled numeric PIDs are never signalled.
            let result = unsafe {
                libc::syscall(
                    libc::SYS_pidfd_send_signal,
                    self.fd.as_raw_fd(),
                    signal,
                    std::ptr::null::<libc::siginfo_t>(),
                    0,
                )
            };
            if result < 0 {
                let error = io::Error::last_os_error();
                if error.raw_os_error() != Some(libc::ESRCH) {
                    return Err(error);
                }
            }
            Ok(())
        }

        pub fn exited(&self) -> io::Result<bool> {
            let mut poll = libc::pollfd {
                fd: self.fd.as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            };
            if unsafe { libc::poll(&mut poll, 1, 0) } < 0 {
                return Err(io::Error::last_os_error());
            }
            Ok(poll.revents & libc::POLLIN != 0)
        }

        pub async fn wait_without_reaping(&self) -> io::Result<ExitStatus> {
            loop {
                // SAFETY: initialized siginfo buffer, valid owned child pidfd.
                let mut info: libc::siginfo_t = unsafe { std::mem::zeroed() };
                let result = unsafe {
                    libc::waitid(
                        libc::P_PIDFD,
                        self.fd.as_raw_fd() as libc::id_t,
                        &mut info,
                        libc::WEXITED | libc::WNOWAIT | libc::WNOHANG,
                    )
                };
                if result < 0 {
                    return Err(io::Error::last_os_error());
                }
                if unsafe { info.si_pid() } != 0 {
                    let status = unsafe { info.si_status() };
                    let raw = match info.si_code {
                        libc::CLD_EXITED => status << 8,
                        libc::CLD_KILLED => status,
                        libc::CLD_DUMPED => status | 0x80,
                        _ => return Err(io::Error::other("unexpected child wait status")),
                    };
                    return Ok(ExitStatus::from_raw(raw));
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        }
    }
    #[cfg(test)]
    mod identity_tests {
        use super::*;
        fn stat() -> String {
            let mut fields = ["0"; 20];
            fields[0] = "Z";
            fields[1] = "1";
            fields[2] = "42";
            fields[3] = "42";
            fields[19] = "900";
            format!("42 (name with ) and spaces) {}", fields.join(" "))
        }
        #[test]
        fn transient_stat_recovers_without_skipping_identity() {
            let mut reads = 0;
            let value = read_identity(|| {
                reads += 1;
                Ok(if reads == 1 {
                    "42 (exiting) Z 1".into()
                } else {
                    stat()
                })
            })
            .unwrap();
            assert_eq!(reads, 2);
            assert_eq!(value.group, 42);
            assert_eq!(value.start, 900);
        }
        #[test]
        fn stable_invalid_identity_fails_closed_and_identifies_field() {
            let mut reads = 0;
            let error = read_identity(|| {
                reads += 1;
                Ok("42 (fixture) Z 1".into())
            })
            .err()
            .unwrap();
            assert_eq!(reads, 3);
            assert_eq!(error.kind(), io::ErrorKind::InvalidData);
            assert!(error.to_string().contains("pgrp"));
        }
        #[test]
        fn vanished_procfs_entry_remains_not_found() {
            let error = read_identity(|| Err(io::Error::from(io::ErrorKind::NotFound)))
                .err()
                .unwrap();
            assert_eq!(error.kind(), io::ErrorKind::NotFound);
        }
    }
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;
    use crate::command_ext::GroupSpawnNoWindowExt;

    #[tokio::test]
    async fn owned_group_reproduces_direct_drop_native_survivor() {
        let root = tempfile::tempdir().unwrap();
        let record = root.path().join("pids.json");
        let fixture = std::path::Path::new(&std::env::var("CARGO_MANIFEST_DIR").unwrap())
            .join("../../tests/codex-process-cleanup/fake_app_server.py");
        let mut unrelated = tokio::process::Command::new("sleep")
            .arg("60")
            .kill_on_drop(true)
            .spawn()
            .unwrap();
        let mut child = tokio::process::Command::new("python3")
            .arg(fixture)
            .arg("stall")
            .arg(&record)
            .stdin(std::process::Stdio::piped())
            .kill_on_drop(true)
            .group_spawn_no_window()
            .unwrap();
        tokio::time::timeout(Duration::from_secs(5), async {
            while !record.exists() {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
        let pids: serde_json::Value =
            serde_json::from_slice(&std::fs::read(record).unwrap()).unwrap();
        let pid = pids["native"].as_u64().unwrap() as u32;
        let native = linux::Process::pin(pid, linux::identity(pid).unwrap()).unwrap();
        let wrapper = linux::Process::leader(child.inner().id().unwrap()).unwrap();
        // This is the old discovery lifetime edge: RPC retains stdin, but the
        // AsyncGroupChild is dropped without explicit group cleanup.
        let retained_rpc_stdin = child.inner().stdin.take();
        drop(child);
        let root_exit = tokio::time::timeout(Duration::from_secs(5), async {
            while !wrapper.exited().unwrap() {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await;
        let native_survived = !native.exited().unwrap();
        let unrelated_survived = unrelated.try_wait().unwrap().is_none();
        // Exact fixture cleanup before assertions, including on a failed reproduction.
        wrapper.signal(nix::libc::SIGKILL).unwrap();
        native.signal(nix::libc::SIGKILL).unwrap();
        tokio::time::timeout(Duration::from_secs(5), async {
            while !native.exited().unwrap() {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
        drop(retained_rpc_stdin);
        unrelated.kill().await.unwrap();
        root_exit.unwrap();
        assert!(
            native_survived,
            "expected old direct-child-only drop to leak native"
        );
        assert!(unrelated_survived);
    }

    #[tokio::test]
    async fn owned_group_cleans_orphan_after_wrapper_exit_and_preserves_unrelated() {
        let root = tempfile::tempdir().unwrap();
        let record = root.path().join("pids.json");
        let fixture = std::path::Path::new(&std::env::var("CARGO_MANIFEST_DIR").unwrap())
            .join("../../tests/codex-process-cleanup/fake_app_server.py");
        let mut unrelated = tokio::process::Command::new("sleep")
            .arg("60")
            .kill_on_drop(true)
            .spawn()
            .unwrap();
        let mut child = tokio::process::Command::new("python3")
            .arg(fixture)
            .arg("wrapper-exit")
            .arg(&record)
            .stdin(std::process::Stdio::piped())
            .kill_on_drop(true)
            .group_spawn_no_window()
            .unwrap();
        let mut ownership = OwnedProcessGroup::capture(&mut child, "fixture-natural-exit").unwrap();
        let observed =
            tokio::time::timeout(Duration::from_secs(5), ownership.wait(&mut child)).await;
        let native = std::fs::read(&record).ok().and_then(|bytes| {
            let pids: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
            let pid = pids["native"].as_u64()? as u32;
            linux::Process::pin(pid, linux::identity(pid).ok()?).ok()
        });
        // Always clean the group before asserting the outcome.
        let cleanup = ownership.terminate(&mut child).await;
        let unrelated_alive = unrelated.try_wait().unwrap().is_none();
        unrelated.kill().await.unwrap();
        assert!(observed.unwrap().unwrap().success());
        cleanup.unwrap();
        assert!(unrelated_alive);
        assert!(
            native
                .expect("native was pinned before cleanup")
                .exited()
                .unwrap()
        );
    }

    #[tokio::test]
    async fn owned_group_rejects_unowned_identity_without_signalling() {
        assert!(linux::Process::leader(std::process::id()).is_err());
    }
}
