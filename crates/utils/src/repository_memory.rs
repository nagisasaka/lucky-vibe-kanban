//! Repository memory contracts shared by the host, executors and API.
//! Final records live in the instance DB. Recovery files use volume-backed managed
//! tmp, reproducible inventories use cache, and process locks use container /tmp.

use std::{
    fs::{self, File, OpenOptions},
    io::{self, Read, Write},
    path::{Path, PathBuf},
};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use ts_rs::TS;
use uuid::Uuid;

const MAX_RECORD_BYTES: u64 = 128 * 1024;
// A frozen Manifest plus its readable commit projection; other records keep their limit.
const MAX_SOURCE_PUBLICATION_BYTES: u64 = MAX_RECORD_BYTES * 4;
// Two bounded instruction files, with original/prepared/restoring JSON copies.
const MAX_INTEGRATION_SETUP_BYTES: u64 = 1024 * 1024;
/// Resource guard for full Bootstrap reports, independent of Workflow handoffs.
pub const MAX_BOOTSTRAP_REPORT_BYTES: usize = 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BootstrapReportKind {
    Review,
    Refine,
    ReReview,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct SemanticChanges {
    pub goal: String,
    pub summary: String,
    #[serde(default)]
    pub behavioral_changes: Vec<String>,
    #[serde(default)]
    pub architectural_changes: Vec<String>,
    #[serde(default)]
    pub invariants_affected: Vec<String>,
    #[serde(default)]
    pub decisions: Vec<MemoryDecision>,
    #[serde(default)]
    pub rejected_alternatives: Vec<RejectedAlternative>,
    #[serde(default)]
    pub unresolved_questions: Vec<String>,
    #[serde(default)]
    pub tests: Vec<MemoryTest>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct MemoryDecision {
    pub decision: String,
    pub rationale: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct RejectedAlternative {
    pub alternative: String,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct MemoryTest {
    pub command: Option<String>,
    pub result: MemoryTestResult,
    pub summary: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
pub enum MemoryTestResult {
    Passed,
    Failed,
    NotRun,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct ChangeManifest {
    pub version: u32,
    pub event_id: Uuid,
    pub repository_id: Uuid,
    pub workspace_id: Uuid,
    pub task_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub base_commit: String,
    pub source_commit: String,
    pub target_branch: Option<String>,
    pub changed_paths: Vec<String>,
    #[serde(flatten)]
    pub semantics: SemanticChanges,
}

/// Discovery deliberately returns references rather than unbounded semantic bodies. Publication continues to use the strict reader.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct ManifestReference {
    pub event_id: Uuid,
    pub repository_id: Uuid,
    pub workspace_id: Uuid,
    pub base_commit: String,
    pub source_commit: String,
    pub target_branch: Option<String>,
    pub created_at: DateTime<Utc>,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct ManifestDiscoveryError {
    pub filename: String,
    pub error: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, TS)]
pub struct ManifestDiscoveryPage {
    pub records: Vec<ManifestReference>,
    pub errors: Vec<ManifestDiscoveryError>,
    // Lexical filename cursor; errors also advance it, never hide later items.
    pub next_after: Option<String>,
}

impl ChangeManifest {
    pub fn validate(&self) -> io::Result<()> {
        if self.version != 1 || !commit_id(&self.base_commit) || !commit_id(&self.source_commit) {
            return Err(invalid(
                "Unsupported manifest version or invalid Git identity",
            ));
        }
        if self.changed_paths.iter().any(|path| {
            Path::new(path).is_absolute()
                || path.split(['/', '\\']).any(|part| part == "..")
                || path.contains('\0')
        }) {
            return Err(invalid("Invalid repository-relative changed path"));
        }
        self.semantics.validate()
    }
}

impl SemanticChanges {
    pub fn validate(&self) -> io::Result<()> {
        if self.summary.trim().is_empty()
            || self.summary.len() > 4096
            || self.goal.len() > 8192
            || serde_json::to_vec(self)?.len() > MAX_RECORD_BYTES as usize / 2
        {
            return Err(invalid(
                "Semantic summary is empty or exceeds the memory limit",
            ));
        }
        Ok(())
    }

    /// Project the same semantic record to a short human Git message. Never
    /// append a conversation transcript, evidence payload or machine metadata.
    pub fn commit_message(&self) -> String {
        self.summary
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .chars()
            .take(200)
            .collect()
    }
    /// Only the host's normal source commit uses this projection. The DB
    /// Manifest stays authoritative; Git bodies are never parsed back into it.
    pub fn source_commit_message(&self, run_id: Uuid, include_manifest: bool) -> String {
        let mut message = self.commit_message();
        if include_manifest {
            fn section(message: &mut String, title: &str, entries: Vec<String>) {
                let entries: Vec<_> = entries
                    .into_iter()
                    .filter(|s| !s.trim().is_empty())
                    .collect();
                if entries.is_empty() {
                    return;
                }
                message.push_str(&format!("\n\n{title}\n"));
                for entry in entries {
                    message.push_str("- ");
                    message.push_str(&entry.replace('\n', "\n  "));
                    message.push('\n');
                }
                message.pop();
            }
            section(&mut message, "Purpose", vec![self.goal.clone()]);
            // The subject has its historical 200-character projection. Preserve
            // the complete semantic summary in the body, including longer intent.
            if self.summary.trim() != self.commit_message() {
                section(&mut message, "Summary", vec![self.summary.clone()]);
            }
            section(
                &mut message,
                "Behavioral changes",
                self.behavioral_changes.clone(),
            );
            section(
                &mut message,
                "Architectural changes",
                self.architectural_changes.clone(),
            );
            section(&mut message, "Invariants", self.invariants_affected.clone());
            section(
                &mut message,
                "Decisions and reasons",
                self.decisions
                    .iter()
                    .map(|d| match &d.rationale {
                        Some(reason) if !reason.trim().is_empty() => {
                            format!("{}\nReason: {}", d.decision, reason)
                        }
                        _ => d.decision.clone(),
                    })
                    .collect(),
            );
            section(
                &mut message,
                "Rejected alternatives",
                self.rejected_alternatives
                    .iter()
                    .map(|a| match &a.reason {
                        Some(reason) if !reason.trim().is_empty() => {
                            format!("{}\nReason: {}", a.alternative, reason)
                        }
                        _ => a.alternative.clone(),
                    })
                    .collect(),
            );
            section(
                &mut message,
                "Unresolved questions",
                self.unresolved_questions.clone(),
            );
            section(
                &mut message,
                "Verification summary",
                self.tests
                    .iter()
                    .map(|t| {
                        let result = match t.result {
                            MemoryTestResult::Passed => "Passed",
                            MemoryTestResult::Failed => "Failed",
                            MemoryTestResult::NotRun => "Not run",
                        };
                        match t
                            .summary
                            .as_deref()
                            .filter(|s| !s.trim().is_empty())
                            .or(t.command.as_deref().filter(|c| !c.trim().is_empty()))
                        {
                            Some(summary) => format!("{result}: {summary}"),
                            None => result.to_owned(),
                        }
                    })
                    .collect(),
            );
        }
        format!("{message}\n\nLVK-Memory-Source: {run_id}")
    }
}

fn commit_id(value: &str) -> bool {
    matches!(value.len(), 40 | 64) && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ReconciliationResult {
    Updated,
    NoOp,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct WikiReconciliationReceipt {
    pub event_id: Uuid,
    pub reconciled_at: DateTime<Utc>,
    pub target_commit: String,
    pub wiki_commit: Option<String>,
    pub result: ReconciliationResult,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum RepositoryWikiStatus {
    Disabled,
    Uninitialized,
    Initializing,
    Current,
    Stale,
    Reconciling,
    Error,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RepositoryWikiSyncScope {
    #[default]
    PendingChanges,
    CurrentSource,
    SinceCommit {
        base_commit: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct RepositoryMemoryState {
    pub version: u32,
    pub enabled: bool,
    pub status: RepositoryWikiStatus,
    pub target_branch: Option<String>,
    pub source_commit: Option<String>,
    pub wiki_commit: Option<String>,
    pub last_success: Option<DateTime<Utc>>,
    pub active_run_id: Option<Uuid>,
    // Bootstrap's durable owner is the Workflow, not any individual child.
    #[serde(default)]
    pub bootstrap: Option<OpenWikiBootstrapOwner>,
    pub maintenance_workspace_id: Option<Uuid>,
    #[serde(default)]
    pub maintenance_session_id: Option<Uuid>,
    pub error: Option<String>,
    #[serde(default)]
    pub quality_warnings: Vec<WikiQualityWarning>,
    /// Existing monitor retries only this confirmed, no-longer-consumed artifact set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub artifact_cleanup: Option<RepositoryArtifactCleanup>,
    #[serde(default = "default_memory_language")]
    pub output_language: String,
    #[serde(default)]
    pub active_source_commit: Option<String>,
    #[serde(default)]
    pub active_event_ids: Vec<Uuid>,
    // Digest of host-frozen, run-scoped Sync hints; old in-flight runs retain
    // their already-frozen inline prompt instead.
    #[serde(default)]
    pub active_sync_input_digest: Option<String>,
    // Read-side diagnostics; these do not change canonical Wiki freshness.
    #[serde(default)]
    pub coding_errors: Vec<String>,
    /// Read-side detection on the saved target branch, independent of receipts.
    #[serde(default)]
    pub wiki_exists: bool,
    #[serde(default)]
    pub active_sync_scope: RepositoryWikiSyncScope,
    /// A scoped adoption did not verify older Wiki content. Automatic updates
    /// do not erase that limitation; a full reconciliation can clear it.
    #[serde(default)]
    pub unverified_before: Option<String>,
    /// Saved repository-file maintenance uses the same publication checkpoint.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub file_operation: Option<RepositoryWikiFileOperation>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub rebuild_reference: Option<String>,
    /// Explicit deletion suppresses automatic integrations until explicit init.
    #[serde(default)]
    pub generation_suppressed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct RepositoryArtifactCleanup {
    pub workspace_id: Uuid,
    pub bootstrap_run_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct RepositoryWikiFileOperation {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub source: String,
    pub branch: String,
    pub paths: Vec<String>,
    pub delete: bool,
}

fn default_memory_language() -> String {
    "en".into()
}

impl Default for RepositoryMemoryState {
    fn default() -> Self {
        Self {
            version: 1,
            enabled: false,
            status: RepositoryWikiStatus::Disabled,
            target_branch: None,
            source_commit: None,
            wiki_commit: None,
            last_success: None,
            active_run_id: None,
            bootstrap: None,
            quality_warnings: Vec::new(),
            maintenance_workspace_id: None,
            maintenance_session_id: None,
            error: None,
            artifact_cleanup: None,
            output_language: default_memory_language(),
            active_source_commit: None,
            active_event_ids: Vec::new(),
            active_sync_input_digest: None,
            coding_errors: Vec::new(),
            wiki_exists: false,
            active_sync_scope: RepositoryWikiSyncScope::default(),
            unverified_before: None,
            file_operation: None,
            rebuild_reference: None,
            generation_suppressed: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct OpenWikiBootstrapOwner {
    pub workflow_run_id: Uuid,
}

/// Quality is separate from publication safety and remains tied to its checkpoint.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct WikiQualityWarning {
    pub node_id: String,
    pub summary: String,
    pub findings: Vec<WikiQualityFinding>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct WikiQualityFinding {
    pub title: String,
    pub description: String,
    pub evidence_paths: Vec<String>,
}

impl RepositoryMemoryState {
    pub fn enabled_defaults(target_branch: Option<String>) -> Self {
        Self {
            enabled: true,
            status: RepositoryWikiStatus::Uninitialized,
            target_branch,
            ..Self::default()
        }
    }

    pub fn derived_status(
        &self,
        wiki_exists: bool,
        pending: bool,
        source_matches: bool,
    ) -> RepositoryWikiStatus {
        if !self.enabled {
            return RepositoryWikiStatus::Disabled;
        }
        if matches!(
            self.status,
            RepositoryWikiStatus::Initializing
                | RepositoryWikiStatus::Reconciling
                | RepositoryWikiStatus::Error
        ) {
            return self.status;
        }
        if !wiki_exists {
            return RepositoryWikiStatus::Uninitialized;
        }
        if self.status == RepositoryWikiStatus::Stale
            || self.unverified_before.is_some()
            || pending
            || !source_matches
            || self.last_success.is_none()
        {
            return RepositoryWikiStatus::Stale;
        }
        RepositoryWikiStatus::Current
    }
}

/// An integration record is written *before* source integration and confirmed
/// afterward, so a process crash cannot silently acknowledge lost event IDs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryIntegration {
    pub id: Uuid,
    pub event_ids: Vec<Uuid>,
    pub target_branch: String,
    pub before_commit: String,
    pub integrated_commit: Option<String>,
    #[serde(default)]
    pub source_error: Option<String>,
}

/// Written after host finalisation and validation, before publishing to the
/// integrated branch. This bridges Git publication and outbox acknowledgements.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WikiPublication {
    pub run_id: Uuid,
    pub source_commit: String,
    pub target_branch: String,
    pub maintenance_commit: Option<String>,
    pub no_op: bool,
    #[serde(default)]
    pub quality_warnings: Vec<WikiQualityWarning>,
}

/// Private filesystem journal, not part of the browser API. Original user text
/// stays recoverable even if setup or restoration is interrupted by a restart.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WikiSetupCheckpoint {
    pub version: u32,
    pub workspace_id: Uuid,
    pub repository_path: PathBuf,
    pub source_commit: String,
    pub phase: WikiSetupPhase,
    pub files: Vec<WikiSetupEntry>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WikiSetupPhase {
    Preparing,
    Prepared,
    Restoring,
    Restored,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WikiSetupEntry {
    pub path: String,
    pub original: WikiSetupFile,
    pub prepared: WikiSetupFile,
    pub restoring_from: Option<WikiSetupFile>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum WikiSetupFile {
    Missing,
    Regular { content: String, mode: u32 },
    Symlink { target: PathBuf },
}

/// Auto-merge setup is bound to the owning integration and validated code tree.
/// Separate from standalone journals: a missing journal is never reconstructed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IntegrationWikiSetup {
    pub version: u32,
    pub integration_id: Uuid,
    pub repository_id: Uuid,
    pub session_id: Uuid,
    pub storage_identity: String,
    pub prepared_at: DateTime<Utc>,
    pub final_commit: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub previous_final_commits: Vec<String>,
    pub checkpoint: WikiSetupCheckpoint,
}

/// Frozen before committing source. The manifest's source_commit is the
/// pre-publication HEAD until the verified Git result replaces it. Keeping the
/// semantic draft and expected tree here prevents a retry from attributing a
/// later task's changes or edited summary to the completed run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourcePublication {
    pub manifest: ChangeManifest,
    pub staged_tree: String,
    /// Frozen before Git mutation. Missing on older records: preserve the old compact message.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commit_message: Option<String>,
}

/// Frozen host identity for a coding run. The agent only authors its separate
/// semantic draft; revisions and membership cannot come from that draft.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepositoryMemoryRun {
    pub run_id: Uuid,
    pub repository_id: Uuid,
    pub workspace_id: Uuid,
    pub task_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub base_commit: String,
    pub target_branch: String,
    pub repository_path: PathBuf,
    pub draft_path: PathBuf,
    pub wiki_status: RepositoryWikiStatus,
    #[serde(default = "source_completion_default")]
    pub finalize_source: bool,
}

fn source_completion_default() -> bool {
    true
}

/// Fixed coding contract, delivered once even when a run has several repositories.
pub const CODING_RUN_INSTRUCTIONS: &str = "For source changes, read the Coding and Manifest section of the LVK on-demand guide file and write a JSON semantic draft at the CURRENT draft_path before finishing. Connect confirmed human goals, constraints and decisions to the implementation; distinguish your choices and never invent rationale. Use the same summary for any source commit. Successful normal coding Runs automatically commit source and bind the Manifest; shared drafts must not be Git committed. With no source changes, write no draft or dedicated verification record; report in the ordinary final response.";

/// Read on demand through the launch guide file or optional MCP context.
pub const CODING_RUN_GUIDE: &str = r#"Draft schema: {"goal":"current task intent","summary":"concise human commit message","behavioral_changes":[],"architectural_changes":[],"invariants_affected":[],"decisions":[{"decision":"...","rationale":"..."}],"rejected_alternatives":[{"alternative":"...","reason":"..."}],"unresolved_questions":[],"tests":[{"command":"...","result":"passed|failed|not-run","summary":"..."}]}. Omit empty optional arrays. Preserve confirmed human motivations and meaningful rejected alternatives; approval is authorization, not technical rationale. Tests are short performed/not-performed summaries with reasons, never invented results or logs. Development context stays in the coding Session, not Workspace Memory or another handoff store."#;

impl RepositoryMemoryRun {
    /// Only host-owned current identity is repository-specific. Keep this line
    /// format stable for current-run identification.
    pub fn identity_instructions(&self) -> String {
        let identity = serde_json::to_string(self).expect("memory context serializes");
        format!("LVK Change Manifest (CURRENT host identity JSON): {identity}")
    }
}

/// Refresh every repository identity, followed by one shared coding contract.
/// No attached coding repositories means no Manifest contract (e.g. maintenance).
pub fn coding_run_instructions(runs: &[RepositoryMemoryRun]) -> Option<String> {
    if runs.is_empty() {
        return None;
    }
    let mut parts: Vec<_> = runs
        .iter()
        .map(RepositoryMemoryRun::identity_instructions)
        .collect();
    parts.push("For each repository, its CURRENT identity supersedes earlier LVK run IDs and draft paths for that repository in this conversation. Use its exact draft_path, never a previous run's draft, even when continuing the same task. Run repository-scoped commands in its repository_path, not its parent workspace directory.".into());
    parts.push(CODING_RUN_INSTRUCTIONS.into());
    Some(parts.join("\n\n"))
}

pub fn validate_receipt_identity(value: &WikiReconciliationReceipt) -> io::Result<()> {
    if !commit_id(&value.target_commit)
        || value.wiki_commit.as_ref().is_some_and(|id| !commit_id(id))
    {
        return Err(invalid("Receipt contains invalid Git identity"));
    }
    Ok(())
}

pub fn validate_publication(value: &WikiPublication) -> io::Result<()> {
    if !commit_id(&value.source_commit)
        || value
            .maintenance_commit
            .as_ref()
            .is_some_and(|id| !commit_id(id))
    {
        return Err(invalid("Invalid Wiki publication Git identity"));
    }
    Ok(())
}

#[derive(Debug, Clone)]
pub struct RepositoryMemoryStore {
    root: PathBuf,
    inventory_root: PathBuf,
    lock_root: PathBuf,
}

/// Explicitly release ownership, not just its file descriptor. On Unix a
/// concurrent fork can briefly inherit the open file description before exec;
/// closing only the parent's descriptor then leaves flock held by that child.
#[derive(Debug)]
pub struct RepositoryMemoryLock(File);

impl Drop for RepositoryMemoryLock {
    fn drop(&mut self) {
        if let Err(error) = File::unlock(&self.0) {
            tracing::warn!(%error, "Failed to explicitly release repository-memory lock");
        }
    }
}

impl RepositoryMemoryStore {
    /// Resolve paths only: DB observers neither require nor create artifact storage.
    pub fn paths_for_repository(repo_id: Uuid) -> Self {
        Self::paths_at(
            crate::path::get_vibe_kanban_temp_dir_uninitialized(),
            repo_id,
        )
    }

    fn paths_at(runtime: PathBuf, repo_id: Uuid) -> Self {
        Self {
            root: runtime
                .join("tmp/repository-memory")
                .join(repo_id.to_string())
                .join("knowledge"),
            inventory_root: runtime
                .join("cache/repository-memory")
                .join(repo_id.to_string())
                .join("document-inventories"),
            lock_root: PathBuf::from("/tmp/lvk-repository-locks").join(repo_id.to_string()),
        }
    }

    pub fn for_repository(_repo_name: &str, repo_id: Uuid) -> io::Result<Self> {
        // Writers retain instance validation; DB-only observers never do filesystem I/O.
        let store = Self::paths_at(crate::path::get_vibe_kanban_temp_dir(), repo_id);
        store.provision()?;
        Ok(store)
    }

    /// Isolated fixture roots only; never resolves user shared storage.
    pub fn at_artifacts(base: &Path) -> io::Result<Self> {
        let store = Self {
            root: base.join("knowledge"),
            inventory_root: base.join("cache/document-inventories"),
            lock_root: base.join("container-tmp/locks"),
        };
        store.provision()?;
        Ok(store)
    }

    pub fn writable_artifact_roots(repo_id: Uuid) -> Vec<PathBuf> {
        let store = Self::paths_at(crate::path::get_vibe_kanban_temp_dir(), repo_id);
        vec![store.root, store.inventory_root]
    }

    /// Caller has confirmed durable completion and no remaining artifact readers.
    /// Locks are deliberately outside this tree and are never removed.
    pub fn cleanup_coding(&self, id: Uuid, keep_draft: bool) -> io::Result<()> {
        if !keep_draft {
            self.remove_artifact(&self.draft_path(id))?;
        }
        self.remove_artifact(
            &self
                .root
                .join("source-publications")
                .join(format!("{id}.json")),
        )
    }

    pub fn cleanup_wiki(&self, workspace: Uuid, bootstrap: Option<Uuid>) -> io::Result<()> {
        if let Some(setup) = self.wiki_setup(workspace)?
            && setup.phase != WikiSetupPhase::Restored
        {
            return Err(invalid(
                "Wiki original restoration is unfinished; recovery artifacts retained",
            ));
        }
        self.remove_artifact(
            &self
                .root
                .join("wiki-setups")
                .join(format!("{workspace}.json")),
        )?;
        self.remove_artifact(&self.root.join("sync-inputs").join(workspace.to_string()))?;
        if let Some(id) = bootstrap {
            self.remove_artifact(&self.inventory_root.join(id.to_string()))?;
            self.remove_artifact(&self.root.join("bootstrap-reports").join(id.to_string()))?;
        }
        Ok(())
    }

    pub fn cleanup_integration(&self, id: Uuid) -> io::Result<()> {
        if let Some(setup) = self.integration_wiki_setup(id)? {
            if setup.checkpoint.phase != WikiSetupPhase::Restored || setup.final_commit.is_none() {
                return Err(invalid(
                    "Integration original restoration is unfinished; journal retained",
                ));
            }
            self.remove_artifact(
                &self
                    .root
                    .join("integration-wiki-setups")
                    .join(format!("{id}.json")),
            )?;
        }
        Ok(())
    }

    fn remove_artifact(&self, path: &Path) -> io::Result<()> {
        // A substituted symlink is an error, not permission to clean its target.
        reject_symlinks(path)?;
        match fs::symlink_metadata(path) {
            Ok(meta) if meta.is_dir() => fs::remove_dir_all(path),
            Ok(meta) if meta.is_file() => fs::remove_file(path),
            Ok(_) => Err(invalid("Unexpected recovery artifact type; retained")),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e),
        }
    }

    fn provision(&self) -> io::Result<()> {
        managed_directory(&self.root)?;
        for directory in [
            "drafts",
            "source-publications",
            "wiki-setups",
            "integration-wiki-setups",
            "bootstrap-reports",
            "sync-inputs",
        ] {
            real_directory(&self.root.join(directory))?;
        }
        managed_directory(&self.inventory_root)
    }

    /// Maintenance workspace identity is unique per Sync; records are retained
    /// under the same managed artifact lifecycle as Bootstrap inputs.
    pub fn sync_input_path(&self, workspace_id: Uuid, chunk: Option<u32>) -> PathBuf {
        self.root
            .join("sync-inputs")
            .join(workspace_id.to_string())
            .join(chunk.map_or_else(|| "manifest.json".into(), |n| format!("chunk-{n:06}.json")))
    }

    pub fn save_sync_input<T: Serialize>(
        &self,
        workspace_id: Uuid,
        chunk: Option<u32>,
        value: &T,
    ) -> io::Result<()> {
        let path = self.sync_input_path(workspace_id, chunk);
        managed_directory(path.parent().unwrap())?;
        self.publish(&path, value, false)
    }

    pub fn read_sync_input(&self, workspace_id: Uuid, chunk: Option<u32>) -> io::Result<Vec<u8>> {
        self.read_optional(&self.sync_input_path(workspace_id, chunk))?
            .ok_or_else(|| invalid("Sync input record is missing"))
    }

    /// Bootstrap inputs use the same atomic, bounded I/O in managed cache.
    /// Trust/digests belong to the host's Workflow input, not to this writable folder.
    pub fn document_inventory_path(&self, run_id: Uuid, chunk: Option<u32>) -> PathBuf {
        self.inventory_root
            .join(run_id.to_string())
            .join(chunk.map_or_else(|| "manifest.json".into(), |n| format!("chunk-{n:06}.json")))
    }

    pub fn save_document_inventory<T: Serialize>(
        &self,
        run_id: Uuid,
        chunk: Option<u32>,
        value: &T,
    ) -> io::Result<()> {
        let path = self.document_inventory_path(run_id, chunk);
        managed_directory(path.parent().unwrap())?;
        self.publish(&path, value, false)
    }

    pub fn read_document_inventory(&self, run_id: Uuid, chunk: Option<u32>) -> io::Result<Vec<u8>> {
        self.read_optional(&self.document_inventory_path(run_id, chunk))?
            .ok_or_else(|| invalid("Document inventory record is missing"))
    }

    pub fn bootstrap_report_path(&self, run_id: Uuid, kind: BootstrapReportKind) -> PathBuf {
        self.root
            .join("bootstrap-reports")
            .join(run_id.to_string())
            .join(match kind {
                BootstrapReportKind::Review => "review.json",
                BootstrapReportKind::Refine => "refine.json",
                BootstrapReportKind::ReReview => "re-review.json",
            })
    }

    /// Host-published once, retained until the owning Workflow no longer needs it.
    /// The caller freezes its digest in NodeExecution, outside agent-writable files.
    pub fn save_bootstrap_report<T: Serialize>(
        &self,
        run_id: Uuid,
        kind: BootstrapReportKind,
        report: &T,
    ) -> io::Result<()> {
        let path = self.bootstrap_report_path(run_id, kind);
        managed_directory(path.parent().unwrap())?;
        self.publish_limited(&path, report, false, MAX_BOOTSTRAP_REPORT_BYTES as u64)
    }

    pub fn read_bootstrap_report(
        &self,
        run_id: Uuid,
        kind: BootstrapReportKind,
    ) -> io::Result<Vec<u8>> {
        self.read_optional_limited(
            &self.bootstrap_report_path(run_id, kind),
            MAX_BOOTSTRAP_REPORT_BYTES as u64,
        )?
        .ok_or_else(|| invalid("Bootstrap report is missing"))
    }

    pub fn wiki_setup(&self, workspace_id: Uuid) -> io::Result<Option<WikiSetupCheckpoint>> {
        self.read_json(
            &self
                .root
                .join("wiki-setups")
                .join(format!("{workspace_id}.json")),
        )
    }

    /// Caller holds the repository maintenance lock. Unlike semantic events,
    /// this journal advances through preparation and restoration checkpoints.
    pub fn save_wiki_setup(&self, checkpoint: &WikiSetupCheckpoint) -> io::Result<()> {
        self.publish(
            &self
                .root
                .join("wiki-setups")
                .join(format!("{}.json", checkpoint.workspace_id)),
            checkpoint,
            true,
        )
    }

    pub fn integration_wiki_setup(&self, id: Uuid) -> io::Result<Option<IntegrationWikiSetup>> {
        self.read_optional_limited(
            &self
                .root
                .join("integration-wiki-setups")
                .join(format!("{id}.json")),
            MAX_INTEGRATION_SETUP_BYTES,
        )?
        .map(|bytes| serde_json::from_slice(&bytes).map_err(invalid))
        .transpose()
    }

    /// Caller holds the workspace completion lock; atomic, synced journal replacement.
    pub fn save_integration_wiki_setup(&self, setup: &IntegrationWikiSetup) -> io::Result<()> {
        self.publish_limited(
            &self
                .root
                .join("integration-wiki-setups")
                .join(format!("{}.json", setup.integration_id)),
            setup,
            true,
            MAX_INTEGRATION_SETUP_BYTES,
        )
    }

    pub fn draft_path(&self, run_id: Uuid) -> PathBuf {
        self.root.join("drafts").join(format!("{run_id}.json"))
    }

    pub fn read_draft(&self, run_id: Uuid) -> io::Result<Option<SemanticChanges>> {
        let result: Option<SemanticChanges> = self.read_json(&self.draft_path(run_id))?;
        if let Some(semantics) = &result {
            semantics.validate()?;
        }
        Ok(result)
    }

    pub fn source_publication(&self, run_id: Uuid) -> io::Result<Option<SourcePublication>> {
        let publication: Option<SourcePublication> = self
            .read_optional_limited(
                &self
                    .root
                    .join("source-publications")
                    .join(format!("{run_id}.json")),
                MAX_SOURCE_PUBLICATION_BYTES,
            )?
            .map(|bytes| serde_json::from_slice(&bytes).map_err(invalid))
            .transpose()?;
        if let Some(publication) = &publication {
            publication.manifest.validate()?;
            if publication.manifest.event_id != run_id || !commit_id(&publication.staged_tree) {
                return Err(invalid(
                    "Source publication identity does not match its file",
                ));
            }
        }
        Ok(publication)
    }

    pub fn save_source_publication(&self, publication: &SourcePublication) -> io::Result<()> {
        publication.manifest.validate()?;
        if !commit_id(&publication.staged_tree) {
            return Err(invalid("Invalid source publication tree"));
        }
        self.publish_limited(
            &self
                .root
                .join("source-publications")
                .join(format!("{}.json", publication.manifest.event_id)),
            publication,
            false,
            MAX_SOURCE_PUBLICATION_BYTES,
        )
    }

    /// OS lock, released on process death. Durable active_run_id must also be
    /// reconciled before starting a replacement maintenance agent after restart.
    pub fn try_lock(&self) -> io::Result<RepositoryMemoryLock> {
        self.lock_file("wiki-reconcile.lock")
    }

    /// Held briefly around source/Wiki publication, never while models run.
    pub fn try_integration_lock(&self) -> io::Result<RepositoryMemoryLock> {
        self.lock_file("source-integration.lock")
    }

    /// Duplicate terminal notifications and merge-time recovery must not both
    /// finalise the same workspace. Other workspaces retain independent locks.
    pub fn try_workspace_completion_lock(
        &self,
        workspace_id: Uuid,
    ) -> io::Result<RepositoryMemoryLock> {
        self.lock_file(&format!("source-completion-{workspace_id}.lock"))
    }

    fn lock_file(&self, name: &str) -> io::Result<RepositoryMemoryLock> {
        let directory = &self.lock_root;
        managed_directory(directory)?;
        let path = directory.join(name);
        reject_symlinks(&path)?;
        let mut options = OpenOptions::new();
        options.read(true).write(true).create(true).truncate(false);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options
                .mode(0o600)
                .custom_flags(nix::libc::O_NOFOLLOW | nix::libc::O_NONBLOCK);
        }
        let file = options.open(path)?;
        if !file.metadata()?.is_file() {
            return Err(invalid("Repository lock is not a regular file"));
        }
        file.try_lock().map_err(|error| match error {
            std::fs::TryLockError::WouldBlock => io::Error::from(io::ErrorKind::WouldBlock),
            std::fs::TryLockError::Error(error) => error,
        })?;
        Ok(RepositoryMemoryLock(file))
    }

    fn read_json<T: DeserializeOwned>(&self, path: &Path) -> io::Result<Option<T>> {
        self.read_optional(path)?
            .map(|bytes| serde_json::from_slice(&bytes).map_err(invalid))
            .transpose()
    }

    fn read_optional(&self, path: &Path) -> io::Result<Option<Vec<u8>>> {
        self.read_optional_limited(path, MAX_RECORD_BYTES)
    }

    fn read_optional_limited(&self, path: &Path, limit: u64) -> io::Result<Option<Vec<u8>>> {
        reject_symlinks(path)?;
        let mut options = OpenOptions::new();
        options.read(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            // An agent-writable shared record must not block a phase boundary
            // when replaced with a FIFO, or follow a swapped final symlink.
            options.custom_flags(nix::libc::O_NOFOLLOW | nix::libc::O_NONBLOCK);
        }
        let file = match options.open(path) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error),
        };
        if !file.metadata()?.is_file() {
            return Err(invalid("Memory record is not a regular file"));
        }
        let mut bytes = Vec::new();
        file.take(limit + 1).read_to_end(&mut bytes)?;
        if bytes.len() > limit as usize {
            return Err(invalid("Memory record is too large"));
        }
        Ok(Some(bytes))
    }

    fn publish<T: Serialize>(&self, path: &Path, value: &T, replace: bool) -> io::Result<()> {
        self.publish_limited(path, value, replace, MAX_RECORD_BYTES)
    }

    fn publish_limited<T: Serialize>(
        &self,
        path: &Path,
        value: &T,
        replace: bool,
        limit: u64,
    ) -> io::Result<()> {
        reject_symlinks(path)?;
        let bytes = serde_json::to_vec_pretty(value)?;
        if bytes.len() > limit as usize {
            return Err(invalid("Memory record is too large"));
        }
        if !replace && let Some(existing) = self.read_optional_limited(path, limit)? {
            return if existing == bytes {
                Ok(())
            } else {
                Err(invalid("Immutable event identity conflict"))
            };
        }
        let mut temporary = tempfile::NamedTempFile::new_in(
            path.parent()
                .ok_or_else(|| invalid("Missing record parent"))?,
        )?;
        temporary.write_all(&bytes)?;
        temporary.as_file().sync_all()?;
        if replace {
            temporary.persist(path).map_err(|error| error.error)?;
        } else if let Err(error) = temporary.persist_noclobber(path)
            && (error.error.kind() != io::ErrorKind::AlreadyExists
                || self.read_optional_limited(path, limit)?.as_ref() != Some(&bytes))
        {
            return Err(error.error);
        }
        #[cfg(unix)]
        File::open(path.parent().unwrap())?.sync_all()?;
        Ok(())
    }
}

fn managed_directory(path: &Path) -> io::Result<()> {
    reject_symlinks(path)?;
    let mut builder = fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(path)?;
    reject_symlinks(path)
}

fn real_directory(path: &Path) -> io::Result<()> {
    reject_symlinks(path)?;
    match fs::create_dir(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists && path.is_dir() => Ok(()),
        Err(error) => Err(error),
    }
}

pub fn reject_symlinks(path: &Path) -> io::Result<()> {
    for component in path.ancestors() {
        match fs::symlink_metadata(component) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(invalid(
                    "Symlinks are not allowed in repository memory storage",
                ));
            }
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
    }
    Ok(())
}

fn invalid(error: impl std::fmt::Display) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coding_guidance_keeps_each_current_identity_and_one_contract() {
        let run = |repository_id: Uuid, run_id: Uuid| RepositoryMemoryRun {
            run_id,
            repository_id,
            workspace_id: Uuid::nil(),
            task_id: None,
            created_at: Utc::now(),
            base_commit: "a".repeat(40),
            target_branch: "main".into(),
            repository_path: PathBuf::from(format!("/workspace/{repository_id}")),
            draft_path: PathBuf::from(format!("/drafts/{repository_id}/{run_id}.json")),
            wiki_status: RepositoryWikiStatus::Current,
            finalize_source: true,
        };
        assert!(coding_run_instructions(&[]).is_none());
        let repositories = [Uuid::new_v4(), Uuid::new_v4()];
        let initial = repositories.map(|repo| run(repo, Uuid::new_v4()));
        let continued = repositories.map(|repo| run(repo, Uuid::new_v4()));
        for runs in [&initial, &continued] {
            let instructions = coding_run_instructions(runs).unwrap();
            assert_eq!(instructions.matches(CODING_RUN_INSTRUCTIONS).count(), 1);
            let identities: Vec<RepositoryMemoryRun> = instructions
                .lines()
                .filter_map(|line| {
                    line.strip_prefix("LVK Change Manifest (CURRENT host identity JSON): ")
                })
                .map(|json| serde_json::from_str(json).unwrap())
                .collect();
            assert_eq!(identities.len(), 2);
            for (actual, expected) in identities.iter().zip(runs.iter()) {
                assert_eq!(actual.run_id, expected.run_id);
                assert_eq!(actual.repository_id, expected.repository_id);
                assert_eq!(actual.draft_path, expected.draft_path);
            }
        }
        let next = coding_run_instructions(&continued).unwrap();
        for old in &initial {
            assert!(!next.contains(&old.run_id.to_string()));
        }
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn managed_artifacts_survive_container_replacement_and_user_cleanup() {
        let temp = tempfile::tempdir().unwrap();
        let base = temp.path().join("volume-tmp");
        let store = RepositoryMemoryStore::at_artifacts(&base).unwrap();
        let user = temp.path().join("shared");
        for kind in ["persistent", "tmp", "cache"] {
            fs::create_dir_all(user.join(kind)).unwrap();
        }
        fs::write(user.join("persistent/result"), b"user artifact").unwrap();
        let id = Uuid::new_v4();
        let publication = SourcePublication {
            manifest: fixture_event(),
            staged_tree: "c".repeat(40),
            commit_message: None,
        };
        store.save_source_publication(&publication).unwrap();
        store
            .save_sync_input(id, None, &serde_json::json!({"frozen":true}))
            .unwrap();
        let guard = store.try_lock().unwrap();
        crate::shared_storage::clear(&user, crate::shared_storage::SharedStorageKind::Tmp).unwrap();
        crate::shared_storage::clear(&user, crate::shared_storage::SharedStorageKind::Cache)
            .unwrap();
        assert!(store.try_lock().is_err());
        drop(guard);
        // Container /tmp disappears only after processes exit. Recovery volume stays.
        fs::remove_dir_all(base.join("container-tmp")).unwrap();
        let reopened = RepositoryMemoryStore::at_artifacts(&base).unwrap();
        assert!(
            reopened
                .source_publication(publication.manifest.event_id)
                .unwrap()
                .is_some()
        );
        assert!(!reopened.read_sync_input(id, None).unwrap().is_empty());
        let guard = reopened.try_lock().unwrap();
        reopened
            .cleanup_coding(publication.manifest.event_id, false)
            .unwrap();
        assert!(
            reopened.try_lock().is_err(),
            "cleanup must not replace a held lock inode"
        );
        drop(guard);
        assert_eq!(
            fs::read(user.join("persistent/result")).unwrap(),
            b"user artifact"
        );
    }

    #[test]
    fn originals_are_retained_until_restored_and_cleanup_refuses_symlink_roots() {
        let temp = tempfile::tempdir().unwrap();
        let store = RepositoryMemoryStore::at_artifacts(temp.path()).unwrap();
        let id = Uuid::new_v4();
        let mut setup = WikiSetupCheckpoint {
            version: 1,
            workspace_id: id,
            repository_path: temp.path().join("worktree"),
            source_commit: "a".repeat(40),
            phase: WikiSetupPhase::Prepared,
            files: vec![],
        };
        store.save_wiki_setup(&setup).unwrap();
        assert!(store.cleanup_wiki(id, None).is_err());
        assert!(store.wiki_setup(id).unwrap().is_some());
        setup.phase = WikiSetupPhase::Restored;
        store.save_wiki_setup(&setup).unwrap();
        store.cleanup_wiki(id, None).unwrap();
        assert!(store.wiki_setup(id).unwrap().is_none());
        store.cleanup_wiki(id, None).unwrap();
        #[cfg(unix)]
        {
            let outside = temp.path().join("user-artifacts");
            fs::create_dir(&outside).unwrap();
            fs::write(outside.join("keep"), b"user").unwrap();
            let input = store.root.join("sync-inputs").join(id.to_string());
            std::os::unix::fs::symlink(&outside, &input).unwrap();
            assert!(store.cleanup_wiki(id, None).is_err());
            assert_eq!(fs::read(outside.join("keep")).unwrap(), b"user");
        }
    }

    #[test]
    fn subprocess_lock_probe() {
        let Ok(base) = std::env::var("LVK_TEST_LOCK_BASE") else {
            return;
        };
        let store = RepositoryMemoryStore::at_artifacts(Path::new(&base)).unwrap();
        assert_eq!(
            store.try_lock().is_ok(),
            std::env::var("LVK_TEST_LOCK_AVAILABLE").unwrap() == "yes"
        );
    }

    #[test]
    fn repo_locks_are_shared_by_processes_and_never_deleted_during_cleanup() {
        let temp = tempfile::tempdir().unwrap();
        let store = RepositoryMemoryStore::at_artifacts(temp.path()).unwrap();
        let probe = |available| {
            assert!(
                std::process::Command::new(std::env::current_exe().unwrap())
                    .args(["--exact", "repository_memory::tests::subprocess_lock_probe"])
                    .env("LVK_TEST_LOCK_BASE", temp.path())
                    .env("LVK_TEST_LOCK_AVAILABLE", available)
                    .status()
                    .unwrap()
                    .success()
            );
        };
        let lock = store.try_lock().unwrap();
        probe("no");
        store.cleanup_coding(Uuid::new_v4(), false).unwrap();
        probe("no");
        drop(lock);
        probe("yes");
    }

    #[test]
    fn full_report_limit_does_not_expand_other_shared_records() {
        let temp = tempfile::tempdir().unwrap();
        let store = RepositoryMemoryStore::at_artifacts(temp.path()).unwrap();
        let run = Uuid::new_v4();
        let large = "x".repeat(160 * 1024);
        store
            .save_bootstrap_report(run, BootstrapReportKind::Review, &large)
            .unwrap();
        assert!(
            store
                .read_bootstrap_report(run, BootstrapReportKind::Review)
                .unwrap()
                .len()
                > 128 * 1024
        );
        assert!(store.save_document_inventory(run, None, &large).is_err());
        let too_large = "x".repeat(MAX_BOOTSTRAP_REPORT_BYTES + 1);
        assert!(
            store
                .save_bootstrap_report(run, BootstrapReportKind::Refine, &too_large)
                .is_err()
        );
        assert!(
            !store
                .bootstrap_report_path(run, BootstrapReportKind::Refine)
                .exists()
        );
        std::fs::write(
            store.bootstrap_report_path(run, BootstrapReportKind::Review),
            &too_large,
        )
        .unwrap();
        assert!(
            store
                .read_bootstrap_report(run, BootstrapReportKind::Review)
                .is_err()
        );
    }

    #[test]
    fn changed_branch_without_wiki_is_uninitialized_despite_previous_success() {
        let state = RepositoryMemoryState {
            enabled: true,
            status: RepositoryWikiStatus::Stale,
            last_success: Some(Utc::now()),
            ..Default::default()
        };
        assert_eq!(
            state.derived_status(false, false, false),
            RepositoryWikiStatus::Uninitialized
        );
        assert_eq!(
            state.derived_status(true, false, true),
            RepositoryWikiStatus::Stale
        );
    }

    #[test]
    fn partial_adoption_does_not_advertise_whole_wiki_freshness() {
        let mut state = RepositoryMemoryState {
            status: RepositoryWikiStatus::Current,
            last_success: Some(Utc::now()),
            unverified_before: Some("a".repeat(40)),
            ..RepositoryMemoryState::enabled_defaults(Some("main".into()))
        };
        assert_eq!(
            state.derived_status(true, false, true),
            RepositoryWikiStatus::Stale
        );
        state.unverified_before = None;
        assert_eq!(
            state.derived_status(true, false, true),
            RepositoryWikiStatus::Current
        );
    }

    fn fixture_event() -> ChangeManifest {
        ChangeManifest {
            version: 1,
            event_id: Uuid::new_v4(),
            repository_id: Uuid::new_v4(),
            workspace_id: Uuid::new_v4(),
            task_id: None,
            created_at: Utc::now(),
            base_commit: "a".repeat(40),
            source_commit: "b".repeat(40),
            target_branch: Some("main".into()),
            changed_paths: vec!["src/app.rs".into()],
            semantics: SemanticChanges {
                summary: "fix: preserve repository boundaries".into(),
                ..Default::default()
            },
        }
    }

    #[test]
    fn workspace_completion_locks_are_independent() {
        let temp = tempfile::tempdir().unwrap();
        let store = RepositoryMemoryStore::at_artifacts(temp.path()).unwrap();
        let a = Uuid::new_v4();
        let lock = store.try_workspace_completion_lock(a).unwrap();
        assert_eq!(
            store.try_workspace_completion_lock(a).unwrap_err().kind(),
            io::ErrorKind::WouldBlock
        );
        assert!(store.try_workspace_completion_lock(Uuid::new_v4()).is_ok());
        drop(lock);
        assert!(store.try_workspace_completion_lock(a).is_ok());
    }

    #[test]
    fn dropping_guard_unlocks_even_with_an_inherited_open_file_description() {
        let temp = tempfile::tempdir().unwrap();
        let store = RepositoryMemoryStore::at_artifacts(temp.path()).unwrap();
        let guard = store.try_lock().unwrap();
        // dup shares flock ownership just like a descriptor inherited by fork.
        let inherited = guard.0.try_clone().unwrap();
        assert_eq!(
            store.try_lock().unwrap_err().kind(),
            io::ErrorKind::WouldBlock
        );
        drop(guard);
        let next = store
            .try_lock()
            .expect("the old child's fd must not retain ownership");
        drop(inherited);
        assert_eq!(
            store.try_lock().unwrap_err().kind(),
            io::ErrorKind::WouldBlock
        );
        drop(next);
        assert!(store.try_lock().is_ok());
    }

    #[test]
    fn readable_source_message_preserves_semantics_without_metadata_or_test_payloads() {
        let mut manifest = fixture_event();
        let reason = format!("{}最後の重要な設計理由", "理由\n".repeat(7600));
        manifest.semantics = SemanticChanges {
            goal: "GitHub上でも人とAIが変更の目的を読めるようにする".into(),
            summary: format!("feat: preserve intent {}", "意図".repeat(120)),
            behavioral_changes: vec!["既定OFF、ONで意味情報を併記".into()],
            architectural_changes: vec!["ファイルManifestを正本にする".into()],
            invariants_affected: vec!["再試行では固定済み本文を維持".into()],
            decisions: vec![MemoryDecision {
                decision: "本文を固定する".into(),
                rationale: Some(reason.clone()),
            }],
            rejected_alternatives: vec![RejectedAlternative {
                alternative: "本文からManifestを復元".into(),
                reason: Some("二重管理を避ける".into()),
            }],
            unresolved_questions: vec!["実運用での効果は未検証".into()],
            tests: vec![
                MemoryTest {
                    command: Some("do not copy detailed command payload".into()),
                    result: MemoryTestResult::Passed,
                    summary: Some("代表例が成功".into()),
                },
                MemoryTest {
                    command: None,
                    result: MemoryTestResult::NotRun,
                    summary: Some("実GitHubは範囲外".into()),
                },
                MemoryTest {
                    command: Some("cargo test --lib".into()),
                    result: MemoryTestResult::Passed,
                    summary: None,
                },
                MemoryTest {
                    command: Some("pnpm run check".into()),
                    result: MemoryTestResult::Failed,
                    summary: Some(" \n\t".into()),
                },
                MemoryTest {
                    command: Some(" \t".into()),
                    result: MemoryTestResult::NotRun,
                    summary: Some(" \n".into()),
                },
            ],
        };
        manifest.validate().unwrap();
        let short = manifest
            .semantics
            .source_commit_message(manifest.event_id, false);
        assert_eq!(
            short,
            format!(
                "{}\n\nLVK-Memory-Source: {}",
                manifest.semantics.commit_message(),
                manifest.event_id
            )
        );
        let message = manifest
            .semantics
            .source_commit_message(manifest.event_id, true);
        assert_eq!(message.lines().next(), short.lines().next());
        for value in [
            &manifest.semantics.goal,
            &manifest.semantics.summary,
            &reason,
            &manifest.semantics.behavioral_changes[0],
            &manifest.semantics.architectural_changes[0],
            &manifest.semantics.invariants_affected[0],
            &manifest.semantics.unresolved_questions[0],
        ] {
            assert!(message.contains(&value.replace('\n', "\n  ")));
        }
        assert!(message.contains("二重管理を避ける"));
        assert!(message.contains("Passed: 代表例が成功"));
        assert!(message.contains("Not run: 実GitHubは範囲外"));
        assert!(message.contains("Passed: cargo test --lib"));
        assert!(message.contains("Failed: pnpm run check"));
        assert!(message.ends_with(&format!(
            "\n- Not run\n\nLVK-Memory-Source: {}",
            manifest.event_id
        )));
        assert!(!message.contains("detailed command payload"));
        assert!(!message.contains(&manifest.source_commit));
        assert!(!message.contains(&manifest.base_commit));
        let temp = tempfile::tempdir().unwrap();
        let store = RepositoryMemoryStore::at_artifacts(temp.path()).unwrap();
        let publication = SourcePublication {
            staged_tree: "c".repeat(40),
            commit_message: Some(message.clone()),
            manifest,
        };
        assert!(serde_json::to_vec(&publication).unwrap().len() > MAX_RECORD_BYTES as usize);
        store.save_source_publication(&publication).unwrap();
        assert_eq!(
            store
                .source_publication(publication.manifest.event_id)
                .unwrap()
                .unwrap()
                .commit_message,
            Some(message)
        );
    }

    #[test]
    fn manifest_validation_and_commit_projection() {
        let mut a = fixture_event();
        a.validate().unwrap();
        assert_eq!(
            a.semantics.commit_message(),
            "fix: preserve repository boundaries"
        );
        a.changed_paths.push("../secret".into());
        assert!(a.validate().is_err());
        a.changed_paths.clear();
        a.version = 2;
        assert!(a.validate().is_err());
        assert!(
            serde_json::from_str::<SemanticChanges>(
                r#"{"goal":"x","summary":"y","transcript":"raw"}"#
            )
            .is_err()
        );
    }

    #[test]
    fn current_requires_successful_reconciliation() {
        let mut state = RepositoryMemoryState {
            enabled: true,
            status: RepositoryWikiStatus::Current,
            ..Default::default()
        };
        assert_eq!(
            state.derived_status(true, false, true),
            RepositoryWikiStatus::Stale
        );
        state.last_success = Some(Utc::now());
        assert_eq!(
            state.derived_status(true, false, true),
            RepositoryWikiStatus::Current
        );
        assert_eq!(
            state.derived_status(true, true, true),
            RepositoryWikiStatus::Stale
        );
        assert_eq!(
            state.derived_status(true, false, false),
            RepositoryWikiStatus::Stale
        );
    }

    #[cfg(unix)]
    #[test]
    fn refuses_fifo_inventory_without_waiting_for_a_writer() {
        use std::{ffi::CString, os::unix::ffi::OsStrExt};
        let temp = tempfile::tempdir().unwrap();
        let store = RepositoryMemoryStore::at_artifacts(temp.path()).unwrap();
        let run = Uuid::new_v4();
        store
            .save_document_inventory(run, None, &serde_json::json!({}))
            .unwrap();
        let path = store.document_inventory_path(run, None);
        fs::remove_file(&path).unwrap();
        let path = CString::new(path.as_os_str().as_bytes()).unwrap();
        // SAFETY: the path is a valid, terminated C string for a test-owned path.
        assert_eq!(unsafe { nix::libc::mkfifo(path.as_ptr(), 0o600) }, 0);
        assert!(store.read_document_inventory(run, None).is_err());
    }
}
