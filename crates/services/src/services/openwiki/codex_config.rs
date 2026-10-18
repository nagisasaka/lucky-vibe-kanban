//! Codex's TOML editor keeps trailing comments at EOF when adding project trust
//! tables. That can move OpenWiki's closing marker past unrelated configuration.
//! Repair only that displacement, never a changed MCP invocation or user value.
use std::{fs, io::Write, path::Path};

use anyhow::{Context, ensure};

const START: &str = "# OPENWIKI:MCP:START\n";
const END: &str = "# OPENWIKI:MCP:END\n";
const PREFIX: &str = "# OPENWIKI:MCP:START\n[mcp_servers.openwiki]\ncommand = \"openwiki\"\nargs = [\"mcp\", \"--host\", \"codex\"]\n";

fn repaired_content(current: &str) -> anyhow::Result<Option<String>> {
    if current.matches(START).count() != 1 || current.matches(END.trim_end()).count() != 1 {
        return Ok(None); // The installer reports ambiguous/missing markers.
    }
    let Some(start) = current.find(PREFIX) else {
        return Ok(None);
    };
    let body_start = start + PREFIX.len();
    let Some(end) = current[body_start..]
        .find(END.trim_end())
        .map(|offset| body_start + offset)
    else {
        return Ok(None);
    };
    let displaced = &current[body_start..end];
    if displaced.is_empty() || !displaced.trim_start().starts_with('[') {
        return Ok(None);
    }
    let parsed: toml::Value =
        toml::from_str(current).context("Invalid Codex TOML; no repair made")?;
    let expected: toml::Value = toml::from_str(PREFIX)?;
    if parsed.get("mcp_servers").and_then(|v| v.get("openwiki"))
        != expected.get("mcp_servers").and_then(|v| v.get("openwiki"))
    {
        return Ok(None); // Custom OpenWiki settings remain an explicit conflict.
    }
    let suffix = &current[end + END.trim_end().len()..];
    let suffix = suffix.strip_prefix('\n').unwrap_or(suffix);
    let repaired = format!("{}{END}{displaced}{suffix}", &current[..body_start]);
    ensure!(
        toml::from_str::<toml::Value>(&repaired)? == parsed,
        "Codex repair would change settings"
    );
    Ok(Some(repaired))
}

pub(super) fn repair_displaced_marker(path: &Path) -> anyhow::Result<bool> {
    let _guard = executors::config_write::NATIVE_SETTINGS_WRITE_LOCK
        .lock()
        .map_err(|_| anyhow::anyhow!("Native configuration lock unavailable"))?;
    utils::repository_memory::reject_symlinks(path)?;
    let metadata = match fs::metadata(path) {
        Ok(value) => value,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error.into()),
    };
    ensure!(
        metadata.is_file() && metadata.len() <= 2 * 1024 * 1024,
        "Unexpected Codex configuration file; no repair made"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        ensure!(
            metadata.nlink() == 1,
            "Hard-linked Codex configuration; no repair made"
        );
    }
    let current = fs::read_to_string(path)?;
    let Some(repaired) = repaired_content(&current)? else {
        return Ok(false);
    };
    let mut temporary =
        tempfile::NamedTempFile::new_in(path.parent().context("Missing config parent")?)?;
    temporary
        .as_file()
        .set_permissions(metadata.permissions())?;
    temporary.write_all(repaired.as_bytes())?;
    temporary.as_file().sync_all()?;
    utils::repository_memory::reject_symlinks(path)?;
    ensure!(
        fs::read_to_string(path)? == current,
        "Codex configuration changed during repair; retry"
    );
    temporary.persist(path).map_err(|error| error.error)?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toml_editor_project_insertion_reproduces_marker_displacement() {
        let original = format!("{PREFIX}{END}");
        let mut document = original.parse::<toml_edit::DocumentMut>().unwrap();
        document["projects"] = toml_edit::Item::Table(toml_edit::Table::new());
        document["projects"]["/work/repo"] = toml_edit::Item::Table(toml_edit::Table::new());
        document["projects"]["/work/repo"]["trust_level"] = toml_edit::value("trusted");
        let edited = document.to_string();
        assert!(edited.ends_with(END));
        assert!(!edited.contains(&original));
        let repaired = repaired_content(&edited).unwrap().unwrap();
        assert!(repaired.starts_with(&original));
        assert_eq!(
            toml::from_str::<toml::Value>(&edited).unwrap(),
            toml::from_str::<toml::Value>(&repaired).unwrap()
        );
    }

    #[test]
    fn repairs_displaced_marker_preserving_trust_and_is_idempotent() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("config.toml");
        let trust = "\n[projects.\"/work/a\"]\ntrust_level = \"trusted\"\n\n[projects.\"/work/b\"]\ntrust_level = \"untrusted\"\n";
        let before = format!("model = \"custom\"\n\n{PREFIX}{trust}{END}");
        fs::write(&path, &before).unwrap();
        assert!(repair_displaced_marker(&path).unwrap());
        let after = fs::read_to_string(&path).unwrap();
        assert!(after.contains(&format!("{PREFIX}{END}{trust}")));
        assert_eq!(
            toml::from_str::<toml::Value>(&before).unwrap(),
            toml::from_str::<toml::Value>(&after).unwrap()
        );
        assert!(!repair_displaced_marker(&path).unwrap());
    }

    #[test]
    fn refuses_custom_mcp_values_ambiguous_markers_and_invalid_toml() {
        let trust = "\n[projects.test]\ntrust_level = \"trusted\"\n";
        for text in [
            format!("{PREFIX}{END}"),
            format!("{PREFIX}timeout = 10\n{trust}{END}"),
            format!("{PREFIX}[mcp_servers.openwiki.env]\nCUSTOM = \"keep\"\n{END}"),
            format!("{PREFIX}{trust}{END}{END}"),
            format!("{PREFIX}{trust}{END}")
                .replace("command = \"openwiki\"", "command = \"custom\""),
        ] {
            assert!(repaired_content(&text).unwrap().is_none());
        }
        assert!(repaired_content(&format!("{PREFIX}[invalid\n{END}")).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn refuses_symlink_configuration() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("config.toml");
        let target = temp.path().join("target");
        fs::write(&target, PREFIX).unwrap();
        std::os::unix::fs::symlink(&target, &path).unwrap();
        assert!(repair_displaced_marker(&path).is_err());
        assert_eq!(fs::read_to_string(target).unwrap(), PREFIX);
    }
}
