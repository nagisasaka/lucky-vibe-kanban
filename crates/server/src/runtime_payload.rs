//! Side-effect-free startup probe for copied developer bundles.
use std::{borrow::Cow, collections::BTreeMap};

use anyhow::{Result, ensure};
use rust_embed::RustEmbed;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

fn collect<T: RustEmbed>(prefix: &str, assets: &mut BTreeMap<String, String>) -> Result<()> {
    for name in T::iter() {
        let file = T::get(&name).ok_or_else(|| anyhow::anyhow!("Missing payload"))?;
        ensure!(
            matches!(file.data, Cow::Borrowed(_)),
            "Payload reads source files"
        );
        assets.insert(
            format!("{prefix}/{name}"),
            format!("{:x}", Sha256::digest(&file.data)),
        );
    }
    Ok(())
}

pub fn inspect() -> Result<Value> {
    let mut assets = BTreeMap::new();
    collect::<crate::routes::frontend::Assets>("frontend", &mut assets)?;
    collect::<utils::assets::SoundAssets>("sounds", &mut assets)?;
    collect::<utils::assets::ScriptAssets>("scripts", &mut assets)?;
    ensure!(
        assets.contains_key("frontend/index.html"),
        "Missing frontend entry"
    );
    Ok(json!({"embedded": true, "assets": assets}))
}
