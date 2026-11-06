//! Safe access to LVK's repository/agent-root namespace.
use std::{
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
    sync::Mutex,
};

static IGNORE_LOCK: Mutex<()> = Mutex::new(());

/// Hold .lvk open on Linux so replacing a parent cannot redirect writes.
pub struct LvkDirectory {
    path: PathBuf,
    directory: fs::File,
}

impl LvkDirectory {
    pub fn open(root: &Path) -> io::Result<Self> {
        let path = fs::canonicalize(root)?.join(".lvk");
        match fs::create_dir(&path) {
            Ok(()) => {}
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(e),
        }
        let meta = fs::symlink_metadata(&path)?;
        if !meta.is_dir() || meta.file_type().is_symlink() {
            return Err(io::Error::other(".lvk must be a regular directory"));
        }
        let mut options = fs::OpenOptions::new();
        options.read(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.custom_flags(nix::libc::O_DIRECTORY | nix::libc::O_NOFOLLOW);
        }
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            options.custom_flags(0x02000000 | 0x00200000); // BACKUP_SEMANTICS | OPEN_REPARSE_POINT
        }
        let directory = options.open(&path)?;
        let result = Self { path, directory };
        result.verify()?;
        Ok(result)
    }

    pub fn anchored_path(&self) -> PathBuf {
        #[cfg(target_os = "linux")]
        {
            use std::os::fd::AsRawFd;
            PathBuf::from(format!("/proc/self/fd/{}", self.directory.as_raw_fd()))
        }
        #[cfg(not(target_os = "linux"))]
        {
            self.path.clone()
        }
    }

    pub fn verify(&self) -> io::Result<()> {
        let current = fs::symlink_metadata(&self.path)?;
        if !current.is_dir() || current.file_type().is_symlink() {
            return Err(io::Error::other(".lvk directory changed during placement"));
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            let held = self.directory.metadata()?;
            if current.dev() != held.dev() || current.ino() != held.ino() {
                return Err(io::Error::other(".lvk directory replaced during placement"));
            }
        }
        Ok(())
    }

    /// Append runtime rules when needed. Keep user text and tracked settings visible.
    /// .gitignore itself is intentionally shareable, not self-ignored.
    pub fn ensure_runtime_ignores(&self) -> io::Result<()> {
        let _guard = IGNORE_LOCK
            .lock()
            .map_err(|_| io::Error::other("ignore lock poisoned"))?;
        self.verify()?;
        let path = self.anchored_path().join(".gitignore");
        if let Ok(meta) = fs::symlink_metadata(&path)
            && (!meta.is_file() || meta.file_type().is_symlink() || meta.nlink_count() > 1)
        {
            return Err(io::Error::other(
                ".lvk/.gitignore must be a regular, unlinked file",
            ));
        }
        let mut options = fs::OpenOptions::new();
        options.create(true).read(true).append(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.custom_flags(nix::libc::O_NOFOLLOW | nix::libc::O_NONBLOCK);
        }
        let mut file = options.open(&path)?;
        if !file.metadata()?.is_file() || file.metadata()?.nlink_count() > 1 {
            return Err(io::Error::other("unsafe .lvk/.gitignore"));
        }
        use std::io::Read;
        let mut previous = String::new();
        file.read_to_string(&mut previous)?;
        const RULES: &str = "/shared/\n/attachments/";
        // Put the rules last so earlier negations cannot accidentally expose runtime data.
        // Appending preserves user text; repeating placement is idempotent.
        if !previous
            .lines()
            .collect::<Vec<_>>()
            .ends_with(&["/shared/", "/attachments/"])
        {
            file.write_all(format!("\n# LVK local runtime data\n{RULES}\n").as_bytes())?;
        }
        self.verify()
    }
}

trait LinkCount {
    fn nlink_count(&self) -> u64;
}
impl LinkCount for fs::Metadata {
    fn nlink_count(&self) -> u64 {
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            self.nlink()
        }
        #[cfg(not(unix))]
        {
            1
        }
    }
}

#[cfg(test)]
mod tests {
    use std::process::Command;

    use super::*;

    #[test]
    fn preserves_user_rules_and_keeps_configuration_visible() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(
            Command::new("git")
                .arg("init")
                .arg(tmp.path())
                .output()
                .unwrap()
                .status
                .success()
        );
        let lvk = LvkDirectory::open(tmp.path()).unwrap();
        let ignore = tmp.path().join(".lvk/.gitignore");
        fs::write(&ignore, "# user rule\n/custom/\n!/shared/\n").unwrap();
        lvk.ensure_runtime_ignores().unwrap();
        let first = fs::read_to_string(&ignore).unwrap();
        assert!(first.starts_with("# user rule\n/custom/\n!/shared/\n"));
        lvk.ensure_runtime_ignores().unwrap();
        assert_eq!(fs::read_to_string(&ignore).unwrap(), first);
        for (path, ignored) in [
            (".lvk/shared/cache/data", true),
            (".lvk/attachments/image.png", true),
            (".lvk/integration-validation.json", false),
            (".lvk/preview/vite.config.mjs", false),
            (".lvk/.gitignore", false),
            ("shared/source.rs", false),
        ] {
            let result = Command::new("git")
                .arg("-C")
                .arg(tmp.path())
                .args(["check-ignore", "-q", "--", path])
                .status()
                .unwrap();
            assert_eq!(result.success(), ignored, "{path}");
        }
    }

    #[cfg(unix)]
    #[test]
    fn refuses_symlinked_parent_ignore_and_replaced_parent() {
        use std::os::unix::fs::symlink;
        let tmp = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let path = tmp.path().join(".lvk");
        symlink(outside.path(), &path).unwrap();
        assert!(LvkDirectory::open(tmp.path()).is_err());
        fs::remove_file(&path).unwrap();
        let lvk = LvkDirectory::open(tmp.path()).unwrap();
        let target = outside.path().join("ignore");
        fs::write(&target, "untouched").unwrap();
        symlink(&target, path.join(".gitignore")).unwrap();
        assert!(lvk.ensure_runtime_ignores().is_err());
        assert_eq!(fs::read_to_string(&target).unwrap(), "untouched");
        fs::rename(&path, tmp.path().join("original")).unwrap();
        symlink(outside.path(), &path).unwrap();
        assert!(lvk.ensure_runtime_ignores().is_err());
        assert!(lvk.verify().is_err());
        assert!(!outside.path().join(".gitignore").exists());
    }
}
