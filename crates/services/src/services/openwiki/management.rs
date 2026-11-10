//! OpenWiki owns its generated maintenance region, except explicit configuration.
use std::path::Path;

use anyhow::ensure;
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

/// Enumerate the frozen maintenance region without guessing ownership from content.
/// Markerless indexes are included; configuration survives rebuild and deletion.
pub fn generated_paths(root: &Path, source: &str) -> anyhow::Result<Vec<String>> {
    let snapshot = GitService::new().snapshot(root, source)?;
    let mut paths = Vec::new();
    for entry in snapshot.entries()? {
        if entry.path.starts_with("openwiki/") && !configuration_path(&entry.path) {
            ensure!(
                matches!(entry.mode, 0o100644 | 0o100755),
                "OpenWiki maintenance path is not a regular file: {}",
                entry.path
            );
            paths.push(entry.path);
        }
    }
    Ok(paths)
}

pub fn remove_generated(root: &Path, source: &str) -> anyhow::Result<Vec<String>> {
    let paths = generated_paths(root, source)?;
    // Check the complete set before mutating any files.
    for path in &paths {
        reject_symlinks(&root.join(path))?;
    }
    for path in &paths {
        let path = root.join(path);
        match std::fs::remove_file(path) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
    }
    // Remove only empty parents of maintenance files. Otherwise upstream
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
        "\nThis is a full reconstruction using the saved instructions and ignore rules. The immutable previous Wiki is in this repository's Git commit {source}. Read it via git show {source}:openwiki/<path> (enumerate with git ls-tree -r {source} -- openwiki); never treat the changing working tree as the old version. Respect the saved .openwikiignore when consulting historical pages and report excluded reference paths as coverage limits. Reconsider structure and explanation order, not only wording. Compare important operational knowledge and design constraints from that fixed version against current source/tests/configuration and maintained README/docs. Retain evidenced knowledge, label uncertainty, never invent rationale or blindly inherit old claims. Verify main entry links from README/AGENTS and stable Wiki entry points. Preserve INSTRUCTIONS.md, saved configuration and stable operational entry paths. Report remaining knowledge gaps; complete recovery of all old knowledge is not guaranteed.\n"
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
