use std::{fs, path::Path, process::Command};

use git::{GitCli, GitService};
use git2::Repository;

fn git(path: &Path, args: &[&str]) -> String {
    GitCli::new().git(path, args.iter().copied()).unwrap()
}

fn configure(root: &Path, name: &str, email: &str) {
    git(root, &["config", "--local", "user.name", name]);
    git(root, &["config", "--local", "user.email", email]);
}

#[test]
fn identity_policy_in_isolated_processes() {
    // No global/system config, runtime DB, shared paths or environment mutation
    // within the parallel test process. Children and their repos are disposable.
    for mode in [
        "configured",
        "missing",
        "legacy",
        "overrides",
        "config-overrides",
        "global",
    ] {
        let temp = tempfile::TempDir::new().unwrap();
        let empty = temp.path().join("empty-config");
        fs::write(
            &empty,
            if mode == "global" {
                "[user]\nname = Configured\nemail = configured@example.invalid\n"
            } else {
                ""
            },
        )
        .unwrap();
        let system_config = temp.path().join("system-config");
        fs::write(&system_config, "").unwrap();
        let mut cmd = Command::new(std::env::current_exe().unwrap());
        for (key, _) in std::env::vars_os() {
            if key.to_string_lossy().starts_with("GIT_") {
                cmd.env_remove(key);
            }
        }
        cmd.args(["--exact", "identity_fixture", "--ignored", "--nocapture"])
            .env("LVK_IDENTITY_TEST_MODE", mode)
            .env("LVK_INSTANCE_DIR", temp.path().join("instance"))
            .env("XDG_CONFIG_HOME", temp.path().join("config"))
            .env("HOME", temp.path())
            .env("GIT_CONFIG_GLOBAL", &empty)
            .env("GIT_CONFIG_SYSTEM", &system_config)
            .env_remove("EMAIL");
        if mode == "overrides" {
            cmd.env("GIT_AUTHOR_NAME", "Explicit Author")
                .env("GIT_AUTHOR_EMAIL", "author@example.invalid")
                .env("GIT_COMMITTER_NAME", "Explicit Committer")
                .env("GIT_COMMITTER_EMAIL", "committer@example.invalid")
                .env("GIT_AUTHOR_DATE", "1234567890 +0900")
                .env("GIT_COMMITTER_DATE", "1234567891 -0300");
        }
        let global_before = fs::read(&empty).unwrap();
        let system_before = fs::read(&system_config).unwrap();
        let output = cmd.output().unwrap();
        assert!(
            output.status.success(),
            "{mode}: {}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(fs::read(&empty).unwrap(), global_before);
        assert_eq!(fs::read(&system_config).unwrap(), system_before);
    }
}

#[test]
#[ignore = "only run in a child with isolated Git and LVK environment"]
fn identity_fixture() {
    let mode = std::env::var("LVK_IDENTITY_TEST_MODE").expect("isolated child only");
    let temp = tempfile::TempDir::new().unwrap();
    let root = temp.path().join("repo");
    let repo = Repository::init(&root).unwrap();
    let service = GitService::new();

    if mode == "missing" {
        assert!(!service.pending_initial_identity_setup(&root).unwrap());
        for (name, email) in [
            ("", ""),
            ("Configured", ""),
            ("", "configured@example.invalid"),
        ] {
            configure(&root, name, email);
            let config_before = fs::read(repo.path().join("config")).unwrap();
            let error = service
                .initialize_repo_with_main_branch(&root)
                .unwrap_err()
                .to_string();
            assert!(error.contains("identity"));
            assert_eq!(config_before, fs::read(repo.path().join("config")).unwrap());
            assert!(repo.head().is_err());
        }
        let notice = repo.commondir().join("lvk-identity-guidance");
        assert!(notice.is_file());
        assert!(service.pending_initial_identity_setup(&root).unwrap());
        let error = service
            .require_commit_identity(&root, false)
            .unwrap_err()
            .to_string();
        assert!(
            !error.contains("YOUR NAME"),
            "do not repeat the setup question"
        );
        configure(&root, "Configured", "configured@example.invalid");
        service.initialize_repo_with_main_branch(&root).unwrap();
        assert!(!service.pending_initial_identity_setup(&root).unwrap());
        service.require_commit_identity(&root, false).unwrap();
        return;
    }

    if mode == "legacy" {
        configure(&root, "Vibe Kanban", "noreply@vibekanban.com");
        let before = fs::read(repo.path().join("config")).unwrap();
        let error = service
            .initialize_repo_with_main_branch(&root)
            .unwrap_err()
            .to_string();
        assert!(error.contains("lvk.allowLegacyIdentity"));
        assert_eq!(before, fs::read(repo.path().join("config")).unwrap());
        git(
            &root,
            &["config", "--local", "lvk.allowLegacyIdentity", "true"],
        );
        service.initialize_repo_with_main_branch(&root).unwrap();
        let wt = temp.path().join("worktree");
        git(&root, &["branch", "feature"]);
        service.add_worktree(&root, &wt, "feature", false).unwrap();
        git(
            &root,
            &["config", "--local", "--unset", "lvk.allowLegacyIdentity"],
        );
        let error = service
            .require_commit_identity(&wt, false)
            .unwrap_err()
            .to_string();
        assert!(
            !error.contains("YOUR NAME"),
            "linked worktrees share the notice"
        );
        configure(&root, "Configured", "configured@example.invalid");
        service.require_commit_identity(&wt, false).unwrap();
        // A different repository has independent guidance and identity.
        let other = temp.path().join("other");
        Repository::init(&other).unwrap();
        assert!(
            service
                .require_commit_identity(&other, false)
                .unwrap_err()
                .to_string()
                .contains("YOUR NAME")
        );
        return;
    }

    if mode != "global" {
        configure(&root, "Configured", "configured@example.invalid");
    }
    if mode == "overrides" {
        // Explicit per-command/environment identities also beat an old stored pair.
        configure(&root, "Vibe Kanban", "noreply@vibekanban.com");
    }
    if mode == "config-overrides" {
        for (key, value) in [
            ("author.name", "Explicit Author"),
            ("author.email", "author@example.invalid"),
            ("committer.name", "Explicit Committer"),
            ("committer.email", "committer@example.invalid"),
        ] {
            git(&root, &["config", key, value]);
        }
    }
    service.initialize_repo_with_main_branch(&root).unwrap();
    let initial = git(&root, &["rev-parse", "HEAD"]);
    let expected_author = if matches!(mode.as_str(), "overrides" | "config-overrides") {
        "Explicit Author <author@example.invalid>"
    } else {
        "Configured <configured@example.invalid>"
    };
    let expected_committer = if matches!(mode.as_str(), "overrides" | "config-overrides") {
        "Explicit Committer <committer@example.invalid>"
    } else {
        "Configured <configured@example.invalid>"
    };
    let check = |sha: &str| {
        assert_eq!(
            git(&root, &["show", "-s", "--format=%an <%ae>", sha]).trim(),
            expected_author
        );
        assert_eq!(
            git(&root, &["show", "-s", "--format=%cn <%ce>", sha]).trim(),
            expected_committer
        );
        if mode == "overrides" {
            assert_eq!(
                git(&root, &["show", "-s", "--format=%at %ct", sha]).trim(),
                "1234567890 1234567891"
            );
        }
    };
    check(initial.trim());
    fs::write(root.join("source"), "source").unwrap();
    service.commit(&root, "normal / cleanup").unwrap();
    check("HEAD");
    let before = service.get_head_info(&root).unwrap().oid;
    fs::write(root.join("source"), "frozen source").unwrap();
    let tree = service.stage_source_snapshot(&root).unwrap();
    service
        .commit_staged_snapshot(&root, "source publication", &before, &tree)
        .unwrap();
    check("HEAD");
    fs::write(root.join("wiki"), "wiki").unwrap();
    service
        .commit_paths(&root, "wiki publication", &["wiki".into()])
        .unwrap();
    check("HEAD");
    assert!(!repo.commondir().join("lvk-identity-guidance").exists());

    if mode == "configured" {
        assert!(
            service
                .commit_identity_instructions(&root)
                .unwrap()
                .contains("explicit Git")
        );
        let wt = temp.path().join("linked");
        git(&root, &["branch", "linked"]);
        service.add_worktree(&root, &wt, "linked", false).unwrap();
        git(&root, &["config", "extensions.worktreeConfig", "true"]);
        git(
            &wt,
            &["config", "--worktree", "user.name", "Worktree Choice"],
        );
        git(
            &wt,
            &[
                "config",
                "--worktree",
                "user.email",
                "worktree@example.invalid",
            ],
        );
        fs::write(wt.join("local"), "local").unwrap();
        service.commit(&wt, "worktree identity").unwrap();
        assert_eq!(
            git(&wt, &["show", "-s", "--format=%an <%ae>", "HEAD"]).trim(),
            "Worktree Choice <worktree@example.invalid>"
        );
        check("HEAD"); // worktree config must not leak into the main checkout
        // Explicit CLI --author and original author on cherry-pick are not rewritten.
        fs::write(root.join("authored"), "authored").unwrap();
        git(&root, &["add", "authored"]);
        git(
            &root,
            &[
                "commit",
                "--author=Upstream <upstream@example.invalid>",
                "-m",
                "upstream",
            ],
        );
        let upstream = git(&root, &["rev-parse", "HEAD"]);
        git(&root, &["checkout", "-b", "pick", initial.trim()]);
        git(&root, &["cherry-pick", upstream.trim()]);
        assert_eq!(
            git(&root, &["show", "-s", "--format=%an <%ae>", "HEAD"]).trim(),
            "Upstream <upstream@example.invalid>"
        );
        assert_eq!(
            git(&root, &["show", "-s", "--format=%cn <%ce>", "HEAD"]).trim(),
            expected_committer
        );
        service
            .rebase_branch(&root, &root, "main", initial.trim(), "pick")
            .unwrap();
        assert_eq!(
            git(&root, &["show", "-s", "--format=%an <%ae>", "HEAD"]).trim(),
            "Upstream <upstream@example.invalid>"
        );
    }
}
