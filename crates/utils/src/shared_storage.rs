//! Fixed repository cache/tmp inspection and manual cleanup. Never follow links.
use std::{io, path::Path};

use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, TS)]
#[serde(rename_all = "lowercase")]
pub enum SharedStorageKind {
    Cache,
    Tmp,
}
impl SharedStorageKind {
    pub fn directory(self) -> &'static str {
        match self {
            Self::Cache => "cache",
            Self::Tmp => "tmp",
        }
    }
}

#[derive(Debug, Serialize, TS)]
pub struct SharedStorageSize {
    #[ts(type = "number | null")]
    pub bytes: Option<u64>,
    pub error: Option<String>,
}
#[derive(Debug, Serialize, TS)]
pub struct SharedStorageUsage {
    pub cache: SharedStorageSize,
    pub tmp: SharedStorageSize,
}

pub fn usage(root: &Path) -> SharedStorageUsage {
    let measure = |kind| match inspect(root, kind, false) {
        Ok(bytes) => SharedStorageSize {
            bytes: Some(bytes),
            error: None,
        },
        Err(error) => SharedStorageSize {
            bytes: None,
            error: Some(error.to_string()),
        },
    };
    SharedStorageUsage {
        cache: measure(SharedStorageKind::Cache),
        tmp: measure(SharedStorageKind::Tmp),
    }
}

pub fn clear(root: &Path, kind: SharedStorageKind) -> io::Result<()> {
    inspect(root, kind, true).map(|_| ())
}

#[cfg(target_os = "linux")]
fn inspect(root: &Path, kind: SharedStorageKind, delete: bool) -> io::Result<u64> {
    use std::{
        ffi::CString,
        fs::{self, File},
        os::{
            fd::{AsRawFd, FromRawFd},
            unix::fs::MetadataExt,
        },
        path::{Component, PathBuf},
    };

    use nix::libc;

    fn open(parent: &File, name: &std::ffi::OsStr, no_mounts: bool) -> io::Result<File> {
        use std::os::unix::ffi::OsStrExt;
        let name = CString::new(name.as_bytes())?;
        // A held parent descriptor and NOFOLLOW prevent path swaps redirecting traversal.
        #[repr(C)]
        struct OpenHow {
            flags: u64,
            mode: u64,
            resolve: u64,
        }
        let flags = libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC;
        let fd = if no_mounts {
            // NO_XDEV also rejects same-device bind mounts into persistent/other repos.
            // Older kernels fail closed; never fall back to an unsafe traversal.
            let how = OpenHow {
                flags: flags as u64,
                mode: 0,
                resolve: 0x01 | 0x04 | 0x08,
            };
            unsafe {
                libc::syscall(
                    libc::SYS_openat2,
                    parent.as_raw_fd(),
                    name.as_ptr(),
                    &how,
                    std::mem::size_of::<OpenHow>(),
                ) as i32
            }
        } else {
            unsafe { libc::openat(parent.as_raw_fd(), name.as_ptr(), flags) }
        };
        if fd < 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(unsafe { File::from_raw_fd(fd) })
        }
    }
    fn anchored(file: &File) -> PathBuf {
        PathBuf::from(format!("/proc/self/fd/{}", file.as_raw_fd()))
    }
    fn walk(dir: &File, device: u64, delete: bool) -> io::Result<u64> {
        use std::os::unix::ffi::OsStrExt;
        let mut bytes = 0u64;
        for entry in fs::read_dir(anchored(dir))? {
            let entry = entry?;
            let name = entry.file_name();
            let meta = fs::symlink_metadata(anchored(dir).join(&name))?;
            let flags = if meta.is_dir() {
                let child = open(dir, &name, true)?;
                if child.metadata()?.dev() != device {
                    return Err(io::Error::other("Nested mount is outside shared storage"));
                }
                bytes = bytes.saturating_add(walk(&child, device, delete)?);
                libc::AT_REMOVEDIR
            } else {
                // Symlinks are counted as links, never their targets. Unlinking a hard link
                // also leaves the bytes in any other directory untouched.
                bytes = bytes.saturating_add(meta.len());
                0
            };
            if delete {
                let name = CString::new(name.as_bytes())?;
                if unsafe { libc::unlinkat(dir.as_raw_fd(), name.as_ptr(), flags) } != 0 {
                    return Err(io::Error::last_os_error());
                }
            }
        }
        Ok(bytes)
    }
    if !root.is_absolute() {
        return Err(io::Error::other("Shared root must be absolute"));
    }
    let mut dir = File::open("/")?;
    for component in root.components() {
        match component {
            Component::RootDir => {}
            Component::Normal(name) => dir = open(&dir, name, false)?,
            _ => return Err(io::Error::other("Invalid shared root")),
        }
    }
    let target = open(&dir, std::ffi::OsStr::new(kind.directory()), true)?;
    walk(&target, target.metadata()?.dev(), delete)
}

#[cfg(not(target_os = "linux"))]
fn inspect(_root: &Path, _kind: SharedStorageKind, _delete: bool) -> io::Result<u64> {
    Err(io::Error::other(
        "Safe shared storage operations require Linux",
    ))
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use std::{fs, os::unix::fs::symlink};

    use super::*;

    #[test]
    fn shared_storage_clears_only_selected_contents_and_keeps_links() {
        let fixture = tempfile::tempdir().unwrap();
        let root = fixture.path().join("repo-a");
        for name in ["cache", "tmp", "persistent"] {
            fs::create_dir_all(root.join(name)).unwrap();
        }
        let other = fixture.path().join("repo-b");
        fs::create_dir(&other).unwrap();
        fs::write(other.join("source"), b"safe").unwrap();
        fs::write(root.join("persistent/keep"), b"safe").unwrap();
        fs::write(root.join("tmp/handoff"), b"handoff").unwrap();
        fs::create_dir(root.join("cache/nested")).unwrap();
        fs::write(root.join("cache/nested/data"), b"12345").unwrap();
        symlink(&other, root.join("cache/escape")).unwrap();
        symlink(root.join("persistent"), root.join("cache/persistent-link")).unwrap();
        fs::hard_link(other.join("source"), root.join("cache/hard-link")).unwrap();
        let entry = fixture.path().join("workspace-cache");
        symlink(root.join("cache"), &entry).unwrap();
        assert!(usage(&root).cache.bytes.unwrap() >= 5);
        clear(&root, SharedStorageKind::Cache).unwrap();
        assert_eq!(usage(&root).cache.bytes, Some(0));
        assert!(entry.is_dir());
        assert!(other.join("source").exists());
        assert!(root.join("persistent/keep").exists());
        assert_eq!(usage(&root).tmp.bytes, Some(7));
        clear(&root, SharedStorageKind::Tmp).unwrap();
        assert_eq!(usage(&root).tmp.bytes, Some(0));
    }

    #[test]
    fn shared_storage_reports_delete_failure_without_zeroing_measurement() {
        use std::os::unix::fs::PermissionsExt;
        if unsafe { nix::libc::geteuid() } == 0 {
            return;
        }
        let fixture = tempfile::tempdir().unwrap();
        let root = fixture.path().join("repo");
        let cache = root.join("cache");
        fs::create_dir_all(&cache).unwrap();
        fs::write(cache.join("keep"), b"12345").unwrap();
        fs::set_permissions(&cache, fs::Permissions::from_mode(0o500)).unwrap();
        let result = clear(&root, SharedStorageKind::Cache);
        let measured = usage(&root).cache.bytes;
        fs::set_permissions(&cache, fs::Permissions::from_mode(0o700)).unwrap();
        assert!(result.is_err());
        assert_eq!(measured, Some(5));
        assert!(cache.join("keep").exists());
    }

    #[test]
    fn shared_storage_rejects_replaced_roots_and_kinds_and_missing_is_not_zero() {
        let fixture = tempfile::tempdir().unwrap();
        let root = fixture.path().join("repo");
        let other = fixture.path().join("other");
        fs::create_dir_all(other.join("cache")).unwrap();
        fs::write(other.join("cache/keep"), b"safe").unwrap();
        symlink(&other, &root).unwrap();
        assert!(clear(&root, SharedStorageKind::Cache).is_err());
        fs::remove_file(&root).unwrap();
        fs::create_dir(&root).unwrap();
        symlink(other.join("cache"), root.join("cache")).unwrap();
        assert!(clear(&root, SharedStorageKind::Cache).is_err());
        assert!(other.join("cache/keep").exists());
        assert!(usage(&root).cache.bytes.is_none());
        assert!(usage(&root).tmp.bytes.is_none());
        assert!(serde_json::from_str::<SharedStorageKind>("\"persistent\"").is_err());
        assert!(serde_json::from_str::<SharedStorageKind>("\"../cache\"").is_err());
    }
}
