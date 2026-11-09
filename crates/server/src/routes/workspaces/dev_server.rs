//! Managed DevServer entry shared by UI, stdio and remote MCP. Database process
//! state is observation, never an application readiness/health probe.
use std::{
    collections::HashMap,
    sync::{Arc, LazyLock, Mutex},
};

use axum::{
    Extension, Json, Router,
    extract::{Query, State},
    routing::{get, post},
};
use db::models::{
    execution_process::{
        ExecutionProcess, ExecutionProcessRunReason, ExecutionProcessStatus, ExecutionProcessView,
    },
    session::{CreateSession, Session},
    workspace::Workspace,
    workspace_repo::WorkspaceRepo,
};
use deployment::Deployment;
use executors::actions::{
    ExecutorAction, ExecutorActionType,
    script::{ScriptContext, ScriptRequest, ScriptRequestLanguage},
};
use serde::Deserialize;
use serde_json::{Value, json};
use services::services::container::ContainerService;
use uuid::Uuid;

use crate::{DeploymentImpl, error::ApiError};

pub(crate) async fn start_managed(
    deployment: &DeploymentImpl,
    workspace: &Workspace,
) -> Result<utils::response::ApiResponse<Vec<ExecutionProcessView>>, ApiError> {
    start_managed_observed(deployment, workspace, &mut false).await
}

async fn start_managed_observed(
    deployment: &DeploymentImpl,
    workspace: &Workspace,
    effects_possible: &mut bool,
) -> Result<utils::response::ApiResponse<Vec<ExecutionProcessView>>, ApiError> {
    let pool = &deployment.db().pool;

    let _guard = lock(workspace.id).await;
    let workspace = validate(deployment, workspace.id).await?;
    let existing =
        ExecutionProcess::find_running_dev_servers_by_workspace(pool, workspace.id).await?;
    if !existing.is_empty() {
        return Ok(utils::response::ApiResponse::success(
            existing
                .into_iter()
                .map(ExecutionProcessView::from_process)
                .collect(),
        ));
    }

    let ids: Vec<Uuid> = sqlx::query_scalar("SELECT p.id FROM execution_processes p JOIN sessions s ON s.id=p.session_id WHERE s.workspace_id=? AND p.run_reason='devserver'").bind(workspace.id).fetch_all(pool).await?;
    for id in ids {
        if !deployment.container().script_settled(id).await? {
            return Err(ApiError::Conflict(
                "Previous DevServer process cleanup is unconfirmed".into(),
            ));
        }
    }

    let repos = WorkspaceRepo::find_repos_for_workspace(pool, workspace.id).await?;
    let repos_with_dev_script: Vec<_> = repos
        .iter()
        .filter(|r| {
            r.dev_server_script
                .as_ref()
                .is_some_and(|s| !s.trim().is_empty())
        })
        .collect();

    if repos_with_dev_script.is_empty() {
        return Ok(utils::response::ApiResponse::error(
            "No dev server script configured for any repository in this workspace",
        ));
    }

    let session = match Session::find_latest_by_workspace_id(pool, workspace.id).await? {
        Some(s) => s,
        None => {
            *effects_possible = true;
            Session::create(
                pool,
                &CreateSession {
                    executor: Some("dev-server".to_string()),
                    name: None,
                },
                Uuid::new_v4(),
                workspace.id,
            )
            .await?
        }
    };

    let mut execution_processes = Vec::new();
    for repo in repos_with_dev_script {
        let executor_action = ExecutorAction::new(
            ExecutorActionType::ScriptRequest(ScriptRequest {
                script: repo.dev_server_script.clone().unwrap(),
                language: ScriptRequestLanguage::Bash,
                context: ScriptContext::DevServer,
                working_dir: Some(repo.worktree_name().to_owned()),
            }),
            None,
        );

        *effects_possible = true;
        let execution_process = deployment
            .container()
            .start_execution(
                &workspace,
                &session,
                &executor_action,
                &ExecutionProcessRunReason::DevServer,
            )
            .await?;
        execution_processes.push(execution_process);
    }

    deployment
        .track_if_analytics_allowed(
            "dev_server_started",
            serde_json::json!({
                "workspace_id": workspace.id.to_string(),
            }),
        )
        .await;

    Ok(utils::response::ApiResponse::success(
        execution_processes
            .into_iter()
            .map(ExecutionProcessView::from_process)
            .collect(),
    ))
}
// Weak entries avoid retaining one mutex per deleted Workspace indefinitely.
type Locks = HashMap<Uuid, std::sync::Weak<tokio::sync::Mutex<()>>>;
static LOCKS: LazyLock<Mutex<Locks>> = LazyLock::new(|| Mutex::new(HashMap::new()));
pub(crate) async fn lock(id: Uuid) -> tokio::sync::OwnedMutexGuard<()> {
    let mutex = {
        let mut locks = LOCKS.lock().expect("DevServer lock map");
        locks.retain(|_, v| v.strong_count() > 0);
        let mutex = locks.get(&id).and_then(|v| v.upgrade()).unwrap_or_default();
        locks.insert(id, Arc::downgrade(&mutex));
        mutex
    };
    mutex.lock_owned().await
}

pub(crate) async fn validate(deployment: &DeploymentImpl, id: Uuid) -> Result<Workspace, ApiError> {
    let pool = &deployment.db().pool;
    let workspace = Workspace::find_by_id(pool, id)
        .await?
        .ok_or(ApiError::Workspace(
            db::models::workspace::WorkspaceError::WorkspaceNotFound,
        ))?;
    workspace.require_interactive()?;
    if workspace.archived || workspace.worktree_deleted {
        return Err(ApiError::Conflict(
            "Workspace is archived or its worktree is deleted".into(),
        ));
    }
    if db::models::integration::workspace_owner(pool, id)
        .await?
        .is_some()
    {
        return Err(ApiError::Conflict(
            "Workspace reserved by Integration".into(),
        ));
    }
    Ok(workspace)
}

pub(crate) async fn require_target(
    pool: &sqlx::SqlitePool,
    project: Uuid,
    workspace: Uuid,
) -> Result<(), ApiError> {
    let belongs: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM local_workspace_links l JOIN local_issues i ON i.id=l.issue_id AND i.project_id=l.project_id WHERE l.workspace_id=? AND l.project_id=?)")
        .bind(workspace).bind(project).fetch_one(pool).await?;
    if !belongs {
        return Err(ApiError::Conflict(
            "Workspace does not belong to Project".into(),
        ));
    }
    Ok(())
}

pub(crate) async fn process_records(
    pool: &sqlx::SqlitePool,
    workspace: Uuid,
) -> Result<Vec<ExecutionProcess>, sqlx::Error> {
    // Retain the latest result for every repository script as well as any
    // older still-running processes. A partial multi-repository launch must
    // not hide a failed script behind another script which remains running.
    let processes: Vec<ExecutionProcess> = sqlx::query_as(
        "WITH records AS (SELECT p.*, p.rowid AS record_order, ROW_NUMBER() OVER (PARTITION BY json_extract(p.executor_action,'$.typ.working_dir') ORDER BY p.created_at DESC,p.rowid DESC) AS latest FROM execution_processes p JOIN sessions s ON s.id=p.session_id WHERE s.workspace_id=? AND p.run_reason='devserver') SELECT id,session_id,run_reason,executor_action,status,exit_code,dropped,started_at,completed_at,created_at,updated_at FROM records WHERE latest=1 OR status='running' ORDER BY created_at DESC,record_order DESC"
    ).bind(workspace).fetch_all(pool).await?;
    Ok(processes)
}

pub(crate) async fn status(
    deployment: &DeploymentImpl,
    project: Uuid,
    workspace: Uuid,
) -> Result<Value, ApiError> {
    let pool = &deployment.db().pool;
    require_target(pool, project, workspace).await?;
    let current = Workspace::find_by_id(pool, workspace)
        .await?
        .ok_or(ApiError::Workspace(
            db::models::workspace::WorkspaceError::WorkspaceNotFound,
        ))?;
    current.require_interactive()?;
    let processes = process_records(pool, workspace).await?;
    let ids: Vec<Uuid> = sqlx::query_scalar("SELECT p.id FROM execution_processes p JOIN sessions s ON s.id=p.session_id WHERE s.workspace_id=? AND p.run_reason='devserver'").bind(workspace).fetch_all(pool).await?;
    let mut settled = true;
    for id in ids {
        settled &= deployment.container().script_settled(id).await?;
    }
    let running = processes
        .iter()
        .any(|p| p.status == ExecutionProcessStatus::Running);
    let mut running_verified = running;
    for process in &processes {
        if process.status == ExecutionProcessStatus::Running {
            running_verified &=
                deployment.container().script_running(process.id).await? == Some(true);
        }
    }
    let processes: Vec<_> = processes.into_iter().map(|p| json!({"process_id":p.id,"session_id":p.session_id,"status":p.status,"exit_code":p.exit_code,"started_at":p.started_at,"completed_at":p.completed_at})).collect();
    Ok(
        json!({"project_id":project,"workspace_id":workspace,"observed_at":chrono::Utc::now(),"state":if running_verified {"running"} else if running {"running_unconfirmed"} else if settled {"stopped"} else {"stopping_or_unconfirmed"},"process_tracking_settled":settled,"processes":processes,"observation":"managed process records; application readiness and health are not checked"}),
    )
}

pub(crate) async fn change(
    deployment: &DeploymentImpl,
    project: Uuid,
    workspace: Uuid,
    start: bool,
) -> Result<Value, ApiError> {
    // Same admission boundary as UI mutations, including Integration reservations.
    let _admission = services::services::integration_admission::MUTATIONS
        .lock()
        .await;
    change_admitted(deployment, project, workspace, start).await
}
async fn change_admitted(
    deployment: &DeploymentImpl,
    project: Uuid,
    workspace: Uuid,
    start: bool,
) -> Result<Value, ApiError> {
    require_target(&deployment.db().pool, project, workspace).await?;
    let current = validate(deployment, workspace).await?;
    let mut failure = None;
    let mut effects_possible = false;
    let action: Result<(), ApiError> = async {
        if start {
            let response = start_managed_observed(deployment, &current, &mut effects_possible).await?;
            if !response.is_success() {
                failure = response.message().map(str::to_owned);
            }
        } else {
            let _guard = lock(workspace).await;
            validate(deployment, workspace).await?;
            let ids: Vec<Uuid> = sqlx::query_scalar("SELECT p.id FROM execution_processes p JOIN sessions s ON s.id=p.session_id WHERE s.workspace_id=? AND p.run_reason='devserver'").bind(workspace).fetch_all(&deployment.db().pool).await?;
            for id in ids {
                let Some(process) = ExecutionProcess::find_by_id(&deployment.db().pool, id).await? else { continue; };
                if process.status != ExecutionProcessStatus::Running && deployment.container().script_settled(id).await? { continue; }
                effects_possible = true;
                if deployment.container().stop_execution(&process, ExecutionProcessStatus::Killed).await.is_err() {
                    failure = Some("Managed DevServer stop failed; inspect current process state".to_owned());
                }
            }
        }
        Ok(())
    }.await;
    if let Err(error) = action {
        if !effects_possible {
            return Err(error);
        }
        tracing::error!("Managed DevServer action outcome unconfirmed");
        failure = Some(
            "Managed DevServer action outcome unconfirmed; inspect current process state"
                .to_owned(),
        );
    }
    let mut observed = match status(deployment, project, workspace).await {
        Ok(observed) => observed,
        Err(error) if !effects_possible => return Err(error),
        Err(_) => {
            failure = Some(
                "Managed DevServer observation unavailable; inspect current process state"
                    .to_owned(),
            );
            json!({"project_id":project,"workspace_id":workspace,"state":"observation_unavailable","target_state_observed":false})
        }
    };
    observed["requested_action"] = json!(if start { "start" } else { "stop" });
    observed["target_state_observed"] =
        json!(observed["state"] == if start { "running" } else { "stopped" });
    observed["accepted"] = json!(failure.is_none());
    observed["failure_reason"] = json!(failure);
    if failure.is_some() {
        observed["error"] = change_failure_diagnostic(effects_possible).value();
    }
    Ok(observed)
}

fn change_failure_diagnostic(effects_possible: bool) -> utils::mcp_error::Diagnostic {
    use utils::mcp_error::{Category, Diagnostic, Outcome};
    Diagnostic::new(
        if effects_possible {
            Category::OutcomeUnknown
        } else {
            Category::Validation
        },
        if effects_possible {
            Outcome::Unknown
        } else {
            Outcome::Failed
        },
    )
}

#[cfg(test)]
mod error_contract_tests {
    use super::*;
    #[test]
    fn mutation_phase_controls_failure_outcome() {
        let preflight = change_failure_diagnostic(false).value();
        assert_eq!(preflight["category"], "validation");
        assert_eq!(preflight["outcome"], "failed");
        let partial = change_failure_diagnostic(true).value();
        assert_eq!(partial["outcome"], "unknown");
        assert_eq!(partial["recovery"], "inspect_receipt_and_state");
    }
}

#[derive(Deserialize)]
pub(super) struct Target {
    project_id: Uuid,
}
pub(super) fn router() -> Router<DeploymentImpl> {
    Router::new()
        .route("/status", get(read))
        .route("/start", post(start))
        .route("/stop", post(stop))
}
async fn read(
    Extension(w): Extension<Workspace>,
    State(d): State<DeploymentImpl>,
    Query(t): Query<Target>,
) -> Result<Json<utils::response::ApiResponse<Value>>, ApiError> {
    Ok(Json(utils::response::ApiResponse::success(
        status(&d, t.project_id, w.id).await?,
    )))
}
// Middleware already holds MUTATIONS on these HTTP routes. Remote acquires it in change().
async fn http_change(
    d: DeploymentImpl,
    w: Workspace,
    t: Target,
    start: bool,
) -> Result<Json<utils::response::ApiResponse<Value>>, ApiError> {
    require_target(&d.db().pool, t.project_id, w.id).await?;
    // Release-free variant is dispatched below; admission is held by the route middleware.
    Ok(Json(utils::response::ApiResponse::success(
        change_admitted(&d, t.project_id, w.id, start).await?,
    )))
}
async fn start(
    Extension(w): Extension<Workspace>,
    State(d): State<DeploymentImpl>,
    Query(t): Query<Target>,
) -> Result<Json<utils::response::ApiResponse<Value>>, ApiError> {
    http_change(d, w, t, true).await
}
async fn stop(
    Extension(w): Extension<Workspace>,
    State(d): State<DeploymentImpl>,
    Query(t): Query<Target>,
) -> Result<Json<utils::response::ApiResponse<Value>>, ApiError> {
    http_change(d, w, t, false).await
}
