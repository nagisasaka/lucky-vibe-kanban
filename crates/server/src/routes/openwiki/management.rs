//! Explicit settings writes/deletion; no model execution or implicit generation.
use serde::Serialize;
use services::services::openwiki::{WikiPublicationRequest, management as files};
use utils::repository_memory::RepositoryWikiFileOperation;

use super::*;

#[derive(Debug, Serialize, TS)]
pub struct RepositoryWikiFile {
    pub path: String,
    pub content: String,
    pub default_content: String,
    pub exists: bool,
}

#[derive(Debug, Serialize, TS)]
pub struct RepositoryWikiFiles {
    pub repository_id: Uuid,
    pub branch: String,
    pub source: String,
    pub files: Vec<RepositoryWikiFile>,
}

#[derive(Debug, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct SaveRepositoryWikiFile {
    pub branch: String,
    pub source: String,
    pub path: String,
    pub content: String,
}

#[derive(Debug, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct DeleteRepositoryWiki {
    pub branch: String,
    pub source: String,
    pub confirmation: String,
}

#[derive(Debug, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct RebuildRepositoryWiki {
    pub branch: String,
    pub source: String,
    #[ts(optional)]
    pub project_id: Option<Uuid>,
}

/// The selected branch is authoritative; when its checkout is available, show
/// the actual user file as well, including edits that Git has not committed.
fn worktree_content(root: &FsPath, path: &str) -> anyhow::Result<Option<String>> {
    let path = root.join(path);
    reject_symlinks(&path)?;
    match std::fs::metadata(&path) {
        Ok(metadata) => {
            anyhow::ensure!(
                metadata.is_file() && metadata.len() <= 1024 * 1024,
                "Configuration is not a regular UTF-8 file within the size limit"
            );
            Ok(Some(std::fs::read_to_string(path)?))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.into()),
    }
}

pub(super) fn ensure_configuration_clean(
    deployment: &DeploymentImpl,
    repo: &Repo,
    source: &str,
    paths: &[&str],
) -> anyhow::Result<()> {
    if deployment.git().get_head_info(&repo.path)?.oid != source {
        return Ok(());
    }
    let snapshot = deployment.git().snapshot(&repo.path, source)?;
    let entries = snapshot.entries()?;
    for path in paths {
        let committed = entries
            .iter()
            .find(|entry| entry.path == *path)
            .map(|entry| snapshot.read_blob(entry, 1024 * 1024))
            .transpose()?
            .flatten();
        let actual = worktree_content(&repo.path, path)?.map(String::into_bytes);
        anyhow::ensure!(
            actual == committed,
            "{path} has uncommitted changes; commit or resolve them before repository settings save or Wiki generation. The actual file was preserved."
        );
    }
    Ok(())
}

pub async fn get_files(
    State(deployment): State<DeploymentImpl>,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<RepositoryWikiFiles>>, ApiError> {
    let repo = repo(&deployment, id).await?;
    let (state, _) = status_snapshot(&deployment, id).await?;
    let branch = state
        .target_branch
        .context("Choose a target branch")
        .map_err(api_error)?;
    let source = deployment.git().get_branch_oid(&repo.path, &branch)?;
    let snapshot = deployment.git().snapshot(&repo.path, &source)?;
    let entries = snapshot.entries()?;
    let mut values = Vec::new();
    for (path, default) in [
        (files::INSTRUCTIONS, files::DEFAULT_INSTRUCTIONS),
        (files::IGNORE, files::DEFAULT_IGNORE),
    ] {
        let entry = entries.iter().find(|entry| entry.path == path);
        let actual = if deployment.git().get_head_info(&repo.path)?.oid == source {
            worktree_content(&repo.path, path).map_err(api_error)?
        } else {
            None
        };
        let exists = actual.is_some() || entry.is_some();
        let content = if let Some(actual) = actual {
            actual
        } else if let Some(entry) = entry {
            String::from_utf8(
                snapshot
                    .read_blob(entry, 1024 * 1024)?
                    .context("Configuration file too large")
                    .map_err(api_error)?,
            )
            .map_err(api_error)?
        } else {
            default.into()
        };
        values.push(RepositoryWikiFile {
            path: path.into(),
            content,
            default_content: default.into(),
            exists,
        });
    }
    Ok(Json(ApiResponse::success(RepositoryWikiFiles {
        repository_id: id,
        branch,
        source,
        files: values,
    })))
}

pub async fn save_file(
    State(deployment): State<DeploymentImpl>,
    Path(id): Path<Uuid>,
    Json(request): Json<SaveRepositoryWikiFile>,
) -> Result<Json<ApiResponse<RepositoryWikiFiles>>, ApiError> {
    if !files::configuration_path(&request.path) || request.content.len() > 1024 * 1024 {
        return Err(api_error("Invalid OpenWiki configuration file or size"));
    }
    mutate(
        &deployment,
        id,
        &request.branch,
        &request.source,
        Some((&request.path, &request.content)),
        None,
    )
    .await
    .map_err(api_error)?;
    get_files(State(deployment), Path(id)).await
}

pub async fn delete_wiki(
    State(deployment): State<DeploymentImpl>,
    Path(id): Path<Uuid>,
    Json(request): Json<DeleteRepositoryWiki>,
) -> Result<Json<ApiResponse<RepositoryMemoryState>>, ApiError> {
    mutate(
        &deployment,
        id,
        &request.branch,
        &request.source,
        None,
        Some(&request.confirmation),
    )
    .await
    .map_err(api_error)?;
    get_status(State(deployment), Path(id)).await
}

async fn mutate(
    deployment: &DeploymentImpl,
    id: Uuid,
    branch: &str,
    source: &str,
    save: Option<(&str, &str)>,
    confirmation: Option<&str>,
) -> anyhow::Result<()> {
    let _admission = services::services::integration_admission::MUTATIONS
        .lock()
        .await;
    let repo = repo(deployment, id).await?;
    if save.is_none() {
        anyhow::ensure!(
            confirmation == Some(format!("{}/openwiki", repo.name).as_str()),
            "Repository deletion confirmation does not match"
        );
    }
    workspace_manager::shared_resources::ensure_repository_memory(&deployment.db().pool, &repo)
        .await?;
    let store = RepositoryMemoryStore::for_repository(&deployment.db().pool, &repo.name, id)?;
    let _lock = store.try_lock()?;
    let mut state = store.state().await?;
    anyhow::ensure!(
        state.active_run_id.is_none()
            && state.bootstrap.is_none()
            && state.file_operation.is_none(),
        "Repository maintenance is active"
    );
    anyhow::ensure!(
        state.target_branch.as_deref() == Some(branch)
            && deployment.git().get_branch_oid(&repo.path, branch)? == source,
        "Repository target changed; reload settings before saving"
    );
    anyhow::ensure!(
        !deployment.git().is_remote_branch(&repo.path, branch)?,
        "Select a local integrated branch"
    );
    if let Some((path, _)) = save {
        ensure_configuration_clean(deployment, &repo, source, &[path])?;
    }
    recover_integrations(&store, &repo.path).await?;
    anyhow::ensure!(
        !unresolved_integration(&store).await?,
        "Source integration is unresolved"
    );
    let storage = deployment
        .git()
        .storage_identity(&repo.path)?
        .to_string_lossy()
        .into_owned();
    let pending = integration_busy(&deployment.db().pool, &storage).await?;
    anyhow::ensure!(
        !pending,
        "Auto-merge is active; finish it before changing Wiki files"
    );
    let workspace = super::super::workspaces::create::create_workspace_record_with_owner(
        deployment,
        Some(format!("OpenWiki settings: {}", repo.display_name)),
        Some(&db::models::workspace_usage::WorkspaceExecutionOwner::new(
            db::models::workspace_usage::OPENWIKI_SYNC,
            repo.id,
            None,
        )),
    )
    .await?;
    let result = async {
        let preparation = preparation::begin(&deployment.db().pool, workspace.id).await?;
        WorkspaceRepo::create_many(
            &deployment.db().pool,
            workspace.id,
            &[CreateWorkspaceRepo {
                repo_id: id,
                target_branch: branch.into(),
            }],
        )
        .await?;
        let container = deployment
            .container()
            .ensure_container_exists(&workspace)
            .await?;
        let root = PathBuf::from(container).join(&repo.name);
        anyhow::ensure!(
            deployment.git().get_head_info(&root)?.oid == source
                && deployment
                    .git()
                    .get_diff_file_paths(&root, &source.parse()?)?
                    .is_empty(),
            "Repository setup changed the maintenance snapshot"
        );
        let paths = if let Some((path, content)) = save {
            let destination = root.join(path);
            reject_symlinks(&destination)?;
            std::fs::create_dir_all(
                destination
                    .parent()
                    .context("Missing configuration parent")?,
            )?;
            std::fs::write(destination, content)?;
            vec![path.into()]
        } else {
            let mut paths = files::remove_generated(&root, source)?;
            anyhow::ensure!(!paths.is_empty(), "No generated Wiki to delete");
            for name in ["AGENTS.md", "CLAUDE.md"] {
                let path = root.join(name);
                reject_symlinks(&path)?;
                if path.exists() {
                    let original = std::fs::read_to_string(&path)?;
                    let updated = files::remove_managed_block(&original)?;
                    if original != updated {
                        std::fs::write(path, updated)?;
                        paths.push(name.into());
                    }
                }
            }
            paths
        };
        // Persist ownership before any Git publication. Recovery replays only this
        // immutable candidate, never reruns deletion against a newer branch.
        state.file_operation = Some(RepositoryWikiFileOperation {
            id: Uuid::new_v4(),
            workspace_id: workspace.id,
            source: source.into(),
            branch: branch.into(),
            paths,
            delete: save.is_none(),
        });
        let operation = state.file_operation.as_ref().unwrap();
        Workspace::bind_execution_owner(
            &deployment.db().pool,
            workspace.id,
            db::models::workspace_usage::OPENWIKI_SYNC,
            operation.id,
        )
        .await?;
        state.maintenance_workspace_id = Some(workspace.id);
        state.maintenance_session_id = None;
        store.save_state(&state).await?;
        drop(preparation);
        recover(deployment, &repo, &store, &mut state).await
    }
    .await;
    if let Err(error) = &result
        && store.state().await?.file_operation.is_none()
        && let Some(current) = Workspace::find_by_id(&deployment.db().pool, workspace.id).await?
        && let Some(owner) = current.execution_owner
        && owner.result.is_none()
    {
        Workspace::save_execution_result(
            &deployment.db().pool,
            workspace.id,
            db::models::workspace_usage::OPENWIKI_SYNC,
            owner.run_id,
            &db::models::workspace_usage::WorkspaceExecutionResult {
                status: db::models::workspace_usage::WorkspaceExecutionTerminalStatus::Failed,
                completed_at: chrono::Utc::now(),
                error: Some(format!("{error:#}")),
                wiki_commit: None,
                no_op: false,
            },
        )
        .await?;
    }
    result
}

pub(super) async fn recover(
    deployment: &DeploymentImpl,
    repo: &Repo,
    store: &RepositoryMemoryStore,
    state: &mut RepositoryMemoryState,
) -> anyhow::Result<()> {
    let operation = state
        .file_operation
        .clone()
        .context("Missing file operation")?;
    let workspace = Workspace::find_by_id(&deployment.db().pool, operation.workspace_id)
        .await?
        .context("Maintenance workspace missing")?;
    let root = PathBuf::from(
        workspace
            .container_ref
            .as_ref()
            .context("Maintenance path missing")?,
    )
    .join(&repo.name);
    let request = WikiPublicationRequest {
        repository_root: &repo.path,
        maintenance_root: &root,
        maintenance_branch: &workspace.branch,
        target_branch: &operation.branch,
        source_commit: &operation.source,
        run_id: operation.id,
    };
    let result = services::services::openwiki::publish_repository_files(
        deployment.git(),
        store,
        &request,
        Some(&operation.paths),
    )
    .await;
    if let Err(error) = &result {
        state.error = Some(format!("OpenWiki file publication unresolved: {error:#}"));
        state.status = RepositoryWikiStatus::Error;
        if store.publication(operation.id).await?.is_none() {
            // No durable publication exists, so canonical Git was never touched.
            // Settle this failed candidate and permit a deliberate fresh request.
            state.file_operation = None;
            let outcome = db::models::workspace_usage::WorkspaceExecutionResult {
                status: db::models::workspace_usage::WorkspaceExecutionTerminalStatus::Failed,
                completed_at: chrono::Utc::now(),
                error: state.error.clone(),
                wiki_commit: None,
                no_op: false,
            };
            let mut tx = deployment.db().pool.begin_with("BEGIN IMMEDIATE").await?;
            Workspace::save_execution_result_on(
                &mut tx,
                operation.workspace_id,
                db::models::workspace_usage::OPENWIKI_SYNC,
                Some(operation.id),
                &outcome,
            )
            .await?;
            store.save_reconciliation_on(&mut tx, state, &[]).await?;
            tx.commit().await?;
        } else {
            store.save_state(state).await?;
        }
    }
    let (wiki_commit, no_op) = result?;
    let outcome = db::models::workspace_usage::WorkspaceExecutionResult {
        status: db::models::workspace_usage::WorkspaceExecutionTerminalStatus::Succeeded,
        completed_at: chrono::Utc::now(),
        error: None,
        wiki_commit,
        no_op,
    };
    if operation.delete {
        state.source_commit = None;
        state.wiki_commit = None;
        state.last_success = None;
        state.unverified_before = None;
        state.wiki_exists = false;
        state.generation_suppressed = true;
        state.rebuild_reference = None;
        state.active_source_commit = None;
        state.active_event_ids.clear();
        state.active_sync_input_digest = None;
        state.active_sync_scope = RepositoryWikiSyncScope::default();
        state.maintenance_workspace_id = None;
        state.maintenance_session_id = None;
        state.status = if state.enabled {
            RepositoryWikiStatus::Uninitialized
        } else {
            RepositoryWikiStatus::Disabled
        };
    }
    if !operation.delete {
        state.status = if !state.enabled {
            RepositoryWikiStatus::Disabled
        } else if state.last_success.is_none() {
            RepositoryWikiStatus::Uninitialized
        } else {
            RepositoryWikiStatus::Stale
        };
    }
    state.file_operation = None;
    state.error = None;
    state.maintenance_workspace_id = None;
    state.maintenance_session_id = None;
    let mut tx = deployment.db().pool.begin_with("BEGIN IMMEDIATE").await?;
    Workspace::save_execution_result_on(
        &mut tx,
        operation.workspace_id,
        db::models::workspace_usage::OPENWIKI_SYNC,
        Some(operation.id),
        &outcome,
    )
    .await?;
    store.save_reconciliation_on(&mut tx, state, &[]).await?;
    tx.commit().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn git(root: &FsPath, args: &[&str]) -> String {
        let result = std::process::Command::new("git")
            .arg("-C")
            .arg(root)
            .args(args)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        String::from_utf8(result.stdout).unwrap().trim().into()
    }

    #[tokio::test]
    #[ignore = "requires a fresh task-owned lvk-openwiki-files-* LVK_INSTANCE_DIR; run alone"]
    async fn settings_and_deletion_use_isolated_publication_and_never_start_generation()
    -> anyhow::Result<()> {
        let instance = PathBuf::from(std::env::var("LVK_INSTANCE_DIR")?);
        anyhow::ensure!(
            instance.is_absolute()
                && instance
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with("lvk-openwiki-files-")
        );
        anyhow::ensure!(!instance.join("data/db.v2.sqlite").exists());
        let shutdown = tokio_util::sync::CancellationToken::new();
        let deployment = DeploymentImpl::new(shutdown.clone()).await?;
        let result = exercise(&deployment, &instance).await;
        shutdown.cancel();
        result
    }

    async fn exercise(deployment: &DeploymentImpl, instance: &FsPath) -> anyhow::Result<()> {
        anyhow::ensure!(utils::assets::asset_dir().starts_with(instance));
        let root = instance.join("fixture");
        std::fs::create_dir_all(root.join("openwiki"))?;
        git(&root, &["init", "-b", "main"]);
        git(&root, &["config", "user.name", "Fixture"]);
        git(&root, &["config", "user.email", "fixture@example.invalid"]);
        std::fs::write(root.join("source"), "current implementation")?;
        std::fs::write(root.join("openwiki/index.md"), "# Generated index")?;
        std::fs::write(
            root.join("openwiki/old.md"),
            "---\ngenerated: { by: codex }\n---\n# Operational knowledge",
        )?;
        std::fs::write(root.join("openwiki/human.md"), "human documentation")?;
        std::fs::write(
            root.join("AGENTS.md"),
            "prefix\n<!-- OPENWIKI:START -->generated<!-- OPENWIKI:END -->\nsuffix  \n",
        )?;
        git(&root, &["add", "."]);
        git(&root, &["commit", "-m", "fixture source"]);
        let repo = Repo::find_or_create(&deployment.db().pool, &root, "Fixture").await?;
        let _ = configure(
            State(deployment.clone()),
            Path(repo.id),
            Json(ConfigureRepositoryMemory {
                enabled: true,
                target_branch: "main".into(),
                output_language: "en".into(),
            }),
        )
        .await?;
        let Json(response) = get_files(State(deployment.clone()), Path(repo.id)).await?;
        let initial = response
            .into_data()
            .context("Missing configuration response")?;
        assert!(initial.files.iter().all(|file| !file.exists));
        assert!(initial.files[1].content.is_empty());
        assert!(!root.join(files::INSTRUCTIONS).exists());
        let source = initial.source.clone();
        std::fs::write(
            root.join(files::INSTRUCTIONS),
            "uncommitted user instructions",
        )?;
        let Json(actual) = get_files(State(deployment.clone()), Path(repo.id)).await?;
        assert_eq!(
            actual.into_data().unwrap().files[0].content,
            "uncommitted user instructions"
        );
        assert!(
            mutate(
                deployment,
                repo.id,
                "main",
                &source,
                Some((files::INSTRUCTIONS, "replacement")),
                None
            )
            .await
            .is_err()
        );
        assert_eq!(
            std::fs::read_to_string(root.join(files::INSTRUCTIONS))?,
            "uncommitted user instructions"
        );
        std::fs::remove_file(root.join(files::INSTRUCTIONS))?;
        mutate(
            deployment,
            repo.id,
            "main",
            &source,
            Some((files::INSTRUCTIONS, "new saved instructions")),
            None,
        )
        .await?;
        let source = git(&root, &["rev-parse", "HEAD"]);
        assert_eq!(
            std::fs::read_to_string(root.join("openwiki/old.md"))?,
            "---\ngenerated: { by: codex }\n---\n# Operational knowledge"
        );
        assert!(
            mutate(
                deployment,
                repo.id,
                "main",
                &initial.source,
                Some((files::IGNORE, "secret/")),
                None
            )
            .await
            .is_err()
        );
        mutate(
            deployment,
            repo.id,
            "main",
            &source,
            Some((files::IGNORE, "# saved exclusions\n")),
            None,
        )
        .await?;
        let source = git(&root, &["rev-parse", "HEAD"]);
        assert!(
            mutate(
                deployment,
                repo.id,
                "main",
                &source,
                None,
                Some("wrong/openwiki")
            )
            .await
            .is_err()
        );
        let store =
            RepositoryMemoryStore::for_repository(&deployment.db().pool, &repo.name, repo.id)?;
        let mut state = store.state().await?;
        state.active_run_id = Some(Uuid::new_v4());
        store.save_state(&state).await?;
        let confirmation = format!("{}/openwiki", repo.name);
        assert!(
            mutate(
                deployment,
                repo.id,
                "main",
                &source,
                None,
                Some(&confirmation)
            )
            .await
            .is_err()
        );
        state.active_run_id = None;
        store.save_state(&state).await?;
        mutate(
            deployment,
            repo.id,
            "main",
            &source,
            None,
            Some(&confirmation),
        )
        .await?;
        let (state, _) = status_snapshot(deployment, repo.id).await?;
        assert!(!state.wiki_exists && state.generation_suppressed);
        let ordinary_sync = sync(
            State(deployment.clone()),
            Path(repo.id),
            axum::body::Bytes::new(),
        )
        .await;
        assert!(ordinary_sync.is_err());
        assert!(
            ordinary_sync
                .unwrap_err()
                .to_string()
                .contains("explicit Initialize")
        );
        let visible = super::super::canonical_snapshot(deployment, repo.id).await?;
        assert!(!visible.exists && visible.index.is_none() && visible.pages.is_empty());
        assert_eq!(state.status, RepositoryWikiStatus::Uninitialized);
        assert!(
            state.last_success.is_none()
                && state.wiki_commit.is_none()
                && state.file_operation.is_none()
        );
        assert_eq!(
            std::fs::read_to_string(root.join(files::INSTRUCTIONS))?,
            "new saved instructions"
        );
        assert_eq!(
            std::fs::read_to_string(root.join(files::IGNORE))?,
            "# saved exclusions\n"
        );
        assert_eq!(
            std::fs::read_to_string(root.join("openwiki/human.md"))?,
            "human documentation"
        );
        assert_eq!(
            std::fs::read_to_string(root.join("AGENTS.md"))?,
            "prefix\n\nsuffix  \n"
        );
        assert!(!root.join("openwiki/old.md").exists());
        assert!(
            git(&root, &["show", &format!("{source}:openwiki/old.md")])
                .contains("Operational knowledge")
        );
        super::super::recover_repository(deployment, &repo).await?;
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM agent_runs")
            .fetch_one(&deployment.db().pool)
            .await?;
        assert_eq!(count, 0);
        Ok(())
    }
}
