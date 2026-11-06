use std::str::FromStr;

use axum::{Extension, extract::State, response::Json};
use db::models::session::Session;
use deployment::Deployment;
use executors::{
    executors::BaseCodingAgent, profile::ExecutorConfig, runtime::ProviderSessionReference,
};
use sqlx::{SqlitePool, types::Json as SqlJson};
use utils::response::ApiResponse;
use uuid::Uuid;

use crate::{DeploymentImpl, error::ApiError};

pub(super) async fn get_executor_config(
    Extension(session): Extension<Session>,
    State(deployment): State<DeploymentImpl>,
) -> Result<Json<ApiResponse<Option<ExecutorConfig>>>, ApiError> {
    let config = read_executor_config(&deployment.db().pool, session.id).await?;
    Ok(Json(ApiResponse::success(config)))
}

/// Read the immutable launch request, not script processes or mutable presets.
pub(crate) async fn read_executor_config(
    pool: &SqlitePool,
    session_id: Uuid,
) -> Result<Option<ExecutorConfig>, ApiError> {
    let config = sqlx::query_scalar::<_, SqlJson<ExecutorConfig>>(
        r#"
        SELECT json_extract(attempt.request_envelope, '$.executor_config')
        FROM agent_run_attempts attempt
        JOIN agent_runs run ON run.id = attempt.agent_run_id
        WHERE run.session_id = ?
        ORDER BY julianday(attempt.created_at) DESC, attempt.rowid DESC
        LIMIT 1
        "#,
    )
    .bind(session_id)
    .fetch_optional(pool)
    .await?;
    if let Some(config) = config {
        return Ok(Some(config.0));
    }

    // Native adoption can bind a session before its first VK run launches.
    let binding = sqlx::query_scalar::<_, SqlJson<ProviderSessionReference>>(
        "SELECT session_reference FROM agent_provider_sessions WHERE session_id = ? LIMIT 1",
    )
    .bind(session_id)
    .fetch_optional(pool)
    .await?;
    binding
        .map(|binding| config_from_binding(&binding.0))
        .transpose()
}

pub(super) async fn get_execution_settings(
    Extension(session): Extension<Session>,
    State(deployment): State<DeploymentImpl>,
) -> Result<Json<ApiResponse<serde_json::Value>>, ApiError> {
    Ok(Json(ApiResponse::success(
        read_execution_settings(&deployment.db().pool, session.id).await?,
    )))
}

pub(crate) async fn read_execution_settings(
    pool: &SqlitePool,
    session_id: Uuid,
) -> Result<serde_json::Value, ApiError> {
    use serde_json::json;
    let session = Session::find_by_id(pool, session_id)
        .await?
        .ok_or_else(|| ApiError::BadRequest("Session missing".into()))?;
    let config = read_executor_config(pool, session_id).await?;
    let binding = sqlx::query_scalar::<_, SqlJson<ProviderSessionReference>>(
        "SELECT session_reference FROM agent_provider_sessions WHERE session_id=? LIMIT 1",
    )
    .bind(session_id)
    .fetch_optional(pool)
    .await?
    .map(|v| v.0);
    let adopted = binding
        .as_ref()
        .and_then(|b| b.metadata.as_ref())
        .and_then(|m| m.get("source"))
        .and_then(|s| s.as_str())
        == Some("native_adopted");
    let path: Option<String> =
        sqlx::query_scalar("SELECT container_ref FROM workspaces WHERE id=?")
            .bind(session.workspace_id)
            .fetch_one(pool)
            .await?;
    let capabilities = match (&config, path) {
        (Some(config), Some(path)) if !adopted => {
            Some(next_run_capabilities(config, std::path::Path::new(&path)).await)
        }
        _ => None,
    };
    Ok(
        json!({"session_id":session_id,"workspace_id":session.workspace_id,
        "executor_config":config,"capabilities":capabilities,
        "native_session_id":binding.as_ref().map(|b| &b.provider_session_id),
        "native_adopted":adopted,"settings_change_allowed":!adopted,
        "settings_change_reason":if adopted {Some("native_adopted Session configuration and selected_skills fingerprints are fixed; create a new Session to change settings")} else {None},
        "engine_change_requires_new_session":true,"applies_to":"next_run",
        "provider_settings_verified":false}),
    )
}

/// Advertised values are validated using the same provider discovery as the UI.
/// Provider adoption of these values is only known after an actual launch.
#[derive(Debug, serde::Serialize)]
pub(crate) struct NextRunCapabilities {
    pub execution_modes: Vec<executors::profile::ExecutionMode>,
    pub reasoning_options: Vec<executors::model_selector::ReasoningOption>,
    pub effective_model_id: Option<String>,
    pub reasoning_scope: &'static str,
    pub unavailable_reason: Option<String>,
}

pub(crate) async fn next_run_capabilities(
    config: &ExecutorConfig,
    workdir: &std::path::Path,
) -> NextRunCapabilities {
    use executors::{
        executors::{CodingAgent, StandardCodingAgentExecutor},
        profile::{ExecutionMode, ExecutorConfigs},
    };
    let modes = if config.executor == BaseCodingAgent::Codex {
        vec![
            ExecutionMode::Code,
            ExecutionMode::Review,
            ExecutionMode::Plan,
            ExecutionMode::Goal,
        ]
    } else {
        vec![]
    };
    let mut result = NextRunCapabilities {
        execution_modes: modes,
        reasoning_options: vec![],
        effective_model_id: None,
        reasoning_scope: "selected_model",
        unavailable_reason: None,
    };
    let Some(mut agent) = ExecutorConfigs::get_cached().get_coding_agent(&config.profile_id())
    else {
        result.unavailable_reason = Some(
            "Runtime profile no longer exists; start a new Session with a valid profile".into(),
        );
        return result;
    };
    agent.apply_overrides(config);
    let preset = agent.get_preset_options();
    let model_id = preset.model_id;
    result.effective_model_id = model_id.clone();
    let discovered = match agent {
        CodingAgent::Codex(codex) => {
            match tokio::time::timeout(
                std::time::Duration::from_secs(30),
                codex.discover_next_run_reasoning(workdir.to_path_buf()),
            )
            .await
            {
                Ok(Ok((model, options))) => {
                    result.reasoning_scope = if model.is_some() {
                        "selected_model"
                    } else {
                        "all_advertised_models"
                    };
                    result.effective_model_id = model;
                    result.reasoning_options = options;
                    if result.reasoning_options.is_empty() {
                        result.unavailable_reason = Some("No compatible reasoning options were advertised for the selected model or across live advertised models".into());
                    }
                }
                Ok(Err(_)) => {
                    result.unavailable_reason =
                        Some("Unable to verify Codex model discovery".into())
                }
                Err(_) => {
                    result.unavailable_reason = Some("Codex model discovery timed out".into())
                }
            }
            return result;
        }
        CodingAgent::ClaudeCode(claude) => match claude.supports_effort_flag().await {
            Ok(true) => Ok(executors::executors::claude::claude_model_options(true)),
            Ok(false) => Err("Installed Claude Code CLI does not support --effort".into()),
            Err(_) => Err("Unable to verify Claude Code CLI --effort support".into()),
        },
        #[cfg(feature = "qa-mode")]
        CodingAgent::QaMock(_) => {
            Err("Reasoning overrides are not supported by this provider".into())
        }
    };
    match discovered {
        Ok(models) => {
            if let Some(model) = model_id
                .as_ref()
                .and_then(|id| models.iter().find(|model| &model.id == id))
            {
                result.reasoning_options = model.reasoning_options.clone();
                if result.reasoning_options.is_empty() {
                    result.unavailable_reason =
                        Some("The selected model advertises no reasoning options".into());
                }
            } else {
                result.unavailable_reason = Some("The effective model cannot be identified in provider discovery; select a model before setting reasoning".into());
            }
        }
        Err(error) => result.unavailable_reason = Some(error),
    }
    result
}

pub(crate) async fn apply_next_run_overrides(
    saved: &ExecutorConfig,
    overrides: &executors::profile::NextRunOverrides,
    workdir: &std::path::Path,
) -> Result<ExecutorConfig, ApiError> {
    use executors::profile::{ExecutionMode, SettingOverride};
    let mut config = saved.clone();
    if let Some(mode) = &overrides.execution_mode {
        if config.executor != BaseCodingAgent::Codex {
            return Err(ApiError::BadRequest(
                "execution_mode is supported only by Codex; changing engine requires a new Session"
                    .into(),
            ));
        }
        config.execution_mode = match mode {
            SettingOverride::Set {
                value: ExecutionMode::PlanWithGoal,
            } => {
                return Err(ApiError::BadRequest(
                    "plan_with_goal is a legacy mode; select plan or goal explicitly".into(),
                ));
            }
            SettingOverride::Set { value } => Some(*value),
            SettingOverride::Reset {} => None,
        };
        // Mode reset also discards Goal-only launch options. Goal is still explicit intent.
        if config.execution_mode != Some(ExecutionMode::Goal) {
            config.goal_token_budget = None;
            config.goal_max_concurrent_agents = None;
        }
    }
    if let Some(reasoning) = &overrides.reasoning_id {
        config.reasoning_id = match reasoning {
            SettingOverride::Reset {} => None,
            SettingOverride::Set { value } => {
                let capabilities = next_run_capabilities(&config, workdir).await;
                validate_reasoning(value, &capabilities)?;
                Some(value.clone())
            }
        };
    }
    Ok(config)
}

fn validate_reasoning(value: &str, capabilities: &NextRunCapabilities) -> Result<(), ApiError> {
    if !capabilities
        .reasoning_options
        .iter()
        .any(|option| option.id == value)
    {
        return Err(ApiError::BadRequest(format!(
            "reasoning_id {value:?} is not available for the selected provider/model: {}",
            capabilities
                .unavailable_reason
                .as_deref()
                .unwrap_or("value was not advertised by provider discovery")
        )));
    }
    Ok(())
}

fn config_from_binding(binding: &ProviderSessionReference) -> Result<ExecutorConfig, ApiError> {
    if let Some(config) = binding
        .metadata
        .as_ref()
        .and_then(|metadata| metadata.get("profile_context"))
        .and_then(|context| context.get("executor_config"))
    {
        return serde_json::from_value(config.clone())
            .map_err(|error| ApiError::Database(sqlx::Error::Decode(Box::new(error))));
    }
    let (executor, variant) = binding.runtime_profile_id.split_once(':').map_or(
        (binding.runtime_profile_id.as_str(), None),
        |(executor, variant)| (executor, Some(variant.to_owned())),
    );
    let executor = BaseCodingAgent::from_str(executor)
        .map_err(|error| ApiError::Database(sqlx::Error::Decode(Box::new(error))))?;
    Ok(ExecutorConfig {
        variant,
        ..ExecutorConfig::new(executor)
    })
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use executors::runtime::PROVIDER_SESSION_REFERENCE_SCHEMA_VERSION;
    use serde_json::json;

    use super::*;

    #[tokio::test]
    async fn next_run_override_preserves_unrelated_fields_and_clears_goal_only_options() {
        use executors::profile::{ExecutionMode, NextRunOverrides, SettingOverride};
        let saved = ExecutorConfig {
            model_id: Some("custom-model".into()),
            variant: Some("CUSTOM".into()),
            reasoning_id: Some("high".into()),
            permission_policy: Some(executors::model_selector::PermissionPolicy::Supervised),
            execution_mode: Some(ExecutionMode::Goal),
            goal_token_budget: Some(5000),
            goal_max_concurrent_agents: Some(0),
            ..ExecutorConfig::new(BaseCodingAgent::Codex)
        };
        let path = std::path::Path::new(".");
        assert_eq!(
            apply_next_run_overrides(&saved, &NextRunOverrides::default(), path)
                .await
                .unwrap(),
            saved
        );
        for mode in [ExecutionMode::Review, ExecutionMode::Code] {
            let overrides = NextRunOverrides {
                execution_mode: Some(SettingOverride::Set { value: mode }),
                ..Default::default()
            };
            let actual = apply_next_run_overrides(&saved, &overrides, path)
                .await
                .unwrap();
            let mut expected = saved.clone();
            expected.execution_mode = Some(mode);
            expected.goal_token_budget = None;
            expected.goal_max_concurrent_agents = None;
            assert_eq!(actual, expected);
        }
        let overrides: NextRunOverrides =
            serde_json::from_value(json!({"reasoning_id":{"action":"reset"}})).unwrap();
        let actual = apply_next_run_overrides(&saved, &overrides, path)
            .await
            .unwrap();
        let mut expected = saved.clone();
        expected.reasoning_id = None;
        assert_eq!(actual, expected);
        let overrides: NextRunOverrides =
            serde_json::from_value(json!({"execution_mode":{"action":"reset"}})).unwrap();
        let actual = apply_next_run_overrides(&saved, &overrides, path)
            .await
            .unwrap();
        assert_eq!(actual.execution_mode, None);
        assert_eq!(actual.goal_token_budget, None);
        assert_eq!(actual.reasoning_id, saved.reasoning_id);
    }

    #[tokio::test]
    async fn unsupported_provider_mode_and_legacy_goal_mode_are_rejected() {
        use executors::profile::NextRunOverrides;
        let overrides: NextRunOverrides =
            serde_json::from_value(json!({"execution_mode":{"action":"set","value":"review"}}))
                .unwrap();
        assert!(
            apply_next_run_overrides(
                &ExecutorConfig::new(BaseCodingAgent::ClaudeCode),
                &overrides,
                std::path::Path::new(".")
            )
            .await
            .is_err()
        );
        let overrides: NextRunOverrides = serde_json::from_value(
            json!({"execution_mode":{"action":"set","value":"plan_with_goal"}}),
        )
        .unwrap();
        assert!(
            apply_next_run_overrides(
                &ExecutorConfig::new(BaseCodingAgent::Codex),
                &overrides,
                std::path::Path::new(".")
            )
            .await
            .is_err()
        );
    }

    #[test]
    fn reasoning_values_must_be_advertised_for_effective_model() {
        let mut capabilities = NextRunCapabilities {
            execution_modes: vec![],
            reasoning_options: executors::model_selector::ReasoningOption::from_names([
                "high", "max",
            ]),
            effective_model_id: Some("model".into()),
            reasoning_scope: "selected_model",
            unavailable_reason: None,
        };
        assert!(validate_reasoning("max", &capabilities).is_ok());
        assert!(validate_reasoning("ultra", &capabilities).is_err());
        assert!(validate_reasoning("", &capabilities).is_err());
        capabilities.reasoning_options.clear();
        capabilities.unavailable_reason = Some("CLI does not support --effort".into());
        assert!(validate_reasoning("high", &capabilities).is_err());
    }

    fn binding(profile: &str) -> ProviderSessionReference {
        ProviderSessionReference {
            schema_version: PROVIDER_SESSION_REFERENCE_SCHEMA_VERSION,
            provider_id: "codex".into(),
            runtime_profile_id: profile.into(),
            provider_session_id: "native-session".into(),
            observed_at: Utc::now(),
            metadata: None,
        }
    }

    #[test]
    fn native_binding_preserves_profile_and_adoption_overrides() {
        let mut reference = binding("CODEX:PLAN");
        assert_eq!(
            config_from_binding(&reference).unwrap().variant.as_deref(),
            Some("PLAN")
        );
        let config = ExecutorConfig {
            variant: Some("PLAN".into()),
            model_id: Some("provider/custom/model".into()),
            agent_id: Some("plan".into()),
            reasoning_id: Some("high".into()),
            ..ExecutorConfig::new(BaseCodingAgent::Codex)
        };
        reference.metadata = Some(json!({"profile_context": {"executor_config": config}}));
        assert_eq!(config_from_binding(&reference).unwrap(), config);
        assert!(config_from_binding(&binding("UNKNOWN:PLAN")).is_err());
    }

    #[tokio::test]
    async fn empty_session_does_not_borrow_another_sessions_binding() {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        sqlx::raw_sql(
            "CREATE TABLE agent_runs (id BLOB, session_id BLOB);
             CREATE TABLE agent_run_attempts (agent_run_id BLOB, request_envelope TEXT, created_at TEXT);
             CREATE TABLE agent_provider_sessions (session_id BLOB, session_reference TEXT);",
        ).execute(&pool).await.unwrap();
        let owner = Uuid::new_v4();
        sqlx::query("INSERT INTO agent_provider_sessions VALUES (?, ?)")
            .bind(owner)
            .bind(SqlJson(binding("CODEX:PLAN")))
            .execute(&pool)
            .await
            .unwrap();
        assert!(
            read_executor_config(&pool, Uuid::new_v4())
                .await
                .unwrap()
                .is_none()
        );
        assert_eq!(
            read_executor_config(&pool, owner)
                .await
                .unwrap()
                .unwrap()
                .variant
                .as_deref(),
            Some("PLAN")
        );
    }

    #[tokio::test]
    async fn next_run_mode_and_reset_survive_immutable_launch_readback() {
        use executors::profile::{ExecutionMode, NextRunOverrides};
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        sqlx::raw_sql(
            "CREATE TABLE agent_runs (id BLOB, session_id BLOB);
             CREATE TABLE agent_run_attempts (agent_run_id BLOB, request_envelope TEXT, created_at TEXT);
             CREATE TABLE agent_provider_sessions (session_id BLOB, session_reference TEXT);",
        ).execute(&pool).await.unwrap();
        let session = Uuid::new_v4();
        let run = Uuid::new_v4();
        sqlx::query("INSERT INTO agent_runs VALUES (?, ?)")
            .bind(run)
            .bind(session)
            .execute(&pool)
            .await
            .unwrap();
        let mut saved = ExecutorConfig {
            execution_mode: Some(ExecutionMode::Review),
            reasoning_id: Some("high".into()),
            model_id: Some("custom-model".into()),
            ..ExecutorConfig::new(BaseCodingAgent::Codex)
        };
        for (date, input) in [
            (
                "2026-11-06T01:00:00Z",
                json!({"execution_mode":{"action":"set","value":"code"}}),
            ),
            (
                "2026-11-06T02:00:00Z",
                json!({"execution_mode":{"action":"reset"},"reasoning_id":{"action":"reset"}}),
            ),
        ] {
            let overrides: NextRunOverrides = serde_json::from_value(input).unwrap();
            let expected = apply_next_run_overrides(&saved, &overrides, std::path::Path::new("."))
                .await
                .unwrap();
            sqlx::query("INSERT INTO agent_run_attempts VALUES (?, ?, ?)")
                .bind(run)
                .bind(SqlJson(json!({"executor_config":expected})))
                .bind(date)
                .execute(&pool)
                .await
                .unwrap();
            let restored = read_executor_config(&pool, session).await.unwrap().unwrap();
            assert_eq!(restored, expected);
            assert_eq!(restored.model_id.as_deref(), Some("custom-model"));
            saved = restored;
        }
        assert_eq!(saved.execution_mode, None);
        assert_eq!(saved.reasoning_id, None);
    }

    #[tokio::test]
    async fn latest_attempt_restores_all_overrides_and_respects_session_boundary() {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        sqlx::raw_sql(
            "CREATE TABLE agent_runs (id BLOB, session_id BLOB);
             CREATE TABLE agent_run_attempts (agent_run_id BLOB, request_envelope TEXT, created_at TEXT);
             CREATE TABLE agent_provider_sessions (session_id BLOB, session_reference TEXT);",
        ).execute(&pool).await.unwrap();
        let session_id = Uuid::new_v4();
        let run_id = Uuid::new_v4();
        let sibling_run_id = Uuid::new_v4();
        for (run, session) in [(run_id, session_id), (sibling_run_id, Uuid::new_v4())] {
            sqlx::query("INSERT INTO agent_runs VALUES (?, ?)")
                .bind(run)
                .bind(session)
                .execute(&pool)
                .await
                .unwrap();
        }
        let expected = ExecutorConfig {
            variant: Some("PLAN".into()),
            model_id: Some("provider/model:latest".into()),
            agent_id: Some("plan".into()),
            reasoning_id: Some("high".into()),
            permission_policy: Some(executors::model_selector::PermissionPolicy::Supervised),
            execution_mode: Some(executors::profile::ExecutionMode::Review),
            goal_token_budget: Some(123456),
            goal_max_concurrent_agents: Some(0),
            ..ExecutorConfig::new(BaseCodingAgent::Codex)
        };
        // Mixed SQLite/RFC3339 dates must order by time, not their separator.
        for (run, date, config) in [
            (
                run_id,
                "2026-09-07T09:00:00Z",
                ExecutorConfig::new(BaseCodingAgent::Codex),
            ),
            (run_id, "2026-09-07 10:00:00", expected.clone()),
            (
                sibling_run_id,
                "2026-09-07T11:00:00Z",
                ExecutorConfig::new(BaseCodingAgent::ClaudeCode),
            ),
        ] {
            sqlx::query("INSERT INTO agent_run_attempts VALUES (?, ?, ?)")
                .bind(run)
                .bind(SqlJson(json!({"executor_config": config})))
                .bind(date)
                .execute(&pool)
                .await
                .unwrap();
        }
        assert_eq!(
            read_executor_config(&pool, session_id).await.unwrap(),
            Some(expected)
        );
        // A malformed latest snapshot is an error, never fallback to an older run/default.
        sqlx::query("INSERT INTO agent_run_attempts VALUES (?, ?, '2026-09-07T12:00:00Z')")
            .bind(run_id)
            .bind("{}")
            .execute(&pool)
            .await
            .unwrap();
        assert!(read_executor_config(&pool, session_id).await.is_err());
    }
}
