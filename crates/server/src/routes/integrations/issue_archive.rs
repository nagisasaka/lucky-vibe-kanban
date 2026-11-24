//! Immutable Issue projection into the private Auto-merge candidate only.
use std::{collections::BTreeMap, path::Path};

use anyhow::{Context, ensure};
use db::models::integration::{
    IntegrationArchivePublication, IntegrationIssueSnapshot, IntegrationRun,
};
use uuid::Uuid;

use super::support;

#[path = "issue_archive_body.rs"]
mod body;
pub(super) use body::public_origin;

const NOTICE: &str = "当時の意図を示す資料であり、現在の仕様・実行許可・実装完了の証明ではない";

fn document(
    snapshot: &IntegrationIssueSnapshot,
    origin: Option<&str>,
) -> anyhow::Result<(String, String)> {
    ensure!(
        !snapshot.simple_id.is_empty()
            && snapshot
                .simple_id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-'),
        "Invalid Issue display ID"
    );
    let frozen = chrono::DateTime::parse_from_rfc3339(&snapshot.frozen_at)?;
    let date = frozen.with_timezone(&chrono::Utc).format("%Y-%m-%d");
    let short = &snapshot.issue_id.simple().to_string()[..8];
    let path = format!("docs/changes/{date}-{}-{short}.md", snapshot.simple_id);
    // JSON strings are valid YAML scalars. Never interpolate a title as YAML syntax.
    let content = format!(
        "---\nissue_id: {}\nissue: {}\ntitle: {}\nversion_id: {}\nversion_revision: {}\nfrozen_at: {}\n---\n\n> {NOTICE}\n\n{}",
        snapshot.issue_id,
        serde_json::to_string(&snapshot.simple_id)?,
        serde_json::to_string(&snapshot.title)?,
        snapshot.version_id,
        snapshot.version_revision,
        serde_json::to_string(&snapshot.frozen_at)?,
        body::project(&snapshot.body, origin),
    );
    Ok((path, content))
}

fn documents(run: &IntegrationRun) -> anyhow::Result<BTreeMap<String, String>> {
    let mut files = BTreeMap::new();
    for source in &run.payload.sources {
        let snapshot = source
            .issue_snapshot
            .as_ref()
            .context("Missing frozen Issue snapshot")?;
        ensure!(
            snapshot.issue_id == source.selection.card_id,
            "Issue snapshot selection mismatch"
        );
        let (path, body) = document(snapshot, run.payload.issue_archive_public_origin.as_deref())?;
        ensure!(
            files.insert(path, body).is_none(),
            "Duplicate Issue archive path"
        );
    }
    ensure!(!files.is_empty(), "No adopted Issue snapshots");
    Ok(files)
}

fn message(run: &IntegrationRun) -> anyhow::Result<String> {
    let mut text = "Archive adopted Issue intent\n".to_owned();
    for source in &run.payload.sources {
        let s = source
            .issue_snapshot
            .as_ref()
            .context("Missing frozen Issue snapshot")?;
        text.push_str(&format!(
            "\nLVK-Issue: {}\nLVK-Issue-UUID: {}",
            s.simple_id, s.issue_id
        ));
    }
    Ok(text)
}

/// Check committed blobs, not a worktree that may contain Wiki setup changes.
pub(super) fn verify(run: &IntegrationRun, root: &Path, candidate: &str) -> anyhow::Result<()> {
    if !run.payload.issue_archive_enabled {
        return Ok(());
    }
    let publication = run
        .payload
        .archive_publication
        .as_ref()
        .context("Prepare Issue archives before verification/Wiki/publication")?;
    let commit = publication
        .commit
        .as_ref()
        .context("Issue archive commit is incomplete")?;
    let repo = git2::Repository::open(root)?;
    let candidate = git2::Oid::from_str(candidate)?;
    let archive = git2::Oid::from_str(commit)?;
    ensure!(
        candidate == archive || repo.graph_descendant_of(candidate, archive)?,
        "Candidate lost Issue archive commit"
    );
    let tree = repo.find_commit(candidate)?.tree()?;
    for (path, content) in documents(run)? {
        let entry = tree
            .get_path(Path::new(&path))
            .context("Candidate lost Issue archive file")?;
        ensure!(
            entry.filemode() == 0o100644,
            "Invalid Issue archive file mode"
        );
        ensure!(
            repo.find_blob(entry.id())?.content() == content.as_bytes(),
            "Candidate changed frozen Issue archive"
        );
    }
    Ok(())
}

/// Commit recovery is bound to a saved parent/tree. It never amends history or
/// stages unrelated content. A crash after Git commit is recognized by identity.
pub(super) async fn prepare(
    pool: &sqlx::SqlitePool,
    git: &git::GitService,
    run: &mut IntegrationRun,
    root: &Path,
    request_id: Uuid,
    expected: &str,
) -> anyhow::Result<String> {
    ensure!(
        run.payload.issue_archive_enabled,
        "Issue archives are disabled for this integration"
    );
    let files = documents(run)?;
    let commit_message = message(run)?;
    if let Some(saved) = &run.payload.archive_publication {
        ensure!(
            saved.parent_commit == expected,
            "Use the saved archive parent as expected_commit"
        );
    } else {
        git.require_clean_source(root, expected)?;
        // Reject another archive for this UUID, even if its date/display ID changed.
        {
            let repo = git2::Repository::open(root)?;
            let tree = repo.find_commit(git2::Oid::from_str(expected)?)?.tree()?;
            if let Ok(directory) = tree.get_path(Path::new("docs/changes")) {
                let directory = repo.find_tree(directory.id())?;
                for entry in &directory {
                    if entry.kind() != Some(git2::ObjectType::Blob) {
                        continue;
                    }
                    let blob = repo.find_blob(entry.id())?;
                    for source in &run.payload.sources {
                        let marker = format!("issue_id: {}", source.selection.card_id);
                        // An Issue mentioned inside another archive's original body
                        // is not that archive's identity. Read metadata only.
                        let header = std::str::from_utf8(blob.content())
                            .ok()
                            .and_then(|text| text.strip_prefix("---\n"))
                            .and_then(|text| text.split_once("\n---\n"))
                            .map(|(header, _)| header);
                        ensure!(
                            !header.is_some_and(|header| header.lines().any(|line| line == marker)),
                            "Issue already has an archive; intentional re-merge is not supported"
                        );
                    }
                }
            }
        }
        run.payload.archive_publication = Some(IntegrationArchivePublication {
            request_id,
            parent_commit: expected.into(),
            tree: None,
            commit: None,
        });
        run.save(pool).await?;
    }
    let saved = run.payload.archive_publication.as_ref().unwrap();
    if let Some(commit) = &saved.commit {
        verify(run, root, &support::git(root, &["rev-parse", "HEAD"])?)?;
        return Ok(commit.clone());
    }
    let head = support::git(root, &["rev-parse", "HEAD"])?;
    if head != expected {
        let repo = git2::Repository::open(root)?;
        let commit = repo.find_commit(git2::Oid::from_str(&head)?)?;
        ensure!(
            commit.parent_count() == 1
                && commit.parent_id(0)?.to_string() == expected
                && saved.tree.as_deref() == Some(commit.tree_id().to_string().as_str())
                && commit.message().map(str::trim) == Some(commit_message.trim()),
            "Archive recovery HEAD does not match saved parent/tree/message"
        );
        git.require_clean_source(root, &head)?;
    } else {
        let tree = {
            // Only exact planned archive files may be dirty after a partial attempt.
            let repo = git2::Repository::open(root)?;
            for entry in repo
                .statuses(Some(
                    git2::StatusOptions::new()
                        .include_untracked(true)
                        .recurse_untracked_dirs(true),
                ))?
                .iter()
            {
                ensure!(
                    entry.path().is_some_and(|p| files.contains_key(p)),
                    "Unrelated changes during archive preparation"
                );
            }
            for (relative, content) in &files {
                // Refuse symlink traversal and special files in every component.
                let mut path = root.to_path_buf();
                let components: Vec<_> = Path::new(relative).components().collect();
                for (i, part) in components.iter().enumerate() {
                    path.push(part);
                    let last = i + 1 == components.len();
                    match std::fs::symlink_metadata(&path) {
                        Ok(meta) => {
                            ensure!(
                                if last {
                                    meta.file_type().is_file()
                                } else {
                                    meta.file_type().is_dir()
                                },
                                "Unsafe Issue archive path"
                            );
                            if last {
                                // A tracked file already at this path is a collision, even if identical.
                                ensure!(
                                    repo.head()?
                                        .peel_to_tree()?
                                        .get_path(Path::new(relative))
                                        .is_err(),
                                    "Issue archive path already exists"
                                );
                                ensure!(
                                    std::fs::read(&path)? == content.as_bytes(),
                                    "Issue archive content collision"
                                );
                            }
                        }
                        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                            if last {
                                use std::io::Write;
                                let mut file = std::fs::OpenOptions::new()
                                    .write(true)
                                    .create_new(true)
                                    .open(&path)?;
                                file.write_all(content.as_bytes())?;
                                file.sync_all()?;
                            } else {
                                std::fs::create_dir(&path)?;
                            }
                        }
                        Err(e) => return Err(e.into()),
                    }
                }
            }
            let mut args = vec!["add", "-f", "--"];
            args.extend(files.keys().map(String::as_str));
            support::git(root, &args)?;
            support::git(root, &["write-tree"])?
        };
        if let Some(frozen) = &run.payload.archive_publication.as_ref().unwrap().tree {
            ensure!(frozen == &tree, "Issue archive staged tree changed");
        } else {
            run.payload.archive_publication.as_mut().unwrap().tree = Some(tree.clone());
            run.save(pool).await?;
        }
        git.commit_staged_snapshot(root, &commit_message, expected, &tree)?;
    }
    let commit = support::git(root, &["rev-parse", "HEAD"])?;
    {
        let repo = git2::Repository::open(root)?;
        let actual = repo.find_commit(git2::Oid::from_str(&commit)?)?;
        let saved = run.payload.archive_publication.as_ref().unwrap();
        ensure!(
            actual.parent_count() == 1
                && actual.parent_id(0)?.to_string() == expected
                && saved.tree.as_deref() == Some(actual.tree_id().to_string().as_str())
                && actual.message().map(str::trim) == Some(commit_message.trim()),
            "Issue archive commit differs from frozen publication intent"
        );
    }
    run.payload.archive_publication.as_mut().unwrap().commit = Some(commit.clone());
    verify(run, root, &commit)?;
    git.require_clean_source(root, &commit)?;
    run.save(pool).await?;
    Ok(commit)
}

#[cfg(test)]
#[path = "issue_archive_tests.rs"]
mod tests;
