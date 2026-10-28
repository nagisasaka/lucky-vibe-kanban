mod common;
use std::{fs, path::Path};

use common::FixtureIdentity;
use git::{GitCli, GitService};
use tempfile::TempDir;

fn command(path: &Path, args: &[&str]) {
    GitCli::new().git(path, args.iter().copied()).unwrap();
}

#[test]
fn summary_diff_counts_committed_uncommitted_untracked_and_unknown_binary() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("repo");
    let git = GitService::new();
    git.initialize_fixture(&path).unwrap();
    command(&path, &["config", "user.name", "Fixture"]);
    command(&path, &["config", "user.email", "fixture@example.invalid"]);
    fs::write(path.join("text-to-binary"), "before\n").unwrap();
    fs::write(path.join("binary-to-text"), b"before\0").unwrap();
    fs::write(path.join("deleted"), "removed\n").unwrap();
    git.commit(&path, "base").unwrap();
    command(&path, &["checkout", "-b", "feature"]);
    fs::write(path.join("committed"), "committed\n").unwrap();
    git.commit(&path, "feature change").unwrap();
    fs::write(path.join("untracked"), "new\nline\n").unwrap();
    fs::write(path.join("text-to-binary"), b"after\0").unwrap();
    fs::write(path.join("binary-to-text"), "after\n").unwrap();
    fs::remove_file(path.join("deleted")).unwrap();
    let base = git.get_base_commit(&path, "feature", "main").unwrap();
    let classified = git.get_diffs_with_binary_flags(&path, &base, None).unwrap();
    for (diff, binary) in &classified {
        let name = diff
            .new_path
            .as_deref()
            .or(diff.old_path.as_deref())
            .unwrap();
        assert_eq!(
            *binary,
            matches!(name, "text-to-binary" | "binary-to-text"),
            "{name}"
        );
    }
    let diffs = git.get_diffs(&path, &base, None).unwrap();
    assert_eq!(diffs.len(), 5);
    for (name, expected) in [
        ("committed", (Some(1), Some(0))),
        ("untracked", (Some(2), Some(0))),
        ("deleted", (Some(0), Some(1))),
        ("text-to-binary", (None, None)),
        ("binary-to-text", (None, None)),
    ] {
        let diff = diffs
            .iter()
            .find(|d| d.new_path.as_deref().or(d.old_path.as_deref()) == Some(name))
            .unwrap();
        assert_eq!((diff.additions, diff.deletions), expected, "{name}");
    }
}

#[test]
fn unknown_text_is_not_classified_as_binary() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("repo");
    let git = GitService::new();
    git.initialize_fixture(&path).unwrap();
    command(&path, &["config", "user.name", "Fixture"]);
    command(&path, &["config", "user.email", "fixture@example.invalid"]);
    fs::write(path.join("base"), "base\n").unwrap();
    git.commit(&path, "base").unwrap();
    command(&path, &["checkout", "-b", "feature"]);
    fs::write(path.join("invalid-utf8"), [0xff, 0xfe]).unwrap();
    fs::write(path.join("large-text"), vec![b'a'; 2 * 1024 * 1024 + 1]).unwrap();
    fs::write(path.join("binary"), b"binary\0").unwrap();
    let base = git.get_base_commit(&path, "feature", "main").unwrap();
    assert!(
        git.get_diffs_with_binary_flags(&path.join("missing"), &base, None)
            .is_err()
    );
    let diffs = git.get_diffs_with_binary_flags(&path, &base, None).unwrap();
    assert_eq!(diffs.len(), 3);
    for (diff, binary) in diffs {
        assert_eq!((diff.additions, diff.deletions), (None, None));
        assert_eq!(binary, diff.new_path.as_deref() == Some("binary"));
    }
}
