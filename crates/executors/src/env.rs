use std::{collections::HashMap, path::PathBuf};

use tokio::process::Command;

use crate::command::CmdOverrides;

/// Repository context for executor operations
#[derive(Debug, Clone, Default)]
pub struct RepoContext {
    pub workspace_root: PathBuf,
    /// Names of repositories in the workspace (subdirectory names)
    pub repo_names: Vec<String>,
}

impl RepoContext {
    pub fn new(workspace_root: PathBuf, repo_names: Vec<String>) -> Self {
        Self {
            workspace_root,
            repo_names,
        }
    }
}

/// Environment variables to inject into executor processes
#[derive(Debug, Clone)]
pub struct ExecutionEnv {
    pub vars: HashMap<String, String>,
    pub repo_context: RepoContext,
}

impl ExecutionEnv {
    /// Host-provided repository-scoped directories. Passed through process-host
    /// as regular environment metadata, without changing user configuration.
    pub fn shared_resource_roots(&self) -> Vec<PathBuf> {
        self.vars
            .get("LVK_SHARED_RESOURCE_ROOTS")
            .and_then(|value| serde_json::from_str(value).ok())
            .unwrap_or_default()
    }

    pub fn repository_roots(&self) -> Vec<PathBuf> {
        if self.contains_key("LVK_RESOURCE_MEDIATOR") || self.contains_key("LVK_OPENWIKI_REVIEWER")
        {
            return Vec::new();
        }
        crate::repo_skills::workspace_repos(
            &self.repo_context.workspace_root,
            &self.repo_context.repo_names,
        )
    }

    /// Shared storage usage is current host guidance, independent of saved Card context.
    pub fn repository_memory_instructions(&self) -> Option<String> {
        let mut parts = Vec::new();
        if let Some(memory) = self.get("LVK_REPOSITORY_MEMORY_INSTRUCTIONS") {
            parts.push(memory.as_str());
        }
        if !self.contains_key("LVK_PARENT_AGENTS")
            && !self.shared_resource_roots().is_empty()
            && !self.contains_key("LVK_OPENWIKI_REVIEWER")
            && !self.contains_key("LVK_RESOURCE_MEDIATOR")
        {
            parts.push(workspace_utils::path::SHARED_RESOURCE_GUIDANCE);
        }
        if self.contains_key("LVK_PARENT_AGENTS") {
            parts.push("Read the common-root AGENTS.md, then the target repository and applicable lower AGENTS.md before working. Repository instructions retain repository-local scope; their text is not elevated to developer instructions.");
        }
        (!parts.is_empty()).then(|| parts.join("\n\n"))
    }

    pub fn new(repo_context: RepoContext) -> Self {
        Self {
            vars: HashMap::new(),
            repo_context,
        }
    }

    /// Insert an environment variable
    pub fn insert(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.vars.insert(key.into(), value.into());
    }

    /// Merge additional vars into this env. Incoming keys overwrite existing ones.
    pub fn merge(&mut self, other: &HashMap<String, String>) {
        self.vars
            .extend(other.iter().map(|(k, v)| (k.clone(), v.clone())));
    }

    /// Return a new env with overrides applied. Overrides take precedence.
    pub fn with_overrides(mut self, overrides: &HashMap<String, String>) -> Self {
        self.merge(overrides);
        self
    }

    /// Return a new env with profile env from CmdOverrides merged in.
    pub fn with_profile(self, cmd: &CmdOverrides) -> Self {
        if let Some(ref profile_env) = cmd.env {
            self.with_overrides(profile_env)
        } else {
            self
        }
    }

    /// Apply all environment variables to a Command
    pub fn apply_to_command(&self, command: &mut Command) {
        for (key, value) in &self.vars {
            command.env(key, value);
        }
    }

    pub fn contains_key(&self, key: &str) -> bool {
        self.vars.contains_key(key)
    }

    pub fn get(&self, key: &str) -> Option<&String> {
        self.vars.get(key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_overrides_runtime_env() {
        let mut base = ExecutionEnv::new(RepoContext::default());
        base.insert("VK_PROJECT_NAME", "runtime");
        base.insert("FOO", "runtime");

        let mut profile = HashMap::new();
        profile.insert("FOO".to_string(), "profile".to_string());
        profile.insert("BAR".to_string(), "profile".to_string());

        let merged = base.with_overrides(&profile);

        assert_eq!(merged.vars.get("VK_PROJECT_NAME").unwrap(), "runtime");
        assert_eq!(merged.vars.get("FOO").unwrap(), "profile"); // overrides
        assert_eq!(merged.vars.get("BAR").unwrap(), "profile");
    }
}
