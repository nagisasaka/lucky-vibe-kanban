use std::sync::{Arc, OnceLock};

use async_trait::async_trait;
use tokio::sync::RwLock;
use utils::{self, command_ext::NoWindowExt};
use uuid::Uuid;

use crate::services::config::{Config, SoundFile};

/// Trait for sending push notifications. Implementations can use
/// platform-specific OS commands, Tauri's notification plugin, etc.
#[async_trait]
pub trait PushNotifier: Send + Sync + 'static {
    async fn send(&self, title: &str, message: &str, workspace_id: Option<Uuid>);
}

/// Global push notifier set before server startup (e.g., by the Tauri app).
/// Falls back to `DefaultPushNotifier` if not set.
static GLOBAL_PUSH_NOTIFIER: OnceLock<Arc<dyn PushNotifier>> = OnceLock::new();

/// Register a custom push notifier globally. Must be called before the server
/// starts (i.e., before `LocalDeployment::new()`). Typically called from the
/// Tauri app to inject a `TauriNotifier` that uses the native notification API.
pub fn set_global_push_notifier(notifier: Arc<dyn PushNotifier>) {
    let _ = GLOBAL_PUSH_NOTIFIER.set(notifier);
}

/// Get the global push notifier, or `DefaultPushNotifier` if none was set.
pub fn get_global_push_notifier() -> Arc<dyn PushNotifier> {
    GLOBAL_PUSH_NOTIFIER
        .get()
        .cloned()
        .unwrap_or_else(|| Arc::new(DefaultPushNotifier))
}

/// Default push notifier using platform-specific OS commands.
/// Used as a fallback when no Tauri app handle is available.
pub struct DefaultPushNotifier;

/// Cache for WSL root path from PowerShell
static WSL_ROOT_PATH_CACHE: OnceLock<Option<String>> = OnceLock::new();

#[async_trait]
impl PushNotifier for DefaultPushNotifier {
    async fn send(&self, title: &str, message: &str, _workspace_id: Option<Uuid>) {
        if cfg!(target_os = "macos") {
            send_macos_notification(title, message).await;
        } else if cfg!(target_os = "linux") && !utils::is_wsl2() {
            send_linux_notification(title, message).await;
        } else if cfg!(target_os = "windows") || (cfg!(target_os = "linux") && utils::is_wsl2()) {
            send_windows_notification(title, message).await;
        }
    }
}

#[derive(Clone, Copy)]
pub enum NotificationEvent {
    Question = 1,
    Approval = 2,
    Success = 4,
    Failure = 8,
}

/// Service for handling cross-platform notifications including sound alerts and push notifications
#[derive(Clone)]
pub struct NotificationService {
    config: Arc<RwLock<Config>>,
    push_notifier: Arc<dyn PushNotifier>,
}

impl std::fmt::Debug for NotificationService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NotificationService")
            .field("config", &self.config)
            .finish()
    }
}

impl NotificationService {
    pub fn new(config: Arc<RwLock<Config>>) -> Self {
        Self {
            config,
            push_notifier: get_global_push_notifier(),
        }
    }

    /// Send both sound and push notifications if enabled.
    /// `workspace_id` is forwarded to the push notifier so Tauri can emit a
    /// navigation event when the notification is clicked.
    pub async fn notify(
        &self,
        event: NotificationEvent,
        title: &str,
        message: &str,
        workspace_id: Option<Uuid>,
    ) {
        let config = self.config.read().await.notifications.clone();

        if config.sound_enabled {
            Self::play_sound_notification(&config.sound_file).await;
        }

        // Sound preparation may await filesystem/platform work. Use the latest
        // choices at push admission and keep the read guard until the notifier
        // accepts the send, so a completed config save governs subsequent sends.
        let latest = self.config.read().await;
        if latest.notifications.push_enabled
            && latest.notifications.push_event_mask & event as u8 != 0
        {
            self.push_notifier.send(title, message, workspace_id).await;
        }
    }

    /// Play a system sound notification across platforms
    async fn play_sound_notification(sound_file: &SoundFile) {
        let file_path = match sound_file.get_path().await {
            Ok(path) => path,
            Err(e) => {
                tracing::error!("Failed to create cached sound file: {}", e);
                return;
            }
        };

        // Use platform-specific sound notification
        // Note: spawn() calls are intentionally not awaited - sound notifications should be fire-and-forget
        if cfg!(target_os = "macos") {
            let _ = tokio::process::Command::new("afplay")
                .arg(&file_path)
                .spawn();
        } else if cfg!(target_os = "linux") && !utils::is_wsl2() {
            // Try different Linux audio players
            if tokio::process::Command::new("paplay")
                .arg(&file_path)
                .spawn()
                .is_ok()
            {
                // Success with paplay
            } else if tokio::process::Command::new("aplay")
                .arg(&file_path)
                .spawn()
                .is_ok()
            {
                // Success with aplay
            } else {
                // Try system bell as fallback
                let _ = tokio::process::Command::new("echo")
                    .arg("-e")
                    .arg("\\a")
                    .spawn();
            }
        } else if cfg!(target_os = "windows") || (cfg!(target_os = "linux") && utils::is_wsl2()) {
            // Convert WSL path to Windows path if in WSL2
            let file_path = if utils::is_wsl2() {
                if let Some(windows_path) = wsl_to_windows_path(&file_path).await {
                    windows_path
                } else {
                    file_path.to_string_lossy().to_string()
                }
            } else {
                file_path.to_string_lossy().to_string()
            };

            let _ = tokio::process::Command::new("powershell.exe")
                .arg("-c")
                .arg(format!(
                    r#"(New-Object Media.SoundPlayer "{file_path}").PlaySync()"#
                ))
                .no_window()
                .spawn();
        }
    }
}

// --- Platform-specific push notification helpers (used by DefaultPushNotifier) ---

/// Send macOS notification using osascript
async fn send_macos_notification(title: &str, message: &str) {
    let script = format!(
        r#"display notification "{message}" with title "{title}" sound name "Glass""#,
        message = message.replace('"', r#"\""#),
        title = title.replace('"', r#"\""#)
    );

    let _ = tokio::process::Command::new("osascript")
        .arg("-e")
        .arg(script)
        .spawn();
}

/// Send Linux notification using notify-rust
async fn send_linux_notification(title: &str, message: &str) {
    use notify_rust::Notification;

    let title = title.to_string();
    let message = message.to_string();

    let _handle = tokio::task::spawn_blocking(move || {
        match Notification::new()
            .summary(&title)
            .body(&message)
            .timeout(10000)
            .show()
        {
            Ok(_) => {}
            Err(e) => {
                let err_str = e.to_string();
                if err_str.contains("ServiceUnknown")
                    || err_str.contains("org.freedesktop.Notifications")
                {
                    tracing::warn!("Linux notification daemon not available: {}", e);
                } else {
                    tracing::warn!("Failed to send Linux notification: {}", e);
                }
            }
        }
    });
    drop(_handle); // Don't await, fire-and-forget
}

/// Send Windows/WSL notification using PowerShell toast script
async fn send_windows_notification(title: &str, message: &str) {
    let script_path = match utils::get_powershell_script().await {
        Ok(path) => path,
        Err(e) => {
            tracing::error!("Failed to get PowerShell script: {}", e);
            return;
        }
    };

    // Convert WSL path to Windows path if in WSL2
    let script_path_str = if utils::is_wsl2() {
        if let Some(windows_path) = wsl_to_windows_path(&script_path).await {
            windows_path
        } else {
            script_path.to_string_lossy().to_string()
        }
    } else {
        script_path.to_string_lossy().to_string()
    };

    let _ = tokio::process::Command::new("powershell.exe")
        .arg("-NoProfile")
        .arg("-ExecutionPolicy")
        .arg("Bypass")
        .arg("-File")
        .arg(script_path_str)
        .arg("-Title")
        .arg(title)
        .arg("-Message")
        .arg(message)
        .no_window()
        .spawn();
}

/// Get WSL root path via PowerShell (cached)
async fn get_wsl_root_path() -> Option<String> {
    if let Some(cached) = WSL_ROOT_PATH_CACHE.get() {
        return cached.clone();
    }

    match tokio::process::Command::new("powershell.exe")
        .arg("-c")
        .arg("(Get-Location).Path -replace '^.*::', ''")
        .current_dir("/")
        .no_window()
        .output()
        .await
    {
        Ok(output) => {
            match String::from_utf8(output.stdout) {
                Ok(pwd_str) => {
                    let pwd = pwd_str.trim();
                    tracing::info!("WSL root path detected: {}", pwd);

                    // Cache the result
                    let _ = WSL_ROOT_PATH_CACHE.set(Some(pwd.to_string()));
                    return Some(pwd.to_string());
                }
                Err(e) => {
                    tracing::error!("Failed to parse PowerShell pwd output as UTF-8: {}", e);
                }
            }
        }
        Err(e) => {
            tracing::error!("Failed to execute PowerShell pwd command: {}", e);
        }
    }

    // Cache the failure result
    let _ = WSL_ROOT_PATH_CACHE.set(None);
    None
}

/// Convert WSL path to Windows UNC path for PowerShell
async fn wsl_to_windows_path(wsl_path: &std::path::Path) -> Option<String> {
    let path_str = wsl_path.to_string_lossy();

    // Relative paths work fine as-is in PowerShell
    if !path_str.starts_with('/') {
        tracing::debug!("Using relative path as-is: {}", path_str);
        return Some(path_str.to_string());
    }

    // Get cached WSL root path from PowerShell
    if let Some(wsl_root) = get_wsl_root_path().await {
        // Simply concatenate WSL root with the absolute path - PowerShell doesn't mind /
        let windows_path = format!("{wsl_root}{path_str}");
        tracing::debug!("WSL path converted: {} -> {}", path_str, windows_path);
        Some(windows_path)
    } else {
        tracing::error!(
            "Failed to determine WSL root path for conversion: {}",
            path_str
        );
        None
    }
}

#[cfg(test)]
mod event_tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;

    struct Recorder(AtomicUsize);
    #[async_trait]
    impl PushNotifier for Recorder {
        async fn send(&self, _: &str, _: &str, _: Option<Uuid>) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }

    #[tokio::test]
    async fn os_event_selection_and_master_switch_apply_immediately() {
        let recorder = Arc::new(Recorder(AtomicUsize::new(0)));
        let config = Arc::new(RwLock::new(Config::default()));
        config.write().await.notifications.sound_enabled = false;
        let service = NotificationService {
            config: config.clone(),
            push_notifier: recorder.clone(),
        };
        for event in [
            NotificationEvent::Question,
            NotificationEvent::Approval,
            NotificationEvent::Success,
            NotificationEvent::Failure,
        ] {
            config.write().await.notifications.push_event_mask = 15 ^ event as u8;
            let before = recorder.0.load(Ordering::SeqCst);
            service.notify(event, "fixture", "fixture", None).await;
            assert_eq!(recorder.0.load(Ordering::SeqCst), before);
            config.write().await.notifications.push_event_mask = event as u8;
            service.notify(event, "fixture", "fixture", None).await;
            assert_eq!(recorder.0.load(Ordering::SeqCst), before + 1);
        }
        config.write().await.notifications.push_enabled = false;
        service
            .notify(NotificationEvent::Failure, "fixture", "fixture", None)
            .await;
        assert_eq!(recorder.0.load(Ordering::SeqCst), 4);
        config.write().await.notifications.push_enabled = true;
        config.write().await.notifications.push_event_mask = 0;
        service
            .notify(NotificationEvent::Failure, "fixture", "fixture", None)
            .await;
        assert_eq!(recorder.0.load(Ordering::SeqCst), 4);
    }

    struct PausedNotifier {
        entered: tokio::sync::Notify,
        release: tokio::sync::Notify,
    }
    #[async_trait]
    impl PushNotifier for PausedNotifier {
        async fn send(&self, _: &str, _: &str, _: Option<Uuid>) {
            self.entered.notify_one();
            self.release.notified().await;
        }
    }

    #[tokio::test]
    async fn os_config_save_waits_for_notifier_admission_to_settle() {
        let notifier = Arc::new(PausedNotifier {
            entered: tokio::sync::Notify::new(),
            release: tokio::sync::Notify::new(),
        });
        let config = Arc::new(RwLock::new(Config::default()));
        config.write().await.notifications.sound_enabled = false;
        let service = NotificationService {
            config: config.clone(),
            push_notifier: notifier.clone(),
        };
        let sending = tokio::spawn(async move {
            service
                .notify(NotificationEvent::Success, "fixture", "fixture", None)
                .await;
        });
        notifier.entered.notified().await;
        assert!(config.try_write().is_err());
        notifier.release.notify_one();
        sending.await.unwrap();
        config.write().await.notifications.push_event_mask = 0;
        assert_eq!(config.read().await.notifications.push_event_mask, 0);
    }

    #[tokio::test]
    async fn os_choices_survive_config_file_reload() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        let mut config = Config::default();
        config.notifications.push_event_mask = 0;
        crate::services::config::save_config_to_file(&config, &path)
            .await
            .unwrap();
        let restored = crate::services::config::load_config_from_file(&path).await;
        assert_eq!(restored.notifications.push_event_mask, 0);
        assert!(restored.notifications.push_enabled);
    }

    #[test]
    fn old_os_config_defaults_and_selected_events_round_trip() {
        let mut value = serde_json::to_value(Config::default()).unwrap();
        value["notifications"]
            .as_object_mut()
            .unwrap()
            .remove("push_event_mask");
        let mut config: Config = serde_json::from_value(value).unwrap();
        assert_eq!(config.notifications.push_event_mask, 15);
        for mask in [0, 1, 2, 4, 8, 15] {
            config.notifications.push_event_mask = mask;
            let restored: Config =
                serde_json::from_slice(&serde_json::to_vec(&config).unwrap()).unwrap();
            assert_eq!(restored.notifications.push_event_mask, mask);
        }
    }
}
