use super::*;

fn fixture() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    git(dir.path(), &["init", "-b", "main"]).unwrap();
    git(
        dir.path(),
        &[
            "-c",
            "user.name=Fixture",
            "-c",
            "user.email=fixture@example.test",
            "commit",
            "--allow-empty",
            "-m",
            "base",
        ],
    )
    .unwrap();
    dir
}

fn save(root: &Path, revision: &str, content: &str) -> Result<ValidationSettings, ApiError> {
    save_settings(
        root,
        SaveValidationSettings {
            expected_revision: revision.into(),
            content: content.into(),
        },
    )
}

#[test]
fn legacy_json_validation_and_multiline_commands() {
    assert_eq!(
        parse_commands("{\"commands\":[\"echo first\\necho second\",\"false\"]}").unwrap(),
        ["echo first\necho second", "false"]
    );
    for text in [
        "{}",
        "[]",
        "null",
        "{",
        r#"{"commands":[]}"#,
        r#"{"commands":[1]}"#,
        r#"{"commands":[" "]}"#,
        r#"{"commands":[" true "]}"#,
        r#"{"commands":[":"]}"#,
        r#"{"commands":["exit 0"]}"#,
        r#"{"commands":["test -f ok"],"extra":true}"#,
    ] {
        assert!(parse_commands(text).is_err(), "{text}");
    }
    assert!(
        parse_commands(&serde_json::json!({"commands": vec!["echo check"; 32]}).to_string())
            .is_ok()
    );
    assert!(
        parse_commands(&serde_json::json!({"commands": vec!["echo check"; 33]}).to_string())
            .is_err()
    );
}

#[test]
fn round_trip_does_not_commit_execute_or_touch_index() {
    let dir = fixture();
    let root = dir.path();
    let before = read_settings(root).unwrap();
    assert!(before.content.is_none());
    let content = "{\n  \"commands\": [\"touch must-not-run\", \"echo first\\necho second\"]\n}\n";
    let saved = save(root, &before.revision, content).unwrap();
    assert_eq!(saved.content.as_deref(), Some(content));
    assert_eq!(saved.revision, read_settings(root).unwrap().revision);
    assert_eq!(before.head, saved.head);
    assert!(!root.join("must-not-run").exists());
    assert!(
        git(root, &["diff", "--cached", "--name-only"])
            .unwrap()
            .is_empty()
    );
    assert!(matches!(
        save(root, &before.revision, content),
        Err(ApiError::Conflict(_))
    ));
}

#[test]
fn rejects_external_changes_branch_changes_and_invalid_save_without_overwriting() {
    let dir = fixture();
    let root = dir.path();
    let first = read_settings(root).unwrap();
    let saved = save(root, &first.revision, r#"{"commands":["echo original"]}"#).unwrap();
    assert!(save(root, &saved.revision, r#"{"commands":[]}"#).is_err());
    assert_eq!(read_settings(root).unwrap().content, saved.content);
    fs::write(root.join(CONFIG), "invalid existing JSON").unwrap();
    assert!(matches!(
        save(
            root,
            &saved.revision,
            r#"{"commands":["echo replacement"]}"#
        ),
        Err(ApiError::Conflict(_))
    ));
    let invalid = read_settings(root).unwrap();
    assert_eq!(invalid.content.as_deref(), Some("invalid existing JSON"));
    let repaired = save(root, &invalid.revision, r#"{"commands":["echo repaired"]}"#).unwrap();
    git(root, &["checkout", "-b", "other"]).unwrap();
    assert!(matches!(
        save(
            root,
            &repaired.revision,
            r#"{"commands":["echo replacement"]}"#
        ),
        Err(ApiError::Conflict(_))
    ));
}

#[test]
fn busy_storage_and_invalid_paths_fail_without_replacing_file() {
    let dir = fixture();
    let root = dir.path();
    let first = read_settings(root).unwrap();
    let git = git::GitService::new();
    let guard = git.try_publication_guard(root).unwrap();
    assert!(matches!(
        save(root, &first.revision, r#"{"commands":["echo check"]}"#),
        Err(ApiError::Conflict(_))
    ));
    assert!(!root.join(CONFIG).exists());
    drop(guard);
    fs::write(root.join(".lvk"), "keep").unwrap();
    assert!(save(root, &first.revision, r#"{"commands":["echo check"]}"#).is_err());
    assert_eq!(fs::read_to_string(root.join(".lvk")).unwrap(), "keep");
}

#[cfg(unix)]
#[test]
fn rejects_symlink_directory_and_file() {
    let dir = fixture();
    let outside = tempfile::tempdir().unwrap();
    std::os::unix::fs::symlink(outside.path(), dir.path().join(".lvk")).unwrap();
    assert!(read_settings(dir.path()).is_err());
    fs::remove_file(dir.path().join(".lvk")).unwrap();
    fs::create_dir(dir.path().join(".lvk")).unwrap();
    let target = outside.path().join("plan");
    fs::write(&target, "keep").unwrap();
    std::os::unix::fs::symlink(&target, dir.path().join(CONFIG)).unwrap();
    assert!(read_settings(dir.path()).is_err());
    assert_eq!(fs::read_to_string(target).unwrap(), "keep");
}

#[test]
fn concurrent_saves_have_only_one_winner() {
    let dir = fixture();
    let first = read_settings(dir.path()).unwrap();
    let barrier = std::sync::Barrier::new(2);
    std::thread::scope(|scope| {
        let run = |command: &str| {
            barrier.wait();
            save(
                dir.path(),
                &first.revision,
                &serde_json::json!({"commands": [command]}).to_string(),
            )
        };
        let a = scope.spawn(move || run("echo a"));
        let b = scope.spawn(move || run("echo b"));
        let a = a.join().unwrap();
        let b = b.join().unwrap();
        assert_ne!(a.is_ok(), b.is_ok());
        assert!(matches!(
            a.err().or_else(|| b.err()),
            Some(ApiError::Conflict(_))
        ));
    });
}

#[test]
fn size_limit_and_moved_head_cannot_overwrite_configuration() {
    let dir = fixture();
    let before = read_settings(dir.path()).unwrap();
    assert!(matches!(
        save(
            dir.path(),
            &before.revision,
            &"x".repeat(MAX_BYTES as usize + 1)
        ),
        Err(ApiError::PayloadTooLarge)
    ));
    git(
        dir.path(),
        &[
            "-c",
            "user.name=Fixture",
            "-c",
            "user.email=fixture@example.test",
            "commit",
            "--allow-empty",
            "-m",
            "advance",
        ],
    )
    .unwrap();
    assert!(matches!(
        save(
            dir.path(),
            &before.revision,
            r#"{"commands":["echo check"]}"#
        ),
        Err(ApiError::Conflict(_))
    ));
    assert!(!dir.path().join(CONFIG).exists());
    git(dir.path(), &["checkout", "--detach"]).unwrap();
    assert!(read_settings(dir.path()).is_err());
}
