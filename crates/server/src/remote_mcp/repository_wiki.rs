//! Repository Wiki is an adapter over UI admission and retained publication
//! evidence. An MCP receipt settles admission, never the agent or publication.
use anyhow::{Context, ensure};
use db::models::{
    agent_runtime::AgentRunRecord,
    repo::Repo,
    workspace::Workspace,
    workspace_usage::{OPENWIKI_SYNC, WorkspaceExecutionTerminalStatus},
};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};
use utils::repository_memory::{RepositoryMemoryStore, RepositoryWikiStatus};
use uuid::Uuid;

use super::{
    Remote,
    auth::Principal,
    read::{arguments, definition},
};
use crate::{
    DeploymentImpl,
    routes::{local_remote, openwiki},
};

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ReadArgs {
    project_id: Uuid,
    repository_id: Uuid,
}

#[derive(Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct SyncArgs {
    pub project_id: Uuid,
    pub repository_id: Uuid,
    pub request_id: Uuid,
    pub expected_target_branch: String,
    /// Exact source HEAD returned by get_repository_wiki.
    pub expected_source_commit: String,
    /// Omit for current-source reconciliation, as in Repository settings.
    pub from_commit: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ResultArgs {
    project_id: Uuid,
    repository_id: Uuid,
    request_id: Uuid,
}

pub(super) fn tools() -> Vec<rmcp::model::Tool> {
    let mut sync = serde_json::to_value(definition::<SyncArgs>("sync_repository_wiki",
        "Start or retry the existing Repository settings Wiki synchronization for an existing Wiki. Pin the observed branch and source HEAD. Omit from_commit for current source, or select the same since-commit scope as the UI. Never initializes a missing Wiki. Reuse request_id after response loss; acceptance is not Wiki success. Read get_repository_wiki_sync for publication/reconciliation results.")).unwrap();
    sync["annotations"] = json!({"readOnlyHint":false,"destructiveHint":true,"idempotentHint":true,"openWorldHint":true});
    sync["_meta"] =
        json!({"securitySchemes":[{"type":"oauth2","scopes":["lvk:read","lvk:write"]}]});
    vec![
        definition::<ReadArgs>(
            "get_repository_wiki",
            "Read saved Wiki target, current source HEAD, freshness and maintenance identities using the UI status contract. No synchronization or initialization. Discover IDs with list_projects/list_project_repositories first.",
        ),
        serde_json::from_value(sync).unwrap(),
        definition::<ResultArgs>(
            "get_repository_wiki_sync",
            "Read the original request admission and its exact retained Wiki execution/publication result, separately from latest repository freshness. Agent success alone never proves Wiki success. Does not retry or reconcile.",
        ),
    ]
}

async fn require_repository(
    remote: &Remote,
    project: Uuid,
    repository: Uuid,
) -> anyhow::Result<()> {
    remote.require_project(project).await?;
    ensure!(
        local_remote::project_creation_repository_ids(&remote.pool, project)
            .await?
            .contains(&repository),
        "Repository does not belong to this Project"
    );
    Ok(())
}

pub(super) async fn call(
    remote: &Remote,
    principal: &Principal,
    name: &str,
    input: Value,
) -> anyhow::Result<Value> {
    ensure!(principal.allows("lvk:read"), "Read permission required");
    if name == "sync_repository_wiki" {
        ensure!(principal.allows("lvk:write"), "Write permission required");
        let args: SyncArgs = arguments(input.clone())?;
        require_repository(remote, args.project_id, args.repository_id).await?;
        ensure!(
            args.expected_source_commit.len() == 40
                && args
                    .expected_source_commit
                    .bytes()
                    .all(|b| b.is_ascii_hexdigit()),
            "Read the exact current source commit before synchronizing"
        );
        ensure!(
            !args.expected_target_branch.is_empty() && args.expected_target_branch.len() <= 1024,
            "Read the saved target branch before synchronizing"
        );
        ensure!(
            args.from_commit
                .as_ref()
                .is_none_or(|value| (4..=40).contains(&value.len())
                    && value.bytes().all(|b| b.is_ascii_hexdigit())),
            "Use a commit SHA for from_commit"
        );
        let launch = remote.launch.clone().context("Wiki service unavailable")?;
        let owned = remote.clone();
        let receipt = remote.operations().submit(
            remote.principal_key(principal), args.request_id, args.project_id, name.into(), input,
            json!({"repository_id":args.repository_id}), move |_| async move {
                if let Err(error) = require_repository(&owned, args.project_id, args.repository_id).await {
                    return ("failed", json!({"code":"repository_unavailable","message":super::errors::message(&error)}));
                }
                match launch.wiki_sync(args).await {
                    Ok(value) => ("succeeded", value),
                    // Preparation may already have reserved a writer. Never
                    // replay an unknown outcome or classify it as safe to retry.
                    Err(error) => ("uncertain", json!({"code":"wiki_admission_not_confirmed","message":super::errors::message(&error),"next_action":"Read get_repository_wiki and inspect its maintenance execution before any new request; reuse this request_id for receipt recovery","wiki_complete":false})),
                }
            }).await?;
        return Ok(remote.receipt_value(receipt));
    }
    if name == "get_repository_wiki_sync" {
        let args: ResultArgs = arguments(input)?;
        require_repository(remote, args.project_id, args.repository_id).await?;
        let receipt = remote
            .operations()
            .by_request(&remote.principal_key(principal), args.request_id)
            .await?;
        ensure!(
            receipt.project_id == args.project_id && receipt.operation == "sync_repository_wiki",
            "Wiki request not found in this Project"
        );
        let targets: Value = serde_json::from_str(&receipt.targets)?;
        ensure!(
            targets["repository_id"] == json!(args.repository_id),
            "Wiki request does not target this repository"
        );
        let admission = remote.receipt_value(receipt);
        let result = &admission["result"];
        let execution = match (
            result["workspace_id"].as_str(),
            result["agent_run_id"].as_str(),
        ) {
            (Some(workspace), Some(run)) => {
                match execution_result(
                    remote,
                    args.repository_id,
                    Uuid::parse_str(workspace)?,
                    Uuid::parse_str(run)?,
                )
                .await
                {
                    Ok(value) => value,
                    Err(error) => json!({"wiki_complete":false,"status":"evidence_unavailable",
                        "message":super::errors::message(&error),
                        "next_action":"Inspect the retained admission identities and maintenance execution; unavailable evidence is not success or permission to retry"}),
                }
            }
            _ => {
                json!({"wiki_complete":false,"status":"admission_not_confirmed","next_action":"Read admission and get_repository_wiki before any new request"})
            }
        };
        let current = match remote
            .launch
            .as_ref()
            .context("Wiki service unavailable")?
            .wiki_status(args.repository_id)
            .await
        {
            Ok(value) => value,
            Err(error) => json!({"status":"unavailable","message":super::errors::message(&error)}),
        };
        return Ok(json!({"admission":admission,"execution":execution,"repository":current}));
    }
    ensure!(name == "get_repository_wiki", "Unknown Wiki tool");
    let args: ReadArgs = arguments(input)?;
    require_repository(remote, args.project_id, args.repository_id).await?;
    remote
        .launch
        .as_ref()
        .context("Wiki service unavailable")?
        .wiki_status(args.repository_id)
        .await
}

pub(super) async fn status(deployment: &DeploymentImpl, repository: Uuid) -> anyhow::Result<Value> {
    let (state, head) = openwiki::status_snapshot(deployment, repository).await?;
    Ok(
        json!({"repository_id":repository,"enabled":state.enabled,"status":state.status,
        "target_branch":state.target_branch,"current_source_commit":head,"wiki_exists":state.wiki_exists,
        "source_commit":state.source_commit,"wiki_commit":state.wiki_commit,"last_success":state.last_success,
        "active_run_id":state.active_run_id,"maintenance_workspace_id":state.maintenance_workspace_id,
        "maintenance_session_id":state.maintenance_session_id,"bootstrap_active":state.bootstrap.is_some(),
        "active_source_commit":state.active_source_commit,"active_sync_scope":state.active_sync_scope,
        "unverified_before":state.unverified_before,"error_code":state.error.as_deref().map(failure_code),
        "needs_sync":matches!(state.status, RepositoryWikiStatus::Stale | RepositoryWikiStatus::Error),
        "next_action":if state.active_run_id.is_some() || state.bootstrap.is_some() {"Inspect the maintenance execution; wait for host reconciliation"} else if !state.enabled {"Enable repository memory through Repository settings"} else if !state.wiki_exists {"Initialize explicitly through Repository settings"} else {"For an authorized sync/retry, confirm current branch/source and submit a new request_id"},
        "consistency":"live; admission rechecks branch and source under the existing writer lock"}),
    )
}

pub(super) async fn sync(deployment: &DeploymentImpl, args: SyncArgs) -> anyhow::Result<Value> {
    let state = openwiki::sync_checked(
        deployment,
        args.repository_id,
        openwiki::SyncRepositoryMemory {
            project_id: Some(args.project_id),
            from_commit: args.from_commit,
        },
        &args.expected_target_branch,
        &args.expected_source_commit,
    )
    .await?;
    Ok(
        json!({"repository_id":args.repository_id,"workspace_id":state.maintenance_workspace_id,
        "session_id":state.maintenance_session_id,"agent_run_id":state.active_run_id,
        "target_branch":state.target_branch,"source_commit":state.active_source_commit,"scope":state.active_sync_scope,
        "execution_accepted":true,"wiki_complete":false,"next_tool":"get_repository_wiki_sync"}),
    )
}

// Explicit, bounded public diagnostics; private paths/provider logs never leave
// the host. Detailed retained evidence remains in the existing execution UI.
fn failure_code(error: &str) -> &'static str {
    if error.contains("source") || error.contains("Source") {
        "source_validation_failed"
    } else if error.contains("instruction") || error.contains("Instruction") {
        "instruction_restoration_or_validation_failed"
    } else if error.contains("audit") || error.contains("completion") || error.contains("finish") {
        "completion_evidence_failed"
    } else if error.contains("publish") || error.contains("publication") {
        "publication_failed"
    } else {
        "reconciliation_failed"
    }
}

async fn execution_result(
    remote: &Remote,
    repository: Uuid,
    workspace: Uuid,
    run_id: Uuid,
) -> anyhow::Result<Value> {
    let ws = Workspace::find_by_id(&remote.pool, workspace)
        .await?
        .context("Wiki execution workspace unavailable")?;
    let owner = ws
        .execution_owner
        .as_ref()
        .context("Wiki owner unavailable")?;
    ensure!(
        owner.kind == OPENWIKI_SYNC
            && owner.repository_id == Some(repository)
            && owner.run_id == Some(run_id),
        "Wiki execution identity mismatch"
    );
    let run = AgentRunRecord::find(&remote.pool, run_id)
        .await?
        .context("Wiki AgentRun unavailable")?;
    ensure!(
        run.workspace_id == workspace,
        "Wiki AgentRun workspace mismatch"
    );
    let repo = Repo::find_by_id(&remote.pool, repository)
        .await?
        .context("Repository unavailable")?;
    let store = RepositoryMemoryStore::read_only_configured_for_repository(&repo.name, repository)?
        .context("Wiki evidence unavailable")?;
    let mut value = execution_evidence(&store, run_id, run.status, owner.result.as_ref())?;
    value["workspace_id"] = json!(workspace);
    Ok(value)
}

fn execution_evidence(
    store: &RepositoryMemoryStore,
    run_id: Uuid,
    agent_status: executors::runtime::AgentRunStatus,
    result: Option<&db::models::workspace_usage::WorkspaceExecutionResult>,
) -> anyhow::Result<Value> {
    let state = store.state()?;
    let publication = store.publication(run_id)?;
    let finalizing = state.active_run_id == Some(run_id);
    let complete = agent_status == executors::runtime::AgentRunStatus::Succeeded
        && !finalizing
        && publication.is_some()
        && result
            .is_some_and(|result| result.status == WorkspaceExecutionTerminalStatus::Succeeded);
    Ok(json!({"agent_run_id":run_id,"agent_status":agent_status,
        "wiki_complete":complete,"host_finalizing":finalizing,"publication_checkpoint":publication,
        "wiki_commit":result.and_then(|result| result.wiki_commit.as_ref()),
        "reconciliation_status":result.map(|result| result.status),
        "error_code":result.and_then(|result| result.error.as_deref()).map(failure_code),
        "next_action":if complete {"This execution completed; check repository freshness separately"} else if finalizing || result.is_none() {"Inspect the existing execution; wait for host reconciliation, do not start a duplicate"} else {"Inspect the retained failure, refresh get_repository_wiki, then request an authorized sync with the current source"}}))
}

#[cfg(test)]
mod tests {
    use db::models::workspace_usage::WorkspaceExecutionResult;
    use executors::runtime::AgentRunStatus;
    use utils::repository_memory::{RepositoryMemoryState, WikiPublication};

    use super::*;

    #[test]
    fn wiki_result_requires_publication_and_settled_reconciliation_not_agent_success() {
        let temp = tempfile::tempdir().unwrap();
        let store = RepositoryMemoryStore::at_persistent(temp.path()).unwrap();
        let run = Uuid::new_v4();
        let mut state = RepositoryMemoryState {
            active_run_id: Some(run),
            active_source_commit: Some("a".repeat(40)),
            ..RepositoryMemoryState::enabled_defaults(Some("main".into()))
        };
        store.save_state(&state).unwrap();
        let read = |result: Option<&WorkspaceExecutionResult>| {
            execution_evidence(&store, run, AgentRunStatus::Succeeded, result).unwrap()
        };
        assert_eq!(read(None)["wiki_complete"], false);
        // A publication journal is intent, not proof that the target was committed.
        store
            .save_publication(&WikiPublication {
                run_id: run,
                source_commit: "a".repeat(40),
                target_branch: "main".into(),
                maintenance_commit: Some("b".repeat(40)),
                no_op: false,
            })
            .unwrap();
        let mut result = WorkspaceExecutionResult {
            status: WorkspaceExecutionTerminalStatus::Failed,
            completed_at: chrono::Utc::now(),
            error: Some(
                "Integrated source advanced before Wiki publication; /private/secret".into(),
            ),
            wiki_commit: None,
            no_op: false,
        };
        openwiki::record_reconciliation_result(
            &store,
            &mut state,
            Err(anyhow::anyhow!(
                "Integrated source advanced before Wiki publication"
            )),
        )
        .unwrap();
        let failed_result = result.clone();
        let failed = read(Some(&result));
        assert_eq!(failed["wiki_complete"], false);
        assert_eq!(failed["error_code"], "source_validation_failed");
        assert!(!failed.to_string().contains("/private/secret"));
        assert_eq!(store.state().unwrap().status, RepositoryWikiStatus::Error);
        // A later legitimate retry gets a new run; the old run remains failed.
        let retry = Uuid::new_v4();
        state.active_run_id = Some(retry);
        state.active_source_commit = Some("c".repeat(40));
        store.save_state(&state).unwrap();
        store
            .save_publication(&WikiPublication {
                run_id: retry,
                source_commit: "c".repeat(40),
                target_branch: "main".into(),
                maintenance_commit: None,
                no_op: true,
            })
            .unwrap();
        result.status = WorkspaceExecutionTerminalStatus::Succeeded;
        result.error = None;
        result.wiki_commit = Some("c".repeat(40));
        result.no_op = true;
        assert_eq!(
            execution_evidence(&store, retry, AgentRunStatus::Succeeded, Some(&result)).unwrap()["wiki_complete"],
            false
        );
        openwiki::record_reconciliation_result(
            &store,
            &mut state,
            Ok((Some("c".repeat(40)), true)),
        )
        .unwrap();
        assert_eq!(
            execution_evidence(&store, retry, AgentRunStatus::Succeeded, Some(&result)).unwrap()["wiki_complete"],
            true
        );
        assert_eq!(read(Some(&failed_result))["wiki_complete"], false);
        assert_eq!(
            read(Some(&failed_result))["reconciliation_status"],
            "failed"
        );
        assert_eq!(
            execution_evidence(&store, retry, AgentRunStatus::Failed, Some(&result)).unwrap()["wiki_complete"],
            false
        );
        assert_eq!(
            execution_evidence(
                &store,
                Uuid::new_v4(),
                AgentRunStatus::Succeeded,
                Some(&result)
            )
            .unwrap()["wiki_complete"],
            false
        );
    }
}
