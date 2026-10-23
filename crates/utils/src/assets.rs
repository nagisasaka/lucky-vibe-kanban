use directories::ProjectDirs;
use rust_embed::RustEmbed;

const PROJECT_ROOT: &str = env!("CARGO_MANIFEST_DIR");

pub fn asset_dir() -> std::path::PathBuf {
    let path = asset_path(crate::instance::instance_dir());

    if !path.exists() {
        std::fs::create_dir_all(&path).expect("Failed to create asset directory");
    }
    path
}

/// Resolve the configured data destination without creating directories.
pub fn asset_dir_path() -> std::io::Result<std::path::PathBuf> {
    Ok(asset_path(crate::instance::instance_dir_read_only()?))
}

fn asset_path(root: Option<std::path::PathBuf>) -> std::path::PathBuf {
    if let Some(root) = root {
        root.join("data")
    } else if cfg!(debug_assertions) {
        std::path::PathBuf::from(PROJECT_ROOT).join("../../dev_assets")
    } else {
        prod_asset_dir_path()
    }
}

pub fn prod_asset_dir_path() -> std::path::PathBuf {
    ProjectDirs::from("ai", "bloop", "vibe-kanban")
        .expect("OS didn't give us a home directory")
        .data_dir()
        .to_path_buf()
}

pub fn config_path() -> std::path::PathBuf {
    asset_dir().join("config.json")
}

pub fn profiles_path() -> std::path::PathBuf {
    asset_dir().join("profiles.json")
}

#[derive(RustEmbed)]
#[folder = "../../assets/sounds"]
pub struct SoundAssets;

#[derive(RustEmbed)]
#[folder = "../../assets/scripts"]
pub struct ScriptAssets;
