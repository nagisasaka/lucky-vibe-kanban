//! Named URI data only. Browsers/OS handlers own launching and authentication.
use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use thiserror::Error;
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, Deserialize, TS, Error)]
#[error("{message}")]
pub struct EditorOpenError {
    pub message: String,
}

fn invalid(message: impl Into<String>) -> EditorOpenError {
    EditorOpenError {
        message: message.into(),
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, TS)]
pub struct EditorConfig {
    /// None is uninitialized; Some([]) is an intentionally empty list.
    /// Unknown legacy editor/command/SSH fields are ignored, never executed.
    #[serde(default)]
    pub entries: Option<Vec<EditorEntry>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct EditorEntry {
    pub name: String,
    pub uri_template: String,
}

#[derive(Debug, Clone, Serialize, TS)]
pub struct EditorLink {
    pub name: String,
    pub url: Option<String>,
    pub unavailable_reason: Option<String>,
}

impl EditorConfig {
    pub fn validate(&self) -> Result<(), EditorOpenError> {
        let mut names = HashSet::new();
        for entry in self.entries.iter().flatten() {
            if entry.name.trim().is_empty() || entry.name.chars().count() > 80 {
                return Err(invalid("Menu names must contain 1–80 characters"));
            }
            if !names.insert(entry.name.trim()) {
                return Err(invalid("Menu names must be unique"));
            }
            expand_template(&entry.uri_template, "/workspace", Some("/workspace/file"))?;
        }
        Ok(())
    }

    pub fn links(&self, directory: &str, file: Option<&str>) -> Vec<EditorLink> {
        self.entries
            .iter()
            .flatten()
            .map(|entry| {
                let result = expand_template(&entry.uri_template, directory, file);
                EditorLink {
                    name: entry.name.clone(),
                    url: result.as_ref().ok().cloned(),
                    unavailable_reason: result.err().map(|error| error.to_string()),
                }
            })
            .collect()
    }
}

fn encode(value: &str, path: bool) -> String {
    let mut result = String::new();
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || b"-._~".contains(&byte) || (path && byte == b'/') {
            result.push(byte as char);
        } else {
            use std::fmt::Write;
            write!(result, "%{byte:02X}").expect("writing to string");
        }
    }
    result
}

/// Expand only path and query *values*. Fixed escapes are preserved verbatim.
/// This is a URI syntax check, deliberately not a scheme security policy.
pub fn expand_template(
    template: &str,
    directory: &str,
    file: Option<&str>,
) -> Result<String, EditorOpenError> {
    if template.is_empty() || template.len() > 8192 || template.chars().any(char::is_control) {
        return Err(invalid(
            "URI templates must contain 1–8192 bytes and no control characters",
        ));
    }
    let colon = template
        .find(':')
        .ok_or_else(|| invalid("An absolute URI scheme is required"))?;
    let scheme = &template[..colon];
    if !scheme.starts_with(|c: char| c.is_ascii_alphabetic())
        || !scheme
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"+-.".contains(&b))
    {
        return Err(invalid("Invalid URI scheme"));
    }
    let fragment = template.find('#').unwrap_or(template.len());
    let query = template[..fragment].find('?').unwrap_or(fragment);
    let after_scheme = colon + 1;
    let path_start = if template[after_scheme..].starts_with("//") {
        let start = after_scheme + 2;
        start + template[start..query].find('/').unwrap_or(query - start)
    } else {
        after_scheme
    };
    let mut output = String::new();
    let mut cursor = 0;
    while cursor < template.len() {
        let rest = &template[cursor..];
        if rest.starts_with('<') {
            let end = rest
                .find('>')
                .ok_or_else(|| invalid("Unclosed placeholder"))?;
            let value = match &rest[..=end] {
                "<lvk_workspace_dir>" => directory,
                "<lvk_file_path>" => file.ok_or_else(|| {
                    invalid("This template requires a file; select a file to use it")
                })?,
                other => return Err(invalid(format!("Undefined placeholder: {other}"))),
            };
            let in_path = cursor >= path_start && cursor < query;
            let in_query_value = cursor > query && cursor < fragment && {
                let part = template[query + 1..cursor].rsplit('&').next().unwrap_or("");
                part.contains('=')
            };
            if !in_path && !in_query_value {
                return Err(invalid(
                    "Placeholders are supported only in URI paths and query values",
                ));
            }
            output.push_str(&encode(value, in_path));
            cursor += end + 1;
        } else {
            let ch = rest.chars().next().expect("nonempty");
            if ch == '>' {
                return Err(invalid("Unexpected placeholder delimiter"));
            }
            if ch == '%'
                && (rest.len() < 3 || !rest.as_bytes()[1..3].iter().all(u8::is_ascii_hexdigit))
            {
                return Err(invalid(
                    "Invalid percent escape; use %25 for a literal percent sign",
                ));
            }
            // Encode fixed characters outside RFC 3986's unreserved/reserved
            // sets, without re-encoding existing %XX sequences.
            if !ch.is_ascii_alphanumeric() && !"-._~:/?#[]@!$&'()*+,;=%".contains(ch) {
                output.push_str(&encode(&ch.to_string(), false));
            } else {
                output.push(ch);
            }
            cursor += ch.len_utf8();
        }
    }
    let parsed = url::Url::parse(&output).map_err(|e| invalid(format!("Invalid URI: {e}")))?;
    if !template[after_scheme..].starts_with("//")
        && (output[after_scheme..].starts_with("//") || parsed.has_host())
    {
        return Err(invalid(
            "Path expansion would create a URI authority; remove the extra slash before the placeholder",
        ));
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uri_path_and_query_encoding() {
        let path = "/work/日本語 a%?#&/file";
        let encoded = "/work/%E6%97%A5%E6%9C%AC%E8%AA%9E%20a%25%3F%23%26/file";
        for prefix in [
            "vscode://vscode-remote/ssh-remote+host",
            "vscode://vscode-remote/wsl+Ubuntu",
            "https://ide.example/open",
            "http://ide.example/open",
        ] {
            assert_eq!(
                expand_template(&format!("{prefix}<lvk_workspace_dir>"), path, None).unwrap(),
                format!("{prefix}{encoded}")
            );
        }
        assert_eq!(
            expand_template(
                "jetbrains://idea/navigate/reference?projectHint=<lvk_workspace_dir>&fixed=a%20b",
                path,
                None
            )
            .unwrap(),
            format!(
                "jetbrains://idea/navigate/reference?projectHint={}&fixed=a%20b",
                encoded.replace('/', "%2F")
            )
        );
        assert_eq!(
            expand_template(
                "custom:/open<lvk_file_path>?file=<lvk_file_path>",
                "/work",
                Some("/a b%")
            )
            .unwrap(),
            "custom:/open/a%20b%25?file=%2Fa%20b%25"
        );
        assert_eq!(
            expand_template("https://ide/a%20b/日本 語?q=x%25", path, None).unwrap(),
            "https://ide/a%20b/%E6%97%A5%E6%9C%AC%20%E8%AA%9E?q=x%25"
        );
    }

    #[test]
    fn rejects_bad_syntax_and_missing_context_without_scheme_policy() {
        for uri in [
            "<scheme>:/x",
            "x://<lvk_workspace_dir>/",
            "x:/#<lvk_workspace_dir>",
            "x:/?<lvk_workspace_dir>=x",
            "x:/<unknown>",
            "x:/<lvk_workspace_dir>",
            "https:<lvk_workspace_dir>",
            "x:/<lvk_file_path>",
            "x:/<broken",
            "x:/>",
            "x:/%z0",
            "x:/%",
            "no scheme",
            "x:/\n",
        ] {
            assert!(expand_template(uri, "/work", None).is_err(), "{uri}");
        }
        for uri in ["javascript:alert(1)", "file:///tmp", "arbitrary:fixed"] {
            assert!(expand_template(uri, "/work", None).is_ok());
        }
    }

    #[test]
    fn legacy_data_is_ignored_and_empty_is_preserved() {
        let old: EditorConfig = serde_json::from_str(r#"{"editor_type":"UNKNOWN_IDE","custom_command":"do not execute","remote_ssh_host":"old"}"#).unwrap();
        assert!(old.entries.is_none());
        let empty: EditorConfig = serde_json::from_str(r#"{"entries":[]}"#).unwrap();
        assert!(empty.entries.unwrap().is_empty());
    }

    #[test]
    fn file_context_keeps_directory_links_and_folder_context_explains_file_only() {
        let config = EditorConfig {
            entries: Some(vec![
                EditorEntry {
                    name: "Folder".into(),
                    uri_template: "custom:/open<lvk_workspace_dir>".into(),
                },
                EditorEntry {
                    name: "File".into(),
                    uri_template: "custom:/open<lvk_file_path>".into(),
                },
            ]),
        };
        let folder = config.links("/repo", None);
        assert_eq!(folder[0].url.as_deref(), Some("custom:/open/repo"));
        assert!(folder[1].url.is_none());
        assert!(
            folder[1]
                .unavailable_reason
                .as_deref()
                .unwrap()
                .contains("requires a file")
        );
        let file = config.links("/repo", Some("/repo/file"));
        assert_eq!(file[0].url, folder[0].url);
        assert_eq!(file[1].url.as_deref(), Some("custom:/open/repo/file"));
    }

    #[test]
    fn legacy_v2_through_v5_editor_fields_do_not_break_schema_loading() {
        // Their complete historical migration chain is outside this change.
        // Check that arbitrary old editor values cannot break schema loading.
        use crate::services::config::versions::{v2, v3, v4, v5};
        macro_rules! check {
            ($config:ty) => {{
                let mut value = serde_json::to_value(<$config>::default()).unwrap();
                value["workspace_dir"] = serde_json::json!("/retained");
                value["editor"] = serde_json::json!({"editor_type":"UNKNOWN", "custom_command":"never run"});
                let config: $config = serde_json::from_value(value).unwrap();
                assert_eq!(config.workspace_dir.as_deref(), Some("/retained"));
                assert!(config.editor.entries.is_none());
            }};
        }
        check!(v2::Config);
        check!(v3::Config);
        check!(v4::Config);
        check!(v5::Config);
    }

    #[test]
    fn names_and_limits() {
        let too_long = "x".repeat(81);
        for names in [
            vec![""],
            vec![" "],
            vec!["A", " A "],
            vec![too_long.as_str()],
        ] {
            let config = EditorConfig {
                entries: Some(
                    names
                        .into_iter()
                        .map(|name| EditorEntry {
                            name: name.into(),
                            uri_template: "x:/".into(),
                        })
                        .collect(),
                ),
            };
            assert!(config.validate().is_err());
        }
        assert!(expand_template(&format!("x:/{}", "x".repeat(8192)), "/", None).is_err());
    }
}
