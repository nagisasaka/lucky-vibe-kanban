//! Project utilities resolve registered main checkouts, never Workspace paths.
//! Viewer DTOs retain their historical `workspace_id` field; on this route it
//! carries the Project scope. Authorisation always uses Project membership.
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    response::Response,
    routing::get,
};
use db::models::repo::Repo;
use deployment::Deployment;
use serde::Deserialize;
use serde_json::{Value, json};
use utils::response::ApiResponse;
use uuid::Uuid;

use super::workspaces::files;
use crate::{DeploymentImpl, error::ApiError};

#[derive(Deserialize)]
pub(super) struct UtilityQuery {
    pub repo_id: Option<Uuid>,
    pub path: Option<String>,
}

pub(super) fn router() -> Router<DeploymentImpl> {
    Router::new().route("/projects/{project_id}/utilities/{kind}", get(read))
}

pub(super) async fn project_repos(
    d: &DeploymentImpl,
    project_id: Uuid,
) -> Result<Vec<Repo>, ApiError> {
    let ids = super::local_remote::project_creation_repository_ids(&d.db().pool, project_id)
        .await
        .map_err(|e| ApiError::BadRequest(e.to_string()))?;
    let mut repos = Vec::new();
    for id in ids {
        if let Some(repo) = Repo::find_by_id(&d.db().pool, id).await? {
            repos.push(repo);
        }
    }
    Ok(repos)
}

pub(super) async fn project_repo(
    d: &DeploymentImpl,
    project_id: Uuid,
    repo_id: Uuid,
) -> Result<Repo, ApiError> {
    project_repos(d, project_id)
        .await?
        .into_iter()
        .find(|r| r.id == repo_id)
        .ok_or_else(|| ApiError::BadRequest("Repository does not belong to Project".into()))
}

async fn read(
    State(d): State<DeploymentImpl>,
    Path((project_id, kind)): Path<(Uuid, String)>,
    Query(q): Query<UtilityQuery>,
) -> Result<Response, ApiError> {
    use axum::response::IntoResponse;
    let repos = project_repos(&d, project_id).await?;
    let value = if kind == "info" {
        let mut result = Vec::new();
        for repo in repos {
            let enabled = super::openwiki::status_snapshot(&d, repo.id)
                .await
                .map(|(state, _)| state.enabled)
                .ok();
            result.push(json!({"id":repo.id,"name":repo.display_name,"path":repo.path,"wiki_enabled":enabled}));
        }
        json!({"repos":result})
    } else if kind == "tree" {
        let mut nodes = Vec::new();
        for repo in repos {
            let root = tokio::fs::canonicalize(&repo.path).await?;
            let (entries, truncated) =
                files::list_directory_entries(project_id, &repo, &root, &root, "").await?;
            nodes.push(json!({"workspace_id":project_id,"repo_id":repo.id,"repo_name":repo.name,"repo_display_name":repo.display_name,"entries":entries,"truncated":truncated}));
        }
        json!({"repos":nodes})
    } else {
        let repo = repos
            .into_iter()
            .find(|r| Some(r.id) == q.repo_id)
            .ok_or_else(|| ApiError::BadRequest("Repository does not belong to Project".into()))?;
        let root = tokio::fs::canonicalize(&repo.path).await?;
        if kind == "wiki" {
            let (state, _) = super::openwiki::status_snapshot(&d, repo.id).await?;
            if !state.enabled {
                return Err(ApiError::BadRequest("OpenWiki disabled".into()));
            }
            let wiki = tokio::task::spawn_blocking(move || {
                services::services::wiki::openwiki::load_snapshot(&root)
            })
            .await
            .map_err(|e| ApiError::BadRequest(e.to_string()))?
            .map_err(|e| ApiError::BadRequest(e.to_string()))?;
            json!({"workspace_id":project_id,"repo_id":repo.id,"repo_name":repo.name,"repo_display_name":repo.display_name,"source":"main_repository","wiki":wiki,"source_links":[]})
        } else {
            let target = files::normalize_relative_path(q.path.as_deref())?;
            let path = files::canonical_target_under_root(&root, &target).await?;
            let metadata = tokio::fs::metadata(&path).await?;
            match kind.as_str() {
                "directory" if metadata.is_dir() => {
                    let (entries, truncated) = files::list_directory_entries(
                        project_id,
                        &repo,
                        &root,
                        &path,
                        &target.display_path,
                    )
                    .await?;
                    json!({"workspace_id":project_id,"repo_id":repo.id,"repo_name":repo.name,"path":target.display_path,"entries":entries,"truncated":truncated})
                }
                "content" if metadata.is_file() => {
                    let mut content = files::workspace_file_content_from_path(
                        project_id,
                        repo.id,
                        &repo.name,
                        &target.display_path,
                        &path,
                        &metadata,
                    )
                    .await?;
                    let query: String = url::form_urlencoded::Serializer::new(String::new())
                        .append_pair("repo_id", &repo.id.to_string())
                        .append_pair("path", &target.display_path)
                        .finish();
                    content.raw_url =
                        Some(format!("/api/projects/{project_id}/utilities/raw?{query}"));
                    serde_json::to_value(content)
                        .map_err(|e| ApiError::BadRequest(e.to_string()))?
                }
                "raw" if metadata.is_file() => {
                    return files::workspace_file_raw_response(
                        tokio::fs::File::open(path).await?,
                        &target.display_path,
                        &metadata,
                    );
                }
                _ => return Err(ApiError::BadRequest("Invalid utility target".into())),
            }
        }
    };
    Ok((
        [(axum::http::header::CACHE_CONTROL, "no-store")],
        Json(ApiResponse::<Value>::success(value)),
    )
        .into_response())
}
