//! On-demand host guidance, readable without an LVK MCP connection.
use std::{
    fs,
    hash::{Hash, Hasher},
    io::{self, Write},
    path::{Path, PathBuf},
};

pub const PARALLEL_GUIDE: &str = "Use CURRENT discovery_url for public repository activity. Follow next_workspace as after_workspace and manifests.next_after as after_manifest only for needed ranges; errors are not zero activity. Read pinned source at /api/repos/{repository_id}/snapshot?commit=<full-oid>&path=<url-encoded-path>; omit path to enumerate, follow next_after as after_path. Manifest references use url; GET that repository-scoped /api/repos/{repository_id}/change-manifests/{event_id} URL to read the DB record (with the same authentication). Do not treat it as a filesystem path. For a test-only combination POST /api/workspaces/{workspace_id}/integration/preview with repository_id, base_commit (clean full HEAD), peers [{workspace_id,commit}]. Only combine in its returned detached trial path. Preview is not formal publication or Auto-merge.";

/// Reuse the existing ignored attachments namespace and provider-readable cwd.
/// Content-addressed immutable files need no per-Run retention or registry.
pub fn prepare(workspace: &Path) -> io::Result<PathBuf> {
    let lvk = crate::lvk_directory::LvkDirectory::open(workspace)?;
    lvk.ensure_runtime_ignores()?;
    let directory = lvk.anchored_path().join("attachments");
    fs::create_dir_all(&directory)?;
    let meta = fs::symlink_metadata(&directory)?;
    if !meta.is_dir() || meta.file_type().is_symlink() {
        return Err(io::Error::other(
            "Guide directory is not a regular directory",
        ));
    }
    let content = format!(
        "# LVK on-demand execution guide\n\n## Coding and Manifest\n{}\n\n## Parallel discovery\n{}\n\n## LVK product help\nThe image ships public LVK source and ancestor history at /opt/lvk/source (server override LVK_SOURCE_PATH). It is reference material, separate from the running binary. query_repository with source:\"lvk\" and an operation reads the distribution commit without creating a Project; omit target for that commit, or select an explicit commit for history. Existing Project reads keep their IDs and authorization. For conversational general help use ask_help / get_help_conversation when exposed. Check returned build_revision and source commit; missing source or unknown revision is not success. Cite actual source/Wiki/commit/diff, distinguish author quotations from your interpretation, and never invent contribution rules. User repositories and secrets require their own authorization. Do not edit/build/restart LVK or start development merely because help was requested. For development, propose the normal Project/Issue/Workspace flow; an independent clone needs explicit instruction. GitHub remains an optional source; distinguish its version from the running build.\n",
        crate::repository_memory::CODING_RUN_GUIDE,
        PARALLEL_GUIDE
    );
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    content.hash(&mut hash);
    let name = format!("lvk-agent-guide-{:016x}.md", hash.finish());
    let path = directory.join(&name);
    if !path.try_exists()? {
        let mut file = tempfile::NamedTempFile::new_in(&directory)?;
        file.write_all(content.as_bytes())?;
        match file.persist_noclobber(&path) {
            Ok(_) => {}
            Err(error) if error.error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error.error),
        }
    }
    let meta = fs::symlink_metadata(&path)?;
    if !meta.is_file() || meta.file_type().is_symlink() || fs::read_to_string(&path)? != content {
        return Err(io::Error::other(
            "Existing guide is not the expected host content",
        ));
    }
    lvk.verify()?;
    Ok(fs::canonicalize(workspace)?
        .join(".lvk/attachments")
        .join(name))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn guide_is_reused_without_overwriting_unknown_content() {
        let root = tempfile::tempdir().unwrap();
        let path = prepare(root.path()).unwrap();
        let guide = fs::read_to_string(&path).unwrap();
        assert!(guide.contains("## Parallel discovery"));
        assert!(guide.contains("## Coding and Manifest"));
        assert!(!guide.contains("resource-coordination"));
        assert!(!guide.contains("execution-bridge"));
        assert_eq!(path, prepare(root.path()).unwrap());
        fs::write(&path, "user content").unwrap();
        assert!(prepare(root.path()).is_err());
        assert_eq!(fs::read_to_string(path).unwrap(), "user content");
    }
}
