//! Repository-scoped credentials for LVK-owned Git and GitHub CLI subprocesses.
//! Secrets never belong in command arguments, repository config, or API responses.
use std::{
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
    process::Command,
};

use base64::Engine;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Repository(pub String);
impl Repository {
    pub fn parse(value: &str) -> Option<Self> {
        let path = if let Some(p) = value.strip_prefix("git@github.com:") {
            p
        } else if let Some(p) = value.strip_prefix("ssh://git@github.com/") {
            p
        } else if let Some(p) = value.strip_prefix("https://github.com/") {
            p
        } else if let Some(p) = value.strip_prefix("github.com/") {
            p
        } else {
            value
        };
        let parts: Vec<_> = path.split('/').collect();
        if parts.len() < 2
            || (parts.len() > 2
                && !(parts.len() == 4 && parts[2] == "pull" && parts[3].parse::<u64>().is_ok()))
        {
            return None;
        }
        let owner = parts[0];
        let repo = parts[1].strip_suffix(".git").unwrap_or(parts[1]);
        if [owner, repo].iter().any(|s| {
            s.is_empty()
                || *s == "."
                || *s == ".."
                || !s
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
        }) {
            return None;
        }
        Some(Self(format!("{owner}/{repo}").to_ascii_lowercase()))
    }
    pub fn https_url(&self) -> String {
        format!("https://github.com/{}.git", self.0)
    }
}

#[derive(Serialize, Deserialize)]
pub struct Credential {
    token: String,
}
impl Credential {
    pub fn new(token: String) -> io::Result<Self> {
        if !token.starts_with("github_pat_")
            || token.len() > 1024
            || !token
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_')
        {
            return Err(io::Error::other(
                "Enter a GitHub fine-grained personal access token",
            ));
        }
        Ok(Self { token })
    }
    pub fn token(&self) -> &str {
        &self.token
    }
    pub fn redact(&self, text: &str) -> String {
        text.replace(&self.token, "[REDACTED]")
            .replace(&self.basic_auth(), "[REDACTED]")
    }

    fn basic_auth(&self) -> String {
        base64::engine::general_purpose::STANDARD.encode(format!("x-access-token:{}", self.token))
    }

    /// Reset all credential helpers; only answer for this exact HTTPS repository.
    /// `quit=true` prevents Git from trying interactive or unrelated credentials.
    pub fn configure_git(&self, cmd: &mut Command, repo: &Repository) {
        let helper = format!(
            r#"!f() {{ if test "$1" != get; then exit 0; fi; protocol= host= path=; while IFS='=' read -r key value; do case "$key" in protocol) protocol="$value";; host) host="$value";; path) path="$value";; esac; done; path=$(printf '%s' "$path" | tr '[:upper:]' '[:lower:]'); if test "$protocol" = https && test "$host" = github.com && {{ test "$path" = '{0}' || test "$path" = '{0}.git'; }}; then printf 'username=x-access-token\npassword=%s\n' "$LVK_GITHUB_TOKEN"; else printf 'quit=true\n'; fi; }}; f"#,
            repo.0
        );
        let settings = [
            ("credential.helper", "".to_owned()),
            ("credential.helper", helper),
            ("credential.useHttpPath", "true".to_owned()),
            ("http.followRedirects", "false".to_owned()),
            ("http.extraHeader", "".to_owned()),
            (
                "url.https://github.com/.insteadOf",
                "git@github.com:".to_owned(),
            ),
        ];
        let mut settings: Vec<(String, String)> = settings
            .into_iter()
            .map(|(k, v)| (k.to_owned(), v))
            .collect();
        for url in [format!("https://github.com/{}", repo.0), repo.https_url()] {
            let key = format!("http.{url}.extraHeader");
            settings.push((key.clone(), String::new()));
            settings.push((key, format!("Authorization: Basic {}", self.basic_auth())));
        }
        cmd.env_remove("GIT_CONFIG_PARAMETERS")
            .env("GIT_CONFIG_COUNT", settings.len().to_string());
        for (i, (key, value)) in settings.iter().enumerate() {
            cmd.env(format!("GIT_CONFIG_KEY_{i}"), key)
                .env(format!("GIT_CONFIG_VALUE_{i}"), value);
        }
        cmd.env("LVK_GITHUB_TOKEN", &self.token)
            .env("GIT_TERMINAL_PROMPT", "0")
            .env("GIT_ASKPASS", "false")
            .env("SSH_ASKPASS", "false");
        // Git tracing can include credentials or credential-helper output.
        for key in [
            "GIT_TRACE",
            "GIT_TRACE_CURL",
            "GIT_CURL_VERBOSE",
            "GIT_TRACE_PACKET",
            "GIT_TRACE2",
            "GIT_TRACE2_EVENT",
            "GIT_TRACE2_PERF",
        ] {
            cmd.env_remove(key);
        }
    }
    pub fn configure_gh(&self, cmd: &mut Command, repo: &Repository, config: &Path) {
        self.configure_git(cmd, repo);
        cmd.env("GH_TOKEN", &self.token)
            .env("GH_HOST", "github.com")
            .env("GH_CONFIG_DIR", config)
            .env("GH_PROMPT_DISABLED", "1")
            .env_remove("GITHUB_TOKEN")
            .env_remove("GH_ENTERPRISE_TOKEN")
            .env_remove("GITHUB_ENTERPRISE_TOKEN")
            .env_remove("GH_DEBUG");
    }
}

pub struct Store {
    root: PathBuf,
}
impl Default for Store {
    fn default() -> Self {
        Self::at(crate::assets::asset_dir().join("github-credentials"))
    }
}
impl Store {
    pub fn at(root: PathBuf) -> Self {
        Self { root }
    }
    fn path(&self, repo: &Repository) -> PathBuf {
        self.root.join(format!("{}.json", repo.0.replace('/', "!")))
    }
    pub fn contains(&self, repo: &Repository) -> io::Result<bool> {
        match fs::metadata(self.path(repo)) {
            Ok(_) => Ok(true),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(false),
            Err(e) => Err(e),
        }
    }
    pub fn load(&self, repo: &Repository) -> io::Result<Option<Credential>> {
        let bytes = match fs::read(self.path(repo)) {
            Ok(b) => b,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(_) => return Err(io::Error::other("Cannot read GitHub credential")),
        };
        let credential: Credential = serde_json::from_slice(&bytes)
            .map_err(|_| io::Error::other("Invalid GitHub credential storage"))?;
        Ok(Some(Credential::new(credential.token)?))
    }
    pub fn save(&self, repo: &Repository, credential: &Credential) -> io::Result<()> {
        fs::create_dir_all(&self.root)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&self.root, fs::Permissions::from_mode(0o700))?;
        }
        let mut file = tempfile::NamedTempFile::new_in(&self.root)?;
        serde_json::to_writer(&mut file, credential)?;
        file.flush()?;
        file.as_file().sync_all()?;
        file.persist(self.path(repo)).map_err(|e| e.error)?;
        Ok(())
    }
    pub fn remove(&self, repo: &Repository) -> io::Result<()> {
        match fs::remove_file(self.path(repo)) {
            Err(e) if e.kind() != io::ErrorKind::NotFound => Err(e),
            _ => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn scope_is_exact_and_traversal_is_rejected() {
        let repo = Repository::parse("git@github.com:Owner/Repo.git").unwrap();
        assert_eq!(
            repo,
            Repository::parse("https://github.com/owner/repo/pull/123").unwrap()
        );
        for value in [
            "https://evil.test/owner/repo",
            "https://github.com.evil/owner/repo",
            "https://user@github.com/owner/repo",
            "../repo",
            "owner/..",
            "owner/repo?x=1",
            "owner/repo/issues/3",
        ] {
            assert!(Repository::parse(value).is_none(), "{value}");
        }
    }
    #[test]
    fn storage_is_private_replaceable_and_never_debugs_token() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::at(dir.path().join("secrets"));
        let repo = Repository::parse("owner/repo").unwrap();
        let first = Credential::new("github_pat_first".into()).unwrap();
        store.save(&repo, &first).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(store.path(&repo))
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o600
            );
        }
        let second = Credential::new("github_pat_second".into()).unwrap();
        store.save(&repo, &second).unwrap();
        assert_eq!(store.load(&repo).unwrap().unwrap().token(), second.token());
        assert!(
            store
                .load(&Repository::parse("other/repo").unwrap())
                .unwrap()
                .is_none()
        );
        store.remove(&repo).unwrap();
        assert!(store.load(&repo).unwrap().is_none());
    }
    #[test]
    fn corrupt_storage_fails_closed() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::at(dir.path().to_owned());
        let repo = Repository::parse("owner/repo").unwrap();
        fs::write(store.path(&repo), "broken").unwrap();
        assert!(store.load(&repo).is_err());
    }
    #[test]
    fn authorization_header_is_scoped_and_redacted() {
        let token = Credential::new("github_pat_header_test".into()).unwrap();
        let repo = Repository::parse("owner/repo").unwrap();
        for (url, allowed) in [
            (repo.https_url(), true),
            ("https://github.com/other/repo.git".into(), false),
            ("https://evil.test/owner/repo.git".into(), false),
        ] {
            let mut cmd = Command::new("git");
            cmd.args(["config", "--get-urlmatch", "http.extraheader", &url]);
            token.configure_git(&mut cmd, &repo);
            let output = cmd.output().unwrap();
            let text = String::from_utf8_lossy(&output.stdout);
            assert_eq!(text.contains(&token.basic_auth()), allowed);
            assert!(!token.redact(&text).contains(&token.basic_auth()));
            assert!(
                !cmd.get_args()
                    .any(|arg| arg.to_string_lossy().contains(token.token()))
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn credential_helper_rejects_other_hosts_and_repositories() {
        let token = Credential::new("github_pat_test".into()).unwrap();
        let repo = Repository::parse("owner/repo").unwrap();
        for (host, path, allowed) in [
            ("github.com", "owner/repo.git", true),
            ("github.com", "other/repo.git", false),
            ("evil.test", "owner/repo.git", false),
        ] {
            let mut cmd = Command::new("git");
            cmd.args(["credential", "fill"])
                .stdin(std::process::Stdio::piped())
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped());
            token.configure_git(&mut cmd, &repo);
            let mut child = cmd.spawn().unwrap();
            write!(
                child.stdin.take().unwrap(),
                "protocol=https\nhost={host}\npath={path}\n\n"
            )
            .unwrap();
            let out = child.wait_with_output().unwrap();
            assert_eq!(out.status.success(), allowed);
            assert_eq!(
                String::from_utf8_lossy(&out.stdout).contains(token.token()),
                allowed
            );
        }
    }
}
