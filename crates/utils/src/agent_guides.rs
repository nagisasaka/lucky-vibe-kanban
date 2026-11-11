//! On-demand host guidance, readable without an LVK MCP connection.
use std::{
    fs,
    hash::{Hash, Hasher},
    io::{self, Write},
    path::{Path, PathBuf},
};

pub const PARALLEL_GUIDE: &str = "Use CURRENT discovery_url for public repository activity. Follow next_workspace as after_workspace and manifests.next_after as after_manifest only for needed ranges; errors are not zero activity. Read pinned source at /api/repos/{repository_id}/snapshot?commit=<full-oid>&path=<url-encoded-path>; omit path to enumerate, follow next_after as after_path. For a test-only combination POST /api/workspaces/{workspace_id}/integration/preview with repository_id, base_commit (clean full HEAD), peers [{workspace_id,commit}]. Only combine in its returned detached trial path. Preview is not formal publication or Auto-merge.";

pub fn resource_guide() -> String {
    let token = crate::assets::asset_dir().join("access-internal.token");
    format!(
        "Prefer stdio MCP resource and execution-bridge tools, whose contracts and schemas describe atomic claims, waiting, recovery and Runner finish. For raw loopback HTTP only, read {} and send X-LVK-Internal-Token in memory; never print it, put it in URLs/command arguments, send it to other hosts or follow redirects. This capability cannot manage server authentication. Resource API: /api/resource-coordination (snapshot, operations, operations/{{id}}?wait_seconds=25, operations/{{id}}/events and operations/{{id}}/cancel). Runner API: /api/execution-bridge/runners, /sources and /operations/{{id}}/commands. Capture an immutable source with session_id and repository working_dir; select runner_id explicitly and claim its execution slot and every device in the same bundle. Runner working_dir is snapshot-relative; Windows uses PowerShell. Interactive commands require stable id, kind step or finish, and script. Finish runs cleanup and verification; disconnect/cancel/timeout do not prove remote termination. Do not complete with an open operation; only the operator confirms recovery after settled containment.",
        token.display()
    ) + " Before access GET /api/resource-coordination/snapshot. Use one canonical registered physical identity; never bypass its owner or acquire incrementally. POST /operations with request_id (stable UUID, same contents on retry), session_id, purpose, claims [{resource_id,expected_revision,resulting_state:null or verified new state}], script, verification_script, working_dir (workspace-relative locally), timeout_seconds (1–3600), and optional runner {runner_id,source_id,interactive,desktop,cleanup_script}. Include the whole critical section and cleanup in one foreground script; do not detach work. Independently verify idle and resulting state. Wait for the atomic bundle; queued is not completion. Inspect events and actual compatibility; do not blindly replace revisions. Failure/restart/timeout retains ownership for operator recovery; never claim it free. Finish/cancel outstanding operations before completion."
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
        "# LVK on-demand execution guide\n\n## Coding and Manifest\n{}\n\n## Parallel discovery\n{}\n\n## Shared resources and Runner\n{}\n\n## LVK product help\nThe image ships public LVK source and ancestor history at /opt/lvk/source (server override LVK_SOURCE_PATH). It is reference material, separate from the running binary. query_repository with source:\"lvk\" and an operation reads the distribution commit without creating a Project; omit target for that commit, or select an explicit commit for history. Existing Project reads keep their IDs and authorization. For conversational general help use ask_help / get_help_conversation when exposed. Check returned build_revision and source commit; missing source or unknown revision is not success. Cite actual source/Wiki/commit/diff, distinguish author quotations from your interpretation, and never invent contribution rules. User repositories and secrets require their own authorization. Do not edit/build/restart LVK or start development merely because help was requested. For development, propose the normal Project/Issue/Workspace flow; an independent clone needs explicit instruction. GitHub remains an optional source; distinguish its version from the running build.\n",
        crate::repository_memory::CODING_RUN_GUIDE,
        PARALLEL_GUIDE,
        resource_guide()
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
        assert_eq!(path, prepare(root.path()).unwrap());
        fs::write(&path, "user content").unwrap();
        assert!(prepare(root.path()).is_err());
        assert_eq!(fs::read_to_string(path).unwrap(), "user content");
    }
}
