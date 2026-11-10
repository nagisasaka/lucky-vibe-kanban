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
            "list_workspace_diff",
            "List changed reference paths against the Workspace target merge base. Includes untracked changes; private paths are excluded. Not merge eligibility or test evidence.",
        ),
        definition::<FileArgs>(
            "get_workspace_diff",
            "Read one changed path unified diff in character pages up to 16000. Binary/oversized/unreadable content is explicit. Includes source head and base.",
        ),
    ]
}

pub(super) async fn call(remote: &Remote, name: &str, args: Value) -> anyhow::Result<Value> {
    let mut result;
    let target;
    if name == "list_workspace_diff" {
        let args: ListArgs = arguments(args)?;
        target = args.target.clone();
        let source = remote.evidence_source(&target).await?;
        let diff = name == "list_workspace_diff";
        result = tokio::task::spawn_blocking(move || list(&source, &args, diff))
            .await
            .map_err(|_| anyhow::anyhow!("Code read interrupted; retry this read"))??;
    } else {
        ensure!(
            name == "get_workspace_diff",
            super::errors::rejection_with_message(
                utils::mcp_error::Category::Validation,
                "Unknown code read tool"
            )
        );
        let args: FileArgs = arguments(args)?;
        target = args.target.clone();
        let source = remote.evidence_source(&target).await?;
        result = tokio::task::spawn_blocking(move || detail(&source, &args))
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
        super::errors::rejection_with_message(
            utils::mcp_error::Category::Conflict,
            "Workspace checkout branch changed; inspect the Workspace before reading evidence"
        )
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
    let identity = serde_json::to_vec(&(&head, &base, &paths))?;
    let current = revision(
        &args.target,
        if diff { "diff-list" } else { "file-list" },
        &identity,
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
fn detail(source: &Source, args: &FileArgs) -> anyhow::Result<Value> {
    ensure!(
        allowed_path(&args.path),
        super::errors::rejection_with_message(
            utils::mcp_error::Category::Permission,
            "Private or invalid reference path"
        )
    );
    let root = source.root()?;
    let (git, head) = heads(source, &root)?;
    let mut result = {
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
            super::errors::rejection_with_message(
                utils::mcp_error::Category::Conflict,
                "Path is no longer changed; refresh list_workspace_diff"
            )
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
    };
    // Detect a concurrently checked-out commit; working-file edits are detected
    // by each body's revision on continuation.
    ensure!(
        git.get_head_info(&root).map_err(unavailable)?.oid == head,
        super::errors::rejection_with_message(
            utils::mcp_error::Category::Conflict,
            "Checkout changed during read; retry from the first page"
        )
    );
    result["path"] = json!(args.path);
    result["source_head"] = json!(head);
    result["maximum_source_bytes_per_side"] = json!(git::reference::MAX_REFERENCE_BYTES);
    Ok(result)
}

#[cfg(all(test, unix))]
pub(super) mod tests {
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
    pub(in crate::remote_mcp) async fn fixture() -> (tempfile::TempDir, Remote, Value) {
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
    async fn diff_routes_remain_and_file_routes_are_removed() {
        let (_temp, remote, args) = fixture().await;
        let names: Vec<_> = tools().iter().map(|t| t.name.to_string()).collect();
        assert_eq!(names, ["list_workspace_diff", "get_workspace_diff"]);
        assert!(call(&remote, "list_workspace_diff", args).await.is_ok());
        for name in [
            "list_workspace_files",
            "get_workspace_file",
            "search_workspace_files",
            "get_workspace_file_ranges",
        ] {
            assert!(!super::super::tools().iter().any(|t| t.name == name));
        }
    }
    #[tokio::test]
    async fn diff_reads_page_real_git_changes_and_keep_evidence_states_distinct() {
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
        fs::write(temp.path().join("src/000.txt"), "direct Codex change\n").unwrap();
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
            for name in ["get_workspace_diff"] {
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
        for name in ["get_workspace_diff"] {
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
            call(&remote, "get_workspace_diff", file.clone())
                .await
                .is_err()
        );
        for key in ["project_id", "workspace_id", "repository_id"] {
            let mut wrong = file.clone();
            wrong[key] = json!(Uuid::new_v4());
            assert!(call(&remote, "get_workspace_diff", wrong).await.is_err());
        }
        // Switch to a different registered branch without changing stored Workspace identity.
        git(temp.path(), &["symbolic-ref", "HEAD", "refs/heads/main"]);
        assert!(call(&remote, "list_workspace_diff", args).await.is_err());
    }
}
