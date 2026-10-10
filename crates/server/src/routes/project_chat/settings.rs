//! Server-local credentials. Never include secrets in the public config, chat,
//! tool context or errors. Keep environment configuration as a fallback.
use std::{io::Write, path::Path};

use axum::{Json, extract::rejection::JsonRejection};
use executors::config_write::{NATIVE_SETTINGS_WRITE_LOCK, SensitiveWrite};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use ts_rs::TS;
use utils::response::ApiResponse;

use crate::error::ApiError;

#[derive(Default, Serialize, Deserialize)]
struct StoredSettings {
    api_key: Option<String>,
    model: Option<String>,
    reasoning_effort: Option<String>,
}

// Deliberately no Debug/Serialize: credentials must not be logged or echoed.
pub(super) struct ModelSettings {
    pub api_key: String,
    pub model: String,
    pub reasoning_effort: String,
}

#[derive(Debug, Serialize, TS)]
pub struct AstraSettingsView {
    pub revision: String,
    pub configured: bool,
    pub api_key_source: AstraKeySource,
    pub model: String,
    pub reasoning_effort: String,
}

#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum AstraKeySource {
    Saved,
    Environment,
    Missing,
}

#[derive(Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct UpdateAstraSettings {
    pub expected_revision: String,
    pub api_key: SensitiveWrite<String>,
    pub model: String,
    pub reasoning_effort: String,
}

fn path() -> std::path::PathBuf {
    utils::assets::asset_dir().join("astra-settings.json")
}

fn environment() -> StoredSettings {
    let value = |name| {
        std::env::var(name)
            .ok()
            .map(|v| v.trim().to_owned())
            .filter(|v| !v.is_empty())
    };
    StoredSettings {
        api_key: value("OPENAI_API_KEY"),
        model: value("LVK_ASTRA_MODEL"),
        reasoning_effort: value("LVK_ASTRA_REASONING_EFFORT"),
    }
}

fn storage_error(_: impl std::fmt::Display) -> ApiError {
    ApiError::BadRequest("Astra settings could not be read or saved on the server.".into())
}

fn read(path: &Path) -> Result<(StoredSettings, String), ApiError> {
    match std::fs::read(path) {
        Ok(bytes) => {
            let stored = serde_json::from_slice(&bytes).map_err(storage_error)?;
            Ok((stored, format!("{:x}", Sha256::digest(bytes))))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            Ok((StoredSettings::default(), "missing".into()))
        }
        Err(error) => Err(storage_error(error)),
    }
}

fn resolve(saved: &StoredSettings, env: &StoredSettings) -> ModelSettings {
    ModelSettings {
        api_key: saved
            .api_key
            .clone()
            .or_else(|| env.api_key.clone())
            .unwrap_or_default(),
        model: saved
            .model
            .clone()
            .or_else(|| env.model.clone())
            .unwrap_or_default(),
        reasoning_effort: saved
            .reasoning_effort
            .clone()
            .or_else(|| env.reasoning_effort.clone())
            .unwrap_or_else(|| "xhigh".into()),
    }
}

fn view(saved: &StoredSettings, env: &StoredSettings, revision: String) -> AstraSettingsView {
    let effective = resolve(saved, env);
    AstraSettingsView {
        revision,
        configured: !effective.api_key.is_empty() && !effective.model.is_empty(),
        api_key_source: if saved.api_key.is_some() {
            AstraKeySource::Saved
        } else if env.api_key.is_some() {
            AstraKeySource::Environment
        } else {
            AstraKeySource::Missing
        },
        model: effective.model,
        reasoning_effort: effective.reasoning_effort,
    }
}

pub(super) fn resolved() -> Result<ModelSettings, ApiError> {
    let (saved, _) = read(&path())?;
    Ok(resolve(&saved, &environment()))
}

pub(super) async fn get() -> Result<Json<ApiResponse<AstraSettingsView>>, ApiError> {
    let (saved, revision) = read(&path())?;
    Ok(Json(ApiResponse::success(view(
        &saved,
        &environment(),
        revision,
    ))))
}

fn update(
    path: &Path,
    env: &StoredSettings,
    request: UpdateAstraSettings,
) -> Result<AstraSettingsView, ApiError> {
    let _guard = NATIVE_SETTINGS_WRITE_LOCK.lock().map_err(storage_error)?;
    let (mut saved, revision) = read(path)?;
    if request.expected_revision != revision {
        return Err(ApiError::Conflict(
            "Astra settings changed. Reload settings before saving.".into(),
        ));
    }
    let model = request.model.trim();
    if model.is_empty() || model.len() > 200 || model.chars().any(char::is_whitespace) {
        return Err(ApiError::BadRequest("Enter a valid model ID.".into()));
    }
    let effort = request.reasoning_effort.trim();
    if ![
        "none", "minimal", "low", "medium", "high", "xhigh", "max", "ultra",
    ]
    .contains(&effort)
    {
        return Err(ApiError::BadRequest(
            "Choose a supported reasoning effort.".into(),
        ));
    }
    match request.api_key {
        SensitiveWrite::Preserve => {}
        SensitiveWrite::Clear => saved.api_key = None,
        SensitiveWrite::Replace { value } => {
            let key = value.trim();
            if key.is_empty()
                || key.len() > 4096
                || !key.is_ascii()
                || key.chars().any(char::is_whitespace)
                || key.chars().any(char::is_control)
            {
                return Err(ApiError::BadRequest(
                    "Enter a non-empty API key without whitespace.".into(),
                ));
            }
            saved.api_key = Some(key.to_owned());
        }
    }
    saved.model = Some(model.to_owned());
    saved.reasoning_effort = Some(effort.to_owned());
    let parent = path.parent().ok_or_else(|| storage_error("parent"))?;
    std::fs::create_dir_all(parent).map_err(storage_error)?;
    // Same atomic-file pattern as executor profiles. NamedTempFile is owner-only
    // on Unix; replacement cannot inherit an old world's readable permissions.
    let mut temporary = tempfile::NamedTempFile::new_in(parent).map_err(storage_error)?;
    let bytes = serde_json::to_vec_pretty(&saved).map_err(storage_error)?;
    temporary.write_all(&bytes).map_err(storage_error)?;
    temporary.as_file().sync_all().map_err(storage_error)?;
    temporary
        .persist(path)
        .map_err(|e| storage_error(e.error))?;
    Ok(view(&saved, env, format!("{:x}", Sha256::digest(bytes))))
}

pub(super) async fn put(
    request: Result<Json<UpdateAstraSettings>, JsonRejection>,
) -> Result<Json<ApiResponse<AstraSettingsView>>, ApiError> {
    // JSON deserialization errors may contain the supplied secret.
    let Json(request) =
        request.map_err(|_| ApiError::BadRequest("Invalid Astra settings request.".into()))?;
    Ok(Json(ApiResponse::success(update(
        &path(),
        &environment(),
        request,
    )?)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(revision: &str, api_key: SensitiveWrite<String>) -> UpdateAstraSettings {
        UpdateAstraSettings {
            expected_revision: revision.into(),
            api_key,
            model: "test-model".into(),
            reasoning_effort: "high".into(),
        }
    }

    #[test]
    fn credentials_survive_reload_stay_private_and_require_current_revision() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let env = StoredSettings::default();
        let first = update(
            &path,
            &env,
            request(
                "missing",
                SensitiveWrite::Replace {
                    value: "test-secret".into(),
                },
            ),
        )
        .unwrap();
        let public = serde_json::to_string(&first).unwrap();
        assert!(!public.contains("test-secret"));
        assert!(first.configured);
        assert!(matches!(first.api_key_source, AstraKeySource::Saved));
        let before = std::fs::read(&path).unwrap();
        assert!(matches!(
            update(&path, &env, request("missing", SensitiveWrite::Clear)),
            Err(ApiError::Conflict(_))
        ));
        assert_eq!(before, std::fs::read(&path).unwrap());
        let mut next = request(&first.revision, SensitiveWrite::Preserve);
        next.model = "second-model".into();
        let second = update(&path, &env, next).unwrap();
        let (saved, _) = read(&path).unwrap();
        assert_eq!(resolve(&saved, &env).api_key, "test-secret");
        assert_eq!(resolve(&saved, &env).model, "second-model");
        let cleared = update(
            &path,
            &env,
            request(&second.revision, SensitiveWrite::Clear),
        )
        .unwrap();
        assert!(!cleared.configured);
        assert!(
            !std::fs::read_to_string(&path)
                .unwrap()
                .contains("test-secret")
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
    }

    #[test]
    fn environment_fallback_is_not_copied_to_disk_and_clear_restores_it() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let env = StoredSettings {
            api_key: Some("env-secret".into()),
            model: Some("env-model".into()),
            reasoning_effort: None,
        };
        let (saved, revision) = read(&path).unwrap();
        assert_eq!(resolve(&saved, &env).model, "env-model");
        assert_eq!(resolve(&saved, &env).reasoning_effort, "xhigh");
        let first = update(&path, &env, request(&revision, SensitiveWrite::Preserve)).unwrap();
        assert!(matches!(first.api_key_source, AstraKeySource::Environment));
        assert!(
            !std::fs::read_to_string(&path)
                .unwrap()
                .contains("env-secret")
        );
        let second = update(
            &path,
            &env,
            request(
                &first.revision,
                SensitiveWrite::Replace {
                    value: "ui-secret".into(),
                },
            ),
        )
        .unwrap();
        let third = update(
            &path,
            &env,
            request(&second.revision, SensitiveWrite::Clear),
        )
        .unwrap();
        assert!(third.configured);
        assert!(matches!(third.api_key_source, AstraKeySource::Environment));
    }

    #[test]
    fn invalid_or_corrupt_settings_never_overwrite_or_echo_secrets() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let env = StoredSettings::default();
        let error = update(
            &path,
            &env,
            request(
                "missing",
                SensitiveWrite::Replace {
                    value: "secret\ninvalid".into(),
                },
            ),
        )
        .unwrap_err();
        assert!(!error.to_string().contains("secret"));
        assert!(!path.exists());
        std::fs::write(&path, b"secret: malformed").unwrap();
        let error = update(&path, &env, request("missing", SensitiveWrite::Clear)).unwrap_err();
        assert!(!error.to_string().contains("secret"));
        assert_eq!(std::fs::read(&path).unwrap(), b"secret: malformed");
    }
}
