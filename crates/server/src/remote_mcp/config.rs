use std::collections::BTreeSet;

use anyhow::{Context, ensure};
use serde::Deserialize;
use url::Url;
use uuid::Uuid;

/// Operator-owned configuration, never supplied by a tool caller.
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub public_base_url: String,
    pub issuer: String,
    pub introspection_endpoint: String,
    pub introspection_client_id: String,
    pub introspection_secret_env: String,
    pub owner_subject: String,
    pub project_ids: BTreeSet<Uuid>,
}

impl Config {
    pub fn validate(&self) -> anyhow::Result<()> {
        for value in [
            &self.public_base_url,
            &self.issuer,
            &self.introspection_endpoint,
        ] {
            let url = Url::parse(value).context("MCP configuration requires absolute URLs")?;
            ensure!(
                url.scheme() == "https" && url.host_str().is_some(),
                "MCP URLs must use HTTPS"
            );
            ensure!(
                url.username().is_empty()
                    && url.password().is_none()
                    && url.query().is_none()
                    && url.fragment().is_none(),
                "MCP URLs must not contain credentials, query or fragment"
            );
        }
        ensure!(
            !self.public_base_url.ends_with('/'),
            "public_base_url must not end with a slash"
        );
        ensure!(
            !self.owner_subject.trim().is_empty(),
            "MCP owner_subject is required"
        );
        ensure!(
            !self.project_ids.is_empty() && self.project_ids.len() <= 500,
            "MCP requires an explicit allowlist of 1 to 500 projects"
        );
        ensure!(
            !self.introspection_client_id.trim().is_empty(),
            "MCP introspection client is required"
        );
        ensure!(
            !self.introspection_secret_env.is_empty()
                && self
                    .introspection_secret_env
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '_'),
            "Invalid introspection secret environment variable name"
        );
        Ok(())
    }

    pub fn resource(&self) -> String {
        format!("{}/mcp", self.public_base_url)
    }
    pub fn metadata_url(&self) -> String {
        format!(
            "{}/.well-known/oauth-protected-resource/mcp",
            self.public_base_url
        )
    }
    pub fn project_url(&self, id: Uuid) -> String {
        format!("{}/projects/{id}", self.public_base_url)
    }
}
