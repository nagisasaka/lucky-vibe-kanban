//! Image-owned public reference source. No Project registration or host-path API.
use std::{
    path::{Path, PathBuf},
    sync::OnceLock,
};

use anyhow::{Result, ensure};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub(crate) struct SourceInfo {
    pub path: PathBuf,
    pub build_revision: String,
    pub commit: String,
    pub history_scope: &'static str,
}

/// Validate once per server process; distribution validates the full repository.
/// Neither startup nor questions fetch from the network.
pub(crate) fn current() -> Result<SourceInfo> {
    static SOURCE: OnceLock<Result<SourceInfo, String>> = OnceLock::new();
    SOURCE
        .get_or_init(|| {
            let path = std::env::var_os("LVK_SOURCE_PATH")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("/opt/lvk/source"));
            inspect(&path, env!("LVK_BUILD_REVISION")).map_err(|e| e.to_string())
        })
        .clone()
        .map_err(anyhow::Error::msg)
}

fn inspect(path: &Path, revision: &str) -> Result<SourceInfo> {
    ensure!(
        revision.len() == 40 && revision.bytes().all(|b| b.is_ascii_hexdigit()),
        "LVK build revision is unknown; matching bundled source unavailable"
    );
    let path = path
        .canonicalize()
        .map_err(|_| anyhow::anyhow!("Bundled LVK source unavailable"))?;
    let repo = git2::Repository::open(&path)?;
    ensure!(!repo.is_shallow(), "Bundled LVK source history is shallow");
    let expected = git2::Oid::from_str(revision)?;
    let head = repo.head()?.peel_to_commit()?.id();
    ensure!(
        head == expected,
        "Bundled LVK source does not match binary build revision"
    );
    Ok(SourceInfo {
        path,
        build_revision: revision.to_owned(),
        commit: head.to_string(),
        history_scope: "all ancestors of the distribution commit; no promise of all branches or tags",
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_source_requires_matching_known_revision_and_complete_history() {
        let temp = tempfile::tempdir().unwrap();
        let repo = git2::Repository::init(temp.path()).unwrap();
        let sig = git2::Signature::now("Fixture", "fixture@example.invalid").unwrap();
        std::fs::write(temp.path().join("guide.md"), "before\n").unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(Path::new("guide.md")).unwrap();
        let tree = repo.find_tree(index.write_tree().unwrap()).unwrap();
        let first = repo
            .commit(
                Some("HEAD"),
                &sig,
                &sig,
                "Author explains the original problem",
                &tree,
                &[],
            )
            .unwrap();
        let parent = repo.find_commit(first).unwrap();
        let second = repo
            .commit(
                Some("HEAD"),
                &sig,
                &sig,
                "Current release",
                &tree,
                &[&parent],
            )
            .unwrap();
        let info = inspect(temp.path(), &second.to_string()).unwrap();
        assert_eq!(info.commit, second.to_string());
        assert!(inspect(temp.path(), "").is_err());
        assert!(inspect(temp.path(), &first.to_string()).is_err());
        std::fs::write(repo.path().join("shallow"), format!("{second}\n")).unwrap();
        assert!(inspect(temp.path(), &second.to_string()).is_err());
    }
}
