//! Commit identity follows Git, never a service/login/OS identity.
use std::{fs::OpenOptions, io::ErrorKind, path::Path};

use git2::{Signature, Time};

use crate::{GitCli, GitService, GitServiceError};

const SETUP: &str = "Set both user.name and user.email in this repository with git config --local user.name \"YOUR NAME\" and git config --local user.email \"YOUR COMMIT EMAIL\". Choose the values yourself; LVK must not infer them from login or history. Linked worktrees share repository-local configuration. Global/system configuration is not changed. If you intentionally use the old LVK identity, explicitly confirm it with git config --local lvk.allowLegacyIdentity true. Retry after configuring; no history rewrite is needed.";

impl GitService {
    /// Only resume initial creation after our identity guard stopped the empty
    /// repository. Never treat an arbitrary existing directory as a new repo.
    pub fn pending_initial_identity_setup(&self, root: &Path) -> Result<bool, GitServiceError> {
        let repo = self.open_repo(root)?;
        Ok(repo.is_empty()?
            && repo
                .workdir()
                .is_some_and(|path| path.canonicalize().ok() == root.canonicalize().ok())
            && repo.commondir().join("lvk-identity-guidance").is_file())
    }

    /// Resolve through Git itself, including author/committer config, explicit
    /// environment and dates. useConfigOnly disables OS-name/email guessing.
    /// Rebase needs only a committer: its original authors remain untouched.
    pub fn require_commit_identity(
        &self,
        root: &Path,
        preserve_author: bool,
    ) -> Result<(), GitServiceError> {
        self.commit_signatures(root, preserve_author).map(|_| ())
    }

    pub(crate) fn commit_signatures(
        &self,
        root: &Path,
        preserve_author: bool,
    ) -> Result<(Option<Signature<'static>>, Signature<'static>), GitServiceError> {
        let git = GitCli::new();
        let read = |key| -> Result<Signature<'static>, GitServiceError> {
            let ident = git.git(root, ["-c", "user.useConfigOnly=true", "var", key])?;
            let signature = parse_ident(ident.trim())?;
            if signature.name().is_none_or(|name| name.trim().is_empty())
                || signature
                    .email()
                    .is_none_or(|email| email.trim().is_empty())
            {
                return Err(GitServiceError::CommitIdentity("Empty Git identity".into()));
            }
            Ok(signature)
        };
        let signatures = (|| {
            let author = if preserve_author {
                None
            } else {
                Some(read("GIT_AUTHOR_IDENT")?)
            };
            let committer = read("GIT_COMMITTER_IDENT")?;
            Ok::<_, GitServiceError>((author, committer))
        })();
        let (author, committer) = match signatures {
            Ok(pair) => pair,
            Err(_) => {
                return Err(self.identity_error(root, "Git commit identity is missing or invalid")?);
            }
        };
        let legacy = |sig: &Signature<'_>| {
            matches!(
                (sig.name(), sig.email()),
                (Some("Vibe Kanban"), Some("noreply@vibekanban.com"))
                    | (Some("LVK Integration"), Some("integration@localhost"))
            )
        };
        // Only a repository-local opt-in can acknowledge the old service
        // identity. A global setting must not silently opt every repository in.
        let repo = self.open_repo(root)?;
        let local = repo.config()?.open_level(git2::ConfigLevel::Local)?;
        let confirmed = local.get_bool("lvk.allowLegacyIdentity").unwrap_or(false);
        if !confirmed && (legacy(&committer) || author.as_ref().is_some_and(legacy)) {
            return Err(self.identity_error(root, "Git identity matches an old LVK automatic identity; choose your identity or explicitly confirm continued use")?);
        }
        Ok((author, committer))
    }

    /// A durable, atomic notice per common Git directory, shared by
    /// linked worktrees and restarts. This is guidance state, not Git config.
    fn identity_error(
        &self,
        root: &Path,
        summary: &str,
    ) -> Result<GitServiceError, GitServiceError> {
        let repo = self.open_repo(root)?;
        let notice = repo.commondir().join("lvk-identity-guidance");
        let first = match OpenOptions::new().write(true).create_new(true).open(notice) {
            Ok(_) => true,
            Err(error) if error.kind() == ErrorKind::AlreadyExists => false,
            Err(error) => return Err(error.into()),
        };
        let detail = if first {
            SETUP
        } else {
            "Repository identity setup was already explained. Set user.name/user.email or confirm the old identity locally, then retry."
        };
        Ok(GitServiceError::CommitIdentity(format!(
            "{summary} ({}). {detail}",
            root.display()
        )))
    }

    /// Agents keep their normal Git commands (including --author and cherry-pick).
    /// Give setup instructions once, rather than repeatedly asking for a name.
    pub fn commit_identity_instructions(&self, root: &Path) -> Result<String, GitServiceError> {
        match self.require_commit_identity(root, false) {
            Ok(()) => Ok("Use this repository's effective Git identity for new commits. Preserve explicit Git author/committer overrides and original authors on rebase/cherry-pick. Never install an LVK/brand fallback or infer a person's name/email.".into()),
            Err(GitServiceError::CommitIdentity(message)) => Ok(format!("{message}\nDo not create commits until the user configures or explicitly confirms the repository identity. You may investigate or edit without committing. Never set identity or acknowledge the legacy identity on the user's behalf.")),
            Err(error) => Err(error),
        }
    }
}

// `git var` emits NAME <EMAIL> UNIX_SECONDS +/-HHMM. Parse from the right so
// spaces in the person's name are preserved. Git has already applied its own
// validation and precedence rules, including date environment variables.
fn parse_ident(ident: &str) -> Result<Signature<'static>, git2::Error> {
    let invalid = || git2::Error::from_str("Invalid Git identity");
    let (person, date) = ident.rsplit_once("> ").ok_or_else(invalid)?;
    let (name, email) = person.rsplit_once(" <").ok_or_else(invalid)?;
    let (seconds, zone) = date.split_once(' ').ok_or_else(invalid)?;
    let seconds = seconds.parse::<i64>().map_err(|_| invalid())?;
    if zone.len() != 5
        || !matches!(zone.as_bytes()[0], b'+' | b'-')
        || !zone.as_bytes()[1..].iter().all(u8::is_ascii_digit)
    {
        return Err(invalid());
    }
    let hours = zone[1..3].parse::<i32>().map_err(|_| invalid())?;
    let minutes = zone[3..5].parse::<i32>().map_err(|_| invalid())?;
    let offset = (hours * 60 + minutes) * if zone.starts_with('-') { -1 } else { 1 };
    Signature::new(name, email, &Time::new(seconds, offset))
}
