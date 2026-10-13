//! Recorded test claims from the existing immutable per-run Change Manifest.
use anyhow::ensure;
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};
use utils::repository_memory::{ChangeManifest, MemoryTestResult, RepositoryMemoryStore};
use uuid::Uuid;

use super::{
    Remote,
    evidence_read::{Source, Target, TextArgs, check_page, revision, text_page},
    read::{arguments, definition},
};

#[derive(Clone, Deserialize, JsonSchema)]
struct RunTarget {
    project_id: Uuid,
    workspace_id: Uuid,
    repository_id: Uuid,
    agent_run_id: Uuid,
}
impl RunTarget {
    fn target(&self) -> Target {
        Target {
            project_id: self.project_id,
            workspace_id: self.workspace_id,
            repository_id: self.repository_id,
        }
    }
}
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ListArgs {
    #[serde(flatten)]
    run: RunTarget,
    #[serde(default)]
    offset: u32,
    #[serde(default = "super::default_limit")]
    #[schemars(range(min = 1, max = 50))]
    limit: u32,
    expected_revision: Option<String>,
}
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct DetailArgs {
    #[serde(flatten)]
    run: RunTarget,
    test_index: u32,
    #[serde(flatten)]
    page: TextArgs,
}
pub(super) fn tools() -> Vec<rmcp::model::Tool> {
    vec![
        definition::<ListArgs>(
            "get_workspace_verification",
            "Read recorded test counts and up to 50 test previews from the selected run's immutable Change Manifest. These are agent-recorded claims for source_commit, not independent validation of the current worktree. Missing records and head mismatches are explicit.",
        ),
        definition::<DetailArgs>(
            "get_workspace_verification_test",
            "Read one selected recorded test's complete command, result and summary as bounded JSON character pages. Use the returned revision for later pages. This never executes the command.",
        ),
    ]
}
pub(super) async fn call(remote: &Remote, name: &str, args: Value) -> anyhow::Result<Value> {
    let (run, list, detail) = match name {
        "get_workspace_verification" => {
            let args: ListArgs = arguments(args)?;
            (args.run.clone(), Some(args), None)
        }
        "get_workspace_verification_test" => {
            let args: DetailArgs = arguments(args)?;
            (args.run.clone(), None, Some(args))
        }
        _ => anyhow::bail!("Unknown verification read tool"),
    };
    let source = remote.evidence_source(&run.target()).await?;
    let belongs: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM agent_runs WHERE id=? AND workspace_id=?)")
            .bind(run.agent_run_id)
            .bind(run.workspace_id)
            .fetch_one(&remote.pool)
            .await?;
    ensure!(belongs, "Run does not exist in this Workspace");
    let target = run.target();
    let mut result = tokio::task::spawn_blocking(move || {
        let store =
            RepositoryMemoryStore::read_only_for_repository(&source.repo.name, source.repo.id)
                .map_err(|_| anyhow::anyhow!("Verification storage unavailable or unsafe"))?;
        let head = current_head(&source);
        let manifest = load(store.as_ref(), &run)?;
        let result = if let Some(args) = list {
            listing(manifest.as_ref(), head.as_deref(), &args)
        } else {
            test_detail(
                manifest.as_ref(),
                head.as_deref(),
                &detail.expect("selected detail"),
            )
        }?;
        ensure!(
            current_head(&source) == head,
            "Workspace head changed; refresh verification evidence"
        );
        Ok::<_, anyhow::Error>(result)
    })
    .await
    .map_err(|_| anyhow::anyhow!("Verification read interrupted; retry this read"))??;
    remote.evidence_link(&target, &mut result).await?;
    Ok(result)
}
fn current_head(source: &Source) -> Option<String> {
    let head = git::GitService::new()
        .get_head_info(&source.root().ok()?)
        .ok()?;
    (head.branch == source.workspace.branch).then_some(head.oid)
}
fn load(
    store: Option<&RepositoryMemoryStore>,
    run: &RunTarget,
) -> anyhow::Result<Option<ChangeManifest>> {
    let manifest = store
        .map(|store| store.event(run.agent_run_id))
        .transpose()
        .map_err(|_| {
            anyhow::anyhow!("Verification record is invalid, unsafe, unreadable or exceeds 128 KiB")
        })?
        .flatten();
    if let Some(manifest) = &manifest {
        ensure!(
            manifest.repository_id == run.repository_id
                && manifest.workspace_id == run.workspace_id
                && manifest.event_id == run.agent_run_id,
            "Verification record does not belong to this run, Workspace and repository"
        );
    }
    Ok(manifest)
}
fn metadata(manifest: Option<&ChangeManifest>, head: Option<&str>, run: &RunTarget) -> Value {
    json!({"available":manifest.is_some(),"agent_run_id":run.agent_run_id,"source_commit":manifest.map(|m|&m.source_commit),"base_commit":manifest.map(|m|&m.base_commit),"recorded_at":manifest.map(|m|m.created_at),"current_head":head,"source_matches_current_head":manifest.zip(head).map(|(m,h)|m.source_commit==h),"evidence":"agent_recorded_test_claims","current_worktree_verification":"not_established","merge_status":"not_observed","wiki_status":"not_observed"})
}
fn listing(
    manifest: Option<&ChangeManifest>,
    head: Option<&str>,
    args: &ListArgs,
) -> anyhow::Result<Value> {
    let tests = manifest
        .map(|m| m.semantics.tests.as_slice())
        .unwrap_or_default();
    let current = revision(
        &args.run.target(),
        "verification-list",
        &serde_json::to_vec(&(args.run.agent_run_id, manifest, head))?,
    );
    check_page(
        args.offset,
        args.limit,
        50,
        args.expected_revision.as_deref(),
        &current,
        tests.len(),
    )?;
    let mut result = metadata(manifest, head, &args.run);
    let end = (args.offset as usize + args.limit as usize).min(tests.len());
    result["counts"] = json!({"total":tests.len(),"passed":tests.iter().filter(|t|t.result==MemoryTestResult::Passed).count(),"failed":tests.iter().filter(|t|t.result==MemoryTestResult::Failed).count(),"not_run":tests.iter().filter(|t|t.result==MemoryTestResult::NotRun).count()});
    result["items"] = json!(tests.iter().enumerate().skip(args.offset as usize).take(args.limit as usize).map(|(i,t)|json!({"index":i,"result":t.result,"command_preview":t.command.as_ref().map(|s|s.chars().take(500).collect::<String>()),"command_truncated":t.command.as_ref().is_some_and(|s|s.chars().count()>500),"summary_preview":t.summary.as_ref().map(|s|s.chars().take(500).collect::<String>()),"summary_truncated":t.summary.as_ref().is_some_and(|s|s.chars().count()>500)})).collect::<Vec<_>>());
    result["revision"] = json!(current);
    result["offset"] = json!(args.offset);
    result["has_more"] = json!(end < tests.len());
    result["next_offset"] = json!(if end < tests.len() { Some(end) } else { None });
    Ok(result)
}
fn test_detail(
    manifest: Option<&ChangeManifest>,
    head: Option<&str>,
    args: &DetailArgs,
) -> anyhow::Result<Value> {
    let manifest = manifest
        .ok_or_else(|| anyhow::anyhow!("No recorded verification manifest for this run"))?;
    let test = manifest
        .semantics
        .tests
        .get(args.test_index as usize)
        .ok_or_else(|| {
            anyhow::anyhow!("Test index is outside this manifest; refresh verification listing")
        })?;
    let content = serde_json::to_string(test)?;
    // Bind the selected source and head to the page identity, including when the
    // requested record has identical text to another run or test.
    let kind = format!(
        "verification:{}:{}:{}:{:?}",
        args.run.agent_run_id, args.test_index, manifest.source_commit, head
    );
    let mut result = text_page(&args.run.target(), &kind, Some(&content), &args.page)?;
    result.as_object_mut().unwrap().extend(
        metadata(Some(manifest), head, &args.run)
            .as_object()
            .unwrap()
            .clone(),
    );
    result["format"] = json!("json");
    result["test_index"] = json!(args.test_index);
    Ok(result)
}

#[cfg(test)]
mod tests {
    use utils::repository_memory::{MemoryTest, SemanticChanges};

    use super::*;

    fn run() -> RunTarget {
        RunTarget {
            project_id: Uuid::from_u128(1),
            workspace_id: Uuid::from_u128(2),
            repository_id: Uuid::from_u128(3),
            agent_run_id: Uuid::from_u128(4),
        }
    }
    fn manifest(run: &RunTarget) -> ChangeManifest {
        ChangeManifest {
            version: 1,
            event_id: run.agent_run_id,
            repository_id: run.repository_id,
            workspace_id: run.workspace_id,
            task_id: None,
            created_at: chrono::Utc::now(),
            base_commit: "a".repeat(40),
            source_commit: "b".repeat(40),
            target_branch: Some("main".into()),
            changed_paths: vec![],
            semantics: SemanticChanges {
                summary: "Test fixture".into(),
                tests: (0..105)
                    .map(|i| MemoryTest {
                        command: Some(if i == 0 {
                            "界".repeat(3000)
                        } else {
                            format!("test {i}")
                        }),
                        result: match i % 3 {
                            0 => MemoryTestResult::Passed,
                            1 => MemoryTestResult::Failed,
                            _ => MemoryTestResult::NotRun,
                        },
                        summary: Some("Recorded scope only".into()),
                    })
                    .collect(),
                ..Default::default()
            },
        }
    }
    #[test]
    fn recorded_verification_is_bounded_pinned_and_never_implies_current_worktree_success() {
        let temp = tempfile::tempdir().unwrap();
        let run = run();
        let mut args = ListArgs {
            run: run.clone(),
            offset: 0,
            limit: 50,
            expected_revision: None,
        };
        assert!(load(None, &run).unwrap().is_none());
        let missing = listing(None, None, &args).unwrap();
        assert_eq!(missing["available"], false);
        assert_eq!(missing["current_worktree_verification"], "not_established");
        assert!(!temp.path().join("knowledge").exists());
        let store = RepositoryMemoryStore::at_persistent(temp.path()).unwrap();
        let manifest = manifest(&run);
        store.publish_event(&manifest).unwrap();
        let record = load(Some(&store), &run).unwrap().unwrap();
        assert_eq!(record, manifest);
        let first = listing(Some(&record), Some(&record.source_commit), &args).unwrap();
        assert_eq!(
            first["counts"],
            json!({"total":105,"passed":35,"failed":35,"not_run":35})
        );
        assert_eq!(first["items"].as_array().unwrap().len(), 50);
        assert_eq!(first["source_matches_current_head"], true);
        assert_eq!(first["current_worktree_verification"], "not_established");
        assert_eq!(
            first["items"][0]["command_preview"]
                .as_str()
                .unwrap()
                .chars()
                .count(),
            500
        );
        assert_eq!(first["items"][0]["command_truncated"], true);
        args.offset = 50;
        args.expected_revision = Some(first["revision"].as_str().unwrap().into());
        assert_eq!(
            listing(Some(&record), Some(&record.source_commit), &args).unwrap()["next_offset"],
            100
        );
        args.offset = 100;
        let last = listing(Some(&record), Some(&record.source_commit), &args).unwrap();
        assert_eq!(last["items"].as_array().unwrap().len(), 5);
        assert_eq!(last["has_more"], false);
        let changed_head = "c".repeat(40);
        assert!(listing(Some(&record), Some(&changed_head), &args).is_err());
        args.offset = 0;
        args.expected_revision = None;
        assert_eq!(
            listing(Some(&record), Some(&changed_head), &args).unwrap()["source_matches_current_head"],
            false
        );
        assert_eq!(
            listing(Some(&record), None, &args).unwrap()["source_matches_current_head"],
            Value::Null
        );
        let mut detail = DetailArgs {
            run: run.clone(),
            test_index: 0,
            page: TextArgs {
                offset: 0,
                limit: 100,
                expected_revision: None,
            },
        };
        let first = test_detail(Some(&record), Some(&record.source_commit), &detail).unwrap();
        assert_eq!(first["content"].as_str().unwrap().chars().count(), 100);
        assert_eq!(first["has_more"], true);
        detail.page.offset = 100;
        detail.page.expected_revision = Some(first["revision"].as_str().unwrap().into());
        test_detail(Some(&record), Some(&record.source_commit), &detail).unwrap();
        assert!(test_detail(Some(&record), Some(&changed_head), &detail).is_err());
        detail.test_index = 1;
        assert!(test_detail(Some(&record), Some(&record.source_commit), &detail).is_err());
        let mut wrong = run.clone();
        wrong.workspace_id = Uuid::new_v4();
        assert!(load(Some(&store), &wrong).is_err());
        wrong = run;
        wrong.repository_id = Uuid::new_v4();
        assert!(load(Some(&store), &wrong).is_err());
        std::fs::write(
            temp.path()
                .join(format!("knowledge/events/{}.json", record.event_id)),
            "{",
        )
        .unwrap();
        assert!(load(Some(&store), &args.run).is_err());
    }

    #[tokio::test]
    async fn verification_requires_run_workspace_and_repository_membership() {
        use db::models::{
            repo::Repo,
            workspace_repo::{CreateWorkspaceRepo, WorkspaceRepo},
        };
        let (remote, _, _, target, _) = crate::remote_mcp::runtime_write_tests::fixture().await;
        let temp = tempfile::tempdir().unwrap();
        let repo = Repo::find_or_create(&remote.pool, temp.path(), "verification-fixture")
            .await
            .unwrap();
        WorkspaceRepo::create_many(
            &remote.pool,
            target.workspace_id,
            &[CreateWorkspaceRepo {
                repo_id: repo.id,
                target_branch: "main".into(),
            }],
        )
        .await
        .unwrap();
        let mut args = json!({"project_id":target.project_id,"workspace_id":target.workspace_id,"repository_id":repo.id,"agent_run_id":Uuid::new_v4()});
        let error = call(&remote, "get_workspace_verification", args.clone())
            .await
            .unwrap_err();
        assert!(
            error.to_string().contains("Run does not exist"),
            "{error:#}"
        );
        args["agent_run_id"] = json!(target.agent_run_id);
        let result = call(&remote, "get_workspace_verification", args.clone())
            .await
            .unwrap();
        assert_eq!(result["available"], false);
        assert_eq!(result["current_head"], Value::Null);
        assert_eq!(result["workspace_id"], json!(target.workspace_id));
        let mut detail_args = args.clone();
        detail_args["test_index"] = json!(0);
        let error = call(
            &remote,
            "get_workspace_verification_test",
            detail_args.clone(),
        )
        .await
        .unwrap_err();
        assert!(
            error.to_string().contains("No recorded verification"),
            "{error:#}"
        );
        detail_args["unexpected"] = json!(true);
        assert!(serde_json::from_value::<DetailArgs>(detail_args).is_err());
        let mut unknown = args.clone();
        unknown["unexpected"] = json!(true);
        assert!(serde_json::from_value::<ListArgs>(unknown).is_err());
        sqlx::query(
            "UPDATE workspaces SET workspace_kind='direct_folder',container_ref=? WHERE id=?",
        )
        .bind(temp.path().to_str().unwrap())
        .bind(target.workspace_id)
        .execute(&remote.pool)
        .await
        .unwrap();
        for command in [
            vec!["init", "-b", "fixture"],
            vec![
                "-c",
                "user.name=Fixture",
                "-c",
                "user.email=fixture@example.invalid",
                "commit",
                "--allow-empty",
                "-m",
                "fixture",
            ],
        ] {
            let output = std::process::Command::new("git")
                .arg("-C")
                .arg(temp.path())
                .args(command)
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
        let output = std::process::Command::new("git")
            .arg("-C")
            .arg(temp.path())
            .args(["rev-parse", "HEAD"])
            .output()
            .unwrap();
        assert!(output.status.success());
        let head = String::from_utf8(output.stdout).unwrap();
        let result = call(&remote, "get_workspace_verification", args.clone())
            .await
            .unwrap();
        assert_eq!(result["current_head"], head.trim());
        std::fs::write(temp.path().join("uncommitted.txt"), "new change").unwrap();
        let result = call(&remote, "get_workspace_verification", args.clone())
            .await
            .unwrap();
        assert_eq!(result["current_worktree_verification"], "not_established");
        let output = std::process::Command::new("git")
            .arg("-C")
            .arg(temp.path())
            .args(["switch", "-c", "different"])
            .output()
            .unwrap();
        assert!(output.status.success());
        let result = call(&remote, "get_workspace_verification", args.clone())
            .await
            .unwrap();
        assert_eq!(result["current_head"], Value::Null);
        args["repository_id"] = json!(Uuid::new_v4());
        assert!(
            call(&remote, "get_workspace_verification", args.clone())
                .await
                .is_err()
        );
        args["repository_id"] = json!(repo.id);
        args["project_id"] = json!(Uuid::new_v4());
        assert!(
            call(&remote, "get_workspace_verification", args)
                .await
                .is_err()
        );
        for table in [
            "remote_mcp_operations",
            "project_chat_messages",
            "agent_run_commands",
        ] {
            assert_eq!(
                sqlx::query_scalar::<_, i64>(&format!("SELECT count(*) FROM {table}"))
                    .fetch_one(&remote.pool)
                    .await
                    .unwrap(),
                0
            );
        }
    }
}
