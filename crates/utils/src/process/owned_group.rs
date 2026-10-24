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
                for member in self.root.members()? {
                    member.signal(signal)?;
                }
                tokio::time::sleep(Duration::from_secs(2)).await;
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

impl Drop for OwnedProcessGroup {
    fn drop(&mut self) {
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
        session: u32,
        pub start: u64,
    }

    pub(super) fn identity(pid: u32) -> io::Result<Identity> {
        let stat = std::fs::read_to_string(format!("/proc/{pid}/stat"))?;
        let fields: Vec<_> = stat
            .rsplit_once(')')
            .ok_or_else(|| io::Error::other("invalid stat"))?
            .1
            .split_whitespace()
            .collect();
        let number = |index: usize| -> io::Result<u64> {
            fields
                .get(index)
                .and_then(|value| value.parse().ok())
                .ok_or_else(|| io::Error::other("invalid process identity"))
        };
        Ok(Identity {
            parent: number(1)? as u32,
            group: number(2)? as u32,
            session: number(3)? as u32,
            start: number(19)?,
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
                if observed.group != self.identity.group
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
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;
    use crate::command_ext::GroupSpawnNoWindowExt;

    #[tokio::test]
    async fn owned_group_cleans_orphan_after_wrapper_exit_and_preserves_unrelated() {
        let root = tempfile::tempdir().unwrap();
        let record = root.path().join("pids.json");
        let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
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
