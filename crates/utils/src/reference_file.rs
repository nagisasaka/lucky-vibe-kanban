//! Bounded reference reads anchored to an open directory. Never follow symlinks
//! in any relative component, including during concurrent checkout changes.
use std::{io, path::Path};

#[cfg(unix)]
pub fn read(root: &Path, relative: &Path, limit: usize) -> io::Result<Option<Vec<u8>>> {
    use std::{
        fs::{File, OpenOptions},
        io::Read,
        os::{
            fd::{AsRawFd, FromRawFd},
            unix::fs::OpenOptionsExt,
        },
        path::Component,
    };

    use nix::{
        fcntl::{OFlag, openat},
        sys::stat::Mode,
    };

    let parts: Vec<_> = relative.components().collect();
    if parts.is_empty() || parts.iter().any(|p| !matches!(p, Component::Normal(_))) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Invalid reference path",
        ));
    }
    let mut parent = OpenOptions::new()
        .read(true)
        .custom_flags(nix::libc::O_NOFOLLOW | nix::libc::O_DIRECTORY)
        .open(root)?;
    for (index, part) in parts.iter().enumerate() {
        let directory = index + 1 != parts.len();
        let mut flags = OFlag::O_RDONLY | OFlag::O_CLOEXEC | OFlag::O_NOFOLLOW | OFlag::O_NONBLOCK;
        if directory {
            flags |= OFlag::O_DIRECTORY;
        }
        let descriptor = match openat(
            Some(parent.as_raw_fd()),
            Path::new(part.as_os_str()),
            flags,
            Mode::empty(),
        ) {
            Ok(fd) => fd,
            Err(nix::errno::Errno::ENOENT) => return Ok(None),
            Err(error) => return Err(io::Error::from_raw_os_error(error as i32)),
        };
        // SAFETY: openat returned a fresh owned descriptor. File takes sole
        // ownership and closes it on every path, including early returns.
        let file = unsafe { File::from_raw_fd(descriptor) };
        if directory {
            parent = file;
            continue;
        }
        if !file.metadata()?.is_file() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Reference is not a regular file",
            ));
        }
        let mut bytes = Vec::new();
        file.take(limit as u64 + 1).read_to_end(&mut bytes)?;
        return Ok(Some(bytes));
    }
    unreachable!("nonempty path")
}

#[cfg(not(unix))]
pub fn read(_root: &Path, _relative: &Path, _limit: usize) -> io::Result<Option<Vec<u8>>> {
    // Do not silently fall back to a racy check-then-open on unsupported hosts.
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "Confined reference reads require a Unix server",
    ))
}

#[cfg(all(test, unix))]
mod tests {
    use std::fs;

    use super::*;

    #[test]
    fn confines_every_component_and_bounds_regular_file_reads() {
        let temp = tempfile::tempdir().unwrap();
        fs::create_dir(temp.path().join("nested")).unwrap();
        fs::write(temp.path().join("nested/file"), b"abcdef").unwrap();
        assert_eq!(
            read(temp.path(), Path::new("nested/file"), 3).unwrap(),
            Some(b"abcd".to_vec())
        );
        assert_eq!(
            read(temp.path(), Path::new("absent/file"), 3).unwrap(),
            None
        );
        for path in ["../outside", "/etc/passwd", ""] {
            assert!(read(temp.path(), Path::new(path), 3).is_err());
        }
        assert!(read(temp.path(), Path::new("nested"), 3).is_err());
        std::os::unix::fs::symlink("nested", temp.path().join("redirect")).unwrap();
        assert!(read(temp.path(), Path::new("redirect/file"), 3).is_err());
        std::os::unix::fs::symlink("nested/file", temp.path().join("link")).unwrap();
        assert!(read(temp.path(), Path::new("link"), 3).is_err());
        nix::unistd::mkfifo(&temp.path().join("fifo"), nix::sys::stat::Mode::S_IRUSR).unwrap();
        assert!(read(temp.path(), Path::new("fifo"), 3).is_err());
    }
}
