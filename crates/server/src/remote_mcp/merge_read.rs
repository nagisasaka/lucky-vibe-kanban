//! Read local merge evidence; publication remains exclusively in formal Integration.
use anyhow::ensure;
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::Row;
use uuid::Uuid;

use super::{
    Remote,
    evidence_read::Target,
    read::{arguments, definition, page},
};

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ListArgs {
    project_id: Uuid,
    #[serde(default = "super::default_limit")]
    #[schemars(range(min = 1, max = 50))]
    limit: u32,
    cursor: Option<Uuid>,
}
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct DetailArgs {
    #[serde(flatten)]
    target: Target,
}
const MEMBERS: &str = "SELECT wr.id,wr.workspace_id,wr.repo_id FROM workspace_repos wr JOIN workspaces w ON w.id=wr.workspace_id JOIN local_workspace_links l ON l.workspace_id=w.id JOIN local_issues i ON i.id=l.issue_id AND i.project_id=l.project_id WHERE l.project_id=?";

pub(super) fn tools() -> Vec<rmcp::model::Tool> {
    vec![
        definition::<ListArgs>(
            "list_project_merge_status",
            "Read bounded Workspace/repository merge observations for a Project. included means current committed source has local target-history evidence and no observed worktree changes; it does not prove tests or Wiki success. Remote refs are not fetched. Follow next_cursor.",
        ),
        definition::<DetailArgs>(
            "get_workspace_merge_status",
            "Inspect one Workspace repository's local source/target heads, uncommitted counts and matching formal publication evidence. Old merge history alone does not prove the current head is published. This never merges or fetches.",
        ),
    ]
}
pub(super) async fn call(remote: &Remote, name: &str, args: Value) -> anyhow::Result<Value> {
    match name {
        "list_project_merge_status" => remote.merge_list(arguments(args)?).await,
        "get_workspace_merge_status" => {
            remote
                .merge_status(arguments::<DetailArgs>(args)?.target)
                .await
        }
        _ => anyhow::bail!("Unknown merge read tool"),
    }
}
impl Remote {
    pub(super) async fn merge_status(&self, target: Target) -> anyhow::Result<Value> {
        let source = self.evidence_source(&target).await?;
        let issue = self
            .require_workspace(target.project_id, target.workspace_id)
            .await?;
        let root = source.root().ok();
        let branch = source.workspace.branch.clone();
        let target_branch = source.target_branch.clone();
        let observation = match root.clone() {
            Some(root) => {
                let branch = branch.clone();
                let target_branch = target_branch.clone();
                tokio::task::spawn_blocking(move || {
                    git::GitService::new().reference_merge_observation(
                        &root,
                        &branch,
                        &target_branch,
                    )
                })
                .await
                .ok()
                .and_then(Result::ok)
            }
            None => None,
        };
        let mut result = json!({"workspace_id":target.workspace_id,"repository_id":target.repository_id,"issue_id":issue,"target_branch":target_branch,"status":"unknown","reason":"Checkout, refs or status unavailable, changed, or unsupported","target_observation":"local_refs_without_fetch","verification":"not_observed","wiki":"not_observed"});
        if let (Some(observation), Some(root)) = (observation, root) {
            let rows=sqlx::query("SELECT DISTINCT r.id,json_extract(r.payload,'$.result_commit') AS result_commit FROM integration_runs r,json_each(r.payload,'$.sources') s WHERE r.project_id=? AND r.repository_id=? AND r.target_ref=? AND json_extract(r.payload,'$.published')=1 AND json_extract(s.value,'$.selection.workspace_id')=? AND json_extract(s.value,'$.selection.card_id')=? AND json_extract(s.value,'$.commit')=? ORDER BY r.updated_at DESC,r.id DESC LIMIT 51")
                .bind(target.project_id).bind(target.repository_id).bind(format!("refs/heads/{target_branch}")).bind(target.workspace_id.to_string()).bind(issue.to_string()).bind(&observation.source_head).fetch_all(&self.pool).await?;
            let more = rows.len() > 50;
            let receipts = rows
                .into_iter()
                .take(50)
                .map(|row| {
                    Ok((
                        row.try_get::<Uuid, _>("id")?,
                        row.try_get::<Option<String>, _>("result_commit")?,
                    ))
                })
                .collect::<Result<Vec<_>, sqlx::Error>>()?;
            // Old direct/PR records do not pin their source head. Keep this
            // ambiguity explicit instead of treating their presence as success.
            let legacy_commits:Vec<Option<String>>=sqlx::query_scalar("SELECT merge_commit FROM merges WHERE workspace_id=? AND repo_id=? AND target_branch_name=? AND merge_type='direct' UNION ALL SELECT merge_commit_sha FROM pull_requests WHERE workspace_id=? AND repo_id=? AND target_branch_name=? AND pr_status='merged' LIMIT 51")
                .bind(target.workspace_id).bind(target.repository_id).bind(&target_branch).bind(target.workspace_id).bind(target.repository_id).bind(&target_branch).fetch_all(&self.pool).await?;
            let legacy = !legacy_commits.is_empty();
            let checked = observation.clone();
            let target_branch2 = target_branch.clone();
            let evidence = tokio::task::spawn_blocking(move || {
                let git = git::GitService::new();
                let mut found = None;
                let mut unavailable = false;
                for (id, commit) in receipts {
                    match commit.map(|commit| git.is_ancestor(&root, &commit, &checked.target_head))
                    {
                        Some(Ok(true)) => {
                            found = Some(id);
                            break;
                        }
                        Some(Ok(false)) => {}
                        _ => unavailable = true,
                    }
                }
                // A later/unreadable legacy publication may itself be a squash
                // of this source. One older ancestor cannot resolve that ambiguity.
                let after_legacy_publication = !legacy_commits.is_empty()
                    && legacy_commits.len() <= 50
                    && legacy_commits.into_iter().all(|commit| {
                        commit.is_some_and(|commit| {
                            commit != checked.source_head
                                && git
                                    .is_ancestor(&root, &commit, &checked.source_head)
                                    .unwrap_or(false)
                                && git
                                    .is_ancestor(&root, &commit, &checked.target_head)
                                    .unwrap_or(false)
                        })
                    });
                let stable = git
                    .reference_merge_observation(&root, &branch, &target_branch2)
                    .is_ok_and(|now| now == checked);
                (found, unavailable, stable, after_legacy_publication)
            })
            .await
            .map_err(|_| anyhow::anyhow!("Merge inspection interrupted; refresh this read"))?;
            let dirty = observation.tracked_changes + observation.untracked_changes > 0;
            let (status, reason) = if !evidence.2 {
                ("unknown", "Repository changed during inspection")
            } else if observation.operation_in_progress || observation.conflicts > 0 {
                ("unknown", "Git operation or conflicts require inspection")
            } else if dirty {
                ("unmerged", "Uncommitted or untracked changes remain")
            } else if observation.source_in_target_history || evidence.0.is_some() {
                (
                    "included",
                    "Current committed source has target-history evidence",
                )
            } else if evidence.3 && !more && !evidence.1 {
                (
                    "unmerged",
                    "New source commits follow an observed legacy publication",
                )
            } else if legacy || more || evidence.1 {
                (
                    "unknown",
                    "Merge history cannot establish publication of this source head",
                )
            } else {
                (
                    "unmerged",
                    "Current source is outside observed target history with no matching formal publication",
                )
            };
            result["status"] = json!(status);
            result["reason"] = json!(reason);
            result["git"] = serde_json::to_value(observation)?;
            result["matching_integration_id"] = json!(evidence.0);
            result["publication_records_incomplete"] = json!(more);
            result["legacy_merge_history_present"] = json!(legacy);
            result["post_legacy_publication_changes_observed"] = json!(evidence.3);
        }
        self.evidence_link(&target, &mut result).await?;
        Ok(result)
    }
    async fn merge_list(&self, args: ListArgs) -> anyhow::Result<Value> {
        self.require_project(args.project_id).await?;
        ensure!(
            (1..=50).contains(&args.limit),
            "limit must be between 1 and 50"
        );
        if let Some(cursor) = args.cursor {
            let sql = format!("SELECT EXISTS(SELECT 1 FROM ({MEMBERS}) WHERE id=?)");
            let exists: bool = sqlx::query_scalar(&sql)
                .bind(args.project_id)
                .bind(cursor)
                .fetch_one(&self.pool)
                .await?;
            ensure!(exists, "Stale or foreign merge cursor; restart listing");
        }
        let sql =
            format!("SELECT * FROM ({MEMBERS}) WHERE (? IS NULL OR id>?) ORDER BY id LIMIT ?");
        let rows = sqlx::query(&sql)
            .bind(args.project_id)
            .bind(args.cursor)
            .bind(args.cursor)
            .bind(args.limit + 1)
            .fetch_all(&self.pool)
            .await?;
        let mut items = Vec::new();
        for row in rows {
            let id: Uuid = row.try_get("id")?;
            if items.len() == args.limit as usize {
                items.push(json!({"id":id}));
                break;
            }
            let mut result = self
                .merge_status(Target {
                    project_id: args.project_id,
                    workspace_id: row.try_get("workspace_id")?,
                    repository_id: row.try_get("repo_id")?,
                })
                .await?;
            result["id"] = json!(id);
            items.push(result);
        }
        Ok(page(items, args.limit))
    }
    pub(super) async fn project_merges(&self, project: Uuid) -> anyhow::Result<Value> {
        let sql = format!("{MEMBERS} ORDER BY wr.id");
        let rows = sqlx::query(&sql)
            .bind(project)
            .fetch_all(&self.pool)
            .await?;
        let mut counts = json!({"included":0,"unmerged":0,"unknown":0});
        let mut items = Vec::new();
        for row in &rows {
            let mut result = self
                .merge_status(Target {
                    project_id: project,
                    workspace_id: row.try_get("workspace_id")?,
                    repository_id: row.try_get("repo_id")?,
                })
                .await?;
            let status = result["status"].as_str().unwrap_or("unknown");
            counts[status] = json!(counts[status].as_u64().unwrap_or(0) + 1);
            if items.len() < 20 {
                result["id"] = json!(row.try_get::<Uuid, _>("id")?);
                items.push(result);
            }
        }
        let missing:i64=sqlx::query_scalar("SELECT count(*) FROM workspaces w JOIN local_workspace_links l ON l.workspace_id=w.id JOIN local_issues i ON i.id=l.issue_id AND i.project_id=l.project_id WHERE l.project_id=? AND NOT EXISTS(SELECT 1 FROM workspace_repos wr WHERE wr.workspace_id=w.id)")
            .bind(project).fetch_one(&self.pool).await?;
        let more = rows.len() > items.len();
        let next = if more {
            items.last().map(|item| item["id"].clone())
        } else {
            None
        };
        Ok(
            json!({"unit":"workspace_repository","counts":counts,"workspaces_without_repository":missing,"items":items,"has_more":more,"next_cursor":next,"next_tool":"list_project_merge_status","consistency":"live; refresh before any publication request"}),
        )
    }
}

#[cfg(test)]
mod tests {
    use std::{fs, path::Path, process::Command};

    use db::models::{
        integration::{IntegrationPayload, IntegrationSelection, IntegrationSource},
        repo::Repo,
        workspace_repo::{CreateWorkspaceRepo, WorkspaceRepo},
    };

    use super::*;
    fn git(root: &Path, args: &[&str]) -> String {
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
        String::from_utf8(output.stdout).unwrap().trim().into()
    }
    async fn fixture() -> (tempfile::TempDir, Remote, Target, Uuid) {
        let (remote, _, _, run, issue) = crate::remote_mcp::runtime_write_tests::fixture().await;
        let temp = tempfile::tempdir().unwrap();
        git(temp.path(), &["init", "-b", "main"]);
        git(temp.path(), &["config", "user.name", "Fixture"]);
        git(
            temp.path(),
            &["config", "user.email", "fixture@example.invalid"],
        );
        fs::write(temp.path().join("source.txt"), "base\n").unwrap();
        git(temp.path(), &["add", "source.txt"]);
        git(temp.path(), &["commit", "-m", "base"]);
        git(temp.path(), &["switch", "-c", "fixture"]);
        let repo = Repo::find_or_create(&remote.pool, temp.path(), "merge-fixture")
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
        sqlx::query(
            "UPDATE workspaces SET workspace_kind='direct_folder',container_ref=? WHERE id=?",
        )
        .bind(temp.path().to_str().unwrap())
        .bind(run.workspace_id)
        .execute(&remote.pool)
        .await
        .unwrap();
        (
            temp,
            remote,
            Target {
                project_id: run.project_id,
                workspace_id: run.workspace_id,
                repository_id: repo.id,
            },
            issue,
        )
    }
    #[tokio::test]
    async fn merge_reads_pin_current_heads_and_preserve_dirty_unknown_and_publication_states() {
        let (temp, remote, target, issue) = fixture().await;
        let clean = remote.merge_status(target.clone()).await.unwrap();
        assert_eq!(clean["status"], "included");
        assert_eq!(clean["wiki"], "not_observed");
        let index = fs::read(temp.path().join(".git/index")).unwrap();
        let marker = temp.path().join("fsmonitor-ran");
        git(
            temp.path(),
            &[
                "config",
                "core.fsmonitor",
                &format!("touch {}", marker.display()),
            ],
        );
        fs::write(temp.path().join(".env"), "private change\n").unwrap();
        let dirty = remote.merge_status(target.clone()).await.unwrap();
        assert_eq!(dirty["status"], "unmerged");
        assert_eq!(dirty["git"]["untracked_changes"], 1);
        assert!(!dirty.to_string().contains("private change"));
        assert!(!marker.exists());
        assert_eq!(fs::read(temp.path().join(".git/index")).unwrap(), index);
        git(temp.path(), &["config", "--unset", "core.fsmonitor"]);
        fs::remove_file(temp.path().join(".env")).unwrap();
        fs::write(temp.path().join("source.txt"), "feature\n").unwrap();
        assert_eq!(
            remote.merge_status(target.clone()).await.unwrap()["git"]["tracked_changes"],
            1
        );
        git(temp.path(), &["add", "source.txt"]);
        git(temp.path(), &["commit", "-m", "feature"]);
        let source = git(temp.path(), &["rev-parse", "HEAD"]);
        assert_eq!(
            remote.merge_status(target.clone()).await.unwrap()["status"],
            "unmerged"
        );
        git(temp.path(), &["switch", "main"]);
        git(temp.path(), &["merge", "--squash", "fixture"]);
        git(temp.path(), &["commit", "-m", "published candidate"]);
        let published = git(temp.path(), &["rev-parse", "HEAD"]);
        git(temp.path(), &["switch", "fixture"]);
        assert_ne!(source, published);
        let payload = IntegrationPayload {
            published: true,
            result_commit: Some(published),
            sources: vec![IntegrationSource {
                selection: IntegrationSelection {
                    card_id: issue,
                    workspace_id: target.workspace_id,
                    expected_commit: source.clone(),
                },
                branch: "fixture".into(),
                commit: source,
                title: "Fixture".into(),
                description: None,
                status_id: Uuid::new_v4(),
                requirements_revision: 1,
                event_ids: vec![],
                related_workspaces: vec![target.workspace_id],
                done_result: None,
            }],
            ..Default::default()
        };
        let integration = Uuid::new_v4();
        sqlx::query("INSERT INTO integration_runs(id,request_key,project_id,repository_id,storage_identity,target_ref,status,payload) VALUES(?,?,?,?, 'fixture','refs/heads/main','succeeded',?)")
            .bind(integration).bind(integration.to_string()).bind(target.project_id).bind(target.repository_id).bind(serde_json::to_string(&payload).unwrap()).execute(&remote.pool).await.unwrap();
        let included = remote.merge_status(target.clone()).await.unwrap();
        assert_eq!(included["status"], "included");
        assert_eq!(included["git"]["source_in_target_history"], false);
        assert_eq!(included["matching_integration_id"], json!(integration));
        fs::write(temp.path().join("source.txt"), "more work\n").unwrap();
        assert_eq!(
            remote.merge_status(target.clone()).await.unwrap()["status"],
            "unmerged"
        );
        git(temp.path(), &["add", "source.txt"]);
        git(temp.path(), &["commit", "-m", "more work"]);
        let newer = remote.merge_status(target.clone()).await.unwrap();
        assert_eq!(newer["status"], "unmerged");
        assert_eq!(newer["matching_integration_id"], Value::Null);
        db::models::merge::Merge::create_direct(
            &remote.pool,
            target.workspace_id,
            target.repository_id,
            "main",
            &git(temp.path(), &["rev-parse", "main"]),
        )
        .await
        .unwrap();
        assert_eq!(
            remote.merge_status(target.clone()).await.unwrap()["status"],
            "unknown"
        );
        git(temp.path(), &["reset", "--hard", "main"]);
        fs::write(
            temp.path().join("source.txt"),
            "work after native publication\n",
        )
        .unwrap();
        git(temp.path(), &["add", "source.txt"]);
        git(temp.path(), &["commit", "-m", "after publication"]);
        let after_publication = remote.merge_status(target.clone()).await.unwrap();
        assert_eq!(after_publication["status"], "unmerged");
        assert_eq!(
            after_publication["post_legacy_publication_changes_observed"],
            true
        );
        db::models::merge::Merge::create_direct(
            &remote.pool,
            target.workspace_id,
            target.repository_id,
            "main",
            &"f".repeat(40),
        )
        .await
        .unwrap();
        // Do not let an older, readable record hide a later ambiguous publication.
        assert_eq!(
            remote.merge_status(target.clone()).await.unwrap()["status"],
            "unknown"
        );
        git(temp.path(), &["config", "core.sparseCheckout", "true"]);
        assert_eq!(
            remote.merge_status(target.clone()).await.unwrap()["status"],
            "unknown"
        );
        git(temp.path(), &["config", "core.sparseCheckout", "false"]);
        git(temp.path(), &["switch", "-c", "different"]);
        assert_eq!(
            remote.merge_status(target.clone()).await.unwrap()["status"],
            "unknown"
        );
        let mut foreign = target;
        foreign.project_id = Uuid::new_v4();
        assert!(remote.merge_status(foreign).await.is_err());
        for table in ["remote_mcp_operations", "agent_run_commands"] {
            assert_eq!(
                sqlx::query_scalar::<_, i64>(&format!("SELECT count(*) FROM {table}"))
                    .fetch_one(&remote.pool)
                    .await
                    .unwrap(),
                0
            );
        }
    }

    #[tokio::test]
    async fn project_merge_pages_count_unknown_checkouts_without_claiming_success() {
        let (temp, remote, target, issue) = fixture().await;
        for n in 0..104 {
            let repo = Repo::find_or_create(
                &remote.pool,
                &temp.path().join(format!("absent-{n}")),
                "Missing",
            )
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
        }
        let summary = remote.project_merges(target.project_id).await.unwrap();
        assert_eq!(
            summary["counts"],
            json!({"included":1,"unmerged":0,"unknown":104})
        );
        assert_eq!(summary["items"].as_array().unwrap().len(), 20);
        assert_eq!(summary["has_more"], true);
        let mut args = json!({"project_id":target.project_id,"limit":50});
        let mut ids = std::collections::HashSet::new();
        loop {
            let result = call(&remote, "list_project_merge_status", args.clone())
                .await
                .unwrap();
            assert!(result["items"].as_array().unwrap().len() <= 50);
            assert!(!result.to_string().contains(temp.path().to_str().unwrap()));
            for item in result["items"].as_array().unwrap() {
                assert!(ids.insert(item["id"].as_str().unwrap().to_owned()));
            }
            if result["has_more"] == false {
                break;
            }
            args["cursor"] = result["next_cursor"].clone();
        }
        assert_eq!(ids.len(), 105);
        let next = call(
            &remote,
            "list_project_merge_status",
            json!({"project_id":target.project_id,"cursor":summary["next_cursor"]}),
        )
        .await
        .unwrap();
        assert_eq!(next["items"].as_array().unwrap().len(), 20);
        args["cursor"] = json!(Uuid::new_v4());
        assert!(
            call(&remote, "list_project_merge_status", args)
                .await
                .is_err()
        );
        let project2 = Uuid::new_v4();
        sqlx::query("INSERT INTO projects(id,name) VALUES(?,'Other')")
            .bind(project2)
            .execute(&remote.pool)
            .await
            .unwrap();
        sqlx::query("UPDATE local_issues SET project_id=? WHERE id=?")
            .bind(project2)
            .bind(issue)
            .execute(&remote.pool)
            .await
            .unwrap();
        assert_eq!(
            remote.project_merges(target.project_id).await.unwrap()["items"],
            json!([])
        );
    }
}
