use anyhow::Error;
use executors::{executors::BaseCodingAgent, profile::ExecutorProfileId};
use serde::{Deserialize, Serialize};
use ts_rs::TS;
pub use v8::{
    EditorConfig, GitHubConfig, NotificationConfig, SendMessageShortcut, ShowcaseState, SoundFile,
    ThemeMode, UiLanguage,
};

use crate::services::config::versions::v8;

fn default_git_branch_prefix() -> String {
    "vk".to_string()
}

fn default_pr_auto_description_enabled() -> bool {
    true
}

fn default_hidden_agents() -> Vec<BaseCodingAgent> {
    vec![
        #[cfg(feature = "qa-mode")]
        BaseCodingAgent::QaMock,
    ]
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
pub struct Config {
    pub config_version: String,
    pub theme: ThemeMode,
    pub executor_profile: ExecutorProfileId,
    pub disclaimer_acknowledged: bool,
    pub onboarding_acknowledged: bool,
    pub notifications: NotificationConfig,
    pub editor: EditorConfig,
    pub github: GitHubConfig,
    pub analytics_enabled: bool,
    pub workspace_dir: Option<String>,
    pub last_app_version: Option<String>,
    pub show_release_notes: bool,
    #[serde(default)]
    pub language: UiLanguage,
    /// Last browser locale supplied with an explicit server-wide config save.
    /// This is a notification hint, not a user preference or UI language override.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub notification_browser_language: Option<String>,
    #[serde(default = "default_git_branch_prefix")]
    pub git_branch_prefix: String,
    #[serde(default)]
    pub showcases: ShowcaseState,
    #[serde(default = "default_pr_auto_description_enabled")]
    pub pr_auto_description_enabled: bool,
    #[serde(default)]
    pub pr_auto_description_prompt: Option<String>,
    #[serde(default)]
    pub send_message_shortcut: SendMessageShortcut,
    #[serde(default = "default_hidden_agents")]
    pub hidden_agents: Vec<BaseCodingAgent>,
    /// Default for new Auto-merge runs, including chat/MCP submissions.
    #[serde(default)]
    pub integration_skip_validation: bool,
}

impl Config {
    pub fn notification_language(&self) -> UiLanguage {
        match self.language {
            UiLanguage::Ja => UiLanguage::Ja,
            UiLanguage::Browser if self.notification_browser_language.as_deref() == Some("ja") => {
                UiLanguage::Ja
            }
            _ => UiLanguage::En,
        }
    }

    pub fn capture_browser_notification_language(&mut self, hint: Option<&str>, previous: &Config) {
        self.notification_browser_language = hint
            .map(|hint| match hint.split(['-', '_']).next() {
                Some(base) if base.eq_ignore_ascii_case("ja") => "ja".to_string(),
                _ => "en".to_string(),
            })
            .or_else(|| previous.notification_browser_language.clone());
    }

    fn from_v8_config(old_config: v8::Config) -> Self {
        Self {
            config_version: "v9".to_string(),
            theme: old_config.theme,
            executor_profile: old_config.executor_profile,
            disclaimer_acknowledged: old_config.disclaimer_acknowledged,
            onboarding_acknowledged: old_config.onboarding_acknowledged,
            notifications: old_config.notifications,
            editor: old_config.editor,
            github: old_config.github,
            analytics_enabled: old_config.analytics_enabled,
            workspace_dir: old_config.workspace_dir,
            last_app_version: old_config.last_app_version,
            show_release_notes: old_config.show_release_notes,
            language: old_config.language,
            notification_browser_language: None,
            git_branch_prefix: old_config.git_branch_prefix,
            showcases: old_config.showcases,
            pr_auto_description_enabled: old_config.pr_auto_description_enabled,
            pr_auto_description_prompt: old_config.pr_auto_description_prompt,
            send_message_shortcut: old_config.send_message_shortcut,
            // Provider visibility is rebuilt from the V1 registry; removed
            // provider entries are intentionally not carried across config
            // versions.
            hidden_agents: default_hidden_agents(),
            integration_skip_validation: false,
        }
    }

    pub fn from_previous_version(raw_config: &str) -> Result<Self, Error> {
        let old_config = v8::Config::from(raw_config.to_string());
        Ok(Self::from_v8_config(old_config))
    }
}

impl From<String> for Config {
    fn from(raw_config: String) -> Self {
        if let Ok(config) = serde_json::from_str::<Config>(&raw_config)
            && config.config_version == "v9"
        {
            return config;
        }

        match Self::from_previous_version(&raw_config) {
            Ok(config) => {
                tracing::info!("Config upgraded to v9");
                config
            }
            Err(e) => {
                tracing::warn!("Config migration failed: {}, using default", e);
                Self::default()
            }
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            config_version: "v9".to_string(),
            theme: ThemeMode::System,
            executor_profile: ExecutorProfileId::new(BaseCodingAgent::ClaudeCode),
            disclaimer_acknowledged: false,
            onboarding_acknowledged: false,
            notifications: NotificationConfig::default(),
            editor: EditorConfig::default(),
            github: GitHubConfig::default(),
            analytics_enabled: true,
            workspace_dir: None,
            last_app_version: None,
            show_release_notes: false,
            language: UiLanguage::default(),
            notification_browser_language: None,
            git_branch_prefix: default_git_branch_prefix(),
            showcases: ShowcaseState::default(),
            pr_auto_description_enabled: true,
            pr_auto_description_prompt: None,
            send_message_shortcut: SendMessageShortcut::default(),
            hidden_agents: default_hidden_agents(),
            integration_skip_validation: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notification_browser_hint_is_optional_persisted_and_never_overrides_explicit_language() {
        let original = Config::default();
        assert!(original.notification_browser_language.is_none());
        let mut next = original.clone();
        next.capture_browser_notification_language(Some("ja-JP"), &original);
        assert!(matches!(next.notification_language(), UiLanguage::Ja));
        let stored = serde_json::to_string(&next).unwrap();
        let reloaded: Config = serde_json::from_str(&stored).unwrap();
        assert_eq!(
            reloaded.notification_browser_language.as_deref(),
            Some("ja")
        );
        let mut old_client_save = original.clone();
        old_client_save.capture_browser_notification_language(None, &reloaded);
        assert!(matches!(
            old_client_save.notification_language(),
            UiLanguage::Ja
        ));
        next.language = UiLanguage::En;
        assert!(matches!(next.notification_language(), UiLanguage::En));
        next.language = UiLanguage::Fr;
        assert!(matches!(next.notification_language(), UiLanguage::En));
        next.language = UiLanguage::Browser;
        next.capture_browser_notification_language(Some("zh-TW"), &reloaded);
        assert!(matches!(next.notification_language(), UiLanguage::En));
        let mut legacy = serde_json::to_value(Config::default()).unwrap();
        legacy
            .as_object_mut()
            .unwrap()
            .remove("notification_browser_language");
        assert!(matches!(
            serde_json::from_value::<Config>(legacy)
                .unwrap()
                .notification_language(),
            UiLanguage::En
        ));
    }

    #[test]
    fn editor_migration_preserves_other_settings_and_explicit_empty() {
        for version in ["v9", "v8", "v7", "v6"] {
            let mut value = match version {
                "v6" => serde_json::to_value(super::super::v6::Config::default()),
                "v7" => serde_json::to_value(super::super::v7::Config::default()),
                "v8" => serde_json::to_value(v8::Config::default()),
                _ => serde_json::to_value(Config::default()),
            }
            .unwrap();
            value["git_branch_prefix"] = serde_json::json!("retained");
            value["workspace_dir"] = serde_json::json!("/custom/workspaces");
            value["analytics_enabled"] = serde_json::json!(false);
            value["editor"] = serde_json::json!({"editor_type":"FUTURE_IDE","custom_command":"never execute", "remote_ssh_host":"old"});
            let migrated = Config::from(value.to_string());
            // Verify the existing v6→v9 migration path and the current format.
            assert_eq!(
                migrated.workspace_dir.as_deref(),
                Some("/custom/workspaces"),
                "{version}"
            );
            assert!(!migrated.analytics_enabled, "{version}");
            assert!(migrated.editor.entries.is_none());
        }
        let mut value = serde_json::to_value(Config::default()).unwrap();
        value["editor"] = serde_json::json!({"entries":[]});
        value["git_branch_prefix"] = serde_json::json!("retained");
        let config = Config::from(value.to_string());
        assert_eq!(config.git_branch_prefix, "retained");
        assert!(config.editor.entries.unwrap().is_empty());
    }

    #[test]
    fn integration_preference_defaults_and_survives_restart() {
        let mut value = serde_json::to_value(Config::default()).unwrap();
        value
            .as_object_mut()
            .unwrap()
            .remove("integration_skip_validation");
        assert!(!Config::from(value.to_string()).integration_skip_validation);
        value["integration_skip_validation"] = serde_json::json!(true);
        let config = Config::from(value.to_string());
        assert!(Config::from(serde_json::to_string(&config).unwrap()).integration_skip_validation);
    }

    #[test]
    fn retired_cloud_fields_do_not_reset_local_settings() {
        let mut value = serde_json::to_value(Config::default()).unwrap();
        value["relay_enabled"] = serde_json::json!(true);
        value["host_nickname"] = serde_json::json!("old-host");
        value["remote_onboarding_acknowledged"] = serde_json::json!(true);
        value["git_branch_prefix"] = serde_json::json!("retained");
        value["commit_reminder_enabled"] = serde_json::json!(true);
        value["commit_reminder_prompt"] = serde_json::json!("old reminder");
        let config: Config = serde_json::from_value(value).unwrap();
        assert_eq!(config.git_branch_prefix, "retained");
        let saved = serde_json::to_value(config).unwrap();
        assert!(saved.get("commit_reminder_enabled").is_none());
        assert!(saved.get("commit_reminder_prompt").is_none());
        assert!(saved.get("relay_enabled").is_none());
        assert!(saved.get("host_nickname").is_none());
        let mut old = serde_json::to_value(v8::Config::default()).unwrap();
        old["git_branch_prefix"] = serde_json::json!("retained-v8");
        old["commit_reminder_enabled"] = serde_json::json!(true);
        old["commit_reminder_prompt"] = serde_json::json!("old reminder");
        let migrated = Config::from(old.to_string());
        assert_eq!(migrated.git_branch_prefix, "retained-v8");
        assert!(
            serde_json::to_value(migrated)
                .unwrap()
                .get("commit_reminder_enabled")
                .is_none()
        );
    }

    #[test]
    fn default_config_hides_non_primary_agents() {
        let hidden_agents = Config::default().hidden_agents;

        assert!(!hidden_agents.contains(&BaseCodingAgent::ClaudeCode));
        assert!(!hidden_agents.contains(&BaseCodingAgent::Codex));
        #[cfg(feature = "qa-mode")]
        assert_eq!(hidden_agents, vec![BaseCodingAgent::QaMock]);
        #[cfg(not(feature = "qa-mode"))]
        assert!(hidden_agents.is_empty());
    }

    #[test]
    fn missing_hidden_agents_uses_default_hidden_agents() {
        let mut raw_config = serde_json::to_value(Config::default()).unwrap();
        raw_config.as_object_mut().unwrap().remove("hidden_agents");

        let config: Config = serde_json::from_value(raw_config).unwrap();

        assert_eq!(config.hidden_agents, default_hidden_agents());
    }
}
