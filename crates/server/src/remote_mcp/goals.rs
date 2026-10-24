//! Goal controls adapt the existing UI command/follow-up paths. Receipts describe
//! admission/delivery, never provider completion or stopped process containment.
use anyhow::ensure;
use db::models::session::Session;
use deployment::Deployment;
use executors::{
    executors::BaseCodingAgent,
    model_selector::PermissionPolicy,
    profile::{ExecutionMode, ExecutorConfig},
    runtime::{AgentGoalStatus, AgentRunPortCommand},
};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::{Row, SqlitePool};
use uuid::Uuid;

use super::{
    Remote,
    auth::Principal,
    read::{arguments, definition},
    runtime_write::{Target, identity, validate_run},
};
use crate::{
    DeploymentImpl,
    error::ApiError,
    routes::{agent_runs, sessions},
};

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ReadArgs {
    #[serde(flatten)]
    target: Target,
    /// Optional command_id returned by get_operation. Delivery is not completion.
    command_id: Option<Uuid>,
}
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct WriteArgs {
    #[serde(flatten)]
    target: Target,
    request_id: Uuid,
    /// Copy revision from get_goal; refresh after any concurrent Goal change.
    expected_revision: String,
}
#[derive(Clone, Copy, Debug)]
pub(super) enum Action {
    Pause,
    Resume,
    Cancel,
    Clear,
}
impl Action {
    pub(super) fn command(self) -> AgentRunPortCommand {
        match self {
            Self::Pause | Self::Resume => AgentRunPortCommand::GoalUpdate {
                objective: None,
                status: Some(if matches!(self, Self::Pause) {
                    AgentGoalStatus::Paused
                } else {
                    AgentGoalStatus::Active
                }),
                token_budget: None,
            },
            Self::Cancel => AgentRunPortCommand::Cancel {
                reason: "Explicit MCP Goal run cancellation".into(),
            },
            Self::Clear => AgentRunPortCommand::GoalClear,
        }
    }
}

pub(super) fn tools() -> Vec<rmcp::model::Tool> {
    let mut result = vec![definition::<ReadArgs>(
        "get_goal",
        "Read the selected Goal's recorded status, accounting, run lifecycle, revision and optional command delivery. is_current_run=false means historical state. This is the same persisted projection used by the UI, not a live provider RPC. Read objective with get_execution(section=goal).",
    )];
    for (name, description) in [
        (
            "pause_goal",
            "Pause the current active Goal through the UI GoalUpdate command; keeps its objective, budget and accounting. Does not prove the current turn stopped.",
        ),
        (
            "resume_goal",
            "Resume a paused, blocked (after resolving its cause), usageLimited or budgetLimited Goal. Does not remove limits or resolve approvals. For a finished run, uses the UI /goal resume follow-up in the same Codex Session, retaining its provider Goal and saved permissions (PLAN becomes SUPERVISED). Never creates a new objective.",
        ),
        (
            "cancel_goal",
            "Request cancellation of the current Goal's AgentRun through the UI Cancel path. This stops execution, not marks the Goal complete or clears it. The provider may retain a saved Goal.",
        ),
        (
            "clear_goal",
            "Permanently clear the current Goal through the UI Clear Goal command. This ends Goal management, not marks achievement or proves run termination. There is deliberately no finish-as-success operation.",
        ),
    ] {
        let tool = definition::<WriteArgs>(
            name,
            &format!(
                "{description} Requires the latest run and get_goal revision. Preserve request_id and identical input on retry. Read get_operation, then get_goal with its command_id (or newly returned agent_run_id). Receipt success means request delivery only; run/Goal terminal states are separate."
            ),
        );
        let mut value = serde_json::to_value(tool).expect("schema");
        value["annotations"] = json!({"readOnlyHint":false,"destructiveHint":true,"idempotentHint":true,"openWorldHint":false});
        value["_meta"] =
            json!({"securitySchemes":[{"type":"oauth2","scopes":["lvk:read","lvk:write"]}]});
        result.push(serde_json::from_value(value).expect("schema"));
    }
    result
}

fn terminal(status: &str) -> bool {
    matches!(
        status,
        "succeeded" | "failed" | "cancelled" | "crashed" | "audit_failed"
    )
}

pub(super) async fn observation(
    pool: &SqlitePool,
    target: &Target,
    command: Option<Uuid>,
) -> Result<Value, ApiError> {
    let row = sqlx::query("SELECT r.status,r.projection_status,r.updated_at,r.provider_id,s.last_event_sequence,s.projection_status AS state_projection,json_extract(s.state_json,'$.goal.status') AS goal_status,json_extract(s.state_json,'$.goal.token_budget') AS budget,json_extract(s.state_json,'$.goal.tokens_used') AS tokens,json_extract(s.state_json,'$.goal.time_used_seconds') AS seconds,NOT EXISTS(SELECT 1 FROM agent_runs n WHERE n.session_id=r.session_id AND (n.created_at>r.created_at OR (n.created_at=r.created_at AND n.rowid>r.rowid))) AS current_run FROM agent_runs r JOIN sessions session ON session.id=r.session_id LEFT JOIN agent_run_state s ON s.agent_run_id=r.id WHERE r.id=? AND r.session_id=? AND r.workspace_id=? AND session.workspace_id=?")
        .bind(target.agent_run_id).bind(target.session_id).bind(target.workspace_id).bind(target.workspace_id).fetch_optional(pool).await?
        .ok_or_else(||ApiError::Conflict("Goal target does not belong to this Session and Workspace".into()))?;
    let status: String = row.try_get("status")?;
    let goal: Option<String> = row.try_get("goal_status")?;
    let revision = format!(
        "{}:{}:{}",
        target.agent_run_id,
        row.try_get::<String, _>("updated_at")?,
        row.try_get::<Option<i64>, _>("last_event_sequence")?
            .unwrap_or(-1)
    );
    let delivery: Option<String> = if let Some(command) = command {
        sqlx::query_scalar(
            "SELECT delivery_status FROM agent_run_commands WHERE command_id=? AND agent_run_id=?",
        )
        .bind(command)
        .bind(target.agent_run_id)
        .fetch_optional(pool)
        .await?
    } else {
        None
    };
    Ok(
        json!({"project_id":target.project_id,"workspace_id":target.workspace_id,"session_id":target.session_id,"agent_run_id":target.agent_run_id,
        "revision":revision,"is_current_run":row.try_get::<bool,_>("current_run")?,"run_status":status,"run_terminal":terminal(&status),
        "projection_status":row.try_get::<String,_>("projection_status")?,"state_projection_status":row.try_get::<Option<String>,_>("state_projection")?,"provider_id":row.try_get::<String,_>("provider_id")?,
        "goal_status":goal,"goal_complete":goal.as_deref()==Some("complete"),"goal_present":goal.is_some(),
        "token_budget":row.try_get::<Option<i64>,_>("budget")?,"tokens_used":row.try_get::<Option<i64>,_>("tokens")?,"time_used_seconds":row.try_get::<Option<i64>,_>("seconds")?,
        "command_id":command,"command_delivery_status":delivery,"observed_at":chrono::Utc::now(),"source":"persisted UI projection; absence is not proof of provider clearance or process termination"}),
    )
}

pub(super) async fn validate(
    pool: &SqlitePool,
    target: &Target,
    action: Action,
    revision: &str,
) -> Result<bool, ApiError> {
    let state = observation(pool, target, None).await?;
    let finished = state["run_terminal"] == true;
    validate_run(pool, target, finished).await?;
    let goal = state["goal_status"].as_str().unwrap_or("");
    let allowed = match action {
        Action::Pause => !finished && goal == "active",
        Action::Resume => {
            if finished {
                !goal.is_empty() && goal != "complete"
            } else {
                matches!(
                    goal,
                    "paused" | "blocked" | "usageLimited" | "budgetLimited"
                )
            }
        }
        Action::Cancel | Action::Clear => !finished && !goal.is_empty(),
    };
    if !allowed
        || state["revision"] != revision
        || state["provider_id"] != "codex"
        || state["projection_status"] != "current"
        || state["state_projection_status"] != "current"
    {
        return Err(ApiError::Conflict(
            "Goal state changed or this action is unavailable; refresh get_goal".into(),
        ));
    }
    Ok(finished)
}

// Mirrors the saved-Goal UI request, preserving every unrelated override.
fn resume_config(mut config: ExecutorConfig) -> Result<ExecutorConfig, ApiError> {
    if config.executor != BaseCodingAgent::Codex {
        return Err(ApiError::Conflict(
            "Saved Goal resume requires Codex".into(),
        ));
    }
    config.execution_mode = Some(ExecutionMode::Goal);
    if config.permission_policy == Some(PermissionPolicy::Plan) {
        config.permission_policy = Some(PermissionPolicy::Supervised);
    }
    Ok(config)
}

pub(super) async fn dispatch(
    deployment: &DeploymentImpl,
    target: Target,
    operation: Uuid,
    action: Action,
    revision: String,
) -> Result<Value, ApiError> {
    let pool = &deployment.db().pool;
    let guard = deployment
        .queued_message_service()
        .lock_session(target.session_id)
        .await;
    let finished = validate(pool, &target, action, &revision).await?;
    if finished {
        // follow_up_checked takes this same lock and repeats latest/terminal and
        // queue admission. Two distinct resume requests cannot start two runs.
        drop(guard);
        let session = Session::find_by_id(pool, target.session_id)
            .await?
            .ok_or_else(|| ApiError::Conflict("Session missing".into()))?;
        let mut config = sessions::executor_config::read_executor_config(pool, session.id)
            .await?
            .ok_or_else(|| {
                ApiError::Conflict("Stored executor configuration unavailable".into())
            })?;
        config = resume_config(config)?;
        let result = sessions::follow_up_checked(
            deployment.clone(),
            session,
            sessions::CreateFollowUpAttempt {
                // Must remain the exact slash command; appending an operation marker
                // would change its parser semantics.
                prompt: "/goal resume".into(),
                selected_skills: None,
                executor_config: config,
                resume_session_id: None,
                resume_scope_path: None,
            },
            Some((target.agent_run_id, target.project_id)),
        )
        .await?
        .0
        .into_data()
        .ok_or_else(|| ApiError::Conflict("Resume receipt unavailable".into()))?;
        return Ok(
            json!({"agent_run_id":result.agent_run_id,"session_id":target.session_id,"execution_accepted":true,"goal_transition_confirmed":false,"run_terminal":false}),
        );
    }
    let _ = agent_runs::dispatch_control(
        deployment,
        target.agent_run_id,
        identity(operation, format!("mcp:{operation}")),
        action.command(),
    )
    .await?;
    Ok(
        json!({"agent_run_id":target.agent_run_id,"session_id":target.session_id,"command_id":operation,"instruction_accepted":true,"goal_transition_confirmed":false,"termination_confirmed":false}),
    )
}

pub(super) async fn call(
    remote: &Remote,
    principal: &Principal,
    name: &str,
    input: Value,
) -> anyhow::Result<Value> {
    ensure!(principal.allows("lvk:read"), "Read permission required");
    if name == "get_goal" {
        let args: ReadArgs = arguments(input)?;
        remote
            .require_workspace(args.target.project_id, args.target.workspace_id)
            .await?;
        return Ok(observation(&remote.pool, &args.target, args.command_id).await?);
    }
    ensure!(
        principal.allows("lvk:write"),
        "Read and write permissions required"
    );
    let args: WriteArgs = arguments(input.clone())?;
    let action = match name {
        "pause_goal" => Action::Pause,
        "resume_goal" => Action::Resume,
        "cancel_goal" => Action::Cancel,
        "clear_goal" => Action::Clear,
        _ => anyhow::bail!("Unknown Goal operation"),
    };
    remote
        .require_workspace(args.target.project_id, args.target.workspace_id)
        .await?;
    let runtime = remote
        .runtime
        .clone()
        .ok_or_else(|| anyhow::anyhow!("Runtime dispatch unavailable"))?;
    let owned = remote.clone();
    let receipt = remote.operations().submit(remote.principal_key(principal),args.request_id,args.target.project_id,name.into(),input,json!({"workspace_id":args.target.workspace_id,"session_id":args.target.session_id,"agent_run_id":args.target.agent_run_id}),move |operation| async move {
        if validate(&owned.pool,&args.target,action,&args.expected_revision).await.is_err() {
            return ("failed",json!({"code":"goal_state_conflict","message":"Refresh get_goal; target, revision or lifecycle changed"}));
        }
        match runtime.goal(args.target,operation,action,args.expected_revision).await {
            Ok(value)=>("succeeded",value),
            Err(_)=>("uncertain",json!({"code":"dispatch_outcome_unavailable","message":"Read get_operation and get_goal before a new request; delivery may already have occurred"})),
        }
    }).await?;
    Ok(remote.receipt_value(receipt))
}

#[cfg(test)]
mod tests;
