//! Read-only repository references for external consumers. Git identities and
//! the normal merge base stay in GitService; this module never runs a shell,
//! textconv, external diff or repository hook.
use std::path::Path;

use git2::{ObjectType, Repository};
use serde::Serialize;

use crate::{Commit, GitService, GitServiceError};

pub const MAX_REFERENCE_BYTES: usize = 512 * 1024;

#[derive(Debug, Serialize)]
#[serde(tag = "status", content = "content", rename_all = "snake_case")]
pub enum Content {
    Text(String),
    Missing,
    Binary,
    TooLarge,
    Unsupported,
}

fn content(bytes: &[u8]) -> Content {
    if bytes.len() > MAX_REFERENCE_BYTES {
        return Content::TooLarge;
    }
    if bytes.contains(&0) {
        return Content::Binary;
    }
    match std::str::from_utf8(bytes) {
        Ok(text) => Content::Text(text.to_owned()),
        Err(_) => Content::Binary,
    }
}
fn invalid(message: &str) -> GitServiceError {
    GitServiceError::InvalidRepository(message.into())
}
/// Same private-file boundary as agent reference tools, applied to each path
/// before reading either historical blobs or worktree files.
pub fn allowed_path(path: &str) -> bool {
    !path.is_empty()
        && path.len() <= 4096
        && !path.contains(['\\', '\0', ':'])
        && path.split('/').all(|part| {
            !part.is_empty()
                && !part.starts_with('.')
                && !matches!(
                    part.to_ascii_lowercase().as_str(),
                    "credentials" | "secrets"
                )
        })
        && !matches!(
            Path::new(path)
                .extension()
                .and_then(|p| p.to_str())
                .map(str::to_ascii_lowercase)
                .as_deref(),
            Some("pem" | "key" | "p12" | "pfx")
        )
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MergeObservation {
    pub source_head: String,
    pub target_head: String,
    pub source_in_target_history: bool,
    pub tracked_changes: usize,
    pub untracked_changes: usize,
    pub conflicts: usize,
    pub operation_in_progress: bool,
}

impl GitService {
    /// Local refs and status only: never fetch, refresh the index, or execute
    /// repository-configured commands. Counts include private files, not bodies.
    pub fn reference_merge_observation(
        &self,
        root: &Path,
        source_branch: &str,
        target_branch: &str,
    ) -> Result<MergeObservation, GitServiceError> {
        let repo = Repository::open(root)?;
        if repo
            .config()?
            .get_bool("core.sparseCheckout")
            .unwrap_or(false)
        {
            return Err(invalid(
                "Sparse checkout status is not supported by this reader",
            ));
        }
        let head = self.get_head_info(root)?;
        if head.branch != source_branch {
            return Err(invalid("Workspace branch changed"));
        }
        let target_head = self.get_branch_oid(root, target_branch)?;
        let mut options = git2::StatusOptions::new();
        options
            .include_untracked(true)
            .recurse_untracked_dirs(true)
            .include_ignored(false)
            .update_index(false);
        let statuses = repo.statuses(Some(&mut options))?;
        if statuses.len() > 100_000 {
            return Err(invalid("Worktree status exceeds reference limit"));
        }
        let mut result = MergeObservation {
            source_in_target_history: self.is_ancestor(root, &head.oid, &target_head)?,
            source_head: head.oid,
            target_head,
            tracked_changes: 0,
            untracked_changes: 0,
            conflicts: 0,
            operation_in_progress: repo.state() != git2::RepositoryState::Clean,
        };
        for entry in statuses.iter() {
            let path = entry
                .path()
                .ok_or_else(|| invalid("Non-UTF-8 status path"))?;
            if path
                .split('/')
                .any(|part| utils::path::ALWAYS_SKIP_DIRS.contains(&part))
            {
                continue;
            }
            let status = entry.status();
            result.conflicts += usize::from(status.is_conflicted());
            if status.is_wt_new() {
                result.untracked_changes += 1;
            } else if status != git2::Status::CURRENT {
                result.tracked_changes += 1;
            }
        }
        let after = self.get_head_info(root)?;
        if after.oid != result.source_head
            || after.branch != source_branch
            || self.get_branch_oid(root, target_branch)? != result.target_head
        {
            return Err(invalid("Repository heads changed during inspection"));
        }
        Ok(result)
    }

    /// Inspect worktree/index changes without staging files into a temporary
    /// index: git add can execute clean filters and write objects even on reads.
    pub fn reference_diff_paths(
        &self,
        root: &Path,
        base: &Commit,
    ) -> Result<Vec<String>, GitServiceError> {
        let repo = Repository::open(root)?;
        let tree = repo.find_commit(base.as_oid())?.tree()?;
        let mut options = git2::DiffOptions::new();
        options
            .include_untracked(true)
            .recurse_untracked_dirs(true)
            .include_typechange(true);
        let diff = repo.diff_tree_to_workdir_with_index(Some(&tree), Some(&mut options))?;
        if diff.deltas().len() > 100_000 {
            return Err(invalid("Repository diff exceeds reference listing limit"));
        }
        let mut paths = Vec::new();
        for delta in diff.deltas() {
            for file in [delta.old_file(), delta.new_file()] {
                if let Some(path) = file.path() {
                    let path = path
                        .to_str()
                        .ok_or_else(|| invalid("Non-UTF-8 changed path"))?;
                    if allowed_path(path) {
                        paths.push(path.to_owned());
                    }
                }
            }
        }
        paths.sort();
        paths.dedup();
        Ok(paths)
    }

    pub fn reference_files(&self, root: &Path) -> Result<Vec<String>, GitServiceError> {
        let repo = Repository::open(root)?;
        let index = repo.index()?;
        if index.len() > 100_000 {
            return Err(invalid("Repository index exceeds reference listing limit"));
        }
        let mut paths = Vec::new();
        for entry in index.iter() {
            let path =
                std::str::from_utf8(&entry.path).map_err(|_| invalid("Non-UTF-8 tracked path"))?;
            if allowed_path(path) && matches!(entry.mode, 0o100644 | 0o100755) {
                paths.push(path.to_owned());
            }
        }
        paths.sort();
        paths.dedup();
        Ok(paths)
    }

    pub fn reference_file(&self, root: &Path, path: &str) -> Result<Content, GitServiceError> {
        if !allowed_path(path) {
            return Err(invalid("Private or invalid reference path"));
        }
        let repo = Repository::open(root)?;
        let index = repo.index()?;
        let entry = index
            .get_path(Path::new(path), 0)
            .ok_or_else(|| invalid("File is untracked or conflicted"))?;
        if !matches!(entry.mode, 0o100644 | 0o100755) {
            return Ok(Content::Unsupported);
        }
        worktree_content(root, path)
    }

    /// A per-path comparison, so renames appear as the source deletion and
    /// destination addition. Neither side can redirect to another private path.
    pub fn reference_diff(
        &self,
        root: &Path,
        base: &Commit,
        path: &str,
    ) -> Result<(Content, Content), GitServiceError> {
        if !allowed_path(path) {
            return Err(invalid("Private or invalid reference path"));
        }
        let repo = Repository::open(root)?;
        let tree = repo.find_commit(base.as_oid())?.tree()?;
        let old = match tree.get_path(Path::new(path)) {
            Ok(entry)
                if entry.kind() == Some(ObjectType::Blob)
                    && matches!(entry.filemode(), 0o100644 | 0o100755) =>
            {
                let blob = repo.find_blob(entry.id())?;
                if blob.size() > MAX_REFERENCE_BYTES {
                    Content::TooLarge
                } else {
                    content(blob.content())
                }
            }
            Ok(_) => Content::Unsupported,
            Err(error) if error.code() == git2::ErrorCode::NotFound => Content::Missing,
            Err(error) => return Err(error.into()),
        };
        Ok((old, worktree_content(root, path)?))
    }
}
fn worktree_content(root: &Path, path: &str) -> Result<Content, GitServiceError> {
    Ok(
        match utils::reference_file::read(root, Path::new(path), MAX_REFERENCE_BYTES)? {
            Some(bytes) => content(&bytes),
            None => Content::Missing,
        },
    )
}
