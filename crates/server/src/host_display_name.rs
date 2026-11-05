//! Instance-scoped cosmetic identity. Never changes OS hostname or machine IDs.
use std::{
    io,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

#[derive(Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct HostDisplayName {
    pub host_display_name: Option<String>,
}

pub(crate) fn path() -> io::Result<PathBuf> {
    Ok(utils::assets::asset_dir_path()?.join("host-display-name.json"))
}

pub(crate) async fn load(path: &Path) -> io::Result<HostDisplayName> {
    match tokio::fs::read(path).await {
        Ok(bytes) => serde_json::from_slice(&bytes).map_err(io::Error::other),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(HostDisplayName::default()),
        Err(e) => Err(e),
    }
}

pub(crate) fn normalize(value: Option<String>) -> Result<HostDisplayName, &'static str> {
    let value = value
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    if value.as_ref().is_some_and(|s| {
        s.chars().count() > 80
            || s.chars().any(|c| {
                c.is_control() || matches!(c, '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}')
            })
    }) {
        return Err("Display name must be at most 80 characters without control characters");
    }
    Ok(HostDisplayName {
        host_display_name: value,
    })
}

pub(crate) async fn save(path: &Path, value: &HostDisplayName) -> io::Result<()> {
    // Unique sibling + rename gives readers either complete old or complete new JSON.
    let temporary = path.with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
    let result = async {
        tokio::fs::write(
            &temporary,
            serde_json::to_vec(value).map_err(io::Error::other)?,
        )
        .await?;
        tokio::fs::rename(&temporary, path).await
    }
    .await;
    if result.is_err() {
        let _ = tokio::fs::remove_file(&temporary).await;
    }
    result
}

pub(crate) async fn hostname() -> Option<String> {
    tokio::fs::read_to_string("/proc/sys/kernel/hostname")
        .await
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

pub(crate) async fn connection_metadata(connection: serde_json::Value) -> serde_json::Value {
    let settings = match path() {
        Ok(path) => load(&path).await.ok(),
        Err(_) => None,
    };
    append_connection_fields(connection, settings, hostname().await)
}
fn append_connection_fields(
    mut connection: serde_json::Value,
    settings: Option<HostDisplayName>,
    hostname: Option<String>,
) -> serde_json::Value {
    if let Some(object) = connection.as_object_mut() {
        object.insert(
            "host_display_name".into(),
            serde_json::json!(settings.as_ref().and_then(|s| s.host_display_name.as_ref())),
        );
        object.insert("hostname".into(), serde_json::json!(hostname));
        object.insert(
            "host_display_name_available".into(),
            serde_json::json!(settings.is_some()),
        );
    }
    connection
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn host_display_name_persists_and_isolates_instances() {
        let a = tempfile::tempdir().unwrap();
        let b = tempfile::tempdir().unwrap();
        let a = a.path().join("host-display-name.json");
        let b = b.path().join("host-display-name.json");
        assert!(load(&a).await.unwrap().host_display_name.is_none());
        save(&a, &normalize(Some("  Rabbit 一号  ".into())).unwrap())
            .await
            .unwrap();
        assert_eq!(
            load(&a).await.unwrap().host_display_name.as_deref(),
            Some("Rabbit 一号")
        );
        assert!(load(&b).await.unwrap().host_display_name.is_none());
        save(&a, &normalize(Some("  ".into())).unwrap())
            .await
            .unwrap();
        assert!(load(&a).await.unwrap().host_display_name.is_none());
        tokio::fs::write(&a, b"invalid").await.unwrap();
        assert!(load(&a).await.is_err());
        assert!(
            save(&b.join("missing"), &HostDisplayName::default())
                .await
                .is_err()
        );
    }
    #[test]
    fn host_display_name_connection_preserves_legacy_identity() {
        let legacy = serde_json::json!({"server":"LVK","permissions":["read"],"automatic_notifications":false,"monitoring":false});
        let value = append_connection_fields(
            legacy.clone(),
            Some(normalize(Some("Rabbit".into())).unwrap()),
            Some("real-host".into()),
        );
        for (key, expected) in legacy.as_object().unwrap() {
            assert_eq!(&value[key], expected);
        }
        assert_eq!(value["host_display_name"], "Rabbit");
        assert_eq!(value["hostname"], "real-host");
        let unavailable = append_connection_fields(legacy, None, Some("real-host".into()));
        assert!(unavailable["host_display_name"].is_null());
        assert_eq!(unavailable["host_display_name_available"], false);
    }
    #[test]
    fn host_display_name_validation() {
        assert!(normalize(Some("x".repeat(80))).is_ok());
        for text in ["x".repeat(81), "bad\nname".into(), "bad\u{202e}name".into()] {
            assert!(normalize(Some(text)).is_err());
        }
    }
}
