//! Only the two server-configured storage destinations, never caller paths.
use std::path::Path;

use serde_json::{Value, json};

use super::field;

fn entry(target: &str, observation: Value) -> Value {
    json!({"target":target, "scope":"configured_destination_filesystem", "observation":observation})
}

pub(super) fn initial() -> Value {
    json!([
        entry("lvk_data", field::<Value>(Err("not_observed"))),
        entry("worktree_base", field::<Value>(Err("not_observed")))
    ])
}

pub(super) fn snapshot(send: &tokio::sync::mpsc::Sender<(&'static str, Value)>) {
    destinations(send, observe);
}

fn destinations(
    send: &tokio::sync::mpsc::Sender<(&'static str, Value)>,
    observe: impl Fn(&Path) -> Result<Value, &'static str>,
) {
    type Resolve = fn() -> std::io::Result<std::path::PathBuf>;
    for (key, target, resolve) in [
        (
            "disks.0",
            "lvk_data",
            utils::assets::asset_dir_path as Resolve,
        ),
        (
            "disks.1",
            "worktree_base",
            worktree_manager::WorktreeManager::get_worktree_base_dir_read_only as Resolve,
        ),
    ] {
        if send.is_closed() {
            return;
        }
        let result = resolve()
            .map_err(|_| "unreadable_destination")
            .and_then(|path| observe(&path));
        let _ = send.blocking_send((key, entry(target, field(result))));
    }
}

fn usage(total: u64, free: u64, available: u64) -> Result<Value, &'static str> {
    if free > total || available > free || total == 0 {
        return Err("invalid_filesystem_counters");
    }
    let used = total - free;
    let usable = used
        .checked_add(available)
        .filter(|v| *v > 0)
        .ok_or("invalid_filesystem_counters")?;
    Ok(
        json!({"total_bytes":total, "used_bytes":used, "available_bytes":available,
        "used_percent":used as f64 / usable as f64 * 100.0,
        "percent_basis":"used / (used + available_to_unprivileged_user)"}),
    )
}

#[cfg(target_os = "linux")]
fn observe(path: &Path) -> Result<Value, &'static str> {
    use std::os::{fd::AsRawFd, unix::fs::MetadataExt};
    let file = std::fs::File::open(path).map_err(|_| "unreadable_destination")?;
    let metadata = file.metadata().map_err(|_| "unreadable_destination")?;
    if !metadata.is_dir() {
        return Err("destination_not_directory");
    }
    let mut stats = std::mem::MaybeUninit::<libc::statvfs>::uninit();
    // SAFETY: live file descriptor and correctly sized writable output. Read
    // the structure only after a successful syscall; no paths are returned.
    if unsafe { libc::fstatvfs(file.as_raw_fd(), stats.as_mut_ptr()) } != 0 {
        return Err("unreadable_filesystem");
    }
    let stats = unsafe { stats.assume_init() };
    let bytes = |blocks: u64| {
        blocks
            .checked_mul(stats.f_frsize)
            .ok_or("filesystem_counter_overflow")
    };
    let mut result = usage(
        bytes(stats.f_blocks)?,
        bytes(stats.f_bfree)?,
        bytes(stats.f_bavail)?,
    )?;
    result["filesystem_id"] = json!(metadata.dev().to_string());
    Ok(result)
}

#[cfg(not(target_os = "linux"))]
fn observe(_: &Path) -> Result<Value, &'static str> {
    Err("unsupported_platform")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disk_usage_accounts_for_reserved_blocks_and_invalid_counters() {
        let v = usage(1000, 300, 200).unwrap();
        assert_eq!(v["available_bytes"], 200);
        assert!((v["used_percent"].as_f64().unwrap() - 77.7777777778).abs() < 0.0001);
        for (total, free, available) in [(0, 0, 0), (10, 11, 1), (10, 5, 6), (10, 10, 0)] {
            assert!(usage(total, free, available).is_err());
        }
        assert_eq!(usage(10, 10, 10).unwrap()["used_percent"], 0.0);
    }

    #[test]
    fn separate_destinations_keep_distinct_filesystems_and_partial_errors() {
        for failed in [false, true] {
            let (send, mut recv) = tokio::sync::mpsc::channel(2);
            destinations(&send, |path| {
                if Some(path) == utils::assets::asset_dir_path().ok().as_deref() {
                    Ok(json!({"filesystem_id":"1", "used_percent":70}))
                } else if failed {
                    Err("unreadable_destination")
                } else {
                    Ok(json!({"filesystem_id":"2", "used_percent":80}))
                }
            });
            let (key, data) = recv.try_recv().unwrap();
            assert_eq!(key, "disks.0");
            assert_eq!(data["target"], "lvk_data");
            assert_eq!(data["observation"]["value"]["filesystem_id"], "1");
            let (key, worktrees) = recv.try_recv().unwrap();
            assert_eq!(key, "disks.1");
            assert_eq!(worktrees["target"], "worktree_base");
            if failed {
                assert_eq!(worktrees["observation"]["status"], "unknown");
            } else {
                assert_eq!(worktrees["observation"]["value"]["filesystem_id"], "2");
            }
        }
    }

    #[test]
    fn missing_destination_is_unknown_and_not_created() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("absent");
        assert!(observe(&path).is_err());
        assert!(!path.exists());
    }
}
