//! Minimal helpers around the GitHub CLI (`gh`).
//!
//! This module provides low-level access to the GitHub CLI for operations
//! the REST client does not cover well.

use std::{
    ffi::{OsStr, OsString},
    io::Write,
    path::Path,
    process::Command,
};

use chrono::{DateTime, Utc};
use db::models::merge::MergeStatus;
use serde::Deserialize;
use tempfile::NamedTempFile;
use thiserror::Error;
use url::Url;
use utils::{command_ext::NoWindowExt, shell::resolve_executable_path_blocking};

use crate::types::{
    CreatePrRequest, PrComment, PrCommentAuthor, PrReviewComment, PullRequestDetail,
    ReviewCommentUser,
};

#[derive(Debug, Clone)]
pub struct GitHubRepoInfo {
    pub owner: String,
    pub repo_name: String,
    /// GitHub hostname (e.g., "github.com" or enterprise hostname)
    pub hostname: Option<String>,
}

impl GitHubRepoInfo {
    pub fn repo_spec(&self) -> String {
        match &self.hostname {
            Some(host) => format!("{}/{}/{}", host, self.owner, self.repo_name),
            None => format!("{}/{}", self.owner, self.repo_name),
        }
    }
}

#[derive(Deserialize)]
struct GhRepoViewResponse {
    owner: GhRepoOwner,
    name: String,
    url: String,
}

#[derive(Deserialize)]
struct GhRepoOwner {
    login: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GhCommentResponse {
    id: String,
    author: Option<GhUserLogin>,
    #[serde(default)]
    author_association: String,
    #[serde(default)]
    body: String,
    created_at: Option<DateTime<Utc>>,
    #[serde(default)]
    url: String,
}

#[derive(Deserialize)]
struct GhCommentsWrapper {
    comments: Vec<GhCommentResponse>,
}

#[derive(Deserialize)]
struct GhUserLogin {
    login: Option<String>,
}

#[derive(Deserialize)]
struct GhReviewCommentResponse {
    id: i64,
    user: Option<GhUserLogin>,
    #[serde(default)]
    body: String,
    created_at: Option<DateTime<Utc>>,
    #[serde(default)]
    html_url: String,
    #[serde(default)]
    path: String,
    line: Option<i64>,
    side: Option<String>,
    #[serde(default)]
    diff_hunk: String,
    #[serde(default)]
    author_association: String,
}

#[derive(Deserialize)]
struct GhMergeCommit {
    oid: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GhPrResponse {
    number: i64,
    url: String,
    #[serde(default)]
    state: String,
    merged_at: Option<DateTime<Utc>>,
    merge_commit: Option<GhMergeCommit>,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    base_ref_name: Option<String>,
    #[serde(default)]
    head_ref_name: Option<String>,
    #[serde(default)]
    updated_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Error)]
pub enum GhCliError {
    #[error("GitHub CLI (`gh`) executable not found or not runnable")]
    NotAvailable,
    #[error("GitHub CLI command failed: {0}")]
    CommandFailed(String),
    #[error("GitHub CLI authentication failed: {0}")]
    AuthFailed(String),
    #[error("GitHub CLI returned unexpected output: {0}")]
    UnexpectedOutput(String),
}

#[derive(Debug, Clone, Default)]
pub struct GhCli;

impl GhCli {
    pub fn new() -> Self {
        Self {}
    }

    /// Ensure the GitHub CLI binary is discoverable.
    fn ensure_available(&self) -> Result<(), GhCliError> {
        resolve_executable_path_blocking("gh").ok_or(GhCliError::NotAvailable)?;
        Ok(())
    }

    fn run<I, S>(&self, target: &str, args: I, dir: Option<&Path>) -> Result<String, GhCliError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        self.ensure_available()?;
        let gh = resolve_executable_path_blocking("gh").ok_or(GhCliError::NotAvailable)?;
        self.run_with_store(
            target,
            args,
            dir,
            &utils::github_credentials::Store::default(),
            &gh,
        )
    }

    fn run_with_store<I, S>(
        &self,
        target: &str,
        args: I,
        dir: Option<&Path>,
        store: &utils::github_credentials::Store,
        executable: &Path,
    ) -> Result<String, GhCliError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let mut cmd = Command::new(executable);
        if let Some(d) = dir {
            cmd.current_dir(d);
        }
        for arg in args {
            cmd.arg(arg);
        }
        let scoped = utils::github_credentials::Repository::parse(target);
        let credential = scoped
            .as_ref()
            .map(|repo| store.load(repo))
            .transpose()
            .map_err(|_| {
                GhCliError::AuthFailed(
                    "Cannot read configured GitHub token; update it in repository settings".into(),
                )
            })?
            .flatten();
        let config = tempfile::tempdir().map_err(|_| {
            GhCliError::CommandFailed("Cannot prepare GitHub authentication".into())
        })?;
        if let (Some(token), Some(repo)) = (&credential, &scoped) {
            token.configure_gh(&mut cmd, repo, config.path());
        }
        let output = cmd
            .no_window()
            .output()
            .map_err(|err| GhCliError::CommandFailed(err.to_string()))?;

        if output.status.success() {
            let text = String::from_utf8_lossy(&output.stdout).to_string();
            return Ok(credential
                .as_ref()
                .map_or_else(|| text.clone(), |c| c.redact(&text)));
        }

        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stderr = credential
            .as_ref()
            .map_or_else(|| stderr.clone(), |c| c.redact(&stderr));

        // Check exit code first - gh CLI uses exit code 4 for auth failures
        if output.status.code() == Some(4) {
            return Err(GhCliError::AuthFailed(stderr));
        }

        // Fall back to string matching for older gh versions or other auth scenarios
        let lower = stderr.to_ascii_lowercase();
        if lower.contains("authentication failed")
            || lower.contains("must authenticate")
            || lower.contains("bad credentials")
            || lower.contains("unauthorized")
            || lower.contains("gh auth login")
        {
            return Err(GhCliError::AuthFailed(stderr));
        }

        Err(GhCliError::CommandFailed(stderr))
    }

    pub fn get_repo_info(
        &self,
        remote_url: &str,
        repo_path: &Path,
    ) -> Result<GitHubRepoInfo, GhCliError> {
        let raw = self.run(
            remote_url,
            ["repo", "view", remote_url, "--json", "owner,name,url"],
            Some(repo_path),
        )?;
        Self::parse_repo_info_response(&raw)
    }

    fn parse_repo_info_response(raw: &str) -> Result<GitHubRepoInfo, GhCliError> {
        let resp: GhRepoViewResponse = serde_json::from_str(raw).map_err(|e| {
            GhCliError::UnexpectedOutput(format!("Failed to parse gh repo view response: {e}"))
        })?;

        let hostname = Url::parse(&resp.url)
            .ok()
            .and_then(|u| u.host_str().map(String::from));

        Ok(GitHubRepoInfo {
            owner: resp.owner.login,
            repo_name: resp.name,
            hostname,
        })
    }

    /// Run `gh pr create` and parse the response.
    ///
    /// The `repo_path` parameter specifies the working directory for the command.
    /// This is required for compatibility with older `gh` CLI versions (e.g., v2.4.0)
    /// that require running from within a git repository.
    pub fn create_pr(
        &self,
        request: &CreatePrRequest,
        repo_info: &GitHubRepoInfo,
        repo_path: &Path,
    ) -> Result<PullRequestDetail, GhCliError> {
        // Write body to temp file to avoid shell escaping and length issues
        let body = request.body.as_deref().unwrap_or("");
        let mut body_file = NamedTempFile::new()
            .map_err(|e| GhCliError::CommandFailed(format!("Failed to create temp file: {e}")))?;
        body_file
            .write_all(body.as_bytes())
            .map_err(|e| GhCliError::CommandFailed(format!("Failed to write body: {e}")))?;

        let repo_spec = repo_info.repo_spec();

        let mut args: Vec<OsString> = Vec::with_capacity(14);
        args.push(OsString::from("pr"));
        args.push(OsString::from("create"));
        args.push(OsString::from("--repo"));
        args.push(OsString::from(&repo_spec));
        args.push(OsString::from("--head"));
        args.push(OsString::from(&request.head_branch));
        args.push(OsString::from("--base"));
        args.push(OsString::from(&request.base_branch));
        args.push(OsString::from("--title"));
        args.push(OsString::from(&request.title));
        args.push(OsString::from("--body-file"));
        args.push(body_file.path().as_os_str().to_os_string());

        if request.draft.unwrap_or(false) {
            args.push(OsString::from("--draft"));
        }

        let raw = self.run(&repo_spec, args, Some(repo_path))?;
        Self::parse_pr_create_text(&raw, request)
    }

    /// Retrieve details for a pull request by URL.
    pub fn view_pr(&self, pr_url: &str) -> Result<PullRequestDetail, GhCliError> {
        let raw = self.run(
            pr_url,
            [
                "pr",
                "view",
                pr_url,
                "--json",
                "number,url,state,mergedAt,mergeCommit,title,baseRefName,headRefName",
            ],
            None,
        )?;
        Self::parse_pr_view(&raw)
    }

    /// List pull requests for a branch (includes closed/merged).
    pub fn list_prs_for_branch(
        &self,
        repo_info: &GitHubRepoInfo,
        branch: &str,
    ) -> Result<Vec<PullRequestDetail>, GhCliError> {
        let repo_spec = repo_info.repo_spec();
        let raw = self.run(
            &repo_spec,
            [
                "pr",
                "list",
                "--repo",
                &repo_spec,
                "--state",
                "all",
                "--head",
                branch,
                "--json",
                "number,url,title,headRefName,baseRefName,state,mergedAt,mergeCommit",
            ],
            None,
        )?;
        Self::parse_pr_list(&raw)
    }

    pub fn list_prs(&self, owner: &str, repo: &str) -> Result<Vec<PullRequestDetail>, GhCliError> {
        let repo_spec = format!("{owner}/{repo}");
        let json_fields =
            "number,url,title,headRefName,baseRefName,state,mergedAt,mergeCommit,updatedAt";

        let open_raw = self.run(
            &repo_spec,
            [
                "pr",
                "list",
                "--repo",
                &repo_spec,
                "--state",
                "open",
                "--json",
                json_fields,
            ],
            None,
        )?;

        let closed_raw = self.run(
            &repo_spec,
            [
                "pr",
                "list",
                "--repo",
                &repo_spec,
                "--state",
                "closed",
                "--limit",
                "20",
                "--json",
                json_fields,
            ],
            None,
        )?;

        let mut open_prs: Vec<GhPrResponse> =
            serde_json::from_str(open_raw.trim()).map_err(|err| {
                GhCliError::UnexpectedOutput(format!(
                    "Failed to parse gh pr list (open) response: {err}; raw: {open_raw}"
                ))
            })?;
        let closed_prs: Vec<GhPrResponse> =
            serde_json::from_str(closed_raw.trim()).map_err(|err| {
                GhCliError::UnexpectedOutput(format!(
                    "Failed to parse gh pr list (closed) response: {err}; raw: {closed_raw}"
                ))
            })?;

        open_prs.extend(closed_prs);
        open_prs.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));

        Ok(open_prs
            .into_iter()
            .map(Self::pr_response_to_detail)
            .collect())
    }

    /// Fetch comments for a pull request.
    pub fn get_pr_comments(
        &self,
        repo_info: &GitHubRepoInfo,
        pr_number: i64,
    ) -> Result<Vec<PrComment>, GhCliError> {
        let repo_spec = repo_info.repo_spec();
        let raw = self.run(
            &repo_spec,
            [
                "pr",
                "view",
                &pr_number.to_string(),
                "--repo",
                &repo_spec,
                "--json",
                "comments",
            ],
            None,
        )?;
        Self::parse_pr_comments(&raw)
    }

    /// Fetch inline review comments for a pull request via API.
    pub fn get_pr_review_comments(
        &self,
        repo_info: &GitHubRepoInfo,
        pr_number: i64,
    ) -> Result<Vec<PrReviewComment>, GhCliError> {
        let mut args = vec![
            "api".to_string(),
            format!(
                "repos/{}/{}/pulls/{}/comments",
                repo_info.owner, repo_info.repo_name, pr_number
            ),
        ];
        if let Some(ref host) = repo_info.hostname {
            args.push("--hostname".to_string());
            args.push(host.clone());
        }
        let raw = self.run(&repo_info.repo_spec(), args, None)?;
        Self::parse_pr_review_comments(&raw)
    }

    pub fn pr_checkout(
        &self,
        repo_path: &Path,
        owner: &str,
        repo: &str,
        pr_number: i64,
    ) -> Result<(), GhCliError> {
        self.run(
            &format!("{owner}/{repo}"),
            [
                "pr",
                "checkout",
                &pr_number.to_string(),
                "--repo",
                &format!("{owner}/{repo}"),
                "--force",
            ],
            Some(repo_path),
        )?;
        Ok(())
    }
}

impl GhCli {
    fn parse_pr_create_text(
        raw: &str,
        request: &CreatePrRequest,
    ) -> Result<PullRequestDetail, GhCliError> {
        let pr_url = raw
            .lines()
            .rev()
            .flat_map(|line| line.split_whitespace())
            .map(|token| token.trim_matches(|c: char| c == '<' || c == '>'))
            .find(|token| token.starts_with("http") && token.contains("/pull/"))
            .ok_or_else(|| {
                GhCliError::UnexpectedOutput(format!(
                    "gh pr create did not return a pull request URL; raw output: {raw}"
                ))
            })?
            .trim_end_matches(['.', ',', ';'])
            .to_string();

        let number = pr_url
            .rsplit('/')
            .next()
            .ok_or_else(|| {
                GhCliError::UnexpectedOutput(format!(
                    "Failed to extract PR number from URL '{pr_url}'"
                ))
            })?
            .trim_end_matches(|c: char| !c.is_ascii_digit())
            .parse::<i64>()
            .map_err(|err| {
                GhCliError::UnexpectedOutput(format!(
                    "Failed to parse PR number from URL '{pr_url}': {err}"
                ))
            })?;

        Ok(PullRequestDetail {
            number,
            url: pr_url,
            status: MergeStatus::Open,
            merged_at: None,
            merge_commit_sha: None,
            title: request.title.clone(),
            base_branch: request.base_branch.clone(),
            head_branch: request.head_branch.clone(),
        })
    }

    fn parse_pr_view(raw: &str) -> Result<PullRequestDetail, GhCliError> {
        let pr: GhPrResponse = serde_json::from_str(raw.trim()).map_err(|err| {
            GhCliError::UnexpectedOutput(format!(
                "Failed to parse gh pr view response: {err}; raw: {raw}"
            ))
        })?;
        Ok(Self::pr_response_to_detail(pr))
    }

    fn parse_pr_list(raw: &str) -> Result<Vec<PullRequestDetail>, GhCliError> {
        let prs: Vec<GhPrResponse> = serde_json::from_str(raw.trim()).map_err(|err| {
            GhCliError::UnexpectedOutput(format!(
                "Failed to parse gh pr list response: {err}; raw: {raw}"
            ))
        })?;
        Ok(prs.into_iter().map(Self::pr_response_to_detail).collect())
    }

    fn pr_response_to_detail(pr: GhPrResponse) -> PullRequestDetail {
        let state = if pr.state.is_empty() {
            "OPEN"
        } else {
            &pr.state
        };
        PullRequestDetail {
            number: pr.number,
            url: pr.url,
            status: match state.to_ascii_uppercase().as_str() {
                "OPEN" => MergeStatus::Open,
                "MERGED" => MergeStatus::Merged,
                "CLOSED" => MergeStatus::Closed,
                _ => MergeStatus::Unknown,
            },
            merged_at: pr.merged_at,
            merge_commit_sha: pr.merge_commit.and_then(|c| c.oid),
            title: pr.title.unwrap_or_default(),
            base_branch: pr.base_ref_name.unwrap_or_default(),
            head_branch: pr.head_ref_name.unwrap_or_default(),
        }
    }

    fn parse_pr_comments(raw: &str) -> Result<Vec<PrComment>, GhCliError> {
        let wrapper: GhCommentsWrapper = serde_json::from_str(raw.trim()).map_err(|err| {
            GhCliError::UnexpectedOutput(format!(
                "Failed to parse gh pr view --json comments response: {err}; raw: {raw}"
            ))
        })?;

        Ok(wrapper
            .comments
            .into_iter()
            .map(|c| PrComment {
                id: c.id,
                author: PrCommentAuthor {
                    login: c
                        .author
                        .and_then(|a| a.login)
                        .unwrap_or_else(|| "unknown".to_string()),
                },
                author_association: c.author_association,
                body: c.body,
                created_at: c.created_at.unwrap_or_else(Utc::now),
                url: c.url,
            })
            .collect())
    }

    fn parse_pr_review_comments(raw: &str) -> Result<Vec<PrReviewComment>, GhCliError> {
        let items: Vec<GhReviewCommentResponse> =
            serde_json::from_str(raw.trim()).map_err(|err| {
                GhCliError::UnexpectedOutput(format!(
                    "Failed to parse review comments API response: {err}; raw: {raw}"
                ))
            })?;

        Ok(items
            .into_iter()
            .map(|c| PrReviewComment {
                id: c.id,
                user: ReviewCommentUser {
                    login: c
                        .user
                        .and_then(|u| u.login)
                        .unwrap_or_else(|| "unknown".to_string()),
                },
                body: c.body,
                created_at: c.created_at.unwrap_or_else(Utc::now),
                html_url: c.html_url,
                path: c.path,
                line: c.line,
                side: c.side,
                diff_hunk: c.diff_hunk,
                author_association: c.author_association,
            })
            .collect())
    }
}

#[cfg(all(test, unix))]
mod credential_tests {
    use std::os::unix::fs::PermissionsExt;

    use utils::github_credentials::{Credential, Repository, Store};

    use super::*;

    #[test]
    fn scoped_cli_uses_token_without_arguments_or_fallback_and_redacts_failures() {
        let root = tempfile::tempdir().unwrap();
        let store = Store::at(root.path().join("credentials"));
        let repo = Repository::parse("owner/repo").unwrap();
        let token = Credential::new("github_pat_fake_secret".into()).unwrap();
        store.save(&repo, &token).unwrap();
        let cli = root.path().join("gh");
        std::fs::write(
            &cli,
            r#"#!/bin/sh
[ "$GH_TOKEN" = github_pat_fake_secret ] || exit 9
[ -d "$GH_CONFIG_DIR" ] || exit 9
[ "$GH_HOST" = github.com ] || exit 9
[ "$GH_PROMPT_DISABLED" = 1 ] || exit 9
case "$*" in *github_pat*) exit 9;; esac
printf 'authentication rejected %s' "$GH_TOKEN" >&2
exit 4
"#,
        )
        .unwrap();
        std::fs::set_permissions(&cli, std::fs::Permissions::from_mode(0o700)).unwrap();
        let result = GhCli::new().run_with_store(
            "https://github.com/owner/repo/pull/2",
            ["pr", "view"],
            None,
            &store,
            &cli,
        );
        let error = result.unwrap_err();
        assert!(matches!(error, GhCliError::AuthFailed(_)));
        assert!(!error.to_string().contains(token.token()));
        assert!(error.to_string().contains("[REDACTED]"));
    }

    #[test]
    fn unreadable_token_does_not_launch_cli() {
        let root = tempfile::tempdir().unwrap();
        let store = Store::at(root.path().into());
        std::fs::write(root.path().join("owner!repo.json"), "broken").unwrap();
        let result = GhCli::new().run_with_store(
            "owner/repo",
            ["pr", "list"],
            None,
            &store,
            Path::new("/does-not-exist"),
        );
        assert!(matches!(result, Err(GhCliError::AuthFailed(_))));
    }
}
