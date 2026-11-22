//! Local repository context and read-only pinned source access. Preview creates
//! only a retained detached trial, never promotes source or changes Card state.
use std::path::Path;

use axum::{
    Extension, Json,
    extract::{Path as ApiPath, Query, State},
};
use db::models::{workspace::Workspace, workspace_repo::WorkspaceRepo};
use deployment::Deployment;
use git::GitService;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use utils::response::ApiResponse;
use uuid::Uuid;

use crate::{DeploymentImpl, error::ApiError};

#[derive(Debug, Deserialize)]
pub struct SnapshotQuery {
    pub commit: String,
    pub path: Option<String>,
    pub after_path: Option<String>,
}

#[derive(Debug, Serialize, TS)]
pub struct PinnedFile {
    pub path: String,
    pub object_id: String,
    pub mode: i32,
    pub content: Option<String>,
    pub unavailable_reason: Option<String>,
}

#[derive(Debug, Serialize, TS)]
pub struct PinnedSnapshot {
    pub repository_id: Uuid,
    pub commit: String,
    pub files: Vec<PinnedFile>,
    pub next_after: Option<String>,
}

pub async fn snapshot(
    State(deployment): State<DeploymentImpl>,
    ApiPath(id): ApiPath<Uuid>,
    Query(query): Query<SnapshotQuery>,
) -> Result<Json<ApiResponse<PinnedSnapshot>>, ApiError> {
    let repo = deployment
        .repo()
        .get_by_id(&deployment.db().pool, id)
        .await?;
    let result = tokio::task::spawn_blocking(move || -> Result<_, git::GitServiceError> {
        let reader = GitService::new().snapshot(&repo.path, &query.commit)?;
        let (entries, next_after) = if let Some(path) = &query.path {
            (vec![reader.entry(path)?], None)
        } else {
            let mut entries: Vec<_> = reader.entries()?.into_iter()
                .filter(|entry| query.after_path.as_ref().is_none_or(|after| &entry.path > after))
                .take(101).collect();
            let next = (entries.len() > 100).then(|| entries[99].path.clone());
            entries.truncate(100);
            (entries, next)
        };
        let mut files = Vec::new();
        for entry in entries {
            let (content, unavailable_reason) = if query.path.is_none() { (None, None) }
            else if !matches!(entry.mode, 0o100644 | 0o100755) { (None, Some("Not a regular file; symlinks and submodules are not followed".into())) }
            else { match reader.read_blob(&entry, 128 * 1024)? {
                None => (None, Some("File exceeds inline read bound; read this pinned Git object in segments with Git".into())),
                Some(bytes) => match String::from_utf8(bytes) {
                    Ok(text) => (Some(text), None),
                    Err(_) => (None, Some("Not UTF-8 text".into())),
                },
            }};
            files.push(PinnedFile { path:entry.path, object_id:entry.oid, mode:entry.mode, content, unavailable_reason });
        }
        Ok(PinnedSnapshot { repository_id:id, commit:query.commit, files, next_after })
    }).await.map_err(|error| ApiError::BadRequest(error.to_string()))??;
    Ok(Json(ApiResponse::success(result)))
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct PeerSnapshot {
    pub workspace_id: Uuid,
    pub commit: String,
}

#[derive(Debug, Deserialize, TS)]
pub struct CreatePreview {
    pub repository_id: Uuid,
    pub base_commit: String,
    pub peers: Vec<PeerSnapshot>,
}

#[derive(Debug, Serialize, TS)]
pub struct CombinationPreview {
    pub repository_id: Uuid,
    pub owner_workspace_id: Uuid,
    pub path: String,
    pub base_commit: String,
    pub peers: Vec<PeerSnapshot>,
    pub instructions: String,
}

pub async fn preview(
    State(deployment): State<DeploymentImpl>,
    Extension(workspace): Extension<Workspace>,
    Json(request): Json<CreatePreview>,
) -> Result<Json<ApiResponse<CombinationPreview>>, ApiError> {
    workspace.require_interactive()?;
    let pool = &deployment.db().pool;
    db::models::integration::guard_workspace(pool, workspace.id).await?;
    let repo = WorkspaceRepo::find_repo_for_workspace(pool, workspace.id, request.repository_id)
        .await?
        .ok_or(db::models::repo::RepoError::NotFound)?;
    let mut sources = request.peers.clone();
    sources.push(PeerSnapshot {
        workspace_id: workspace.id,
        commit: request.base_commit.clone(),
    });
    if sources.len() > 100 {
        return Err(ApiError::BadRequest("Too many preview sources".into()));
    }
    for source in &sources {
        if WorkspaceRepo::find_by_workspace_and_repo_id(pool, source.workspace_id, repo.id)
            .await?
            .is_none()
        {
            return Err(ApiError::BadRequest(
                "Preview source belongs to another repository".into(),
            ));
        }
        let peer = Workspace::find_by_id(pool, source.workspace_id)
            .await?
            .ok_or(db::models::workspace::WorkspaceError::WorkspaceNotFound)?;
        let head = deployment.git().get_branch_oid(&repo.path, &peer.branch)?;
        // Allow older pinned commits from this source, never chase a new HEAD.
        deployment.git().snapshot(&repo.path, &source.commit)?;
        if !deployment
            .git()
            .is_ancestor(&repo.path, &source.commit, &head)?
        {
            return Err(ApiError::BadRequest("Pinned preview commit is no longer attributable to its source Workspace; reselect explicitly".into()));
        }
    }
    // A's own uncommitted changes are not silently omitted from a requested
    // combination. Dirty direct-folder sources likewise require explicit commit.
    let container = workspace.container_ref.as_ref().ok_or_else(|| {
        ApiError::BadRequest(
            "Preview owner checkout is missing; commit or restore it before selecting a snapshot"
                .into(),
        )
    })?;
    let root = if workspace.is_direct_folder() {
        Path::new(container).to_path_buf()
    } else {
        Path::new(container).join(repo.worktree_name())
    };
    deployment
        .git()
        .require_clean_source(&root, &request.base_commit)?;
    let path = worktree_manager::WorktreeManager::create_detached_preview(
        &repo.path,
        &request.base_commit,
    )
    .await?;
    Ok(Json(ApiResponse::success(CombinationPreview {
        repository_id:repo.id, owner_workspace_id:workspace.id,
        path:path.to_string_lossy().into_owned(), base_commit:request.base_commit, peers:request.peers,
        instructions:"Retained detached test worktree: combine only the pinned peer commits here and run tests here. No source/target refs or Card status were changed. Never use reset/stash on the original Workspace. Do not finalise trial changes as normal source, invoke Wiki writers, or modify peers. If a peer itself needs changes, report peer change required with evidence. Keep test-only commands in this directory; explicit cleanup is available through normal git worktree remove after inspection.".into(),
    })))
}

#[cfg(test)]
mod auth_tests {
    use std::{path::PathBuf, time::Duration};

    use anyhow::ensure;
    use db::models::{repo::Repo, repository_state::RepositoryMemoryStore};
    use deployment::Deployment;
    use tower::ServiceExt;
    use utils::repository_memory::{ChangeManifest, SemanticChanges};
    use uuid::Uuid;

    #[tokio::test]
    #[ignore = "requires a fresh lvk-parallel-auth-* instance and public URL; run alone"]
    async fn isolated_parallel_loopback_auth() -> anyhow::Result<()> {
        let instance = PathBuf::from(std::env::var("LVK_INSTANCE_DIR")?);
        ensure!(
            instance.is_absolute()
                && instance
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with("lvk-parallel-auth-")
        );
        ensure!(!instance.join("data/db.v2.sqlite").exists());
        ensure!(std::env::var("LVK_ACCESS_DEPLOYMENT").as_deref() == Ok("public"));
        ensure!(std::env::var("LVK_PUBLIC_URL").as_deref() == Ok("https://example.test"));
        let shutdown = tokio_util::sync::CancellationToken::new();
        let deployment = crate::DeploymentImpl::new(shutdown.clone()).await?;
        let result = async {
            let root = instance.join("repo");
            std::fs::create_dir(&root)?;
            let repo = Repo::find_or_create(&deployment.db().pool, &root, "fixture").await?;
            workspace_manager::shared_resources::ensure_repository_memory(
                &deployment.db().pool,
                &repo,
            )
            .await?;
            let store =
                RepositoryMemoryStore::for_repository(&deployment.db().pool, &repo.name, repo.id)?;
            let event = ChangeManifest {
                version: 1,
                event_id: Uuid::new_v4(),
                repository_id: repo.id,
                workspace_id: Uuid::new_v4(),
                task_id: None,
                created_at: chrono::Utc::now(),
                base_commit: "0".repeat(40),
                source_commit: "1".repeat(40),
                target_branch: Some("main".into()),
                changed_paths: vec!["fixture".into()],
                semantics: SemanticChanges {
                    summary: "fixture".into(),
                    ..Default::default()
                },
            };
            store.publish_event(&event).await?;
            let access = server_access::Access::from_env(
                deployment.db().pool.clone(),
                utils::assets::asset_dir(),
            )?;
            let key = server_access::issue_key(
                &deployment.db().pool,
                &instance.join("data/access.key"),
                false,
            )
            .await?;
            let setup_app = access.layer(server_access::router(access.clone()));
            let request = axum::http::Request::builder()
                .method("POST")
                .uri("/api/access/setup")
                .header("host", "example.test")
                .header("x-lvk-csrf", "setup")
                .header("content-type", "application/json")
                .body(axum::body::Body::from(
                    serde_json::json!({"key": key}).to_string(),
                ))?;
            let response = setup_app.clone().oneshot(request).await?;
            ensure!(response.status() == axum::http::StatusCode::OK);
            let cookie = response
                .headers()
                .get("set-cookie")
                .unwrap()
                .to_str()?
                .split(';')
                .next()
                .unwrap()
                .to_owned();
            let body: serde_json::Value =
                serde_json::from_slice(&axum::body::to_bytes(response.into_body(), 65536).await?)?;
            let csrf = body["csrf"].as_str().unwrap();
            let request = axum::http::Request::builder()
                .method("POST")
                .uri("/api/access/basic")
                .header("host", "example.test")
                .header("cookie", cookie)
                .header("x-lvk-csrf", csrf)
                .header("content-type", "application/json")
                .body(axum::body::Body::from(
                    serde_json::json!({"username":"fixture", "password":"a long fixture password"})
                        .to_string(),
                ))?;
            ensure!(setup_app.oneshot(request).await?.status() == axum::http::StatusCode::OK);
            let token_path = instance.join("data/access-internal.token");
            let guide = utils::agent_guides::parallel_guide()?;
            ensure!(
                guide.contains(token_path.to_str().unwrap())
                    && guide.contains("X-LVK-Internal-Token")
            );
            let token = std::fs::read_to_string(token_path)?;
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
            let address = listener.local_addr()?;
            let router = access.layer(
                axum::Router::new()
                    .nest("/api", crate::routes::repo::router())
                    .with_state(deployment.clone()),
            );
            let stop = shutdown.clone();
            let server = tokio::spawn(async move {
                axum::serve(listener, router)
                    .with_graceful_shutdown(async move { stop.cancelled().await })
                    .await
            });
            let client = reqwest::Client::builder()
                .no_proxy()
                .redirect(reqwest::redirect::Policy::none())
                .timeout(Duration::from_secs(10))
                .build()?;
            let discovery = format!("http://{address}/api/repos/{}/parallel-context", repo.id);
            ensure!(
                client.get(&discovery).send().await?.status() == reqwest::StatusCode::FORBIDDEN
            );
            let data: serde_json::Value = client
                .get(&discovery)
                .header("X-LVK-Internal-Token", token.trim())
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            ensure!(
                data["success"] == true && data["data"]["repository_id"] == repo.id.to_string()
            );
            let manifest = format!(
                "http://{address}/api/repos/{}/change-manifests/{}",
                repo.id, event.event_id
            );
            ensure!(client.get(&manifest).send().await?.status() == reqwest::StatusCode::FORBIDDEN);
            let data: serde_json::Value = client
                .get(&manifest)
                .header("X-LVK-Internal-Token", token.trim())
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            ensure!(
                data["success"] == true && data["data"]["event_id"] == event.event_id.to_string()
            );
            shutdown.cancel();
            server.await??;
            anyhow::Ok(())
        }
        .await;
        shutdown.cancel();
        result
    }
}
