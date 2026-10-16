use std::path::Path;

use axum::{Json, extract::State, response::Json as ResponseJson};
use db::models::{
    execution_process::ExecutionProcessRunReason,
    requests::{
        CreateAndStartWorkspaceRequest, CreateAndStartWorkspaceResponse, CreateWorkspaceApiRequest,
        CreateWorkspaceMode,
    },
    session::{CreateSession, Session},
    workspace::{CreateWorkspace, Workspace},
    workspace_repo::{CreateWorkspaceRepo, WorkspaceRepo},
    workspace_usage::WorkspaceExecutionOwner,
};
use deployment::Deployment;
use services::services::container::ContainerService;
use utils::response::ApiResponse;
use uuid::Uuid;

use crate::{
    DeploymentImpl,
    error::ApiError,
    routes::{
        filesystem::validate_directory_path, sessions, workspaces::links::link_workspace_to_issue,
    },
};

pub(crate) async fn create_workspace_record(
    deployment: &DeploymentImpl,
    name: Option<String>,
) -> Result<Workspace, ApiError> {
    create_workspace_record_with_owner(deployment, name, None).await
}

/// Only product admission paths supply ownership, never a public request body.
pub(crate) async fn create_workspace_record_with_owner(
    deployment: &DeploymentImpl,
    name: Option<String>,
    owner: Option<&WorkspaceExecutionOwner>,
) -> Result<Workspace, ApiError> {
    create_workspace_record_with_identity(deployment, name, owner, Uuid::new_v4()).await
}

async fn create_workspace_record_with_identity(
    deployment: &DeploymentImpl,
    name: Option<String>,
    owner: Option<&WorkspaceExecutionOwner>,
    workspace_id: Uuid,
) -> Result<Workspace, ApiError> {
    let branch_label = name
        .as_deref()
        .filter(|branch_label| !branch_label.is_empty())
        .unwrap_or("workspace");
    let git_branch_name = deployment
        .container()
        .git_branch_from_workspace(&workspace_id, branch_label)
        .await;

    let workspace = Workspace::create_with_owner(
        &deployment.db().pool,
        &CreateWorkspace {
            branch: git_branch_name,
            name: name.filter(|workspace_name| !workspace_name.is_empty()),
        },
        workspace_id,
        owner,
    )
    .await?;

    Ok(workspace)
}

pub async fn create_workspace(
    State(deployment): State<DeploymentImpl>,
    Json(payload): Json<CreateWorkspaceApiRequest>,
) -> Result<ResponseJson<ApiResponse<Workspace>>, ApiError> {
    let workspace = create_workspace_record(&deployment, payload.name).await?;

    deployment
        .track_if_analytics_allowed(
            "workspace_created",
            serde_json::json!({
                "workspace_id": workspace.id.to_string(),
            }),
        )
        .await;

    Ok(ResponseJson(ApiResponse::success(workspace)))
}

fn normalize_prompt(prompt: &str) -> Option<String> {
    let trimmed = prompt.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn path_to_api_string(path: &Path) -> String {
    path.to_string_lossy().to_string()
}

async fn validate_direct_folder_path(
    deployment: &DeploymentImpl,
    directory_path: Option<String>,
) -> Result<std::path::PathBuf, ApiError> {
    let raw_path = directory_path
        .as_deref()
        .map(str::trim)
        .filter(|path| !path.is_empty())
        .ok_or_else(|| {
            ApiError::BadRequest(
                "A directory path is required for direct folder workspaces.".to_string(),
            )
        })?;

    validate_directory_path(deployment, raw_path).await
}

async fn create_direct_folder_workspace_record(
    deployment: &DeploymentImpl,
    name: Option<String>,
    directory_path: Option<String>,
) -> Result<Workspace, ApiError> {
    let selected_dir = validate_direct_folder_path(deployment, directory_path).await?;
    let selected_dir_str = path_to_api_string(&selected_dir);
    let workspace_id = Uuid::new_v4();
    let workspace_name = name.filter(|workspace_name| !workspace_name.is_empty());
    let is_git_repo =
        selected_dir.join(".git").exists() && deployment.git().is_repo_openable(&selected_dir);

    if is_git_repo {
        let current_branch = deployment.git().get_current_branch(&selected_dir)?;
        let repo = deployment
            .repo()
            .register(&deployment.db().pool, &selected_dir_str, None)
            .await?;
        let target_branch = repo
            .default_target_branch
            .clone()
            .unwrap_or_else(|| current_branch.clone());
        let container_ref = selected_dir.parent().ok_or_else(|| {
            ApiError::BadRequest(
                "Cannot use a filesystem root as a git direct folder workspace.".to_string(),
            )
        })?;
        let workspace = Workspace::create_direct_folder(
            &deployment.db().pool,
            &CreateWorkspace {
                branch: current_branch,
                name: workspace_name,
            },
            workspace_id,
            &path_to_api_string(container_ref),
        )
        .await?;

        WorkspaceRepo::create_many(
            &deployment.db().pool,
            workspace.id,
            &[CreateWorkspaceRepo {
                repo_id: repo.id,
                target_branch,
            }],
        )
        .await?;

        Ok(workspace)
    } else {
        Workspace::create_direct_folder(
            &deployment.db().pool,
            &CreateWorkspace {
                branch: "direct-folder".to_string(),
                name: workspace_name,
            },
            workspace_id,
            &selected_dir_str,
        )
        .await
        .map_err(ApiError::from)
    }
}

pub async fn create_and_start_workspace(
    State(deployment): State<DeploymentImpl>,
    Json(payload): Json<CreateAndStartWorkspaceRequest>,
) -> Result<ResponseJson<ApiResponse<CreateAndStartWorkspaceResponse>>, ApiError> {
    create_and_start_workspace_admitted(deployment, payload, None).await
}

/// An internal admission receipt may supply a stable Workspace identity. The
/// ordinary UI request schema and creation flow are unchanged.
pub(crate) async fn create_and_start_workspace_admitted(
    deployment: DeploymentImpl,
    payload: CreateAndStartWorkspaceRequest,
    workspace_id: Option<Uuid>,
) -> Result<ResponseJson<ApiResponse<CreateAndStartWorkspaceResponse>>, ApiError> {
    if workspace_id.is_some() && !matches!(payload.mode, CreateWorkspaceMode::Worktree) {
        return Err(ApiError::BadRequest(
            "Admitted Workspace identity requires Worktree mode".into(),
        ));
    }
    let CreateAndStartWorkspaceRequest {
        mode,
        name,
        repos,
        directory_path,
        linked_issue,
        executor_config,
        prompt,
        selected_skills,
        resume_session_id,
        resume_scope_path,
        attachment_ids,
    } = payload;

    let workspace_prompt = normalize_prompt(&prompt).ok_or_else(|| {
        ApiError::BadRequest(
            "A workspace prompt is required. Provide a non-empty `prompt`.".to_string(),
        )
    })?;
    let resume_session_id = resume_session_id
        .as_deref()
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .map(ToOwned::to_owned);
    let resume_scope_path = resume_scope_path
        .as_deref()
        .map(str::trim)
        .filter(|path| !path.is_empty())
        .map(ToOwned::to_owned);

    let managed_workspace = match mode {
        CreateWorkspaceMode::Worktree => {
            if repos.is_empty() {
                return Err(ApiError::BadRequest(
                    "At least one repository is required".to_string(),
                ));
            }

            let mut managed_workspace = deployment
                .workspace_manager()
                .load_managed_workspace(
                    create_workspace_record_with_identity(
                        &deployment,
                        name,
                        None,
                        workspace_id.unwrap_or_else(Uuid::new_v4),
                    )
                    .await?,
                )
                .await?;

            for repo in &repos {
                managed_workspace
                    .add_repository(repo, deployment.git())
                    .await
                    .map_err(ApiError::from)?;
            }

            managed_workspace
        }
        CreateWorkspaceMode::DirectFolder => {
            deployment
                .workspace_manager()
                .load_managed_workspace(
                    create_direct_folder_workspace_record(&deployment, name, directory_path)
                        .await?,
                )
                .await?
        }
    };

    if let Some(ids) = &attachment_ids {
        managed_workspace.associate_attachments(ids).await?;
    }

    let workspace = managed_workspace.workspace.clone();
    tracing::info!("Created workspace {}", workspace.id);

    if let Some(linked_issue) = &linked_issue {
        link_workspace_to_issue(
            &deployment,
            &workspace,
            linked_issue.remote_project_id,
            linked_issue.issue_id,
        )
        .await?;
    }

    deployment.container().create(&workspace).await?;
    let workspace = Workspace::find_by_id(&deployment.db().pool, workspace.id)
        .await?
        .ok_or_else(|| ApiError::BadRequest("Created workspace was not found".to_string()))?;
    let session = Session::create(
        &deployment.db().pool,
        &CreateSession {
            executor: Some(executor_config.executor.to_string()),
            name: None,
        },
        Uuid::new_v4(),
        workspace.id,
    )
    .await?;
    let workspace_repos =
        WorkspaceRepo::find_repos_for_workspace(&deployment.db().pool, workspace.id).await?;
    let repos_with_setup = if workspace.is_direct_folder() {
        Vec::new()
    } else {
        workspace_repos
            .iter()
            .filter(|repo| repo.setup_script.is_some())
            .collect::<Vec<_>>()
    };

    let agent_run = if repos_with_setup.is_empty() {
        sessions::start_coding_agent_execution_for_session(
            &deployment,
            session,
            workspace_prompt,
            selected_skills,
            executor_config.clone(),
            resume_session_id.clone(),
            resume_scope_path.clone(),
        )
        .await?
    } else if repos_with_setup
        .iter()
        .all(|repo| repo.parallel_setup_script)
    {
        for repo in repos_with_setup {
            if let Some(setup_action) = deployment
                .container()
                .setup_actions_for_repos(std::slice::from_ref(repo))
                && let Err(error) = deployment
                    .container()
                    .start_execution(
                        &workspace,
                        &session,
                        &setup_action,
                        &ExecutionProcessRunReason::SetupScript,
                    )
                    .await
            {
                tracing::warn!(
                    workspace_id = %workspace.id,
                    repo_id = %repo.id,
                    "failed to start parallel workspace setup script: {error:#}"
                );
            }
        }
        sessions::start_coding_agent_execution_for_session(
            &deployment,
            session,
            workspace_prompt,
            selected_skills,
            executor_config.clone(),
            resume_session_id.clone(),
            resume_scope_path.clone(),
        )
        .await?
    } else {
        let setup_action = deployment
            .container()
            .setup_actions_for_repos(&workspace_repos)
            .ok_or_else(|| {
                ApiError::BadRequest(
                    "Workspace setup configuration did not produce a setup action".to_string(),
                )
            })?;
        let reserved = sessions::reserve_coding_agent_execution_for_session(
            &deployment,
            session,
            workspace_prompt,
            selected_skills,
            executor_config.clone(),
            resume_session_id,
            resume_scope_path,
        )
        .await?;
        sessions::setup_gate::start_reserved_after_setup(
            &deployment,
            reserved.agent_run_id,
            &setup_action,
        )
        .await?;
        reserved
    };

    deployment
        .track_if_analytics_allowed(
            "workspace_created_and_started",
            serde_json::json!({
                "executor": &executor_config.executor,
                "variant": &executor_config.variant,
                "workspace_id": workspace.id.to_string(),
            }),
        )
        .await;

    Ok(ResponseJson(ApiResponse::success(
        CreateAndStartWorkspaceResponse {
            workspace,
            agent_run,
        },
    )))
}
