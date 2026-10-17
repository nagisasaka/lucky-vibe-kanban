//! Secret-write-only API. This shares the repository-settings access boundary.
use axum::{
    extract::{Path, State},
    response::Json,
};
use deployment::Deployment;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use utils::{
    github_credentials::{Credential, Repository, Store},
    response::ApiResponse,
};
use uuid::Uuid;

use crate::{DeploymentImpl, error::ApiError};

#[derive(Serialize, TS)]
pub struct GithubCredentialStatus {
    pub repository: String,
    pub configured: bool,
}
#[derive(Deserialize, TS)]
pub struct SaveGithubCredential {
    pub repository: String,
    pub token: String,
}
#[derive(Deserialize, TS)]
pub struct GithubCredentialTarget {
    pub repository: String,
}

async fn repositories(deployment: &DeploymentImpl, id: Uuid) -> Result<Vec<Repository>, ApiError> {
    let repo = deployment
        .repo()
        .get_by_id(&deployment.db().pool, id)
        .await?;
    let mut result: Vec<_> = deployment
        .git()
        .list_remotes(&repo.path)?
        .iter()
        .filter_map(|r| Repository::parse(&r.url))
        .collect();
    if let Ok(remote) = deployment.git().get_push_remote(&repo.path)
        && let Some(repository) = Repository::parse(&remote.url)
    {
        result.push(repository);
    }
    result.sort_by(|a, b| a.0.cmp(&b.0));
    result.dedup();
    Ok(result)
}
async fn target(deployment: &DeploymentImpl, id: Uuid, name: &str) -> Result<Repository, ApiError> {
    repositories(deployment, id)
        .await?
        .into_iter()
        .find(|r| r.0 == name)
        .ok_or_else(|| {
            ApiError::BadRequest(
                "Select a GitHub.com repository from this repository's remotes".into(),
            )
        })
}
fn storage_error(_: std::io::Error) -> ApiError {
    ApiError::BadRequest("Unable to access GitHub credential storage".into())
}

pub async fn status(
    State(d): State<DeploymentImpl>,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<Vec<GithubCredentialStatus>>>, ApiError> {
    let store = Store::default();
    let result = repositories(&d, id)
        .await?
        .into_iter()
        .map(|r| {
            let configured = store.contains(&r).map_err(storage_error)?;
            Ok(GithubCredentialStatus {
                repository: r.0,
                configured,
            })
        })
        .collect::<Result<Vec<_>, ApiError>>()?;
    Ok(Json(ApiResponse::success(result)))
}
pub async fn save(
    State(d): State<DeploymentImpl>,
    Path(id): Path<Uuid>,
    Json(body): Json<SaveGithubCredential>,
) -> Result<Json<ApiResponse<()>>, ApiError> {
    if !server_access::actor_is_admin() {
        return Err(ApiError::Forbidden(
            "Only administrators can manage GitHub credentials".into(),
        ));
    }
    let repo = target(&d, id, &body.repository).await?;
    let credential = Credential::new(body.token.trim().to_owned()).map_err(|_| {
        ApiError::BadRequest("Enter a valid fine-grained PAT (github_pat_...)".into())
    })?;
    Store::default()
        .save(&repo, &credential)
        .map_err(storage_error)?;
    Ok(Json(ApiResponse::success(())))
}
pub async fn remove(
    State(d): State<DeploymentImpl>,
    Path(id): Path<Uuid>,
    Json(body): Json<GithubCredentialTarget>,
) -> Result<Json<ApiResponse<()>>, ApiError> {
    if !server_access::actor_is_admin() {
        return Err(ApiError::Forbidden(
            "Only administrators can manage GitHub credentials".into(),
        ));
    }
    let repo = target(&d, id, &body.repository).await?;
    Store::default().remove(&repo).map_err(storage_error)?;
    Ok(Json(ApiResponse::success(())))
}
pub async fn check(
    State(d): State<DeploymentImpl>,
    Path(id): Path<Uuid>,
    Json(body): Json<GithubCredentialTarget>,
) -> Result<Json<ApiResponse<()>>, ApiError> {
    if !server_access::actor_is_admin() {
        return Err(ApiError::Forbidden(
            "Only administrators can manage GitHub credentials".into(),
        ));
    }
    let repo = target(&d, id, &body.repository).await?;
    let credential = Store::default()
        .load(&repo)
        .map_err(storage_error)?
        .ok_or_else(|| ApiError::BadRequest("Register a token first".into()))?;
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|_| ApiError::BadRequest("Cannot initialise GitHub connection".into()))?;
    for suffix in ["", "/pulls?state=open&per_page=1"] {
        let response = client
            .get(format!("https://api.github.com/repos/{}{suffix}", repo.0))
            .bearer_auth(credential.token())
            .header("User-Agent", "LVK")
            .header("Accept", "application/vnd.github+json")
            .send()
            .await
            .map_err(|_| ApiError::BadRequest("Could not connect to GitHub".into()))?;
        if !response.status().is_success() {
            return Err(ApiError::BadRequest(format!(
                "GitHub returned HTTP {}. Check token expiry, selected repository, permissions and organisation approval.",
                response.status().as_u16()
            )));
        }
    }
    Ok(Json(ApiResponse::success(())))
}
