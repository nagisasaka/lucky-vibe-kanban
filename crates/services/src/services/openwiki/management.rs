//! Narrow repository maintenance operations. Human documents and configuration
//! are never inferred to be generated merely because they live under openwiki/.
use std::path::Path;

use anyhow::{Context, ensure};
use git::GitService;
use utils::repository_memory::reject_symlinks;

pub const INSTRUCTIONS: &str = "openwiki/INSTRUCTIONS.md";
pub const IGNORE: &str = ".openwikiignore";
pub const DEFAULT_INSTRUCTIONS: &str =
    include_str!("../../../../../assets/openwiki-instructions.md");
pub const DEFAULT_IGNORE: &str = include_str!("../../../../../assets/openwiki-ignore");

pub fn configuration_path(path: &str) -> bool {
    matches!(path, INSTRUCTIONS | IGNORE)
}

/// Enumerate the frozen generated set, including legacy generated pages. Never
/// remove an entire directory: unknown/human-owned files survive both operations.
pub fn generated_paths(root: &Path, source: &str) -> anyhow::Result<Vec<String>> {
    let snapshot = GitService::new().snapshot(root, source)?;
    let mut pages = std::collections::BTreeSet::new();
    let entries = snapshot.entries()?;
    if let Some(entry) = entries
        .iter()
        .find(|entry| entry.path == "openwiki/.page-manifest.json")
    {
        let manifest: serde_json::Value = serde_json::from_slice(
            &snapshot
                .read_blob(entry, 8 * 1024 * 1024)?
                .context("Page manifest too large")?,
        )?;
        ensure!(
            manifest["schemaVersion"] == 1,
            "Unsupported OpenWiki page manifest"
        );
        for path in manifest["pages"]
            .as_object()
            .context("Invalid page manifest")?
            .keys()
        {
            let path = path.strip_prefix('/').unwrap_or(path);
            ensure!(safe_generated_page(path), "Invalid generated page path");
            pages.insert(path.to_owned());
        }
    }
    let mut result = Vec::new();
    for entry in entries {
        let path = &entry.path;
        if !path.starts_with("openwiki/") || path == INSTRUCTIONS {
            continue;
        }
        let metadata = matches!(
            path.as_str(),
            "openwiki/.page-manifest.json" | "openwiki/.last-update.json" | "openwiki/.run.json"
        ) || path.starts_with("openwiki/.claims/");
        let mut generated = pages.contains(path)
            || matches!(
                path.as_str(),
                "openwiki/index.md" | "openwiki/quickstart.md"
            );
        if !generated
            && safe_generated_page(path)
            && let Some(bytes) = snapshot.read_blob(&entry, 2 * 1024 * 1024)?
            && let Ok(text) = std::str::from_utf8(&bytes)
        {
            let mut lines = text.lines();
            if lines.next() == Some("---") {
                let header = lines
                    .take_while(|line| *line != "---")
                    .collect::<Vec<_>>()
                    .join("\n");
                if let Ok(header) = serde_yaml::from_str::<serde_yaml::Value>(&header) {
                    generated = (path.ends_with("/index.md")
                        && header.get("okf_version").is_some())
                        || header
                            .get("generated")
                            .and_then(|v| v.get("by"))
                            .and_then(|v| v.as_str())
                            .is_some();
                }
            }
        }
        if metadata || generated {
            result.push(path.clone());
        }
    }
    Ok(result)
}

fn safe_generated_page(path: &str) -> bool {
    path.starts_with("openwiki/")
        && path.ends_with(".md")
        && path != INSTRUCTIONS
        && !path.contains(['\\', ':', '\0'])
        && !path.split('/').any(|part| matches!(part, "" | "." | ".."))
}

pub fn remove_generated(root: &Path, source: &str) -> anyhow::Result<Vec<String>> {
    let paths = generated_paths(root, source)?;
    for path in &paths {
        let path = root.join(path);
        reject_symlinks(&path)?;
        match std::fs::remove_file(path) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
    }
    // Remove only empty parents of known generated files. Otherwise upstream
    // index synchronisation would recreate obsolete, empty taxonomy directories.
    let wiki = root.join("openwiki");
    for path in &paths {
        let absolute = root.join(path);
        let mut parent = absolute.parent();
        while let Some(directory) = parent.filter(|directory| *directory != wiki) {
            reject_symlinks(directory)?;
            match std::fs::remove_dir(directory) {
                Ok(()) => {}
                Err(error)
                    if matches!(
                        error.kind(),
                        std::io::ErrorKind::NotFound | std::io::ErrorKind::DirectoryNotEmpty
                    ) => {}
                Err(error) => return Err(error.into()),
            }
            parent = directory.parent();
        }
    }
    Ok(paths)
}

/// Preserve every byte outside the managed markers, including trailing spaces.
pub fn remove_managed_block(text: &str) -> anyhow::Result<String> {
    const START: &str = "<!-- OPENWIKI:START -->";
    const END: &str = "<!-- OPENWIKI:END -->";
    match (text.find(START), text.find(END)) {
        (None, None) => Ok(text.into()),
        (Some(start), Some(end))
            if end > start
                && text.matches(START).count() == 1
                && text.matches(END).count() == 1 =>
        {
            Ok(format!("{}{}", &text[..start], &text[end + END.len()..]))
        }
        _ => anyhow::bail!("Malformed OpenWiki managed instruction markers"),
    }
}

pub fn reference_prompt(source: &str) -> String {
    format!(
        "\nThis is a full reconstruction using the saved instructions and ignore rules. The immutable previous Wiki is in this repository's Git commit {source}. Read it via git show {source}:openwiki/<path> (enumerate with git ls-tree -r {source} -- openwiki); never treat the changing working tree as the old version. Respect the saved .openwikiignore when consulting historical pages and report excluded reference paths as coverage limits. Reconsider structure and explanation order, not only wording. Compare important operational knowledge and design constraints from that fixed version against current source/tests/configuration and maintained README/docs. Retain evidenced knowledge, label uncertainty, never invent rationale or blindly inherit old claims. Verify main entry links from README/AGENTS and stable Wiki entry points. Preserve human-managed documents and configuration. Report remaining knowledge gaps; complete recovery of all old knowledge is not guaranteed.\n"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn managed_block_preserves_outside_bytes_and_rejects_partial_markers() {
        let input =
            "prefix  \r\n<!-- OPENWIKI:START -->\r\nold\r\n<!-- OPENWIKI:END -->\r\nsuffix  \r\n";
        assert_eq!(
            remove_managed_block(input).unwrap(),
            "prefix  \r\n\r\nsuffix  \r\n"
        );
        assert_eq!(remove_managed_block("human  \n").unwrap(), "human  \n");
        assert!(remove_managed_block("<!-- OPENWIKI:START -->").is_err());
    }
}
