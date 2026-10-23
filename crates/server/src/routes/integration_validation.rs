//! File-backed editor for the existing Integration plan. Saving never runs or commits commands.
use std::{
    fs,
    io::{Read, Write},
    path::Path,
};

use axum::{
    Json,
    extract::{Path as RoutePath, State},
};
use deployment::Deployment;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use utils::response::ApiResponse;
use uuid::Uuid;

use crate::{DeploymentImpl, error::ApiError};

const CONFIG: &str = ".lvk/integration-validation.json";
const MAX_BYTES: u64 = 1024 * 1024;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ValidationPlan {
    commands: Vec<String>,
}

/// Shared by the editor and the frozen-target reader: no second validation policy.
pub(crate) fn parse_commands(text: &str) -> anyhow::Result<Vec<String>> {
    let plan: ValidationPlan = serde_json::from_str(text)?;
    anyhow::ensure!(
        !plan.commands.is_empty() && plan.commands.len() <= 32,
        "Validation requires 1–32 commands"
    );
    for command in &plan.commands {
        anyhow::ensure!(
            !command.trim().is_empty() && !matches!(command.trim(), "true" | ":" | "exit 0"),
            "Empty/placeholder validation command"
        );
    }
    Ok(plan.commands)
}

#[derive(Serialize)]
pub struct ValidationSettings {
    pub branch: String,
    pub head: String,
    pub revision: String,
    pub content: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SaveValidationSettings {
    pub expected_revision: String,
    pub content: String,
}

fn git(root: &Path, args: &[&str]) -> Result<String, ApiError> {
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()?;
    if !output.status.success() {
        return Err(ApiError::BadRequest(
            "A checked-out local branch with a commit is required".into(),
        ));
    }
    String::from_utf8(output.stdout)
        .map(|s| s.trim().to_owned())
        .map_err(|_| ApiError::BadRequest("Git returned invalid UTF-8".into()))
}

fn regular_path(path: &Path, directory: bool) -> Result<(), ApiError> {
    match fs::symlink_metadata(path) {
        Ok(meta)
            if !meta.file_type().is_symlink()
                && if directory {
                    meta.is_dir()
                } else {
                    meta.is_file()
                } =>
        {
            Ok(())
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.into()),
        _ => Err(ApiError::BadRequest(
            "Integration settings must be a regular file in a regular .lvk directory (no symlinks)"
                .into(),
        )),
    }
}

pub(crate) fn read_settings(root: &Path) -> Result<ValidationSettings, ApiError> {
    let branch = git(root, &["symbolic-ref", "--quiet", "HEAD"])?;
    let head = git(root, &["rev-parse", "--verify", "HEAD"])?;
    regular_path(&root.join(".lvk"), true)?;
    let path = root.join(CONFIG);
    regular_path(&path, false)?;
    let content = match fs::File::open(path) {
        Ok(file) => {
            let mut bytes = Vec::new();
            file.take(MAX_BYTES + 1).read_to_end(&mut bytes)?;
            if bytes.len() as u64 > MAX_BYTES {
                return Err(ApiError::PayloadTooLarge);
            }
            Some(
                String::from_utf8(bytes)
                    .map_err(|_| ApiError::BadRequest("Settings must be UTF-8".into()))?,
            )
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => return Err(e.into()),
    };
    // Include checkout identity, existence and exact bytes; formatting-only edits conflict too.
    let revision = format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&(&branch, &head, &content)).expect("string tuple"))
    );
    Ok(ValidationSettings {
        branch,
        head,
        revision,
        content,
    })
}

pub(crate) fn save_settings(
    root: &Path,
    request: SaveValidationSettings,
) -> Result<ValidationSettings, ApiError> {
    if request.content.len() as u64 > MAX_BYTES {
        return Err(ApiError::PayloadTooLarge);
    }
    parse_commands(&request.content).map_err(|e| ApiError::BadRequest(e.to_string()))?;
    // Share the existing Git storage lock with publication and all aliases of this repository.
    let git = git::GitService::new();
    let _guard = git
        .try_publication_guard(root)
        .map_err(|_| ApiError::Conflict("Repository is busy; reload and retry".into()))?;
    let before = read_settings(root)?;
    if before.revision != request.expected_revision {
        return Err(ApiError::Conflict(
            "Integration settings or branch changed; reload before saving".into(),
        ));
    }
    fs::create_dir_all(root.join(".lvk"))?;
    let mut temp = tempfile::NamedTempFile::new_in(root.join(".lvk"))?;
    if let Ok(metadata) = fs::metadata(root.join(CONFIG)) {
        temp.as_file().set_permissions(metadata.permissions())?;
    }
    temp.write_all(request.content.as_bytes())?;
    temp.as_file().sync_all()?;
    // Detect external edits made while preparing the replacement as well.
    if read_settings(root)?.revision != before.revision {
        return Err(ApiError::Conflict(
            "Integration settings or branch changed; reload before saving".into(),
        ));
    }
    temp.persist(root.join(CONFIG))
        .map_err(|e| ApiError::Io(e.error))?;
    read_settings(root)
}

pub async fn get(
    State(d): State<DeploymentImpl>,
    RoutePath(id): RoutePath<Uuid>,
) -> Result<Json<ApiResponse<ValidationSettings>>, ApiError> {
    let repo = d.repo().get_by_id(&d.db().pool, id).await?;
    let settings = tokio::task::spawn_blocking(move || read_settings(&repo.path))
        .await
        .map_err(|e| ApiError::Io(std::io::Error::other(e)))??;
    Ok(Json(ApiResponse::success(settings)))
}

pub async fn save(
    State(d): State<DeploymentImpl>,
    RoutePath(id): RoutePath<Uuid>,
    Json(request): Json<SaveValidationSettings>,
) -> Result<Json<ApiResponse<ValidationSettings>>, ApiError> {
    let repo = d.repo().get_by_id(&d.db().pool, id).await?;
    let settings = tokio::task::spawn_blocking(move || save_settings(&repo.path, request))
        .await
        .map_err(|e| ApiError::Io(std::io::Error::other(e)))??;
    Ok(Json(ApiResponse::success(settings)))
}

#[cfg(test)]
mod tests;
