//! Immutable Issue projection into the private Auto-merge candidate only.
use std::{collections::BTreeMap, path::Path};

use anyhow::{Context, ensure};
use db::models::integration::{
    IntegrationArchivePublication, IntegrationIssueSnapshot, IntegrationRun,
};
use uuid::Uuid;

use super::support;

const NOTICE: &str = "当時の意図を示す資料であり、現在の仕様・実行許可・実装完了の証明ではない";

fn document(snapshot: &IntegrationIssueSnapshot) -> anyhow::Result<(String, String)> {
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
        snapshot.body,
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
        let (path, body) = document(snapshot)?;
        ensure!(
            files.insert(path, body).is_none(),
            "Duplicate Issue archive path"
        );
    }
    ensure!(!files.is_empty(), "No adopted Issue snapshots");
    Ok(files)
}

/// Select the most recently introduced current blob in candidate ancestry.
/// Topological order keeps later saves ahead of their ancestors even across merges.
fn latest_archive(
    repo: &git2::Repository,
    parent: &str,
    issue: Uuid,
) -> anyhow::Result<Option<(String, String, String)>> {
    let commit = repo.find_commit(git2::Oid::from_str(parent)?)?;
    let tree = commit.tree()?;
    let mut matches = BTreeMap::new();
    if let Ok(directory) = tree.get_path(Path::new("docs/changes")) {
        for entry in &repo.find_tree(directory.id())? {
            if entry.kind() != Some(git2::ObjectType::Blob) {
                continue;
            }
            let blob = repo.find_blob(entry.id())?;
            let Ok(text) = std::str::from_utf8(blob.content()) else {
                continue;
            };
            let Some((header, rest)) = text
                .strip_prefix("---\n")
                .and_then(|s| s.split_once("\n---\n"))
            else {
                continue;
            };
            let Ok(metadata) = serde_yaml::from_str::<serde_yaml::Value>(header) else {
                continue;
            };
            if metadata
                .get("issue_id")
                .and_then(|v| v.as_str())
                .and_then(|id| Uuid::parse_str(id).ok())
                != Some(issue)
            {
                continue;
            }
            ensure!(
                entry.filemode() == 0o100644,
                "Invalid Issue archive file mode"
            );
            let body = rest
                .strip_prefix(&format!("\n> {NOTICE}\n\n"))
                .context("Invalid existing Issue archive body envelope")?;
            let path = format!(
                "docs/changes/{}",
                entry.name().context("Invalid archive path")?
            );
            matches.insert(path, (entry.id(), text.to_owned(), body.to_owned()));
        }
    }
    if matches.is_empty() {
        return Ok(None);
    }
    let mut history = repo.revwalk()?;
    history.set_sorting(git2::Sort::TOPOLOGICAL)?;
    history.push(commit.id())?;
    for oid in history {
        let commit = repo.find_commit(oid?)?;
        let current = commit.tree()?;
        let parents = commit
            .parents()
            .map(|p| p.tree())
            .collect::<Result<Vec<_>, _>>()?;
        let introduced: Vec<_> = matches
            .iter()
            .filter(|(path, (oid, _, _))| {
                current
                    .get_path(Path::new(path))
                    .is_ok_and(|e| e.id() == *oid)
                    && !parents
                        .iter()
                        .any(|t| t.get_path(Path::new(path)).is_ok_and(|e| e.id() == *oid))
            })
            .collect();
        ensure!(introduced.len() <= 1, "Ambiguous latest Issue archive");
        if let Some((path, (_, content, body))) = introduced.first() {
            return Ok(Some(((*path).clone(), content.clone(), body.clone())));
        }
    }
    anyhow::bail!("Missing Issue archive history")
}

/// The saved parent SHA and admitted snapshots are immutable plan inputs.
/// Recomputing from those objects also supports receipts written by older hosts.
fn planned_documents(
    run: &IntegrationRun,
    root: &Path,
    parent: &str,
) -> anyhow::Result<BTreeMap<String, String>> {
    let mut files = documents(run)?; // retain all snapshot/selection validation
    let repo = git2::Repository::open(root)?;
    for source in &run.payload.sources {
        let snapshot = source.issue_snapshot.as_ref().unwrap();
        let (path, content) = document(snapshot)?;
        if let Some((previous_path, previous_content, body)) =
            latest_archive(&repo, parent, snapshot.issue_id)?
        {
            files.remove(&path);
            let (path, content) = if body == snapshot.body {
                (previous_path, previous_content)
            } else {
                (
                    format!("{}-{}.md", path.trim_end_matches(".md"), run.id),
                    content,
                )
            };
            ensure!(
                files.insert(path, content).is_none(),
                "Duplicate Issue archive path"
            );
        }
    }
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
    for (path, content) in planned_documents(run, root, &publication.parent_commit)? {
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
    let commit_message = message(run)?;
    if let Some(saved) = &run.payload.archive_publication {
        ensure!(
            saved.parent_commit == expected,
            "Use the saved archive parent as expected_commit"
        );
    } else {
        git.require_clean_source(root, expected)?;
        planned_documents(run, root, expected)?;
        run.payload.archive_publication = Some(IntegrationArchivePublication {
            request_id,
            parent_commit: expected.into(),
            tree: None,
            commit: None,
        });
        run.save(pool).await?;
    }
    let planned = planned_documents(run, root, expected)?;
    let files = {
        let repo = git2::Repository::open(root)?;
        let tree = repo.find_commit(git2::Oid::from_str(expected)?)?.tree()?;
        let mut files = BTreeMap::new();
        for (path, content) in planned {
            if let Ok(entry) = tree.get_path(Path::new(&path)) {
                ensure!(
                    entry.filemode() == 0o100644
                        && repo.find_blob(entry.id())?.content() == content.as_bytes(),
                    "Issue archive path already exists"
                );
                // Only the selected previous archive may be reused, never a new path collision.
                ensure!(
                    run.payload.sources.iter().any(|source| {
                        latest_archive(&repo, expected, source.selection.card_id)
                            .ok()
                            .flatten()
                            .is_some_and(|(old, _, body)| {
                                old == path
                                    && source
                                        .issue_snapshot
                                        .as_ref()
                                        .is_some_and(|s| s.body == body)
                            })
                    }),
                    "Issue archive path already exists"
                );
            } else {
                files.insert(path, content);
            }
        }
        files
    };
    let saved = run.payload.archive_publication.as_ref().unwrap();
    if let Some(commit) = &saved.commit {
        verify(run, root, &support::git(root, &["rev-parse", "HEAD"])?)?;
        return Ok(commit.clone());
    }
    if files.is_empty() {
        git.require_clean_source(root, expected)?;
        run.payload.archive_publication.as_mut().unwrap().commit = Some(expected.into());
        verify(run, root, expected)?;
        run.save(pool).await?;
        return Ok(expected.into());
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
