use std::{env, path::PathBuf};

use serde::{Deserialize, Serialize};
use tokio::fs;

#[derive(Debug, Serialize, Deserialize)]
pub struct PortInfo {
    pub main_port: u16,
}

pub async fn write_port_file(main_port: u16) -> std::io::Result<PathBuf> {
    let dir = crate::instance::instance_dir()
        .map(|root| root.join("ports"))
        .unwrap_or_else(|| env::temp_dir().join("vibe-kanban"));
    let path = dir.join("vibe-kanban.port");
    let port_info = PortInfo { main_port };
    let content = serde_json::to_string(&port_info)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    tracing::debug!("Writing ports {:?} to {:?}", port_info, path);
    fs::create_dir_all(&dir).await?;
    fs::write(&path, content).await?;
    Ok(path)
}

pub async fn read_port_file(app_name: &str) -> std::io::Result<u16> {
    read_port_info(app_name).await.map(|info| info.main_port)
}

pub async fn read_port_info(app_name: &str) -> std::io::Result<PortInfo> {
    let dir = crate::instance::instance_dir()
        .map(|root| root.join("ports"))
        .unwrap_or_else(|| env::temp_dir().join(app_name));
    let path = dir.join(format!("{app_name}.port"));
    tracing::debug!("Reading port from {:?}", path);

    let content = fs::read_to_string(&path).await?;

    if let Ok(port_info) = serde_json::from_str::<PortInfo>(&content) {
        return Ok(port_info);
    }

    let port: u16 = content
        .trim()
        .parse()
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

    Ok(PortInfo { main_port: port })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn retired_proxy_port_does_not_break_port_discovery() {
        let old: PortInfo =
            serde_json::from_str(r#"{"main_port":4021,"preview_proxy_port":4022}"#).unwrap();
        assert_eq!(old.main_port, 4021);
        assert_eq!(
            serde_json::to_value(old).unwrap(),
            serde_json::json!({"main_port":4021})
        );
    }
}
