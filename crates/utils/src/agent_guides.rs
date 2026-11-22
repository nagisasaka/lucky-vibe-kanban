//! On-demand host guidance, readable without an LVK MCP connection.
use std::{
    fs,
    hash::{Hash, Hasher},
    io::{self, Write},
    path::{Path, PathBuf},
};

pub const PARALLEL_GUIDE: &str = "Use CURRENT discovery_url for public repository activity. Follow next_workspace as after_workspace and manifests.next_after as after_manifest only for needed ranges; errors are not zero activity. Read pinned source at /api/repos/{repository_id}/snapshot?commit=<full-oid>&path=<url-encoded-path>; omit path to enumerate, follow next_after as after_path. Manifest references use url; GET that repository-scoped /api/repos/{repository_id}/change-manifests/{event_id} URL to read the DB record (with the same authentication). Do not treat it as a filesystem path. For a test-only combination POST /api/workspaces/{workspace_id}/integration/preview with repository_id, base_commit (clean full HEAD), peers [{workspace_id,commit}]. Only combine in its returned detached trial path. Preview is not formal publication or Auto-merge.";

/// Resolve the instance token location without creating or reading it.
pub fn parallel_guide() -> io::Result<String> {
    let token_path = crate::assets::asset_dir_path()?.join("access-internal.token");
    Ok(format!(
        "{PARALLEL_GUIDE}\n\nFor native HTTP to the LVK loopback address, read the instance token from {} into memory and send it as the X-LVK-Internal-Token request header. Use it only for this instance's loopback HTTP endpoints; do not send it externally or follow redirects. Never expose the token in instructions, logs, URLs, or command arguments.",
        token_path.display()
    ))
}

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
        parallel_guide()?
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
    fn parallel_guide_does_not_create_instance_data() {
        if std::env::var_os("LVK_PARALLEL_GUIDE_TEST_CHILD").is_some() {
            let instance = PathBuf::from(std::env::var_os("LVK_INSTANCE_DIR").unwrap());
            let guide = parallel_guide().unwrap();
            assert!(
                guide.contains(
                    instance
                        .join("data/access-internal.token")
                        .to_str()
                        .unwrap()
                )
            );
            assert!(!instance.join("data").exists());
            return;
        }
        let root = tempfile::tempdir().unwrap();
        let result = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "agent_guides::tests::parallel_guide_does_not_create_instance_data",
            ])
            .env("LVK_INSTANCE_DIR", root.path())
            .env("LVK_PARALLEL_GUIDE_TEST_CHILD", "1")
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert!(!root.path().join("data").exists());
    }
    #[test]
    fn guide_is_reused_without_overwriting_unknown_content() {
        let root = tempfile::tempdir().unwrap();
        let path = prepare(root.path()).unwrap();
        let guide = fs::read_to_string(&path).unwrap();
        assert!(guide.contains("## Parallel discovery"));
        assert!(guide.contains("## Coding and Manifest"));
        assert!(guide.contains("X-LVK-Internal-Token"));
        assert!(
            guide.contains(
                &crate::assets::asset_dir_path()
                    .unwrap()
                    .join("access-internal.token")
                    .display()
                    .to_string()
            )
        );
        assert!(!guide.contains("resource-coordination"));
        assert!(!guide.contains("execution-bridge"));
        assert_eq!(path, prepare(root.path()).unwrap());
        fs::write(&path, "user content").unwrap();
        assert!(prepare(root.path()).is_err());
        assert_eq!(fs::read_to_string(path).unwrap(), "user content");
    }
}
