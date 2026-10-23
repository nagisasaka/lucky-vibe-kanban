//! Snapshot of observable OS references. This is not an admission lock for
//! external processes: they can start using a path after the snapshot.
use std::path::Path;

use crate::error::ApiError;

/// Policy A: reject observed use, not unrelated inaccessible processes. This
/// snapshot cannot see other PID namespaces or prevent later external starts.
pub(super) async fn check(root: Option<&str>) -> Result<(), ApiError> {
    let Some(root) = root else {
        return Ok(());
    };
    let root = std::path::PathBuf::from(root);
    let observation = tokio::task::spawn_blocking(move || {
        let assets = if cfg!(debug_assertions) {
            // RustEmbed reads this path at request time in debug builds, even
            // when the executable was copied elsewhere and no fd is held open.
            Some(Path::new(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../packages/local-web/dist"
            )))
        } else {
            None
        };
        observe(&root, assets)
    })
    .await
    .map_err(|error| ApiError::Io(std::io::Error::other(error.to_string())))?;
    match observation {
        Ok(Observation {
            busy: Some((pid, source)),
            ..
        }) => Err(ApiError::Conflict(format!(
            "使用中のWorkspaceはarchiveできません。（PID {pid}: {source}）"
        ))),
        Ok(observation) => {
            if observation.unreadable > 0 {
                tracing::debug!(
                    unreadable = observation.unreadable,
                    "Archive process observation has inaccessible fields; policy A covers observable references only"
                );
            }
            Ok(())
        }
        Err(error) => {
            tracing::warn!(%error, "Archive OS process observation unavailable; policy A cannot protect invisible references");
            Ok(())
        }
    }
}

#[derive(Debug, Default)]
pub(super) struct Observation {
    pub busy: Option<(u32, &'static str)>,
    pub unreadable: usize,
}

#[cfg(target_os = "linux")]
fn within(path: &Path, root: &Path) -> bool {
    std::fs::canonicalize(path)
        .unwrap_or_else(|_| path.to_path_buf())
        .starts_with(root)
}

#[cfg(target_os = "linux")]
pub(super) fn observe(root: &Path, self_assets: Option<&Path>) -> std::io::Result<Observation> {
    use std::{fs, os::unix::ffi::OsStrExt, path::PathBuf};

    let root = fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
    let mut result = Observation::default();
    if self_assets.is_some_and(|path| within(path, &root)) {
        result.busy = Some((std::process::id(), "frontend assets"));
        return Ok(result);
    }
    for entry in fs::read_dir("/proc")? {
        let entry = match entry {
            Ok(entry) => entry,
            Err(_) => {
                result.unreadable += 1;
                continue;
            }
        };
        let Some(pid) = entry
            .file_name()
            .to_str()
            .and_then(|name| name.parse::<u32>().ok())
        else {
            continue;
        };
        let process = entry.path();
        let mut unreadable = 0;
        let mut cwd = None;
        for name in ["exe", "cwd"] {
            match fs::read_link(process.join(name)) {
                Ok(path) => {
                    if within(&path, &root) {
                        result.busy = Some((pid, name));
                        return Ok(result);
                    }
                    if name == "cwd" {
                        cwd = Some(path);
                    }
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(_) => unreadable += 1,
            }
        }
        match fs::read_dir(process.join("fd")) {
            Ok(entries) => {
                for entry in entries {
                    match entry.and_then(|entry| fs::read_link(entry.path())) {
                        Ok(path) if within(&path, &root) => {
                            result.busy = Some((pid, "open file"));
                            return Ok(result);
                        }
                        Ok(_) => {}
                        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                        Err(_) => unreadable += 1,
                    }
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => unreadable += 1,
        }
        // A server can close its descriptor while retaining a mapped asset or
        // library. /proc/maps escapes newlines in pathnames as octal bytes.
        match fs::read(process.join("maps")) {
            Ok(maps) => {
                for line in maps.split(|byte| *byte == b'\n') {
                    let mut rest = line;
                    for _ in 0..5 {
                        rest = rest.trim_ascii_start();
                        let end = rest
                            .iter()
                            .position(u8::is_ascii_whitespace)
                            .unwrap_or(rest.len());
                        rest = &rest[end..];
                    }
                    let rest = rest.trim_ascii_start();
                    if rest.first() != Some(&b'/') {
                        continue;
                    }
                    let mut path = Vec::new();
                    let mut i = 0;
                    while i < rest.len() {
                        if rest[i] == b'\\'
                            && i + 3 < rest.len()
                            && rest[i + 1] <= b'3'
                            && rest[i + 1..i + 4]
                                .iter()
                                .all(|byte| (b'0'..=b'7').contains(byte))
                        {
                            path.push(
                                (rest[i + 1] - b'0') * 64
                                    + (rest[i + 2] - b'0') * 8
                                    + (rest[i + 3] - b'0'),
                            );
                            i += 4;
                        } else {
                            path.push(rest[i]);
                            i += 1;
                        }
                    }
                    if within(Path::new(std::ffi::OsStr::from_bytes(&path)), &root) {
                        result.busy = Some((pid, "mapped file"));
                        return Ok(result);
                    }
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => unreadable += 1,
        }
        // Arguments are path candidates, never logged. A relative argument is
        // interpreted against that process's cwd, not this server's directory.
        match fs::read(process.join("cmdline")) {
            Ok(arguments) => {
                for argument in arguments
                    .split(|byte| *byte == 0)
                    .filter(|arg| !arg.is_empty())
                {
                    let argument = if argument.starts_with(b"-") {
                        argument
                            .splitn(2, |byte| *byte == b'=')
                            .nth(1)
                            .unwrap_or(b"")
                    } else {
                        argument
                    };
                    if argument.is_empty() {
                        continue;
                    }
                    let path = Path::new(std::ffi::OsStr::from_bytes(argument));
                    let path: PathBuf = if path.is_absolute() {
                        path.into()
                    } else if let Some(cwd) = &cwd {
                        cwd.join(path)
                    } else {
                        continue;
                    };
                    if path.exists() && within(&path, &root) {
                        result.busy = Some((pid, "path argument"));
                        return Ok(result);
                    }
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => unreadable += 1,
        }
        // Processes which exited during observation no longer own resources.
        if process.exists() {
            result.unreadable += unreadable;
        }
    }
    Ok(result)
}

#[cfg(not(target_os = "linux"))]
pub(super) fn observe(_root: &Path, _self_assets: Option<&Path>) -> std::io::Result<Observation> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "OS reference observation requires Linux procfs",
    ))
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use std::{
        io::BufRead,
        process::{Child, Command, Stdio},
    };

    use super::*;

    // Only a child created by this fixture can be terminated by this guard.
    struct FixtureChild(Child);
    impl Drop for FixtureChild {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }

    #[test]
    fn manual_archive_observes_unregistered_exe_cwd_files_maps_and_arguments() {
        for (kind, script) in [
            ("cwd", "import os; os.chdir(target)"),
            ("open file", "file = open(target + '/index.html')"),
            (
                "mapped file",
                "import ctypes; file = open(target + '/index.html'); libc = ctypes.CDLL(None); libc.mmap.restype = ctypes.c_void_p; mapping = libc.mmap(None, 7, 1, 2, file.fileno(), 0); assert mapping != ctypes.c_void_p(-1).value; file.close()",
            ),
            ("path argument", "pass"),
        ] {
            let temp = tempfile::tempdir().unwrap();
            let root = temp.path().join("worktree");
            std::fs::create_dir(&root).unwrap();
            std::fs::write(root.join("index.html"), "fixture").unwrap();
            // Pass the target via stdin so only the argument case contains it
            // in cmdline. The readiness line follows opening/chdir/mmap.
            let mut command = Command::new("python3");
            command.arg("-c").arg(format!("import sys,time\ntarget = sys.stdin.readline().strip()\n{script}\nprint('ready', flush=True)\ntime.sleep(60)"));
            if kind == "path argument" {
                command.arg(&root);
            }
            let mut child = FixtureChild(
                command
                    .current_dir(temp.path())
                    .stdin(Stdio::piped())
                    .stdout(Stdio::piped())
                    .spawn()
                    .unwrap(),
            );
            use std::io::Write;
            writeln!(child.0.stdin.take().unwrap(), "{}", root.display()).unwrap();
            let mut ready = String::new();
            std::io::BufReader::new(child.0.stdout.take().unwrap())
                .read_line(&mut ready)
                .unwrap();
            assert_eq!(ready.trim(), "ready");
            let observed = observe(&root, None).unwrap();
            assert_eq!(observed.busy, Some((child.0.id(), kind)));
            drop(child);
            assert!(observe(&root, None).unwrap().busy.is_none());
        }
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("worktree");
        std::fs::create_dir(&root).unwrap();
        let exe = root.join("fixture-sleep");
        std::fs::copy("/bin/sleep", &exe).unwrap();
        let child = FixtureChild(
            Command::new(&exe)
                .arg("60")
                .current_dir(temp.path())
                .spawn()
                .unwrap(),
        );
        // spawn returns after exec has succeeded.
        assert_eq!(
            observe(&root, None).unwrap().busy,
            Some((child.0.id(), "exe"))
        );
        drop(child);
        assert!(observe(&root, None).unwrap().busy.is_none());
    }

    #[test]
    fn manual_archive_observes_self_frontend_without_open_descriptor_and_sibling_is_safe() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("worktree");
        let assets = root.join("packages/local-web/dist");
        std::fs::create_dir_all(&assets).unwrap();
        assert_eq!(
            observe(&root, Some(&assets)).unwrap().busy,
            Some((std::process::id(), "frontend assets"))
        );
        let sibling = temp.path().join("worktree-other");
        std::fs::create_dir(&sibling).unwrap();
        assert!(!within(&sibling, &root));
        assert!(observe(&root, Some(&sibling)).unwrap().busy.is_none());
    }
}
