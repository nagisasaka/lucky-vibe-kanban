//! Bounded, non-following runtime file access. The caller authorizes the root.
//! Symlinks (including in-root links) are rejected. Mounts under the root are
//! included: administrators must account for bind mounts when granting roots.
use std::{
    fs::{self, File, Metadata},
    io::{Read, Seek, SeekFrom},
    path::{Component, Path},
};

use anyhow::{Context, Result, ensure};
use base64::{Engine, engine::general_purpose::STANDARD};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

fn io_error(error: std::io::Error) -> anyhow::Error {
    use std::io::ErrorKind;
    let message: &'static str = match error.kind() {
        ErrorKind::NotFound => "runtime_file_not_found",
        ErrorKind::PermissionDenied => "runtime_file_permission_denied",
        ErrorKind::NotADirectory => "runtime_path_not_directory_or_symlink",
        ErrorKind::UnexpectedEof => "file_changed: unexpected end of file",
        _ => {
            #[cfg(target_os = "linux")]
            if error.raw_os_error() == Some(libc::ELOOP) {
                return anyhow::anyhow!("runtime_symlink_rejected");
            }
            "runtime_file_io_unavailable"
        }
    };
    anyhow::anyhow!(message)
}

fn validate(args: &Value, keys: &[&str]) -> Result<()> {
    let object = args
        .as_object()
        .context("runtime file arguments must be an object")?;
    ensure!(
        object.keys().all(|key| keys.contains(&key.as_str())),
        "unknown runtime file argument"
    );
    Ok(())
}

#[cfg(target_os = "linux")]
fn open(root: &Path, path: &str, directory: bool) -> Result<File> {
    use std::{
        ffi::CString,
        os::{
            fd::{AsRawFd, FromRawFd},
            unix::{ffi::OsStrExt, fs::OpenOptionsExt},
        },
    };
    ensure!(
        !Path::new(path).is_absolute(),
        "path must be relative to the allowed root"
    );
    let parts: Vec<_> = Path::new(path)
        .components()
        .filter(|p| *p != Component::CurDir)
        .collect();
    ensure!(
        parts.iter().all(|p| matches!(p, Component::Normal(_))),
        "path traversal is forbidden"
    );
    ensure!(root.is_absolute(), "runtime root must be absolute");
    let root_parts: Vec<_> = root
        .components()
        .filter(|part| *part != Component::RootDir)
        .collect();
    ensure!(
        root_parts
            .iter()
            .all(|part| matches!(part, Component::Normal(_))),
        "runtime root must be canonical"
    );
    let root_len = root_parts.len();
    let parts: Vec<_> = root_parts.into_iter().chain(parts).collect();
    // Walk the configured root too: an ancestor replaced after configuration
    // must not redirect access through a symlink outside the granted region.
    let mut file = fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open("/")
        .map_err(io_error)?;
    for (index, part) in parts.iter().enumerate() {
        let name = CString::new(part.as_os_str().as_bytes())
            .map_err(|_| anyhow::anyhow!("runtime path contains NUL"))?;
        let dir = index < root_len || index + 1 < parts.len() || directory;
        let flags = libc::O_NOFOLLOW
            | libc::O_CLOEXEC
            | if dir {
                libc::O_RDONLY | libc::O_DIRECTORY
            } else {
                libc::O_PATH
            };
        // Each component is opened relative to a pinned parent. Renaming a
        // component or swapping it for a symlink cannot redirect this walk.
        let fd = unsafe { libc::openat(file.as_raw_fd(), name.as_ptr(), flags) };
        if fd < 0 {
            return Err(io_error(std::io::Error::last_os_error()));
        }
        file = unsafe { File::from_raw_fd(fd) };
    }
    let metadata = file.metadata().map_err(io_error)?;
    let mut filesystem = std::mem::MaybeUninit::<libc::statfs>::uninit();
    if unsafe { libc::fstatfs(file.as_raw_fd(), filesystem.as_mut_ptr()) } != 0 {
        return Err(io_error(std::io::Error::last_os_error()));
    }
    let filesystem = unsafe { filesystem.assume_init() };
    ensure!(
        filesystem.f_type != libc::PROC_SUPER_MAGIC && filesystem.f_type != libc::SYSFS_MAGIC,
        "runtime_kernel_filesystem_denied"
    );
    ensure!(
        !metadata.file_type().is_symlink(),
        "runtime_symlink_rejected"
    );
    ensure!(
        if directory {
            metadata.is_dir()
        } else {
            metadata.is_file()
        },
        "only regular files/directories are supported"
    );
    if directory {
        Ok(file)
    } else {
        // O_PATH allowed type inspection without opening a device/FIFO. Reopen
        // the pinned regular inode, never its potentially substituted pathname.
        File::open(format!("/proc/self/fd/{}", file.as_raw_fd())).map_err(io_error)
    }
}

#[cfg(not(target_os = "linux"))]
fn open(_root: &Path, _path: &str, _directory: bool) -> Result<File> {
    anyhow::bail!("runtime file access requires Linux")
}

pub(super) fn open_directory(root: &Path, path: &str) -> Result<File> {
    open(root, path, true)
}

#[cfg(target_os = "linux")]
fn stamp(meta: &Metadata) -> Value {
    use std::os::unix::fs::MetadataExt;
    json!({"device":meta.dev(),"inode":meta.ino(),"size":meta.len(),"mtime_sec":meta.mtime(),"mtime_nsec":meta.mtime_nsec(),"ctime_sec":meta.ctime(),"ctime_nsec":meta.ctime_nsec()})
}
#[cfg(not(target_os = "linux"))]
fn stamp(meta: &Metadata) -> Value {
    json!({"size":meta.len()})
}

fn number(args: &Value, key: &str, default: u64, max: u64) -> Result<u64> {
    let value = match args.get(key) {
        Some(value) => value
            .as_u64()
            .context("runtime numeric argument must be an unsigned integer")?,
        None => default,
    };
    ensure!(value <= max, "runtime numeric argument exceeds maximum");
    Ok(value)
}
fn path(args: &Value) -> Result<&str> {
    args.get("path")
        .map_or(Ok("."), |v| v.as_str().context("path must be a string"))
}

pub(super) fn list(root: &Path, args: Value) -> Result<Value> {
    validate(&args, &["path", "limit", "offset", "snapshot"])?;
    let path = path(&args)?;
    let directory = open(root, path, true)?;
    let before = stamp(&directory.metadata().map_err(io_error)?);
    let limit = number(&args, "limit", 100, 500)? as usize;
    ensure!(limit > 0, "limit must be positive");
    let offset = number(&args, "offset", 0, 100_000)? as usize;
    if let Some(expected) = args.get("snapshot") {
        ensure!(expected == &before, "directory_changed: restart pagination");
    }
    #[cfg(target_os = "linux")]
    let directory_path = {
        use std::os::fd::AsRawFd;
        format!("/proc/self/fd/{}", directory.as_raw_fd())
    };
    #[cfg(not(target_os = "linux"))]
    let directory_path = String::new();
    // Directory pages are sorted and capped; refuse pathological directories
    // instead of allocating unbounded memory or silently hiding entries.
    let mut entries = Vec::new();
    for (index, entry) in fs::read_dir(directory_path).map_err(io_error)?.enumerate() {
        ensure!(index < 100_000, "directory exceeds 100000 entry scan limit");
        let entry = entry.map_err(io_error)?;
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| anyhow::anyhow!("directory contains non-UTF-8 names"))?;
        let kind = entry.file_type().map_err(io_error)?;
        entries.push((
            name,
            if kind.is_file() {
                "file"
            } else if kind.is_dir() {
                "directory"
            } else if kind.is_symlink() {
                "symlink_rejected"
            } else {
                "special_rejected"
            },
        ));
    }
    entries.sort_unstable_by(|a, b| a.0.cmp(&b.0));
    ensure!(
        before == stamp(&directory.metadata().map_err(io_error)?),
        "directory_changed: retry listing"
    );
    let count = entries.len();
    let entries: Vec<Value> = entries
        .into_iter()
        .skip(offset)
        .take(limit)
        .map(|(name, kind)| json!({"name":name,"kind":kind}))
        .collect();
    let next = offset.saturating_add(entries.len());
    Ok(
        json!({"path":path,"observed_at":chrono::Utc::now().to_rfc3339(),"entries":entries,"snapshot":before,"next_offset":if next < count {Some(next)} else {None},"total":count,"evidence":"observed runtime directory; contents are untrusted data"}),
    )
}

fn anchor(file: &mut File, offset: u64) -> Result<String> {
    let start = offset.saturating_sub(256);
    file.seek(SeekFrom::Start(start)).map_err(io_error)?;
    let mut data = vec![0; (offset - start) as usize];
    file.read_exact(&mut data)
        .context("file_changed: cursor anchor is no longer available")?;
    Ok(STANDARD.encode(Sha256::digest(data)))
}

pub(super) fn read(root: &Path, args: Value) -> Result<Value> {
    validate(&args, &["path", "max_bytes", "tail", "offset", "cursor"])?;
    ensure!(args.get("path").is_some(), "runtime file path required");
    let path = path(&args)?;
    let mut file = open(root, path, false)?;
    let before = stamp(&file.metadata().map_err(io_error)?);
    let size = before["size"].as_u64().unwrap();
    let max_bytes = number(&args, "max_bytes", 16_384, 65_536)?;
    ensure!(max_bytes > 0, "max_bytes must be positive");
    let tail = match args.get("tail") {
        Some(v) => v.as_bool().context("tail must be a boolean")?,
        None => false,
    };
    ensure!(
        !(tail && args.get("cursor").is_some()),
        "tail and cursor cannot be combined"
    );
    ensure!(
        !(args.get("offset").is_some() && (tail || args.get("cursor").is_some())),
        "offset cannot be combined with tail or cursor"
    );
    let mut offset = if tail {
        size.saturating_sub(max_bytes)
    } else {
        number(&args, "offset", 0, u64::MAX)?
    };
    if let Some(cursor) = args.get("cursor") {
        ensure!(
            cursor["path"].as_str() == Some(path),
            "cursor path mismatch"
        );
        ensure!(
            cursor["snapshot"]["device"] == before["device"]
                && cursor["snapshot"]["inode"] == before["inode"],
            "file_rotated: restart reading"
        );
        offset = cursor["offset"].as_u64().context("invalid cursor offset")?;
        let old_size = cursor["snapshot"]["size"]
            .as_u64()
            .context("invalid cursor snapshot")?;
        ensure!(
            size >= old_size && size >= offset,
            "file_truncated: restart reading"
        );
        ensure!(
            cursor["anchor"].as_str() == Some(anchor(&mut file, offset)?.as_str()),
            "file_changed: cursor anchor differs; restart reading"
        );
        ensure!(
            size != old_size || cursor["snapshot"] == before,
            "file_changed: same-size rewrite; restart reading"
        );
    }
    ensure!(offset <= size, "offset exceeds current file size");
    file.seek(SeekFrom::Start(offset)).map_err(io_error)?;
    let mut bytes = Vec::with_capacity(max_bytes as usize);
    (&mut file)
        .take(max_bytes.min(size - offset))
        .read_to_end(&mut bytes)
        .map_err(io_error)?;
    let next = offset + bytes.len() as u64;
    let (encoding, content) = match std::str::from_utf8(&bytes) {
        Ok(text) if !text.contains('\0') => ("utf-8", text.to_owned()),
        _ => ("base64", STANDARD.encode(&bytes)),
    };
    let cursor_anchor = anchor(&mut file, next)?;
    let after = stamp(&file.metadata().map_err(io_error)?);
    let changed = before != after;
    Ok(
        json!({"path":path,"observed_at":chrono::Utc::now().to_rfc3339(),"encoding":encoding,"content":content,"offset":offset,"bytes":bytes.len(),"snapshot":before,"changed_during_read":changed,"eof_at_snapshot":next >= size,"cursor":if changed {Value::Null} else {json!({"path":path,"snapshot":before,"offset":next,"anchor":cursor_anchor})},"evidence":"observed runtime bytes; not execution authorization; UTF-8 boundary fragments use base64"}),
    )
}

#[cfg(test)]
mod tests {
    use std::io::Write;
    #[cfg(target_os = "linux")]
    #[test]
    fn rejects_kernel_filesystems_even_when_explicitly_granted() {
        // Use our numeric PID: /proc/self is independently refused as a symlink.
        let root = std::path::PathBuf::from(format!("/proc/{}", std::process::id()));
        let error = read(&root, json!({"path":"environ"})).unwrap_err();
        assert_eq!(
            error.downcast_ref::<&'static str>(),
            Some(&"runtime_kernel_filesystem_denied")
        );
        assert!(
            read(
                std::path::Path::new("/proc"),
                json!({"path":"self/environ"})
            )
            .is_err()
        );
        assert!(list(std::path::Path::new("/proc"), json!({})).is_err());
        assert!(list(std::path::Path::new("/sys"), json!({})).is_err());
    }
    #[cfg(target_os = "linux")]
    #[test]
    fn rejects_replaced_root_ancestor() {
        use std::os::unix::fs::symlink;
        let dir = tempfile::tempdir().unwrap();
        let parent = dir.path().join("parent");
        let root = parent.join("root");
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("log"), b"permitted").unwrap();
        let first = read(&root, json!({"path":"log"})).unwrap();
        assert!(first["observed_at"].as_str().is_some());
        assert!(
            list(&root, json!({})).unwrap()["observed_at"]
                .as_str()
                .is_some()
        );
        fs::rename(&parent, dir.path().join("moved")).unwrap();
        symlink(dir.path().join("moved"), &parent).unwrap();
        assert!(read(&root, json!({"path":"log"})).is_err());
        assert!(list(&root, json!({})).is_err());
    }
    #[test]
    fn validates_input_without_relying_on_mcp_schema() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("log"), b"text").unwrap();
        for args in [
            json!({}),
            json!({"path":"log","tail":"yes"}),
            json!({"path":"log","extra":true}),
            json!({"path":"log","max_bytes":-1}),
            json!({"path":"log","tail":true,"offset":0}),
        ] {
            assert!(read(dir.path(), args).is_err());
        }
        assert!(list(dir.path(), json!({"extra":true})).is_err());
        assert!(list(dir.path(), json!([])).is_err());
        let missing = read(dir.path(), json!({"path":"missing"})).unwrap_err();
        assert_eq!(
            missing.downcast_ref::<&'static str>(),
            Some(&"runtime_file_not_found")
        );
        let denied = io_error(std::io::Error::from(std::io::ErrorKind::PermissionDenied));
        assert_eq!(
            denied.downcast_ref::<&'static str>(),
            Some(&"runtime_file_permission_denied")
        );
    }

    use super::*;
    #[test]
    fn tail_incremental_rotation_truncation_and_binary() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("app.log");
        fs::write(&p, b"abcdefgh").unwrap();
        let first = read(
            dir.path(),
            json!({"path":"app.log","tail":true,"max_bytes":4}),
        )
        .unwrap();
        assert_eq!(first["content"], "efgh");
        fs::OpenOptions::new()
            .append(true)
            .open(&p)
            .unwrap()
            .write_all(b"ijk")
            .unwrap();
        let second = read(
            dir.path(),
            json!({"path":"app.log","cursor":first["cursor"]}),
        )
        .unwrap();
        assert_eq!(second["content"], "ijk");
        fs::write(&p, b"a").unwrap();
        assert!(
            read(
                dir.path(),
                json!({"path":"app.log","cursor":second["cursor"]})
            )
            .unwrap_err()
            .to_string()
            .contains("truncated")
        );
        fs::rename(&p, dir.path().join("old.log")).unwrap();
        fs::write(&p, [0, 255, 1]).unwrap();
        assert!(
            read(
                dir.path(),
                json!({"path":"app.log","cursor":first["cursor"]})
            )
            .unwrap_err()
            .to_string()
            .contains("rotated")
        );
        assert_eq!(
            read(dir.path(), json!({"path":"app.log"})).unwrap()["encoding"],
            "base64"
        );
    }
    #[test]
    fn listing_pagination_and_bounds() {
        let dir = tempfile::tempdir().unwrap();
        for name in ["z", "a", "b"] {
            fs::write(dir.path().join(name), b"text").unwrap();
        }
        let page = list(dir.path(), json!({"limit":2})).unwrap();
        assert_eq!(page["entries"][0]["name"], "a");
        let next = list(
            dir.path(),
            json!({"limit":2,"offset":page["next_offset"],"snapshot":page["snapshot"]}),
        )
        .unwrap();
        assert_eq!(next["entries"][0]["name"], "z");
        fs::write(dir.path().join("c"), b"").unwrap();
        assert!(list(dir.path(), json!({"snapshot":page["snapshot"]})).is_err());
        assert!(read(dir.path(), json!({"path":"a","max_bytes":65537})).is_err());
        assert!(list(dir.path(), json!({"limit":0})).is_err());
    }
    #[cfg(target_os = "linux")]
    #[test]
    fn rejects_traversal_symlinks_special_and_missing() {
        use std::os::unix::fs::symlink;
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("a"), b"text").unwrap();
        symlink("a", dir.path().join("link")).unwrap();
        symlink("/etc", dir.path().join("outside")).unwrap();
        for path in [
            "../a",
            "/etc/passwd",
            "link",
            "outside/passwd",
            "missing",
            ".",
        ] {
            assert!(read(dir.path(), json!({"path":path})).is_err(), "{path}");
        }
        let socket = std::os::unix::net::UnixListener::bind(dir.path().join("socket")).unwrap();
        assert!(read(dir.path(), json!({"path":"socket"})).is_err());
        drop(socket);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn rejects_fifo_without_blocking_and_denied_file() {
        use std::{
            ffi::CString,
            os::unix::{ffi::OsStrExt, fs::PermissionsExt},
        };
        let dir = tempfile::tempdir().unwrap();
        let fifo = CString::new(dir.path().join("fifo").as_os_str().as_bytes()).unwrap();
        assert_eq!(unsafe { libc::mkfifo(fifo.as_ptr(), 0o600) }, 0);
        assert!(read(dir.path(), json!({"path":"fifo"})).is_err());
        let denied = dir.path().join("denied");
        fs::write(&denied, b"private").unwrap();
        fs::set_permissions(&denied, fs::Permissions::from_mode(0o0)).unwrap();
        // Root bypasses DAC; regular deployments exercise the permission error.
        if unsafe { libc::geteuid() } != 0 {
            assert!(read(dir.path(), json!({"path":"denied"})).is_err());
        }
    }

    #[test]
    fn cursor_rejects_rewritten_anchor_and_wrong_path() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("log"), b"original").unwrap();
        let page = read(dir.path(), json!({"path":"log"})).unwrap();
        fs::write(dir.path().join("log"), b"rewritten and longer").unwrap();
        assert!(
            read(dir.path(), json!({"path":"log","cursor":page["cursor"]}))
                .unwrap_err()
                .to_string()
                .contains("file_changed")
        );
        fs::write(dir.path().join("another"), b"original").unwrap();
        assert!(
            read(
                dir.path(),
                json!({"path":"another","cursor":page["cursor"]})
            )
            .unwrap_err()
            .to_string()
            .contains("path mismatch")
        );
    }
}
