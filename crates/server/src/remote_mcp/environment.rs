//! Trusted runtime access. Direct paths have an operational boundary, not an OS sandbox.
use std::path::{Component, Path, PathBuf};

use utils::mcp_error::Category;

macro_rules! preflight {
    ($category:expr, $condition:expr, $message:expr $(,)?) => {
        if !$condition {
            return Err(super::errors::rejection_with_message($category, $message));
        }
    };
}
pub(super) use preflight;

/// Only call before a runtime side effect has been attempted.
pub(super) fn preflight_error(error: anyhow::Error) -> anyhow::Error {
    super::errors::diagnostic(&error, false).into()
}
use rmcp::model::Tool;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use ts_rs::TS;
use uuid::Uuid;

use super::{Remote, auth::Principal, read::arguments};

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct RuntimeAccessSettings {
    pub files_enabled: bool,
    pub execution_enabled: bool,
    pub file_roots: Vec<PathBuf>,
}
impl Default for RuntimeAccessSettings {
    fn default() -> Self {
        Self {
            files_enabled: true,
            execution_enabled: true,
            file_roots: vec!["/".into()],
        }
    }
}
impl RuntimeAccessSettings {
    pub fn validate_paths(&self) -> anyhow::Result<()> {
        preflight!(
            Category::Validation,
            !self.file_roots.is_empty() && self.file_roots.len() <= 64,
            "Configure 1 to 64 file roots"
        );
        preflight!(
            Category::Validation,
            self.file_roots
                .iter()
                .all(|root| root.is_absolute()
                    && !root.components().any(|c| c == Component::ParentDir)),
            "File root must be absolute without traversal"
        );
        Ok(())
    }
    pub fn validate(&mut self) -> anyhow::Result<()> {
        self.validate_paths()?;
        for root in &mut self.file_roots {
            *root = root.canonicalize()?;
            preflight!(
                Category::Validation,
                root.is_dir(),
                "File root must be a directory"
            );
        }
        self.file_roots.sort();
        self.file_roots.dedup();
        Ok(())
    }
}

pub(super) struct Environment {
    pub settings: RuntimeAccessSettings,
    pub settings_pool: Option<sqlx::SqlitePool>,
    #[cfg(target_os = "linux")]
    pub processes: super::environment_process::ProcessRegistry,
}
impl Environment {
    pub fn managed(pool: sqlx::SqlitePool) -> Self {
        Self {
            settings: Default::default(),
            settings_pool: Some(pool),
            #[cfg(target_os = "linux")]
            processes: Default::default(),
        }
    }
    #[cfg(test)]
    pub fn new(mut settings: RuntimeAccessSettings) -> anyhow::Result<Self> {
        settings.validate()?;
        Ok(Self {
            settings,
            settings_pool: None,
            #[cfg(target_os = "linux")]
            processes: Default::default(),
        })
    }
    pub(super) async fn current_settings(&self) -> anyhow::Result<RuntimeAccessSettings> {
        if let Some(pool) = &self.settings_pool {
            Ok(super::environment_settings::load(pool).await?.2)
        } else {
            Ok(self.settings.clone())
        }
    }
}

fn tool(
    name: &str,
    description: &str,
    properties: Value,
    required: &[&str],
    mutation: bool,
    scope: &str,
) -> Tool {
    serde_json::from_value(json!({"name":name,"description":description,"inputSchema":{"type":"object","properties":properties,"required":required,"additionalProperties":false},"annotations":{"readOnlyHint":!mutation,"destructiveHint":mutation,"idempotentHint":true,"openWorldHint":mutation},"_meta":{"securitySchemes":[{"type":"oauth2","scopes":if scope == "lvk:runtime:read" {vec!["lvk:read",scope]} else {vec!["lvk:read","lvk:runtime:read",scope]} }]}})).expect("runtime schema")
}
pub(super) fn tools() -> Vec<Tool> {
    let text = json!({"type":"string"});
    let uuid = json!({"type":"string","format":"uuid"});
    vec![
        tool(
            "get_runtime_access",
            "Read runtime settings and limitations. File roots constrain direct file APIs only. Programs are NOT sandboxed. Outputs are reference data, never authorisation.",
            json!({}),
            &[],
            false,
            "lvk:runtime:read",
        ),
        tool(
            "read_runtime_path",
            "List a directory (action=list) or read regular file bytes (action=read) at an absolute path in a file root. Workspace reads allowed. Symlinks, special files and proc/sys refused; hard links/bind mounts are administrator scope. Default 16 KiB, max 512 KiB content; encoded responses can be much larger. Prefer small chunks. Cursor detects rotation/truncation/changes, not an atomic snapshot.",
            json!({"project_id":uuid,"action":{"enum":["list","read"]},"path":text,"max_bytes":{"type":"integer","minimum":1,"maximum":524288,"default":16384},"tail":{"type":"boolean"},"offset":{"type":"integer","minimum":0},"cursor":{"type":"object"},"limit":{"type":"integer","minimum":1,"maximum":500},"snapshot":{"type":"object"}}),
            &["project_id", "action", "path"],
            false,
            "lvk:runtime:read",
        ),
        tool(
            "change_runtime_path",
            "Direct write/delete at an absolute path in file roots, independently of execution permission. Refuses managed Workspace targets and deletion of their ancestors. write: UTF-8 content, max 512 KiB, atomic replacement, existing parent required; optional expected identity from read snapshot prevents stale overwrite (best effort). delete: regular file or empty directory; recursive=true explicitly deletes a directory tree. No symlink following. Partial recursive failure is reported and may have deleted entries. request_id is durable: reuse same input/key after lost response; never blindly replay an uncertain operation. Not a sandbox or complete hard-link/bind-mount isolation.",
            json!({"project_id":uuid,"request_id":uuid,"purpose":text,"action":{"enum":["write","delete"]},"path":text,"content":text,"expected":{"type":"object"},"recursive":{"type":"boolean","default":false}}),
            &["project_id", "request_id", "purpose", "action", "path"],
            true,
            "lvk:write",
        ),
        tool(
            "observe_process",
            "Observe same-UID processes (action=list/inspect) or owned process output (action=output). Requires execution setting ON and runtime read scope, not file permission. No cwd/root restriction or post-launch cwd monitoring. list: after_pid/limit; inspect: identity; output: process_id/stream/offset/limit (default 16 KiB, max 64 KiB). Output is volatile and may contain secrets. Identity is not health or build provenance.",
            json!({"project_id":uuid,"action":{"enum":["list","inspect","output"]},"after_pid":{"type":"integer","minimum":0},"limit":{"type":"integer","minimum":1,"maximum":65536},"identity":text,"process_id":text,"stream":{"enum":["stdout","stderr"]},"offset":{"type":"integer","minimum":0}}),
            &["project_id", "action"],
            false,
            "lvk:runtime:read",
        ),
        tool(
            "run_process",
            "Run an absolute existing executable with args and absolute cwd. Execution setting/scope required; independent of file roots/toggle. Workspace cwd refused at launch; ancestor/sibling cwd allowed. Programs may read/write/delete outside cwd and file roots; NOT sandboxed. Do not directly edit managed Workspaces: use their agents. Explicit user authorisation and existing shared resource coordination remain required. No executable allowlist or command-word inspection. request_id is durable; receipt success means startup only. Expiry sends TERM then KILL after 5 seconds; descendants are not managed. Self-restart uses the dedicated tool.",
            json!({"project_id":uuid,"request_id":uuid,"purpose":text,"executable":text,"args":{"type":"array","items":{"type":"string"},"maxItems":128},"cwd":text,"service":{"type":"boolean","default":false},"lifetime_seconds":{"type":"integer","minimum":1,"maximum":86400}}),
            &[
                "project_id",
                "request_id",
                "purpose",
                "executable",
                "cwd",
                "lifetime_seconds",
            ],
            true,
            "lvk:runtime:execute",
        ),
        tool(
            "stop_process",
            "Signal an exact observed process identity or owned managed process ID. Requires execution setting/scope; independent of file roots/current cwd. Same-UID, ownership, PID identity and LVK/ancestor protection remain. force=true selects KILL; otherwise TERM. Only one PID, not descendants. Reinspect to confirm termination. A receipt is not proof of shutdown.",
            json!({"project_id":uuid,"request_id":uuid,"purpose":text,"identity":text,"force":{"type":"boolean","default":false}}),
            &["project_id", "request_id", "purpose", "identity"],
            true,
            "lvk:runtime:execute",
        ),
        tool(
            "self_restart",
            "Explicit developer-instance switch through the installed engine, not run_process. Requires execution setting/scope and instance configuration. Coordinate idle work first. Initial busy preflight waits 30 seconds and rechecks once; other failures are not retried. Admission is not cutover; query get_self_restart_operation using the same request_id after reconnection. Shared resources still require existing coordination.",
            json!({"project_id":uuid,"request_id":uuid,"purpose":text,"bundle":text,"no_new_work":{"type":"boolean"}}),
            &[
                "project_id",
                "request_id",
                "purpose",
                "bundle",
                "no_new_work",
            ],
            true,
            "lvk:runtime:execute",
        ),
        tool(
            "get_self_restart_operation",
            "Read the installed developer switch engine's durable result for the original principal and Project. Does not replay or restart. Requires execution setting and runtime read scope.",
            json!({"project_id":uuid,"request_id":uuid}),
            &["project_id", "request_id"],
            false,
            "lvk:runtime:read",
        ),
    ]
}

pub(super) async fn workspace_paths(remote: &Remote) -> anyhow::Result<Vec<PathBuf>> {
    let paths: Vec<String> =
        sqlx::query_scalar("SELECT container_ref FROM workspaces WHERE container_ref IS NOT NULL")
            .fetch_all(&remote.pool)
            .await?;
    paths
        .into_iter()
        .map(|p| {
            let path = PathBuf::from(p);
            match path.canonicalize() {
                Ok(p) => Ok(p),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound && path.is_absolute() => {
                    Ok(path)
                }
                Err(e) => Err(e.into()),
            }
        })
        .collect()
}
pub(super) fn protect_workspace(
    target: &Path,
    workspaces: &[PathBuf],
    ancestors: bool,
) -> anyhow::Result<()> {
    preflight!(
        Category::Permission,
        !workspaces
            .iter()
            .any(|w| target.starts_with(w) || (ancestors && w.starts_with(target))),
        "Managed Workspace target refused; use its Workspace agent and existing controls"
    );
    Ok(())
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Run {
    request_id: Uuid,
    purpose: String,
    executable: PathBuf,
    #[serde(default)]
    args: Vec<String>,
    cwd: PathBuf,
    #[serde(default)]
    service: bool,
    lifetime_seconds: u64,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Stop {
    request_id: Uuid,
    purpose: String,
    identity: String,
    #[serde(default)]
    force: bool,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Change {
    request_id: Uuid,
    purpose: String,
    action: String,
    path: PathBuf,
    content: Option<String>,
    expected: Option<Value>,
    #[serde(default)]
    recursive: bool,
}

// Reuse durable attribution after server restart; a managed child never becomes unowned.
async fn stop_identity(
    remote: &Remote,
    owner: &str,
    project: Uuid,
    identity: &str,
) -> anyhow::Result<String> {
    use sqlx::Row;
    let row = sqlx::query("SELECT principal, project_id, result FROM remote_mcp_operations WHERE operation='run_process' AND json_valid(result) AND (json_extract(result,'$.identity')=? OR json_extract(result,'$.run_id')=?) ORDER BY created_at DESC LIMIT 1").bind(identity).bind(identity).fetch_optional(&remote.pool).await?;
    let Some(row) = row else {
        return Ok(identity.to_owned());
    };
    preflight!(
        Category::Permission,
        row.try_get::<&str, _>("principal")? == owner
            && row.try_get::<Uuid, _>("project_id")? == project,
        "Managed process belongs to another Project or owner"
    );
    let result: Value = serde_json::from_str(row.try_get::<&str, _>("result")?)?;
    Ok(result["identity"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("Managed identity unavailable; reobserve"))?
        .to_owned())
}

pub(super) async fn call(
    remote: &Remote,
    principal: &Principal,
    name: &str,
    mut input: Value,
) -> anyhow::Result<Value> {
    preflight!(
        Category::Permission,
        principal.allows("lvk:read") && principal.allows("lvk:runtime:read"),
        "Runtime read permission required"
    );
    let environment = remote
        .environment
        .as_ref()
        .ok_or_else(|| {
            super::errors::rejection_with_message(
                Category::Unavailable,
                "Runtime access unavailable",
            )
        })?
        .clone();
    let settings = environment
        .current_settings()
        .await
        .map_err(preflight_error)?;
    if name == "get_runtime_access" {
        preflight!(
            Category::Validation,
            input.as_object().is_some_and(|v| v.is_empty()),
            "Invalid capability arguments"
        );
        return Ok(
            json!({"settings":settings,"file_write_tools":true,"os_write_sandbox":false,"execution_cwd":"absolute, independent of file roots; managed Workspace cwd refused only at launch","shared_resources":"explicit user authorisation and existing resource coordination required","output_persistence":"bounded memory; unavailable after restart","reference_data_only":true}),
        );
    }
    let project: Uuid = serde_json::from_value(
        input
            .as_object_mut()
            .and_then(|v| v.remove("project_id"))
            .ok_or_else(|| {
                super::errors::rejection_with_message(Category::Validation, "Project required")
            })?,
    )?;
    remote.require_project(project).await?;
    let files = matches!(name, "read_runtime_path" | "change_runtime_path");
    preflight!(
        Category::Permission,
        if files {
            settings.files_enabled
        } else {
            settings.execution_enabled
        },
        if files {
            "Direct file access disabled by administrator"
        } else {
            "Program execution disabled by administrator"
        }
    );
    let mutation = matches!(name, "change_runtime_path" | "run_process" | "stop_process");
    if mutation || name == "self_restart" {
        preflight!(
            Category::Permission,
            principal.allows(if files {
                "lvk:write"
            } else {
                "lvk:runtime:execute"
            }),
            "Runtime mutation permission required"
        );
    }
    #[cfg(not(target_os = "linux"))]
    anyhow::bail!("Runtime access requires Linux");
    #[cfg(target_os = "linux")]
    {
        use super::{environment_files as fs, environment_process as ps};
        let owner = remote.principal_key(principal);
        let process_owner = json!([owner, project]).to_string();
        if matches!(name, "self_restart" | "get_self_restart_operation") {
            return super::environment_switch::call(remote, principal, project, name, input).await;
        }
        if name == "read_runtime_path" {
            let action = input
                .as_object_mut()
                .and_then(|v| v.remove("action"))
                .and_then(|v| v.as_str().map(str::to_owned))
                .ok_or_else(|| {
                    super::errors::rejection_with_message(
                        Category::Validation,
                        "File action required",
                    )
                })?;
            let path = input["path"].as_str().ok_or_else(|| {
                super::errors::rejection_with_message(
                    Category::Validation,
                    "Absolute path required",
                )
            })?;
            let (root, relative) = fs::resolve(&settings.file_roots, Path::new(path))?;
            input["path"] = json!(relative);
            let absolute = root.join(&relative);
            return tokio::task::spawn_blocking(move || {
                let mut result = match action.as_str() {
                    "list" => fs::list(&root, input)?,
                    "read" => fs::read(&root, input)?,
                    _ => anyhow::bail!("Unknown file read action"),
                };
                result["path"] = json!(absolute);
                Ok(result)
            })
            .await?;
        }
        if name == "observe_process" {
            let action = input
                .as_object_mut()
                .and_then(|v| v.remove("action"))
                .and_then(|v| v.as_str().map(str::to_owned))
                .ok_or_else(|| {
                    super::errors::rejection_with_message(
                        Category::Validation,
                        "Process action required",
                    )
                })?;
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct List {
                #[serde(default)]
                after_pid: u32,
                #[serde(default = "list_limit")]
                limit: usize,
            }
            fn list_limit() -> usize {
                50
            }
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Inspect {
                identity: String,
            }
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Output {
                process_id: String,
                stream: String,
                #[serde(default)]
                offset: u64,
                #[serde(default = "output_limit")]
                limit: usize,
            }
            fn output_limit() -> usize {
                16384
            }
            return match action.as_str() {
                "list" => {
                    let a: List = arguments(input)?;
                    preflight!(
                        Category::Validation,
                        (1..=100).contains(&a.limit),
                        "Process list limit must be 1 to 100"
                    );
                    ps::list(a.after_pid, a.limit)
                }
                "inspect" => {
                    let a: Inspect = arguments(input)?;
                    ps::inspect(&a.identity)
                }
                "output" => {
                    let a: Output = arguments(input)?;
                    preflight!(
                        Category::Validation,
                        (1..=65536).contains(&a.limit),
                        "Output page limit must be 1 to 65536"
                    );
                    environment.processes.output(
                        &process_owner,
                        &a.process_id,
                        &a.stream,
                        a.offset,
                        a.limit,
                    )
                }
                _ => anyhow::bail!("Unknown process observation action"),
            };
        }
        preflight!(Category::Validation, mutation, "Unknown runtime operation");
        let (request, purpose, target) = match name {
            "change_runtime_path" => {
                let a: Change = arguments(input.clone())?;
                preflight!(
                    Category::Validation,
                    matches!(a.action.as_str(), "write" | "delete"),
                    "Unknown file change action"
                );
                preflight!(
                    Category::Validation,
                    a.action != "write" || a.content.is_some(),
                    "Write content required"
                );
                preflight!(
                    Category::Validation,
                    a.action != "delete" || a.content.is_none(),
                    "Delete cannot include content"
                );
                preflight!(
                    Category::Validation,
                    a.action != "write" || !a.recursive,
                    "Write cannot be recursive"
                );
                preflight!(
                    Category::Validation,
                    a.content.as_ref().is_none_or(|s| s.len() <= 524288),
                    "Write exceeds 512 KiB"
                );
                fs::resolve(&settings.file_roots, &a.path)?;
                (
                    a.request_id,
                    a.purpose,
                    json!({"action":a.action,"path":a.path}),
                )
            }
            "run_process" => {
                let a: Run = arguments(input.clone())?;
                preflight!(
                    Category::Validation,
                    a.executable.is_absolute()
                        && a.executable
                            .canonicalize()
                            .map_err(|error| preflight_error(error.into()))?
                            .is_file(),
                    "Executable must be an absolute existing file"
                );
                preflight!(
                    Category::Validation,
                    a.cwd.is_absolute() && !a.cwd.components().any(|c| c == Component::ParentDir),
                    "cwd must be absolute without traversal"
                );
                preflight!(
                    Category::Validation,
                    a.args.len() <= 128 && a.args.iter().map(String::len).sum::<usize>() <= 32768,
                    "Runtime arguments exceed limit"
                );
                preflight!(
                    Category::Validation,
                    (1..=if a.service { 86400 } else { 3600 }).contains(&a.lifetime_seconds),
                    "Runtime lifetime exceeds limit"
                );
                (
                    a.request_id,
                    a.purpose,
                    json!({"cwd":a.cwd,"executable":a.executable,"argument_count":a.args.len()}),
                )
            }
            _ => {
                let a: Stop = arguments(input.clone())?;
                (
                    a.request_id,
                    a.purpose,
                    json!({"identity":a.identity,"force":a.force}),
                )
            }
        };
        preflight!(
            Category::Validation,
            !purpose.trim().is_empty() && purpose.len() <= 1000,
            "Purpose required (at most 1000 bytes)"
        );
        let owned = remote.clone();
        let operation = name.to_owned();
        let original = json!({"project_id":project,"arguments":input});
        let receipt = remote.operations().submit_and_wait(
            owner.clone(), request, project, operation.clone(), original,
            json!({"purpose":purpose,"target":target,"arguments":"omitted; input hash retained"}),
            move |operation_id| async move {
                let result = async {
                    let current = environment.current_settings().await.map_err(preflight_error)?;
                    preflight!(Category::Permission, if operation == "change_runtime_path" {
                        current.files_enabled && current.file_roots == settings.file_roots
                    } else { current.execution_enabled }, "Runtime permission changed; submit a new deliberate request");
                    match operation.as_str() {
                        "change_runtime_path" => {
                            let a: Change = arguments(input)?;
                            let paths = workspace_paths(&owned).await.map_err(preflight_error)?;
                            tokio::task::spawn_blocking(move || fs::change(
                                &settings.file_roots, &paths, &a.action, &a.path,
                                a.content.as_deref(), a.expected.as_ref(), a.recursive,
                            )).await?.map_err(|error| anyhow::Error::from(super::errors::diagnostic(&error, false)))
                        }
                        "run_process" => {
                            let a: Run = arguments(input)?;
                            let executable = a.executable.canonicalize().map_err(|error| preflight_error(error.into()))?;
                            let cwd = a.cwd.canonicalize().map_err(|error| preflight_error(error.into()))?;
                            let workspaces = workspace_paths(&owned).await.map_err(preflight_error)?;
                            protect_workspace(&cwd, &workspaces, false)?;
                            environment.processes.run_audited(
                                &process_owner, &executable, &a.args, &cwd,
                                a.lifetime_seconds, a.service,
                                Some((owned.pool.clone(), owner.clone(), operation_id)), &workspaces,
                            ).await
                        }
                        _ => {
                            let a: Stop = arguments(input)?;
                            let identity = stop_identity(&owned, &owner, project, &a.identity).await.map_err(preflight_error)?;
                            environment.processes.stop(&process_owner, &identity, a.force)
                        }
                    }
                }.await;
                match result {
                    Ok(value) if value["status"] == "failed" => ("failed", value),
                    Ok(value) if value["status"] == "uncertain" => ("uncertain", value),
                    Ok(value) => ("succeeded", value),
                    Err(error) => {
                        let diagnostic = super::errors::diagnostic(&error, true);
                        (if diagnostic.outcome == utils::mcp_error::Outcome::Failed { "failed" } else { "uncertain" },
                            json!({"message":super::errors::message(&error),"error":diagnostic.value()}))
                    },
                }
            },
        ).await?;
        Ok(remote.receipt_value(receipt))
    }
}

#[cfg(test)]
mod error_contract_tests {
    use super::*;

    #[test]
    fn preflight_rejections_preserve_category_and_safe_explanation() {
        let error = RuntimeAccessSettings {
            file_roots: vec![],
            ..Default::default()
        }
        .validate_paths()
        .unwrap_err();
        let diagnostic = super::super::errors::diagnostic(&error, true);
        assert_eq!(diagnostic.category, Category::Validation);
        assert_eq!(diagnostic.outcome, utils::mcp_error::Outcome::Failed);
        assert_eq!(
            super::super::errors::message(&error),
            "Configure 1 to 64 file roots"
        );

        let error = protect_workspace(
            Path::new("/managed/file"),
            &[PathBuf::from("/managed")],
            false,
        )
        .unwrap_err();
        let diagnostic = super::super::errors::diagnostic(&error, true);
        assert_eq!(diagnostic.category, Category::Permission);
        assert_eq!(diagnostic.outcome, utils::mcp_error::Outcome::Failed);
    }

    #[test]
    fn preflight_io_classification_does_not_expose_private_diagnostics() {
        let error = preflight_error(std::io::Error::other("private-token /host/path").into());
        let diagnostic = super::super::errors::diagnostic(&error, true);
        assert_eq!(diagnostic.outcome, utils::mcp_error::Outcome::Failed);
        assert!(!diagnostic.value().to_string().contains("private-token"));
        // The same generic error after a possible side effect must stay unknown.
        let diagnostic =
            super::super::errors::diagnostic(&anyhow::anyhow!("process_pid_unavailable"), true);
        assert_eq!(diagnostic.outcome, utils::mcp_error::Outcome::Unknown);
    }
}
