use std::path::PathBuf;

use axum::{
    Extension, Json, Router, extract::State, response::Json as ResponseJson, routing::post,
};
use db::models::{
    execution_process::ExecutionProcessView, repo::Repo, workspace::Workspace,
    workspace_repo::WorkspaceRepo,
};
use deployment::Deployment;
use executors::{
    executors::{CodingAgent, ExecutorError},
    profile::{ExecutorConfigs, ExecutorProfileId},
};
use serde::{Deserialize, Serialize};
use services::services::config::editor::EditorLink;
use ts_rs::TS;
use utils::response::ApiResponse;
use uuid::Uuid;

use super::{codex_setup, gh_cli_setup::GhCliSetupError};
use crate::{DeploymentImpl, error::ApiError};

#[derive(Debug, Deserialize, Serialize, TS)]
pub struct RunAgentSetupRequest {
    pub executor_profile_id: ExecutorProfileId,
}

#[derive(Debug, Serialize, TS)]
pub struct RunAgentSetupResponse {}

#[derive(Deserialize, TS)]
pub struct OpenEditorRequest {
    pub repo_id: Option<Uuid>,
    pub file_path: Option<String>,
}

#[derive(Debug, Serialize, TS)]
pub struct OpenEditorResponse {
    pub entries: Vec<EditorLink>,
}

pub fn router() -> Router<DeploymentImpl> {
    Router::new()
        .route("/preview", post(crate::routes::parallel_context::preview))
        .route("/editor/open", post(open_workspace_in_editor))
        .route("/agent/setup", post(run_agent_setup))
        .route("/github/cli/setup", post(gh_cli_setup_handler))
}

#[axum::debug_handler]
pub async fn run_agent_setup(
    Extension(workspace): Extension<Workspace>,
    State(deployment): State<DeploymentImpl>,
    Json(payload): Json<RunAgentSetupRequest>,
) -> Result<ResponseJson<ApiResponse<RunAgentSetupResponse>>, ApiError> {
    let executor_profile_id = payload.executor_profile_id;
    let config = ExecutorConfigs::get_cached();
    let coding_agent = config.get_coding_agent_or_default(&executor_profile_id);
    match coding_agent {
        CodingAgent::Codex(codex) => {
            codex_setup::run_codex_setup(&deployment, &workspace, &codex).await?;
        }
        // Oh My Pi does not expose a VK setup helper; callers must install and
        // authenticate the native `omp` runtime independently.
        CodingAgent::OhMyPi(_) => {
            return Err(ApiError::Executor(ExecutorError::SetupHelperNotSupported));
        }
        _ => return Err(ApiError::Executor(ExecutorError::SetupHelperNotSupported)),
    }

    deployment
        .track_if_analytics_allowed(
            "agent_setup_script_executed",
            serde_json::json!({
                "executor_profile_id": executor_profile_id.to_string(),
                "workspace_id": workspace.id.to_string(),
            }),
        )
        .await;

    Ok(ResponseJson(ApiResponse::success(RunAgentSetupResponse {})))
}

pub async fn open_workspace_in_editor(
    Extension(workspace): Extension<Workspace>,
    State(deployment): State<DeploymentImpl>,
    Json(payload): Json<OpenEditorRequest>,
) -> Result<ResponseJson<ApiResponse<OpenEditorResponse>>, ApiError> {
    let (directory, file) =
        resolve_workspace_editor_path(&deployment, &workspace, &payload).await?;
    let config = deployment.config().read().await;
    let entries = config.editor.links(
        &directory.to_string_lossy(),
        file.as_ref().map(|p| p.to_string_lossy()).as_deref(),
    );
    Ok(ResponseJson(ApiResponse::success(OpenEditorResponse {
        entries,
    })))
}

async fn resolve_workspace_editor_path(
    deployment: &DeploymentImpl,
    workspace: &Workspace,
    request: &OpenEditorRequest,
) -> Result<(PathBuf, Option<PathBuf>), ApiError> {
    let repos =
        WorkspaceRepo::find_repos_for_workspace(&deployment.db().pool, workspace.id).await?;
    resolve_editor_target(workspace, &repos, request).await
}

async fn resolve_editor_target(
    workspace: &Workspace,
    repos: &[Repo],
    request: &OpenEditorRequest,
) -> Result<(PathBuf, Option<PathBuf>), ApiError> {
    // Do not call ensure_container_exists/container_for_inspection: resolving a
    // link must never prepare, restore or touch a worktree.
    let root = services::services::workspace_usage::inspection_root(workspace)
        .map_err(|e| ApiError::BadRequest(e.to_string()))?;
    let repo = if let Some(id) = request.repo_id {
        Some(repos.iter().find(|repo| repo.id == id).ok_or_else(|| {
            ApiError::BadRequest("Repository does not belong to this Workspace".into())
        })?)
    } else if repos.len() == 1 {
        repos.first()
    } else {
        None
    };
    let directory = existing_editor_directory(&match repo {
        Some(repo) => root.join(&repo.name),
        None => root,
    })
    .await?;
    let file = if let Some(path) = request.file_path.as_deref() {
        if request.repo_id.is_none() {
            return Err(ApiError::BadRequest(
                "File targets require a repository and a repo-relative path".into(),
            ));
        }
        let relative = super::files::normalize_relative_path(Some(path))?;
        let target = super::files::canonical_target_under_root(&directory, &relative).await?;
        if !target.is_file() {
            return Err(ApiError::BadRequest("Target file is unavailable".into()));
        }
        Some(target)
    } else {
        None
    };
    Ok((directory, file))
}

pub(crate) async fn existing_editor_directory(path: &std::path::Path) -> Result<PathBuf, ApiError> {
    if !path.is_absolute() || !path.is_dir() {
        return Err(ApiError::BadRequest(
            "Editor directory is unavailable; no worktree will be created".into(),
        ));
    }
    Ok(tokio::fs::canonicalize(path).await?)
}

#[axum::debug_handler]
pub async fn gh_cli_setup_handler(
    Extension(workspace): Extension<Workspace>,
    State(deployment): State<DeploymentImpl>,
) -> Result<ResponseJson<ApiResponse<ExecutionProcessView, GhCliSetupError>>, ApiError> {
    match super::gh_cli_setup::run_gh_cli_setup(&deployment, &workspace).await {
        Ok(execution_process) => {
            deployment
                .track_if_analytics_allowed(
                    "gh_cli_setup_executed",
                    serde_json::json!({
                        "workspace_id": workspace.id.to_string(),
                    }),
                )
                .await;

            Ok(ResponseJson(ApiResponse::success(
                ExecutionProcessView::from_process(execution_process),
            )))
        }
        Err(ApiError::Executor(executors::executors::ExecutorError::ExecutableNotFound {
            program,
        })) if program == "brew" => Ok(ResponseJson(ApiResponse::error_with_data(
            GhCliSetupError::BrewMissing,
        ))),
        Err(ApiError::Executor(ExecutorError::SetupHelperNotSupported)) => Ok(ResponseJson(
            ApiResponse::error_with_data(GhCliSetupError::SetupHelperNotSupported),
        )),
        Err(ApiError::Executor(err)) => Ok(ResponseJson(ApiResponse::error_with_data(
            GhCliSetupError::Other {
                message: err.to_string(),
            },
        ))),
        Err(err) => Err(err),
    }
}

#[cfg(test)]
mod editor_tests {
    use serde_json::json;

    use super::*;

    fn workspace(root: &std::path::Path) -> Workspace {
        serde_json::from_value(json!({
            "id": Uuid::new_v4(), "container_ref": root, "workspace_kind": "worktree",
            "container_ownership": "managed", "usage": "interactive", "branch": "test",
            "created_at": "2026-01-01T00:00:00Z", "updated_at": "2026-01-01T00:00:00Z",
            "archived": false, "pinned": false, "worktree_deleted": false, "arena_status": "active"
        }))
        .unwrap()
    }

    fn repo(name: &str, original: &std::path::Path) -> Repo {
        serde_json::from_value(json!({
            "id": Uuid::new_v4(), "name": name, "display_name": name, "path": original,
            "parallel_setup_script": false,
            "created_at": "2026-01-01T00:00:00Z", "updated_at": "2026-01-01T00:00:00Z"
        }))
        .unwrap()
    }

    #[tokio::test]
    async fn resolves_workspace_repo_and_file_contexts_without_creating_worktrees() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("workspace");
        let original = temp.path().join("original");
        for path in [root.join("one"), root.join("two"), original.clone()] {
            std::fs::create_dir_all(path).unwrap();
        }
        std::fs::write(root.join("one/日本 % file.txt"), "file").unwrap();
        let mut ws = workspace(&root);
        let repos = vec![repo("one", &original), repo("two", &original)];
        let folder = OpenEditorRequest {
            repo_id: None,
            file_path: None,
        };
        assert_eq!(
            resolve_editor_target(&ws, &repos[..1], &folder)
                .await
                .unwrap()
                .0,
            root.join("one")
        );
        assert_eq!(
            resolve_editor_target(&ws, &repos, &folder).await.unwrap().0,
            root
        );
        let specific = OpenEditorRequest {
            repo_id: Some(repos[1].id),
            file_path: None,
        };
        assert_eq!(
            resolve_editor_target(&ws, &repos, &specific)
                .await
                .unwrap()
                .0,
            root.join("two")
        );
        // Repository management uses the registered original, not a worktree.
        assert_eq!(
            existing_editor_directory(&repos[0].path).await.unwrap(),
            original
        );
        let file = OpenEditorRequest {
            repo_id: Some(repos[0].id),
            file_path: Some("日本 % file.txt".into()),
        };
        let (directory, path) = resolve_editor_target(&ws, &repos, &file).await.unwrap();
        assert_eq!(directory, root.join("one"));
        assert_eq!(path.unwrap(), root.join("one/日本 % file.txt"));
        let foreign = OpenEditorRequest {
            repo_id: Some(Uuid::new_v4()),
            file_path: None,
        };
        assert!(resolve_editor_target(&ws, &repos, &foreign).await.is_err());
        ws.archived = true;
        ws.worktree_deleted = true;
        assert!(resolve_editor_target(&ws, &repos, &folder).await.is_err());
        ws.worktree_deleted = false;
        ws.container_ref = Some(temp.path().join("missing").to_string_lossy().into_owned());
        assert!(resolve_editor_target(&ws, &repos, &folder).await.is_err());
        assert!(!temp.path().join("missing").exists());
        ws.archived = false;
        assert!(resolve_editor_target(&ws, &repos, &folder).await.is_err());
        assert!(!temp.path().join("missing").exists());
    }

    #[tokio::test]
    async fn rejects_out_of_repo_and_missing_files() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("workspace");
        std::fs::create_dir_all(root.join("repo")).unwrap();
        let ws = workspace(&root);
        let repos = vec![repo("repo", temp.path())];
        for path in [
            "/etc/passwd",
            "../secret",
            "a/../../secret",
            "C:\\secret",
            "missing",
            "",
        ] {
            let request = OpenEditorRequest {
                repo_id: Some(repos[0].id),
                file_path: Some(path.into()),
            };
            assert!(
                resolve_editor_target(&ws, &repos, &request).await.is_err(),
                "{path}"
            );
        }
        let request = OpenEditorRequest {
            repo_id: None,
            file_path: Some("file".into()),
        };
        assert!(resolve_editor_target(&ws, &repos, &request).await.is_err());
        #[cfg(unix)]
        {
            std::fs::write(temp.path().join("outside"), "secret").unwrap();
            std::os::unix::fs::symlink(temp.path().join("outside"), root.join("repo/link"))
                .unwrap();
            let request = OpenEditorRequest {
                repo_id: Some(repos[0].id),
                file_path: Some("link".into()),
            };
            assert!(resolve_editor_target(&ws, &repos, &request).await.is_err());
        }
    }
}
