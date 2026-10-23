//! Optional instance-local state, independent of the Cargo build profile.
//! With no override the existing desktop/server paths remain unchanged.
use std::path::{Path, PathBuf};

pub fn instance_dir() -> Option<PathBuf> {
    std::env::var_os("LVK_INSTANCE_DIR").map(|value| {
        resolve_instance_dir(Path::new(&value), &crate::assets::prod_asset_dir_path())
            .expect("LVK_INSTANCE_DIR must be an absolute, isolated data directory")
    })
}

/// Resolve an already-initialised instance without creating any directories.
pub fn instance_dir_read_only() -> std::io::Result<Option<PathBuf>> {
    std::env::var_os("LVK_INSTANCE_DIR")
        .map(|value| {
            resolve_existing_instance_dir(Path::new(&value), &crate::assets::prod_asset_dir_path())
        })
        .transpose()
}

fn resolve_instance_dir(path: &Path, stable: &Path) -> std::io::Result<PathBuf> {
    if !path.is_absolute() {
        return Err(std::io::Error::other("Relative instance directory"));
    }
    std::fs::create_dir_all(path)?;
    resolve_existing_instance_dir(path, stable)
}

fn resolve_existing_instance_dir(path: &Path, stable: &Path) -> std::io::Result<PathBuf> {
    if !path.is_absolute() {
        return Err(std::io::Error::other("Relative instance directory"));
    }
    let path = path.canonicalize()?;
    // Canonicalize the nearest existing ancestor so symlink aliases cannot
    // silently redirect an instance into the default stable data directory.
    fn resolved(path: &Path) -> std::io::Result<PathBuf> {
        if path.exists() {
            path.canonicalize()
        } else {
            let parent = path
                .parent()
                .ok_or_else(|| std::io::Error::other("Invalid path"))?;
            Ok(resolved(parent)?.join(path.file_name().unwrap()))
        }
    }
    let stable = resolved(stable)?;
    let data = resolved(&path.join("data"))?;
    if path.starts_with(&stable)
        || stable.starts_with(&path)
        || data.starts_with(&stable)
        || stable.starts_with(&data)
    {
        return Err(std::io::Error::other(
            "Instance overlaps stable data directory",
        ));
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_only_resolution_never_creates_missing_instance() {
        let root = tempfile::tempdir().unwrap();
        let instance = root.path().join("absent");
        let stable = root.path().join("stable");
        assert!(resolve_existing_instance_dir(&instance, &stable).is_err());
        assert!(!instance.exists());
        assert!(resolve_existing_instance_dir(Path::new("relative"), &stable).is_err());
        std::fs::create_dir(&instance).unwrap();
        assert_eq!(
            resolve_existing_instance_dir(&instance, &stable).unwrap(),
            instance
        );
        assert!(!stable.exists());
        assert!(!instance.join("data").exists());
    }

    #[test]
    fn isolated_paths_reject_stable_aliases_and_relative_paths() {
        let root = tempfile::tempdir().unwrap();
        let stable = root.path().join("stable");
        std::fs::create_dir(&stable).unwrap();
        let instance = root.path().join("staging");
        assert_eq!(resolve_instance_dir(&instance, &stable).unwrap(), instance);
        assert!(resolve_instance_dir(Path::new("relative"), &stable).is_err());
        assert!(resolve_instance_dir(&stable, &stable).is_err());
        assert!(resolve_instance_dir(root.path(), &stable).is_err());
        #[cfg(unix)]
        {
            let alias = root.path().join("alias");
            std::os::unix::fs::symlink(&stable, &alias).unwrap();
            assert!(resolve_instance_dir(&alias, &stable).is_err());
            std::os::unix::fs::symlink(&stable, instance.join("data")).unwrap();
            assert!(resolve_instance_dir(&instance, &stable).is_err());
        }
    }
}
