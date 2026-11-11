//! Host-selected restrictions for Workflow reporting phases. Never a prompt policy.
use std::path::Path;

use serde_json::{Value, json};

use crate::{command::CmdOverrides, env::ExecutionEnv, executors::ExecutorError};
pub const ROOTS: &str = "LVK_WORKFLOW_PROTECTED_ROOTS";
pub fn roots(env: &ExecutionEnv) -> Result<Option<Vec<String>>, ExecutorError> {
    let Some(value) = env.get(ROOTS) else {
        return Ok(None);
    };
    let paths: Vec<String> =
        serde_json::from_str(value).map_err(|e| ExecutorError::Io(std::io::Error::other(e)))?;
    if paths.is_empty() || paths.iter().any(|p| !Path::new(p).is_absolute()) {
        return Err(ExecutorError::Io(std::io::Error::other(
            "Invalid host source protection roots",
        )));
    }
    Ok(Some(paths))
}
pub fn validate_launcher(cmd: &CmdOverrides, router: bool) -> Result<(), ExecutorError> {
    if router
        || cmd.base_command_override.is_some()
        || cmd
            .additional_params
            .as_ref()
            .is_some_and(|a| !a.is_empty())
    {
        return Err(ExecutorError::Io(std::io::Error::other(
            "Workflow source protection requires the official provider launcher without arbitrary command or argument overrides",
        )));
    }
    Ok(())
}
pub fn claude_settings(paths: &[String], env: &ExecutionEnv, subscription: Option<Value>) -> Value {
    let mut value = subscription.unwrap_or_else(|| json!({}));
    value["disableAllHooks"] = json!(true);
    value["autoMemoryEnabled"] = json!(false);
    value["sandbox"] = json!({"enabled":true,"failIfUnavailable":true,"allowUnsandboxedCommands":false,"enableWeakerNestedSandbox":false,"enableWeakerNetworkIsolation":false,"allowNestedUserNamespaces":false,"filesystem":{"disabled":false,"allowWrite":writable_roots(env, paths),"denyWrite":paths},"network":{"allowAllUnixSockets":false}});
    value
}

/// Only host-provided temporary/cache directories may be added to a reporting
/// sandbox. Resolve symlinks before checking against the protected source roots.
fn writable_roots(env: &ExecutionEnv, protected: &[String]) -> Vec<String> {
    let candidates = std::iter::once(std::env::temp_dir()).chain(
        env.shared_resource_roots()
            .into_iter()
            .filter(|p| p.file_name().is_some_and(|n| n == "tmp" || n == "cache")),
    );
    let mut roots = Vec::new();
    for path in candidates {
        let path = std::fs::canonicalize(&path).unwrap_or(path);
        if path.is_absolute() && !protected.iter().any(|p| path.starts_with(p)) {
            roots.push(path.to_string_lossy().into_owned());
        }
    }
    roots.sort();
    roots.dedup();
    roots
}

/// A thread-local native Codex profile: read-only source with explicit
/// temporary/cache writes. Never changes a saved provider profile.
pub fn codex_filesystem(env: &ExecutionEnv, protected: &[String]) -> Value {
    let mut entries = serde_json::Map::new();
    entries.insert("/".into(), json!("read"));
    for path in writable_roots(env, protected) {
        entries.insert(path, json!("write"));
    }
    for path in protected {
        entries.insert(path.clone(), json!("read"));
    }
    Value::Object(entries)
}

/// Wrap the whole Claude process as well as its shell tools. Native managed
/// settings retain precedence, but cannot make these protected mounts writable.
/// No fallback, installation, namespace-policy change, or privileged execution.
pub fn claude_command(
    program: &Path,
    env: &ExecutionEnv,
) -> Result<tokio::process::Command, ExecutorError> {
    let Some(paths) = roots(env)? else {
        return Ok(tokio::process::Command::new(program));
    };
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (program, paths);
        Err(ExecutorError::Io(std::io::Error::other(
            "Claude Workflow source protection currently requires Linux bubblewrap",
        )))
    }
    #[cfg(target_os = "linux")]
    {
        let launcher = Path::new("/usr/bin/bwrap");
        if !launcher.is_file() {
            return Err(ExecutorError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "Claude Workflow source protection requires /usr/bin/bwrap; no unsandboxed fallback is allowed",
            )));
        }
        let mut command = tokio::process::Command::new(launcher);
        command.args([
            "--die-with-parent",
            "--new-session",
            "--unshare-user",
            "--unshare-pid",
            "--unshare-ipc",
            "--unshare-uts",
            "--bind",
            "/",
            "/",
            "--proc",
            "/proc",
            "--dev",
            "/dev",
        ]);
        for path in paths {
            let canonical = std::fs::canonicalize(&path)?;
            command.arg("--ro-bind").arg(&canonical).arg(&canonical);
        }
        command.arg("--").arg(program);
        Ok(command)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn restrictions_preserve_auth_and_fail_closed() {
        let v = claude_settings(
            &["/source".into()],
            &ExecutionEnv::new(Default::default()),
            Some(json!({"forceLoginMethod":"claudeai","env":{"ANTHROPIC_API_KEY":""}})),
        );
        assert_eq!(v["forceLoginMethod"], "claudeai");
        assert_eq!(v["env"]["ANTHROPIC_API_KEY"], "");
        assert_eq!(v["sandbox"]["failIfUnavailable"], true);
        assert_eq!(v["sandbox"]["allowUnsandboxedCommands"], false);
        assert_eq!(v["sandbox"]["filesystem"]["denyWrite"][0], "/source");
        assert_eq!(v["disableAllHooks"], true);
    }
    #[test]
    fn codex_profile_separates_source_and_shared_cache() {
        let mut env = ExecutionEnv::new(Default::default());
        env.insert(
            "LVK_SHARED_RESOURCE_ROOTS",
            "[\"/shared/cache\",\"/shared/tmp\",\"/shared/persistent\"]",
        );
        let fs = codex_filesystem(&env, &["/source".into()]);
        assert_eq!(fs["/source"], "read");
        assert_eq!(fs["/shared/cache"], "write");
        assert_eq!(fs["/shared/tmp"], "write");
        assert!(fs.get("/shared/persistent").is_none());
        assert_eq!(fs["/"], "read");
        let claude = claude_settings(&["/source".into()], &env, None);
        let allowed = claude["sandbox"]["filesystem"]["allowWrite"]
            .as_array()
            .unwrap();
        assert!(allowed.contains(&json!("/shared/cache")));
        assert!(allowed.contains(&json!("/shared/tmp")));
        assert!(!allowed.contains(&json!("/shared/persistent")));
        assert_eq!(
            claude["sandbox"]["filesystem"]["denyWrite"],
            json!(["/source"])
        );
    }
    #[cfg(unix)]
    #[test]
    fn cache_symlink_cannot_add_a_protected_source_as_writable() {
        let fixture = tempfile::tempdir().unwrap();
        let source = fixture.path().join("source");
        std::fs::create_dir(&source).unwrap();
        let cache = fixture.path().join("cache");
        std::os::unix::fs::symlink(&source, &cache).unwrap();
        let mut env = ExecutionEnv::new(Default::default());
        env.insert(
            "LVK_SHARED_RESOURCE_ROOTS",
            serde_json::to_string(&[cache]).unwrap(),
        );
        let protected = source.to_string_lossy().into_owned();
        assert!(!writable_roots(&env, std::slice::from_ref(&protected)).contains(&protected));
        assert_eq!(
            codex_filesystem(&env, std::slice::from_ref(&protected))[&protected],
            "read"
        );
    }
    #[test]
    fn reporting_rejects_arbitrary_launcher_arguments() {
        let cmd = CmdOverrides {
            additional_params: Some(vec!["--tools=default".into()]),
            ..Default::default()
        };
        assert!(validate_launcher(&cmd, false).is_err());
        assert!(validate_launcher(&CmdOverrides::default(), true).is_err());
    }
    #[cfg(target_os = "linux")]
    #[test]
    fn missing_outer_sandbox_never_spawns_the_provider() {
        if Path::new("/usr/bin/bwrap").is_file() {
            return;
        }
        let mut env = ExecutionEnv::new(Default::default());
        env.insert(ROOTS, "[\"/source\"]");
        let error = claude_command(Path::new("/bin/false"), &env).unwrap_err();
        assert!(error.to_string().contains("no unsandboxed fallback"));
    }
    #[test]
    fn malformed_host_policy_never_becomes_full_access() {
        let mut e = ExecutionEnv::new(Default::default());
        e.insert(ROOTS, "[]");
        assert!(roots(&e).is_err());
        e.insert(ROOTS, "[\"relative\"]");
        assert!(roots(&e).is_err());
    }
}
