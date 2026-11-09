use axum::{
    Router,
    extract::{Query, State},
    response::Json as ResponseJson,
    routing::get,
};
use db::models::repo::{Repo, SearchResult};
use deployment::Deployment;
use serde::Deserialize;
use services::services::file_search::{SearchMode, SearchQuery};
use utils::response::ApiResponse;
use uuid::Uuid;

use crate::{DeploymentImpl, error::ApiError};

#[derive(Debug, Deserialize)]
pub struct MultiRepoSearchQuery {
    pub q: String,
    #[serde(default)]
    pub mode: SearchMode,
    pub repo_ids: String,
    pub workspace_id: Option<Uuid>,
}

pub async fn search_files(
    State(deployment): State<DeploymentImpl>,
    Query(query): Query<MultiRepoSearchQuery>,
) -> Result<ResponseJson<ApiResponse<Vec<SearchResult>>>, ApiError> {
    let repo_ids: Vec<Uuid> = query
        .repo_ids
        .split(',')
        .filter(|s| !s.trim().is_empty())
        .map(|s| s.trim().parse::<Uuid>())
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| ApiError::BadRequest("Invalid repo_id format".to_string()))?;

    if repo_ids.is_empty() {
        return Err(ApiError::BadRequest(
            "repo_ids parameter is required".to_string(),
        ));
    }

    if query.q.trim().is_empty() {
        return Ok(ResponseJson(ApiResponse::error(
            "Query parameter 'q' is required and cannot be empty",
        )));
    }

    let repos = if let Some(id) = query.workspace_id {
        let pool = &deployment.db().pool;
        let workspace = db::models::workspace::Workspace::find_by_id(pool, id)
            .await?
            .ok_or_else(|| ApiError::BadRequest("Workspace not found".into()))?;
        if workspace.is_direct_folder() {
            return Ok(ResponseJson(ApiResponse::success(Vec::new())));
        }
        let root = workspace
            .container_ref
            .ok_or_else(|| ApiError::BadRequest("Workspace checkout unavailable".into()))?;
        let mut repos =
            db::models::workspace_repo::WorkspaceRepo::find_repos_for_workspace(pool, id).await?;
        if repo_ids.iter().any(|id| !repos.iter().any(|r| r.id == *id)) {
            return Err(ApiError::BadRequest(
                "Repository does not belong to Workspace".into(),
            ));
        }
        repos.retain(|r| repo_ids.contains(&r.id));
        for repo in &mut repos {
            repo.path = repo.worktree_dir(&root);
        }
        repos
    } else {
        // Before creation, preview the same deterministic names used by a new
        // Workspace. This keeps inserted file references distinct for homonyms.
        let mut repos = Repo::find_by_ids(&deployment.db().pool, &repo_ids).await?;
        let names = repos
            .iter()
            .map(|r| (r.id.to_string(), r.name.clone()))
            .collect::<Vec<_>>();
        let placements = utils::path::allocate_workspace_repo_names(&names, &[]);
        for repo in &mut repos {
            repo.worktree_name = Some(placements[&repo.id.to_string()].clone());
        }
        repos
    };

    let search_query = SearchQuery {
        q: query.q,
        mode: query.mode,
    };

    let results = deployment
        .repo()
        .search_files(
            deployment.file_search_cache().as_ref(),
            &repos,
            &search_query,
        )
        .await
        .map_err(|e| {
            tracing::error!("Failed to search files: {}", e);
            ApiError::BadRequest(format!("Search failed: {}", e))
        })?;

    Ok(ResponseJson(ApiResponse::success(results)))
}

pub fn router(deployment: &DeploymentImpl) -> Router<DeploymentImpl> {
    Router::new()
        .route("/search", get(search_files))
        .with_state(deployment.clone())
}
