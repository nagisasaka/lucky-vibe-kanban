//! Explicit repository publication. No checkout, upstream, tracking or config mutation.
use std::path::Path;

use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{GitCli, GitCliError};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS, schemars::JsonSchema)]
pub struct PushObservation {
    pub remote: String,
    pub remote_url: String,
    pub branch: String,
    pub source_sha: String,
    pub destination_branch: String,
    pub remote_sha: Option<String>,
    pub unpublished_commits: u32,
    pub issue_archive_paths: Vec<String>,
}

/// Only one configured GitHub push URL, with no rewrite or mirror ambiguity.
/// Local filesystem URLs are allowed only by the test entry, never by HTTP/MCP.
fn target(root: &Path, name: &str, local_fixture: bool) -> Result<String> {
    ensure!(!name.is_empty() && !name.starts_with('-'), "invalid_remote");
    let repo = git2::Repository::open(root)?;
    let cfg = repo.config()?;
    let mut entries = cfg.entries(None)?;
    while let Some(entry) = entries.next() {
        let entry = entry?;
        let key = entry.name().unwrap_or_default().to_ascii_lowercase();
        ensure!(
            !(key.starts_with("url.")
                && (key.ends_with(".insteadof") || key.ends_with(".pushinsteadof"))),
            "url_rewrite_unsupported"
        );
    }
    ensure!(
        !cfg.get_bool(&format!("remote.{name}.mirror"))
            .unwrap_or(false),
        "mirror_unsupported"
    );
    let remote = repo.find_remote(name)?;
    let key = format!("remote.{name}.pushurl");
    let mut values = Vec::new();
    if let Ok(mut urls) = cfg.multivar(&key, None) {
        while let Some(entry) = urls.next() {
            values.push(entry?.value().unwrap_or_default().to_owned());
        }
    }
    let url = if values.is_empty() {
        let mut urls = cfg.multivar(&format!("remote.{name}.url"), None)?;
        while let Some(entry) = urls.next() {
            values.push(entry?.value().unwrap_or_default().to_owned());
        }
        ensure!(values.len() == 1, "multiple_remote_urls");
        remote.url().unwrap_or_default().to_owned()
    } else {
        ensure!(values.len() == 1, "multiple_push_urls");
        values.remove(0)
    };
    let github = (url.starts_with("https://github.com/")
        || url.starts_with("git@github.com:")
        || url.starts_with("ssh://git@github.com/"))
        && utils::github_credentials::Repository::parse(&url).is_some()
        && !url.contains("/pull/");
    ensure!(
        github || (local_fixture && Path::new(&url).is_absolute()),
        "configured_github_remote_required"
    );
    Ok(url)
}

fn source(root: &Path, branch: &str) -> Result<String> {
    ensure!(
        git2::Branch::name_is_valid(branch).unwrap_or(false),
        "invalid_branch"
    );
    let repo = git2::Repository::open(root)?;
    let reference = repo.find_reference(&format!("refs/heads/{branch}"))?;
    ensure!(
        reference.symbolic_target().is_none(),
        "symbolic_source_unsupported"
    );
    let oid = reference
        .target()
        .ok_or_else(|| anyhow::anyhow!("invalid_source"))?;
    Ok(repo.find_commit(oid)?.id().to_string())
}

pub fn observe(root: &Path, remote: &str, branch: &str) -> Result<PushObservation> {
    observe_inner(root, remote, branch, false)
}
fn observe_inner(
    root: &Path,
    remote: &str,
    branch: &str,
    fixture: bool,
) -> Result<PushObservation> {
    let url = target(root, remote, fixture)?;
    let sha = source(root, branch)?;
    let remote_sha = GitCli::new().remote_branch_sha(root, &url, branch)?;
    let repo = git2::Repository::open(root)?;
    let mut walk = repo.revwalk()?;
    walk.push(git2::Oid::from_str(&sha)?)?;
    if let Some(ref remote_sha) = remote_sha {
        let oid = git2::Oid::from_str(remote_sha)?;
        ensure!(repo.find_commit(oid).is_ok(), "remote_history_unavailable");
        walk.hide(oid)?;
    }
    let mut count = 0u32;
    let mut paths = std::collections::BTreeSet::new();
    for oid in walk {
        count = count.saturating_add(1);
        ensure!(count <= 10_000, "preview_history_limit");
        let commit = repo.find_commit(oid?)?;
        if let Ok(tree) = commit.tree()?.get_path(Path::new("docs/changes"))
            && let Ok(tree) = repo.find_tree(tree.id())
        {
            let walked = tree.walk(git2::TreeWalkMode::PreOrder, |prefix, entry| {
                if let Some(name) = entry.name() {
                    paths.insert(format!("docs/changes/{prefix}{name}"));
                }
                if paths.len() > 200 {
                    git2::TreeWalkResult::Abort
                } else {
                    git2::TreeWalkResult::Ok
                }
            });
            ensure!(paths.len() <= 200, "preview_archive_limit");
            walked?;
        }
    }
    ensure!(paths.len() <= 200, "preview_archive_limit");
    ensure!(
        source(root, branch)? == sha && target(root, remote, fixture)? == url,
        "source_drift"
    );
    Ok(PushObservation {
        remote: remote.into(),
        remote_url: url,
        branch: branch.into(),
        source_sha: sha,
        destination_branch: branch.into(),
        remote_sha,
        unpublished_commits: count,
        issue_archive_paths: paths.into_iter().collect(),
    })
}

/// Safe classifications only; Git output (including URLs) never leaves this boundary.
pub fn failure(error: &GitCliError) -> &'static str {
    let message = error.to_string().to_ascii_lowercase();
    if message.contains("authentication failed")
        || message.contains("could not read username")
        || message.contains("invalid username or password")
        || message.contains("cannot read configured github token")
    {
        "authentication_rejected"
    } else if message.contains("protected branch")
        || message.contains("gh006")
        || message.contains("gh013")
        || message.contains("repository rule")
    {
        "protected_branch"
    } else if message.contains("permission")
        || message.contains("403")
        || message.contains("access denied")
        || message.contains("write access")
    {
        "permission_denied"
    } else if message.contains("non-fast-forward") || message.contains("fetch first") {
        "non_fast_forward"
    } else if message.contains("[remote rejected]") {
        "remote_rejected"
    } else {
        "unknown"
    }
}

pub fn publish(root: &Path, expected: &PushObservation) -> &'static str {
    publish_inner(root, expected, false)
}
fn publish_inner(root: &Path, expected: &PushObservation, fixture: bool) -> &'static str {
    let Ok(repo) = git2::Repository::open(root) else {
        return "target_changed";
    };
    let Ok(mut lock) = repo.transaction() else {
        return "source_busy";
    };
    if lock
        .lock_ref(&format!("refs/heads/{}", expected.branch))
        .is_err()
    {
        return "source_busy";
    }
    if source(root, &expected.branch).as_deref().ok() != Some(&expected.source_sha)
        || expected.destination_branch != expected.branch
    {
        return "source_drift";
    }
    if target(root, &expected.remote, fixture).as_deref().ok() != Some(&expected.remote_url) {
        return "target_changed";
    }
    // Re-observe destination just before publication. This is not a force-with-lease;
    // the server still enforces an ordinary fast-forward under concurrent writers.
    match GitCli::new().remote_branch_sha(root, &expected.remote_url, &expected.branch) {
        Ok(sha) if sha == expected.remote_sha => {}
        Ok(_) => return "remote_drift",
        Err(error) => {
            return match failure(&error) {
                "unknown" => "remote_unavailable",
                code => code,
            };
        }
    }
    match GitCli::new().push_exact(
        root,
        &expected.remote_url,
        &expected.source_sha,
        &expected.branch,
    ) {
        Ok(()) => {
            match GitCli::new().remote_branch_sha(root, &expected.remote_url, &expected.branch) {
                Ok(Some(sha)) if sha == expected.source_sha => "pushed",
                _ => "unknown",
            }
        }
        Err(error) => failure(&error),
    }
}

pub fn verify(root: &Path, expected: &PushObservation) -> Result<Option<String>> {
    ensure!(
        target(root, &expected.remote, false)? == expected.remote_url,
        "target_changed"
    );
    Ok(
        GitCli::new().remote_branch_sha(
            root,
            &expected.remote_url,
            &expected.destination_branch,
        )?,
    )
}

/// All returned codes are fixed strings, never raw subprocess diagnostics.
pub fn error_code(error: &anyhow::Error) -> &'static str {
    if let Some(error) = error.downcast_ref::<GitCliError>() {
        return failure(error);
    }
    match error.to_string().as_str() {
        "source_drift" => "source_drift",
        "remote_drift" => "remote_drift",
        "target_changed" => "target_changed",
        "authentication_rejected" => "authentication_rejected",
        "permission_denied" => "permission_denied",
        "protected_branch" => "protected_branch",
        "non_fast_forward" => "non_fast_forward",
        "unknown" => "unknown",
        "preview_history_limit" => "preview_history_limit",
        "remote_history_unavailable" => "remote_history_unavailable",
        "preview_archive_limit" => "preview_archive_limit",
        _ => "target_unavailable",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn commit(repo: &git2::Repository, message: &str, parent: Option<git2::Oid>) -> git2::Oid {
        let sig = git2::Signature::now("Fixture", "fixture@example.invalid").unwrap();
        let mut builder = repo.treebuilder(None).unwrap();
        builder
            .insert("file", repo.blob(message.as_bytes()).unwrap(), 0o100644)
            .unwrap();
        let tree = repo.find_tree(builder.write().unwrap()).unwrap();
        let parents: Vec<_> = parent
            .map(|id| repo.find_commit(id).unwrap())
            .into_iter()
            .collect();
        repo.commit(
            Some("refs/heads/main"),
            &sig,
            &sig,
            message,
            &tree,
            &parents.iter().collect::<Vec<_>>(),
        )
        .unwrap()
    }
    #[test]
    fn fixture_exact_push_drift_non_ff_and_readback_preserve_workspace() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("source");
        let remote_path = temp.path().join("remote.git");
        let repo = git2::Repository::init(&path).unwrap();
        let bare = git2::Repository::init_bare(&remote_path).unwrap();
        repo.remote("origin", remote_path.to_str().unwrap())
            .unwrap();
        repo.set_head("refs/heads/main").unwrap();
        let a = commit(&repo, "a", None);
        let observation = observe_inner(&path, "origin", "main", true).unwrap();
        assert!(observe(&path, "origin", "main").is_err()); // HTTP cannot publish fixtures/URLs.
        assert_eq!(publish_inner(&path, &observation, true), "pushed");
        assert_eq!(bare.refname_to_id("refs/heads/main").unwrap(), a);
        assert!(repo.find_reference("refs/remotes/origin/main").is_err());
        assert!(
            repo.config()
                .unwrap()
                .get_string("branch.main.remote")
                .is_err()
        );
        let b = commit(&repo, "b", Some(a));
        assert_eq!(publish_inner(&path, &observation, true), "source_drift");
        let observation = observe_inner(&path, "origin", "main", true).unwrap();
        assert_eq!(publish_inner(&path, &observation, true), "pushed");
        repo.reference("refs/heads/main", a, true, "fixture rewind")
            .unwrap();
        let observation = observe_inner(&path, "origin", "main", true).unwrap();
        assert_eq!(publish_inner(&path, &observation, true), "non_fast_forward");
        assert_eq!(bare.refname_to_id("refs/heads/main").unwrap(), b);
        repo.reference("refs/heads/main", b, true, "fixture restore")
            .unwrap();
        let observation = observe_inner(&path, "origin", "main", true).unwrap();
        bare.reference("refs/heads/main", a, true, "fixture remote drift")
            .unwrap();
        assert_eq!(publish_inner(&path, &observation, true), "remote_drift");
        repo.branch("dev", &repo.find_commit(b).unwrap(), false)
            .unwrap();
        let dev = observe_inner(&path, "origin", "dev", true).unwrap();
        assert_eq!(dev.destination_branch, "dev");
        assert_eq!(publish_inner(&path, &dev, true), "pushed");
        assert_eq!(bare.refname_to_id("refs/heads/dev").unwrap(), b);
    }
    #[cfg(unix)]
    #[test]
    fn fixture_remote_rejections_are_distinct_and_never_force() {
        use std::os::unix::fs::PermissionsExt;
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("source");
        let remote = temp.path().join("remote.git");
        let repo = git2::Repository::init(&root).unwrap();
        let bare = git2::Repository::init_bare(&remote).unwrap();
        repo.remote("origin", remote.to_str().unwrap()).unwrap();
        commit(&repo, "source", None);
        let hook = remote.join("hooks/pre-receive");
        for (message, expected) in [
            ("Authentication failed", "authentication_rejected"),
            ("Permission denied 403", "permission_denied"),
            ("GH006 protected branch", "protected_branch"),
        ] {
            std::fs::write(
                &hook,
                format!("#!/bin/sh\necho '{}' >&2\nexit 1\n", message),
            )
            .unwrap();
            std::fs::set_permissions(&hook, std::fs::Permissions::from_mode(0o700)).unwrap();
            let observation = observe_inner(&root, "origin", "main", true).unwrap();
            assert_eq!(publish_inner(&root, &observation, true), expected);
            assert!(bare.find_reference("refs/heads/main").is_err());
        }
    }
    #[test]
    fn reject_ambiguous_targets_and_classify_without_leaking_output() {
        let temp = tempfile::tempdir().unwrap();
        let repo = git2::Repository::init(temp.path()).unwrap();
        repo.remote("origin", "https://github.com/example/repo.git")
            .unwrap();
        assert_eq!(
            target(temp.path(), "origin", false).unwrap(),
            "https://github.com/example/repo.git"
        );
        repo.config()
            .unwrap()
            .set_bool("remote.origin.mirror", true)
            .unwrap();
        assert!(target(temp.path(), "origin", false).is_err());
        repo.config()
            .unwrap()
            .set_bool("remote.origin.mirror", false)
            .unwrap();
        repo.config()
            .unwrap()
            .set_str(
                "url.https://other.invalid/.pushInsteadOf",
                "https://github.com/",
            )
            .unwrap();
        assert!(target(temp.path(), "origin", false).is_err());
        for (text, code) in [
            ("Authentication failed SECRET", "authentication_rejected"),
            ("403 Permission denied SECRET", "permission_denied"),
            ("GH006 protected branch SECRET", "protected_branch"),
            ("GH013 repository rule SECRET", "protected_branch"),
            ("non-fast-forward SECRET", "non_fast_forward"),
            ("connection reset SECRET", "unknown"),
        ] {
            assert_eq!(failure(&GitCliError::CommandFailed(text.into())), code);
        }
    }
}
