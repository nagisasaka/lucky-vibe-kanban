//! Bounded code and per-path diff evidence, reusing GitService identities,
//! merge-base discovery and the normal unified-diff formatter.
use anyhow::ensure;
use git::{
    GitService,
    reference::{Content, allowed_path},
};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};

use super::{
    Remote,
    evidence_read::{ListArgs, Source, Target, TextArgs, check_page, revision, text_page},
    read::{arguments, definition},
};

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct FileArgs {
    #[serde(flatten)]
    target: Target,
    /// Exact path returned by the corresponding list tool.
    path: String,
    #[serde(flatten)]
    page: TextArgs,
}

pub(super) fn tools() -> Vec<rmcp::model::Tool> {
    vec![
        definition::<ListArgs>(
            "list_workspace_files",
            "List tracked reference-code paths in one Workspace repository, up to 50. Private paths, Git symlink entries and submodules are excluded. Use get_workspace_file for content.",
        ),
        definition::<FileArgs>(
            "get_workspace_file",
            "Read one tracked reference file in character pages up to 16000. Binary, oversized, missing or unsupported files are explicit. Only safe regular files below the selected checkout are readable.",
        ),
        definition::<ListArgs>(
            "list_workspace_diff",
            "List changed reference paths against the existing Workspace target merge base, up to 50. Includes untracked changes; private paths are excluded. Renames expose separate old/new paths. This is not merge eligibility or test evidence.",
        ),
        definition::<FileArgs>(
            "get_workspace_diff",
            "Read one changed path's unified text diff in character pages up to 16000. Includes source head and base. Binary/oversized/unreadable content is not reported as an empty successful diff. Renames are compared per path.",
        ),
    ]
}

pub(super) async fn call(remote: &Remote, name: &str, args: Value) -> anyhow::Result<Value> {
    let mut result;
    let target;
    if matches!(name, "list_workspace_files" | "list_workspace_diff") {
        let args: ListArgs = arguments(args)?;
        target = args.target.clone();
        let source = remote.evidence_source(&target).await?;
        let diff = name == "list_workspace_diff";
        result = tokio::task::spawn_blocking(move || list(&source, &args, diff))
            .await
            .map_err(|_| anyhow::anyhow!("Code read interrupted; retry this read"))??;
    } else {
        ensure!(
            matches!(name, "get_workspace_file" | "get_workspace_diff"),
            "Unknown code read tool"
        );
        let args: FileArgs = arguments(args)?;
        target = args.target.clone();
        let source = remote.evidence_source(&target).await?;
        let diff = name == "get_workspace_diff";
        result = tokio::task::spawn_blocking(move || detail(&source, &args, diff))
            .await
            .map_err(|_| anyhow::anyhow!("Code read interrupted; retry this read"))??;
    }
    remote.evidence_link(&target, &mut result).await?;
    Ok(result)
}
fn unavailable(error: git::GitServiceError) -> anyhow::Error {
    if matches!(&error, git::GitServiceError::IoError(error) if error.kind() == std::io::ErrorKind::Unsupported)
    {
        return anyhow::anyhow!(
            "Code content reads are unsupported on this host; use the Linux LVK server or the Workspace review link"
        );
    }
    anyhow::anyhow!(
        "Repository evidence unavailable: file may be unsafe, unreadable, conflicted or untracked; refresh the listing and inspect the Workspace"
    )
}
fn heads(source: &Source, root: &std::path::Path) -> anyhow::Result<(GitService, String)> {
    let git = GitService::new();
    let head = git.get_head_info(root).map_err(unavailable)?;
    ensure!(
        head.branch == source.workspace.branch,
        "Workspace checkout branch changed; inspect the Workspace before reading evidence"
    );
    Ok((git, head.oid))
}
fn list(source: &Source, args: &ListArgs, diff: bool) -> anyhow::Result<Value> {
    let root = source.root()?;
    let (git, head) = heads(source, &root)?;
    let base = if diff {
        Some(
            git.get_base_commit(
                &source.repo.path,
                &source.workspace.branch,
                &source.target_branch,
            )
            .map_err(unavailable)?,
        )
    } else {
        None
    };
    let mut paths = if let Some(base) = &base {
        git.reference_diff_paths(&root, base)
            .map_err(unavailable)?
            .into_iter()
            .filter(|p| allowed_path(p))
            .collect::<Vec<_>>()
    } else {
        git.reference_files(&root).map_err(unavailable)?
    };
    paths.sort();
    let base = base.map(|b| b.to_string());
    let current = revision(
        &args.target,
        if diff { "diff-list" } else { "file-list" },
        &serde_json::to_vec(&(&head, &base, &paths))?,
    );
    check_page(
        args.offset,
        args.limit,
        50,
        args.expected_revision.as_deref(),
        &current,
        paths.len(),
    )?;
    let end = (args.offset as usize + args.limit as usize).min(paths.len());
    Ok(
        json!({"items":paths.iter().skip(args.offset as usize).take(args.limit as usize).map(|path|json!({"path":path})).collect::<Vec<_>>(),"offset":args.offset,"has_more":end<paths.len(),"next_offset":if end<paths.len(){Some(end)}else{None},"revision":current,"source_head":head,"base_commit":base,"private_paths_excluded":true,"merge_status":"not_observed"}),
    )
}
fn status(content: &Content) -> &'static str {
    match content {
        Content::Text(_) => "text",
        Content::Missing => "missing",
        Content::Binary => "binary",
        Content::TooLarge => "too_large",
        Content::Unsupported => "unsupported",
    }
}
fn text(content: &Content) -> Option<&str> {
    match content {
        Content::Text(text) => Some(text),
        Content::Missing => Some(""),
        _ => None,
    }
}
fn detail(source: &Source, args: &FileArgs, diff: bool) -> anyhow::Result<Value> {
    ensure!(
        allowed_path(&args.path),
        "Private or invalid reference path"
    );
    let root = source.root()?;
    let (git, head) = heads(source, &root)?;
    let mut result = if diff {
        let base = git
            .get_base_commit(
                &source.repo.path,
                &source.workspace.branch,
                &source.target_branch,
            )
            .map_err(unavailable)?;
        let paths = git
            .reference_diff_paths(&root, &base)
            .map_err(unavailable)?;
        ensure!(
            paths.contains(&args.path),
            "Path is no longer changed; refresh list_workspace_diff"
        );
        let (old, new) = git
            .reference_diff(&root, &base, &args.path)
            .map_err(unavailable)?;
        let patch = text(&old)
            .zip(text(&new))
            .map(|(old, new)| utils::diff::create_unified_diff(&args.path, old, new));
        let identity = format!(
            "diff:{}:{}:{}:{}:{}",
            args.path,
            head,
            base,
            status(&old),
            status(&new)
        );
        let mut value = text_page(&args.target, &identity, patch.as_deref(), &args.page)?;
        value["old_status"] = json!(status(&old));
        value["new_status"] = json!(status(&new));
        value["base_commit"] = json!(base.to_string());
        value["changed_path"] = json!(true);
        value["text_equal"] = json!(text(&old).zip(text(&new)).map(|(a, b)| a == b));
        value["non_text_changes_require_review"] = json!(true);
        value
    } else {
        let content = git.reference_file(&root, &args.path).map_err(unavailable)?;
        let body = match &content {
            Content::Text(text) => Some(text.as_str()),
            _ => None,
        };
        let mut value = text_page(
            &args.target,
            &format!("file:{}:{}:{}", args.path, head, status(&content)),
            body,
            &args.page,
        )?;
        value["status"] = json!(status(&content));
        value
    };
    // Detect a concurrently checked-out commit; working-file edits are detected
    // by each body's revision on continuation.
    ensure!(
        git.get_head_info(&root).map_err(unavailable)?.oid == head,
        "Checkout changed during read; retry from the first page"
    );
    result["path"] = json!(args.path);
    result["source_head"] = json!(head);
    result["maximum_source_bytes_per_side"] = json!(git::reference::MAX_REFERENCE_BYTES);
    Ok(result)
}

#[cfg(all(test, unix))]
mod tests {
    use std::{fs, path::Path, process::Command};

    use db::models::{
        repo::Repo,
        workspace_repo::{CreateWorkspaceRepo, WorkspaceRepo},
    };
    use serde_json::json;
    use uuid::Uuid;

    use super::*;

    fn git(root: &Path, args: &[&str]) {
        let output = Command::new("git")
            .arg("-C")
            .arg(root)
            .args(args)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    async fn fixture() -> (tempfile::TempDir, Remote, Value) {
        let (remote, _, _, run, _) = super::super::runtime_write_tests::fixture().await;
        let temp = tempfile::tempdir().unwrap();
        let repo = Repo::find_or_create(&remote.pool, temp.path(), "code-fixture")
            .await
            .unwrap();
        // Use DirectFolder to exercise the other root resolution branch too.
        sqlx::query(
            "UPDATE workspaces SET workspace_kind='direct_folder',container_ref=? WHERE id=?",
        )
        .bind(temp.path().to_str().unwrap())
        .bind(run.workspace_id)
        .execute(&remote.pool)
        .await
        .unwrap();
        WorkspaceRepo::create_many(
            &remote.pool,
            run.workspace_id,
            &[CreateWorkspaceRepo {
                repo_id: repo.id,
                target_branch: "main".into(),
            }],
        )
        .await
        .unwrap();
        git(temp.path(), &["init", "-b", "main"]);
        git(temp.path(), &["config", "user.name", "MCP Fixture"]);
        git(
            temp.path(),
            &["config", "user.email", "fixture@example.invalid"],
        );
        fs::create_dir(temp.path().join("src")).unwrap();
        for i in 0..105 {
            fs::write(temp.path().join(format!("src/{i:03}.txt")), "before\n").unwrap();
        }
        fs::write(temp.path().join(".env"), "private-value").unwrap();
        fs::write(temp.path().join("secret.key"), "private-key").unwrap();
        fs::write(temp.path().join("binary.dat"), [0, 1, 2]).unwrap();
        fs::write(
            temp.path().join("huge.txt"),
            "a".repeat(git::reference::MAX_REFERENCE_BYTES + 1),
        )
        .unwrap();
        git(temp.path(), &["add", "."]);
        git(temp.path(), &["commit", "-m", "fixture"]);
        git(temp.path(), &["switch", "-c", "fixture"]);
        (
            temp,
            remote,
            json!({"project_id":run.project_id,"workspace_id":run.workspace_id,"repository_id":repo.id}),
        )
    }

    #[tokio::test]
    async fn code_and_diff_reads_page_real_git_changes_and_keep_evidence_states_distinct() {
        let (temp, remote, args) = fixture().await;
        let original_index = fs::read(temp.path().join(".git/index")).unwrap();
        fs::write(
            temp.path().join(".gitattributes"),
            "src/** filter=reference-test\n",
        )
        .unwrap();
        git(
            temp.path(),
            &[
                "config",
                "filter.reference-test.clean",
                "touch .git/filter-was-run; cat",
            ],
        );
        git(
            temp.path(),
            &["config", "diff.external", "touch .git/diff-was-run"],
        );
        let first = call(&remote, "list_workspace_files", args.clone())
            .await
            .unwrap();
        assert_eq!(first["items"].as_array().unwrap().len(), 20);
        assert!(!first.to_string().contains(".env"));
        let mut next = args.clone();
        next["offset"] = first["next_offset"].clone();
        assert!(
            call(&remote, "list_workspace_files", next.clone())
                .await
                .is_err()
        );
        next["expected_revision"] = first["revision"].clone();
        next["limit"] = json!(50);
        assert_eq!(
            call(&remote, "list_workspace_files", next.clone())
                .await
                .unwrap()["items"]
                .as_array()
                .unwrap()
                .len(),
            50
        );
        next["limit"] = json!(51);
        assert!(call(&remote, "list_workspace_files", next).await.is_err());
        let mut file = args.clone();
        file["path"] = json!("src/000.txt");
        file["limit"] = json!(3);
        let first = call(&remote, "get_workspace_file", file.clone())
            .await
            .unwrap();
        assert_eq!(first["content"], "bef");
        assert_eq!(first["next_offset"], 3);
        file["offset"] = json!(3);
        file["expected_revision"] = first["revision"].clone();
        assert_eq!(
            call(&remote, "get_workspace_file", file.clone())
                .await
                .unwrap()["content"],
            "ore"
        );
        fs::write(temp.path().join("src/000.txt"), "direct Codex change\n").unwrap();
        assert!(call(&remote, "get_workspace_file", file).await.is_err());
        for i in 1..105 {
            fs::write(
                temp.path().join(format!("src/{i:03}.txt")),
                format!("changed {i}\n"),
            )
            .unwrap();
        }
        fs::write(temp.path().join(".env"), "new private-value").unwrap();
        let mut listing = args.clone();
        listing["limit"] = json!(50);
        let first = call(&remote, "list_workspace_diff", listing.clone())
            .await
            .unwrap();
        assert_eq!(first["items"].as_array().unwrap().len(), 50);
        assert_eq!(first["has_more"], true);
        listing["offset"] = json!(100);
        listing["expected_revision"] = first["revision"].clone();
        assert_eq!(
            call(&remote, "list_workspace_diff", listing).await.unwrap()["items"]
                .as_array()
                .unwrap()
                .len(),
            5
        );
        let mut diff = args.clone();
        diff["path"] = json!("src/000.txt");
        let detail = call(&remote, "get_workspace_diff", diff.clone())
            .await
            .unwrap();
        assert!(
            detail["content"]
                .as_str()
                .unwrap()
                .contains("+direct Codex change")
        );
        assert!(detail["content"].as_str().unwrap().contains("-before"));
        assert_eq!(detail["old_status"], "text");
        assert_eq!(detail["new_status"], "text");
        assert_eq!(detail["base_commit"], first["base_commit"]);
        fs::write(temp.path().join("untracked.txt"), "new implementation\n").unwrap();
        diff["path"] = json!("untracked.txt");
        assert!(
            call(&remote, "get_workspace_file", diff.clone())
                .await
                .is_err()
        );
        let added = call(&remote, "get_workspace_diff", diff.clone())
            .await
            .unwrap();
        assert_eq!(added["old_status"], "missing");
        assert_eq!(added["new_status"], "text");
        fs::remove_file(temp.path().join("src/000.txt")).unwrap();
        diff["path"] = json!("src/000.txt");
        assert_eq!(
            call(&remote, "get_workspace_diff", diff.clone())
                .await
                .unwrap()["new_status"],
            "missing"
        );
        fs::write(temp.path().join("binary.dat"), [0, 4, 5]).unwrap();
        for (path, status) in [("binary.dat", "binary"), ("huge.txt", "too_large")] {
            diff["path"] = json!(path);
            let value = call(&remote, "get_workspace_file", diff.clone())
                .await
                .unwrap();
            assert_eq!(value["status"], status);
            assert_eq!(value["available"], false);
        }
        diff["path"] = json!("binary.dat");
        let binary = call(&remote, "get_workspace_diff", diff).await.unwrap();
        assert_eq!(binary["available"], false);
        assert_eq!(binary["text_equal"], Value::Null);
        assert_eq!(binary["new_status"], "binary");
        assert_eq!(
            fs::read(temp.path().join(".git/index")).unwrap(),
            original_index
        );
        assert!(!temp.path().join(".git/filter-was-run").exists());
        assert!(!temp.path().join(".git/diff-was-run").exists());
        let count: i64 = sqlx::query_scalar("SELECT count(*) FROM agent_runs")
            .fetch_one(&remote.pool)
            .await
            .unwrap();
        assert_eq!(count, 1);
        let count: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM sqlite_master WHERE name IN
             ('project_conversations', 'project_chat_messages', 'project_chat_operations', 'project_chat_events')",
        )
            .fetch_one(&remote.pool)
            .await
            .unwrap();
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn reference_paths_cannot_escape_or_read_private_files_or_cross_project() {
        let (temp, remote, args) = fixture().await;
        let mut file = args.clone();
        for path in [
            ".env",
            "secret.key",
            "../outside",
            "/etc/passwd",
            "src/../.env",
            "src\\000.txt",
            ":(glob)**",
        ] {
            file["path"] = json!(path);
            for name in ["get_workspace_file", "get_workspace_diff"] {
                assert!(
                    call(&remote, name, file.clone()).await.is_err(),
                    "{name}: {path}"
                );
            }
        }
        let outside = tempfile::tempdir().unwrap();
        fs::write(outside.path().join("000.txt"), "outside-private-value").unwrap();
        fs::remove_file(temp.path().join("src/000.txt")).unwrap();
        std::os::unix::fs::symlink(
            outside.path().join("000.txt"),
            temp.path().join("src/000.txt"),
        )
        .unwrap();
        file["path"] = json!("src/000.txt");
        for name in ["get_workspace_file", "get_workspace_diff"] {
            let error = call(&remote, name, file.clone())
                .await
                .unwrap_err()
                .to_string();
            assert!(
                !error.contains("outside-private-value")
                    && !error.contains(outside.path().to_str().unwrap())
            );
        }
        fs::remove_dir_all(temp.path().join("src")).unwrap();
        std::os::unix::fs::symlink(outside.path(), temp.path().join("src")).unwrap();
        assert!(
            call(&remote, "get_workspace_file", file.clone())
                .await
                .is_err()
        );
        for key in ["project_id", "workspace_id", "repository_id"] {
            let mut wrong = file.clone();
            wrong[key] = json!(Uuid::new_v4());
            assert!(call(&remote, "get_workspace_file", wrong).await.is_err());
        }
        // Switch to a different registered branch without changing stored Workspace identity.
        git(temp.path(), &["symbolic-ref", "HEAD", "refs/heads/main"]);
        assert!(call(&remote, "list_workspace_files", args).await.is_err());
    }
}
