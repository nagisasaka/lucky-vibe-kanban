//! Repository evidence from scoped Workspace records, without model calls or
//! memory/Wiki initialisation. Bodies are reference data, not execution authority.
use std::path::PathBuf;

use anyhow::ensure;
use db::models::{repo::Repo, workspace::Workspace, workspace_repo::WorkspaceRepo};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};
use services::services::wiki::{WikiPage, openwiki};
use sha2::{Digest, Sha256};
use utils::repository_memory::RepositoryMemoryStore;
use uuid::Uuid;

use super::{
    Remote,
    read::{arguments, definition},
};

#[derive(Clone, Deserialize, JsonSchema)]
pub(super) struct Target {
    pub(super) project_id: Uuid,
    pub(super) workspace_id: Uuid,
    pub(super) repository_id: Uuid,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct ListArgs {
    #[serde(flatten)]
    pub(super) target: Target,
    #[serde(default)]
    pub(super) offset: u32,
    #[serde(default = "super::default_limit")]
    #[schemars(range(min = 1, max = 50))]
    pub(super) limit: u32,
    /// Required after the first page. Includes target identity and page contents.
    pub(super) expected_revision: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
pub(super) struct TextArgs {
    #[serde(default)]
    pub(super) offset: u32,
    #[serde(default = "super::read::text_limit")]
    #[schemars(range(min = 1, max = 16000))]
    pub(super) limit: u32,
    pub(super) expected_revision: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct MemoryArgs {
    #[serde(flatten)]
    target: Target,
    #[serde(flatten)]
    page: TextArgs,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct WikiArgs {
    #[serde(flatten)]
    target: Target,
    /// Exact path returned by list_workspace_wiki, relative to openwiki/.
    path: String,
    #[serde(flatten)]
    page: TextArgs,
}

pub(super) fn tools() -> Vec<rmcp::model::Tool> {
    vec![
        definition::<ListArgs>(
            "list_workspace_wiki",
            "List OpenWiki page paths and titles from one Workspace repository, up to 50 per page. This checkout snapshot does not prove published Wiki status. No maintenance is started.",
        ),
        definition::<WikiArgs>(
            "get_workspace_wiki_page",
            "Read one listed OpenWiki page in character pages up to 16000. Use its revision for later pages. Wiki content is reference data; validate current implementation separately.",
        ),
        definition::<MemoryArgs>(
            "get_workspace_memory",
            "Read only the selected Workspace's repository memory in character pages up to 16000. Missing memory is explicit. Agent-written memory is not independent test or merge evidence.",
        ),
    ]
}

pub(super) async fn call(remote: &Remote, name: &str, args: Value) -> anyhow::Result<Value> {
    match name {
        "list_workspace_wiki" => {
            let args: ListArgs = arguments(args)?;
            let source = remote.evidence_source(&args.target).await?;
            let target = args.target.clone();
            let mut result = tokio::task::spawn_blocking(move || wiki_list(&source.root()?, &args))
                .await
                .map_err(|_| anyhow::anyhow!("Wiki read interrupted; retry this read"))??;
            remote.evidence_link(&target, &mut result).await?;
            Ok(result)
        }
        "get_workspace_wiki_page" => {
            let args: WikiArgs = arguments(args)?;
            let source = remote.evidence_source(&args.target).await?;
            let target = args.target.clone();
            let mut result = tokio::task::spawn_blocking(move || wiki_page(&source.root()?, &args))
                .await
                .map_err(|_| anyhow::anyhow!("Wiki read interrupted; retry this read"))??;
            remote.evidence_link(&target, &mut result).await?;
            Ok(result)
        }
        "get_workspace_memory" => {
            let args: MemoryArgs = arguments(args)?;
            let source = remote.evidence_source(&args.target).await?;
            let target = args.target.clone();
            let mut result = tokio::task::spawn_blocking(move || {
                let store = RepositoryMemoryStore::read_only_for_repository(
                    &source.repo.name,
                    source.repo.id,
                )
                .map_err(|_| {
                    anyhow::anyhow!("Memory unavailable: storage cannot be safely read")
                })?;
                memory_page(store.as_ref(), &args)
            })
            .await
            .map_err(|_| anyhow::anyhow!("Memory read interrupted; retry this read"))??;
            remote.evidence_link(&target, &mut result).await?;
            Ok(result)
        }
        _ => anyhow::bail!("Unknown evidence read tool"),
    }
}

pub(super) struct Source {
    pub(super) repo: Repo,
    pub(super) workspace: Workspace,
    pub(super) target_branch: String,
}
impl Source {
    pub(super) fn root(&self) -> anyhow::Result<PathBuf> {
        ensure!(
            !self.workspace.worktree_deleted,
            "Workspace checkout was deleted; repository evidence unavailable"
        );
        let root = if self.workspace.is_direct_folder() {
            self.repo.path.clone()
        } else {
            // The stored checkout location is never taken from tool parameters.
            PathBuf::from(
                self.workspace
                    .container_ref
                    .as_deref()
                    .ok_or_else(|| anyhow::anyhow!("Workspace checkout is not available"))?,
            )
            .join(&self.repo.name)
        };
        let root = root
            .canonicalize()
            .map_err(|_| anyhow::anyhow!("Workspace checkout is not available"))?;
        ensure!(root.is_dir(), "Workspace checkout is not a directory");
        Ok(root)
    }
}
impl Remote {
    pub(super) async fn evidence_source(&self, target: &Target) -> anyhow::Result<Source> {
        self.require_workspace(target.project_id, target.workspace_id)
            .await?;
        let membership = WorkspaceRepo::find_by_workspace_and_repo_id(
            &self.pool,
            target.workspace_id,
            target.repository_id,
        )
        .await?
        .ok_or_else(|| anyhow::anyhow!("Repository does not belong to this Workspace"))?;
        let repo = Repo::find_by_id(&self.pool, target.repository_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Repository no longer exists"))?;
        let workspace = Workspace::find_by_id(&self.pool, target.workspace_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Workspace no longer exists"))?;
        Ok(Source {
            repo,
            workspace,
            target_branch: membership.target_branch,
        })
    }
    pub(super) async fn evidence_link(
        &self,
        target: &Target,
        result: &mut Value,
    ) -> anyhow::Result<()> {
        // Recheck membership after I/O instead of returning a deleted/relinked target.
        let issue = self
            .require_workspace(target.project_id, target.workspace_id)
            .await?;
        ensure!(
            WorkspaceRepo::find_by_workspace_and_repo_id(
                &self.pool,
                target.workspace_id,
                target.repository_id
            )
            .await?
            .is_some(),
            "Repository no longer belongs to this Workspace"
        );
        result["project_id"] = json!(target.project_id);
        result["workspace_id"] = json!(target.workspace_id);
        result["repository_id"] = json!(target.repository_id);
        result["url"] = json!(self.config.workspace_url(
            target.project_id,
            issue,
            target.workspace_id
        ));
        result["observed_at"] = json!(chrono::Utc::now());
        Ok(())
    }
}

pub(super) fn revision(target: &Target, kind: &str, bytes: &[u8]) -> String {
    let mut hash = Sha256::new();
    hash.update(target.project_id.as_bytes());
    hash.update(target.workspace_id.as_bytes());
    hash.update(target.repository_id.as_bytes());
    hash.update(kind.as_bytes());
    hash.update([0]);
    hash.update(bytes);
    format!("{:x}", hash.finalize())
}
pub(super) fn check_page(
    offset: u32,
    limit: u32,
    max: u32,
    expected: Option<&str>,
    current: &str,
    total: usize,
) -> anyhow::Result<()> {
    ensure!(
        (1..=max).contains(&limit),
        "Page limit is outside the allowed range"
    );
    ensure!(
        offset == 0 || expected.is_some(),
        "expected_revision is required after the first page"
    );
    ensure!(
        expected.is_none_or(|value| value == current),
        "Stale or foreign revision; restart this read"
    );
    ensure!(
        offset as usize <= total,
        "Page offset is outside the current content"
    );
    Ok(())
}
pub(super) fn text_page(
    target: &Target,
    kind: &str,
    text: Option<&str>,
    args: &TextArgs,
) -> anyhow::Result<Value> {
    let current = revision(target, kind, &serde_json::to_vec(&text)?);
    let total = text.unwrap_or_default().chars().count();
    check_page(
        args.offset,
        args.limit,
        16000,
        args.expected_revision.as_deref(),
        &current,
        total,
    )?;
    let end = (args.offset as usize + args.limit as usize).min(total);
    Ok(
        json!({"available":text.is_some(),"revision":current,"content":text.map(|text| text.chars().skip(args.offset as usize).take(args.limit as usize).collect::<String>()),"offset":args.offset,"total_characters":total,"has_more":end<total,"next_offset":if end<total {Some(end)} else {None},"evidence":"reference_only"}),
    )
}
fn memory_page(store: Option<&RepositoryMemoryStore>, args: &MemoryArgs) -> anyhow::Result<Value> {
    let text = store
        .map(|store| store.read_memory(args.target.workspace_id))
        .transpose()
        .map_err(|_| {
            anyhow::anyhow!("Memory unavailable: record is unsafe, unreadable or exceeds 128 KiB")
        })?
        .flatten();
    text_page(&args.target, "memory", text.as_deref(), &args.page)
}
fn wiki_pages(root: &std::path::Path) -> anyhow::Result<(bool, Vec<WikiPage>)> {
    let snapshot = openwiki::load_snapshot(root)
        .map_err(|_| anyhow::anyhow!("Wiki unavailable: snapshot is unsafe, invalid, unreadable or exceeds Viewer limits (512 KiB/page, 2000 pages, 16 MiB total)"))?;
    let mut pages = snapshot.pages;
    pages.extend(snapshot.index);
    pages.sort_by(|a, b| a.path.cmp(&b.path));
    Ok((snapshot.exists, pages))
}
fn wiki_list(root: &std::path::Path, args: &ListArgs) -> anyhow::Result<Value> {
    let (exists, pages) = wiki_pages(root)?;
    // Hash the snapshot, not just filenames: edits during traversal require refresh.
    let current = revision(
        &args.target,
        "wiki-list",
        &serde_json::to_vec(&(exists, &pages))?,
    );
    check_page(
        args.offset,
        args.limit,
        50,
        args.expected_revision.as_deref(),
        &current,
        pages.len(),
    )?;
    let end = (args.offset as usize + args.limit as usize).min(pages.len());
    let items: Vec<_> = pages.iter().skip(args.offset as usize).take(args.limit as usize).map(|page| {
        let title = page.metadata.as_ref().map(|m| m.title.as_str()).unwrap_or(&page.path);
        json!({"path":page.path,"title":title.chars().take(500).collect::<String>(),"title_truncated":title.chars().count()>500})
    }).collect();
    Ok(
        json!({"exists":exists,"items":items,"revision":current,"offset":args.offset,"has_more":end<pages.len(),"next_offset":if end<pages.len() {Some(end)} else {None},"source":"workspace_checkout","publication_status":"not_observed"}),
    )
}
fn wiki_page(root: &std::path::Path, args: &WikiArgs) -> anyhow::Result<Value> {
    ensure!(args.path.len() <= 4096, "Wiki path is too long");
    let (_, pages) = wiki_pages(root)?;
    let page = pages
        .into_iter()
        .find(|p| p.path == args.path)
        .ok_or_else(|| anyhow::anyhow!("Wiki page not found; refresh list_workspace_wiki"))?;
    let mut result = text_page(
        &args.target,
        &format!("wiki:{}", args.path),
        Some(&page.content),
        &args.page,
    )?;
    result["path"] = json!(args.path);
    result["source"] = json!("workspace_checkout");
    result["publication_status"] = json!("not_observed");
    Ok(result)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use db::models::workspace_repo::CreateWorkspaceRepo;

    use super::*;

    fn target() -> Target {
        Target {
            project_id: Uuid::from_u128(1),
            workspace_id: Uuid::from_u128(2),
            repository_id: Uuid::from_u128(3),
        }
    }
    fn text_args() -> TextArgs {
        TextArgs {
            offset: 0,
            limit: 8000,
            expected_revision: None,
        }
    }

    #[test]
    fn wiki_pages_are_bounded_versioned_and_read_only() {
        let temp = tempfile::tempdir().unwrap();
        let mut list = ListArgs {
            target: target(),
            offset: 0,
            limit: 50,
            expected_revision: None,
        };
        assert_eq!(wiki_list(temp.path(), &list).unwrap()["exists"], false);
        assert!(!temp.path().join("openwiki").exists());
        fs::create_dir(temp.path().join("openwiki")).unwrap();
        for i in 0..105 {
            fs::write(
                temp.path().join(format!("openwiki/{i:03}.md")),
                format!("---\ntitle: Page {i}\n---\n{}", "境界".repeat(9000)),
            )
            .unwrap();
        }
        fs::write(
            temp.path().join("openwiki/INSTRUCTIONS.md"),
            "hidden operator instructions",
        )
        .unwrap();
        let first = wiki_list(temp.path(), &list).unwrap();
        assert_eq!(first["items"].as_array().unwrap().len(), 50);
        assert_eq!(first["next_offset"], 50);
        assert!(!first.to_string().contains("境界"));
        list.offset = 50;
        assert!(wiki_list(temp.path(), &list).is_err());
        list.expected_revision = first["revision"].as_str().map(str::to_owned);
        assert_eq!(
            wiki_list(temp.path(), &list).unwrap()["items"]
                .as_array()
                .unwrap()
                .len(),
            50
        );
        list.offset = 100;
        let last = wiki_list(temp.path(), &list).unwrap();
        assert_eq!(last["items"].as_array().unwrap().len(), 5);
        assert_eq!(last["has_more"], false);
        let mut args = WikiArgs {
            target: target(),
            path: "000.md".into(),
            page: text_args(),
        };
        let page = wiki_page(temp.path(), &args).unwrap();
        assert_eq!(page["content"].as_str().unwrap().chars().count(), 8000);
        assert_eq!(page["total_characters"], 18000);
        args.page.offset = 8000;
        args.page.expected_revision = page["revision"].as_str().map(str::to_owned);
        assert!(wiki_page(temp.path(), &args).is_ok());
        args.target.workspace_id = Uuid::new_v4();
        assert!(wiki_page(temp.path(), &args).is_err());
        args.target = target();
        fs::write(temp.path().join("openwiki/000.md"), "changed").unwrap();
        assert!(wiki_page(temp.path(), &args).is_err());
        assert!(wiki_list(temp.path(), &list).is_err());
        args.page = text_args();
        for path in ["../secret", "/etc/passwd", "INSTRUCTIONS.md", "missing.md"] {
            args.path = path.into();
            assert!(wiki_page(temp.path(), &args).is_err());
        }
        list.offset = 0;
        list.expected_revision = None;
        list.limit = 51;
        assert!(wiki_list(temp.path(), &list).is_err());
        assert!(!temp.path().join(".llm-wiki").exists());
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink("/etc/passwd", temp.path().join("openwiki/unsafe.md"))
                .unwrap();
            let error = wiki_list(temp.path(), &list).unwrap_err().to_string();
            assert!(!error.contains("/etc") && !error.contains(temp.path().to_str().unwrap()));
        }
    }

    #[test]
    fn memory_read_never_provisions_and_separates_missing_stale_and_unsafe() {
        let temp = tempfile::tempdir().unwrap();
        assert!(
            RepositoryMemoryStore::read_only_at_persistent(temp.path())
                .unwrap()
                .is_none()
        );
        assert!(!temp.path().join("knowledge").exists());
        // An incomplete existing store must remain incomplete; no state or other directories are created.
        fs::create_dir_all(temp.path().join("knowledge/workspace-memory")).unwrap();
        let store = RepositoryMemoryStore::read_only_at_persistent(temp.path())
            .unwrap()
            .unwrap();
        let mut args = MemoryArgs {
            target: target(),
            page: text_args(),
        };
        assert_eq!(
            memory_page(Some(&store), &args).unwrap()["available"],
            false
        );
        fs::write(
            store.memory_path(args.target.workspace_id),
            "日本語".repeat(7000),
        )
        .unwrap();
        fs::write(
            store.memory_path(Uuid::new_v4()),
            "other Workspace private memory",
        )
        .unwrap();
        let first = memory_page(Some(&store), &args).unwrap();
        assert_eq!(first["content"].as_str().unwrap().chars().count(), 8000);
        assert!(!first.to_string().contains("other Workspace"));
        args.page.offset = 8000;
        args.page.expected_revision = first["revision"].as_str().map(str::to_owned);
        assert!(memory_page(Some(&store), &args).is_ok());
        fs::write(store.memory_path(args.target.workspace_id), "changed").unwrap();
        assert!(memory_page(Some(&store), &args).is_err());
        args.page = text_args();
        fs::write(
            store.memory_path(args.target.workspace_id),
            "a".repeat(128 * 1024 + 1),
        )
        .unwrap();
        assert!(memory_page(Some(&store), &args).is_err());
        assert!(!temp.path().join("knowledge/state.json").exists());
        assert!(!temp.path().join("knowledge/events").exists());
        #[cfg(unix)]
        {
            fs::remove_file(store.memory_path(args.target.workspace_id)).unwrap();
            std::os::unix::fs::symlink("/etc/passwd", store.memory_path(args.target.workspace_id))
                .unwrap();
            let error = memory_page(Some(&store), &args).unwrap_err().to_string();
            assert!(!error.contains("/etc") && !error.contains(temp.path().to_str().unwrap()));
        }
    }

    #[tokio::test]
    async fn evidence_tools_enforce_database_membership_and_links_without_side_effects() {
        let (remote, _, _, run, issue) = super::super::runtime_write_tests::fixture().await;
        let temp = tempfile::tempdir().unwrap();
        let repo = Repo::find_or_create(&remote.pool, temp.path(), "evidence-fixture")
            .await
            .unwrap();
        let checkout = temp.path().join(&repo.name);
        fs::create_dir_all(checkout.join("openwiki")).unwrap();
        fs::write(checkout.join("openwiki/index.md"), "# Real fixture Wiki").unwrap();
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
        sqlx::query("UPDATE workspaces SET container_ref=? WHERE id=?")
            .bind(temp.path().to_str().unwrap())
            .bind(run.workspace_id)
            .execute(&remote.pool)
            .await
            .unwrap();
        let args = json!({"project_id":run.project_id,"workspace_id":run.workspace_id,"repository_id":repo.id});
        let list = call(&remote, "list_workspace_wiki", args.clone())
            .await
            .unwrap();
        assert_eq!(list["items"][0]["path"], "index.md");
        assert_eq!(
            list["url"],
            remote
                .config
                .workspace_url(run.project_id, issue, run.workspace_id)
        );
        let mut page = args.clone();
        page["path"] = json!("index.md");
        page["offset"] = json!(0);
        page["limit"] = json!(16000);
        assert_eq!(
            call(&remote, "get_workspace_wiki_page", page)
                .await
                .unwrap()["content"],
            "# Real fixture Wiki"
        );
        let mut unknown = args.clone();
        unknown["arbitrary_path"] = json!("/etc/passwd");
        assert!(call(&remote, "list_workspace_wiki", unknown).await.is_err());
        for key in ["project_id", "workspace_id", "repository_id"] {
            let mut wrong = args.clone();
            wrong[key] = json!(Uuid::new_v4());
            for name in ["list_workspace_wiki", "get_workspace_memory"] {
                assert!(
                    call(&remote, name, wrong.clone()).await.is_err(),
                    "{name}: {key}"
                );
            }
        }
        let count: i64 = sqlx::query_scalar("SELECT count(*) FROM agent_runs")
            .fetch_one(&remote.pool)
            .await
            .unwrap();
        assert_eq!(count, 1);
        let count: i64 = sqlx::query_scalar("SELECT count(*) FROM project_chat_messages")
            .fetch_one(&remote.pool)
            .await
            .unwrap();
        assert_eq!(count, 0);
        sqlx::query("UPDATE workspaces SET worktree_deleted=1 WHERE id=?")
            .bind(run.workspace_id)
            .execute(&remote.pool)
            .await
            .unwrap();
        assert!(call(&remote, "list_workspace_wiki", args).await.is_err());
    }
}
