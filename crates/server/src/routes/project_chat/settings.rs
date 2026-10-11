//! Model preferences. Legacy saved keys are preserved but never used for inference.
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

pub(super) struct ModelSettings {
    pub model: String,
    pub reasoning_effort: String,
}

#[derive(Debug, Serialize, TS)]
pub struct AstraSettingsView {
    pub revision: String,
    pub configured: bool,
    pub authentication: AstraAuthentication,
    pub model: String,
    pub reasoning_effort: String,
}

#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum AstraAuthentication {
    CodexChatgpt,
}

#[derive(Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct UpdateAstraSettings {
    pub expected_revision: String,
    #[serde(default = "preserve_key")]
    pub api_key: SensitiveWrite<String>,
    pub model: String,
    pub reasoning_effort: String,
}

fn preserve_key() -> SensitiveWrite<String> {
    SensitiveWrite::Preserve
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
        api_key: None,
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
        // The async HTTP handlers fill in the current Codex login state.
        configured: false,
        authentication: AstraAuthentication::CodexChatgpt,
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
    let mut view = view(&saved, &environment(), revision);
    view.configured = super::model::configured().await;
    Ok(Json(ApiResponse::success(view)))
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
        SensitiveWrite::Replace { .. } => {
            return Err(ApiError::BadRequest(
                "Astra uses the server's Codex ChatGPT login, not an API key.".into(),
            ));
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
    let mut view = update(&path(), &environment(), request)?;
    view.configured = super::model::configured().await;
    Ok(Json(ApiResponse::success(view)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(revision: &str) -> UpdateAstraSettings {
        UpdateAstraSettings {
            expected_revision: revision.into(),
            api_key: SensitiveWrite::Preserve,
            model: "test-model".into(),
            reasoning_effort: "xhigh".into(),
        }
    }

    #[test]
    fn migration_preserves_legacy_keys_but_does_not_treat_them_as_authentication() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        std::fs::write(&path, br#"{"api_key":"legacy-secret","model":"old-model"}"#).unwrap();
        let env = StoredSettings::default();
        let (_, revision) = read(&path).unwrap();
        let result = update(&path, &env, request(&revision)).unwrap();
        assert!(!result.configured);
        assert!(matches!(
            result.authentication,
            AstraAuthentication::CodexChatgpt
        ));
        assert!(
            !serde_json::to_string(&result)
                .unwrap()
                .contains("legacy-secret")
        );
        let (saved, _) = read(&path).unwrap();
        assert_eq!(saved.api_key.as_deref(), Some("legacy-secret"));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
        assert_eq!(resolve(&saved, &env).model, "test-model");
        assert!(matches!(
            update(&path, &env, request(&revision)),
            Err(ApiError::Conflict(_))
        ));
        let mut clear = request(&result.revision);
        clear.api_key = SensitiveWrite::Clear;
        update(&path, &env, clear).unwrap();
        assert!(read(&path).unwrap().0.api_key.is_none());
    }

    #[test]
    fn model_preferences_work_without_keys_and_new_keys_are_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let env = StoredSettings {
            model: Some("env-model".into()),
            ..Default::default()
        };
        assert_eq!(resolve(&StoredSettings::default(), &env).model, "env-model");
        assert_eq!(
            resolve(&StoredSettings::default(), &env).reasoning_effort,
            "xhigh"
        );
        let mut replacement = request("missing");
        replacement.api_key = SensitiveWrite::Replace {
            value: "new-secret".into(),
        };
        let error = update(&path, &env, replacement).unwrap_err();
        assert!(!error.to_string().contains("new-secret"));
        assert!(!path.exists());
        let from_json: UpdateAstraSettings = serde_json::from_value(serde_json::json!({"expected_revision":"missing","model":"test-model","reasoning_effort":"high"})).unwrap();
        update(&path, &env, from_json).unwrap();
        assert!(read(&path).unwrap().0.api_key.is_none());
    }

    #[test]
    fn corrupt_settings_are_not_overwritten_or_echoed() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        std::fs::write(&path, b"private malformed settings").unwrap();
        let error = update(&path, &StoredSettings::default(), request("missing")).unwrap_err();
        assert!(!error.to_string().contains("private"));
        assert_eq!(std::fs::read(&path).unwrap(), b"private malformed settings");
    }
}
