//! Remote adapters for existing Session and AgentRun control paths. The trait
//! separates domain dispatch from MCP transport and permits isolated port tests.
use std::sync::Arc;

use anyhow::ensure;
use async_trait::async_trait;
use db::models::{session::Session, workspace::Workspace};
use deployment::Deployment;
use executors::runtime::AgentRunPortCommand;
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::{Row, SqlitePool};
use uuid::Uuid;

use super::{
    Remote,
    auth::Principal,
    questions::Kind,
    read::{arguments, definition},
};
use crate::{
    DeploymentImpl,
    error::ApiError,
    routes::{agent_runs, sessions},
};

#[derive(Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct Target {
    pub project_id: Uuid,
    pub workspace_id: Uuid,
    pub session_id: Uuid,
    /// The explicitly selected latest run in this Session.
    pub agent_run_id: Uuid,
}
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ChangeArgs {
    #[serde(flatten)]
    target: Target,
    request_id: Uuid,
    #[schemars(length(min = 1, max = 32000))]
    prompt: String,
}
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct AnswerArgs {
    #[serde(flatten)]
    target: Target,
    request_id: Uuid,
    question_id: String,
    kind: Kind,
    /// Revision from get_question; a listed preview is not the complete question.
    expected_revision: String,
    #[schemars(length(min = 1, max = 32000))]
    answer: String,
}
#[derive(Clone, Copy)]
pub(super) enum Change {
    Steer,
    Continue,
}

#[async_trait]
pub(super) trait RuntimeActions: Send + Sync {
    async fn change(
        &self,
        target: Target,
        operation: Uuid,
        prompt: String,
        kind: Change,
    ) -> Result<Value, ApiError>;
    #[allow(
        clippy::too_many_arguments,
        reason = "Question delivery binds target, operation identity, actor and exact event evidence"
    )]
    async fn answer(
        &self,
        target: Target,
        operation: Uuid,
        question: String,
        answer: String,
        kind: Kind,
        actor: String,
        event: Option<Uuid>,
    ) -> Result<Value, ApiError>;
}
pub(super) struct ExistingRuntime(pub DeploymentImpl);

fn identity(operation: Uuid, key: String) -> agent_runs::AgentRunControlIdentity {
    agent_runs::AgentRunControlIdentity {
        command_id: operation,
        correlation_id: operation,
        idempotency_key: key,
        created_at: chrono::Utc::now(),
    }
}

#[async_trait]
impl RuntimeActions for ExistingRuntime {
    async fn change(
        &self,
        target: Target,
        operation: Uuid,
        prompt: String,
        kind: Change,
    ) -> Result<Value, ApiError> {
        let pool = &self.0.db().pool;
        match kind {
            Change::Steer => {
                let _guard = self
                    .0
                    .queued_message_service()
                    .lock_session(target.session_id)
                    .await;
                validate_run(pool, &target, false).await?;
                let _ = agent_runs::dispatch_control(
                    &self.0,
                    target.agent_run_id,
                    identity(operation, format!("mcp:{operation}")),
                    AgentRunPortCommand::Steer { content: prompt },
                )
                .await?;
                Ok(
                    json!({"agent_run_id":target.agent_run_id,"session_id":target.session_id,"command_id":operation,"instruction_accepted":true,"implementation_complete":false}),
                )
            }
            Change::Continue => {
                let session = Session::find_by_id(pool, target.session_id)
                    .await?
                    .ok_or_else(|| ApiError::Conflict("Session missing".into()))?;
                let executor_config =
                    sessions::executor_config::read_executor_config(pool, session.id)
                        .await?
                        .ok_or_else(|| {
                            ApiError::Conflict(
                                "Stored Session executor configuration unavailable".into(),
                            )
                        })?;
                let result = sessions::follow_up_checked(
                    self.0.clone(),
                    session,
                    sessions::CreateFollowUpAttempt {
                        prompt: format!("{prompt}\n\nLVK MCP operation: {operation}"),
                        selected_skills: None,
                        executor_config,
                        resume_session_id: None,
                        resume_scope_path: None,
                    },
                    Some((target.agent_run_id, target.project_id)),
                )
                .await?
                .0
                .into_data()
                .ok_or_else(|| ApiError::Conflict("Continuation receipt unavailable".into()))?;
                Ok(
                    json!({"agent_run_id":result.agent_run_id,"session_id":target.session_id,"execution_accepted":true,"implementation_complete":false}),
                )
            }
        }
    }
    async fn answer(
        &self,
        target: Target,
        operation: Uuid,
        question: String,
        answer: String,
        kind: Kind,
        actor: String,
        event: Option<Uuid>,
    ) -> Result<Value, ApiError> {
        let accepted_command = match kind {
            Kind::AsyncSpecification => {
                let session = Session::find_by_id(&self.0.db().pool, target.session_id)
                    .await?
                    .ok_or_else(|| ApiError::Conflict("Session missing".into()))?;
                sessions::questions::answer(
                    &self.0,
                    session,
                    sessions::questions::AnswerAgentQuestion {
                        request_id: operation,
                        question_id: question.clone(),
                        answer,
                    },
                    &actor,
                )
                .await?;
                // A direct UI answer can win the race with identical content.
                // Report its actual durable command, not this remote receipt ID.
                sqlx::query_scalar::<_, Uuid>("SELECT request_id FROM agent_question_answers WHERE question_id=? AND session_id=?")
                    .bind(&question).bind(target.session_id).fetch_one(&self.0.db().pool).await?
            }
            Kind::BlockingSpecification => {
                // dispatch_control owns the same lock and unanswered-state check
                // for both direct UI input and MCP input.
                let _ = agent_runs::dispatch_control_for_input(
                    &self.0,
                    target.agent_run_id,
                    identity(operation, format!("mcp:{operation}")),
                    AgentRunPortCommand::SubmitInput {
                        input_id: question,
                        content: answer,
                    },
                    event,
                )
                .await?;
                operation
            }
            Kind::Approval => {
                return Err(ApiError::Forbidden(
                    "Resolve execution approval in LVK".into(),
                ));
            }
        };
        Ok(
            json!({"agent_run_id":target.agent_run_id,"session_id":target.session_id,"command_id":accepted_command,"reused_answer":accepted_command!=operation,"answer_accepted":true,"implementation_complete":false}),
        )
    }
}

/// Verify parent IDs and current lifecycle without invoking a model. The real
/// dispatch adapter repeats this under its existing Session lock.
pub(super) async fn validate_run(
    pool: &SqlitePool,
    target: &Target,
    terminal: bool,
) -> Result<(), ApiError> {
    let belongs: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM local_workspace_links l JOIN local_issues i ON i.id=l.issue_id AND i.project_id=l.project_id WHERE l.workspace_id=? AND l.project_id=?)")
        .bind(target.workspace_id).bind(target.project_id).fetch_one(pool).await?;
    if !belongs {
        return Err(ApiError::Conflict(
            "Workspace no longer belongs to the selected Project".into(),
        ));
    }
    let row=sqlx::query("SELECT r.id,r.workspace_id,r.status,s.workspace_id AS session_workspace FROM agent_runs r JOIN sessions s ON s.id=r.session_id WHERE r.session_id=? ORDER BY r.created_at DESC,r.rowid DESC LIMIT 1")
        .bind(target.session_id).fetch_optional(pool).await?.ok_or_else(||ApiError::Conflict("Session has no current run".into()))?;
    if row.try_get::<Uuid, _>("id")? != target.agent_run_id
        || row.try_get::<Uuid, _>("workspace_id")? != target.workspace_id
        || row.try_get::<Uuid, _>("session_workspace")? != target.workspace_id
    {
        return Err(ApiError::Conflict(
            "Run selection is stale or does not belong to this Session and Workspace".into(),
        ));
    }
    let status: String = row.try_get("status")?;
    let valid = if terminal {
        matches!(
            status.as_str(),
            "succeeded" | "failed" | "cancelled" | "crashed" | "audit_failed"
        )
    } else {
        matches!(
            status.as_str(),
            "pending" | "starting" | "running" | "awaiting_input" | "awaiting_approval"
        )
    };
    if !valid {
        return Err(ApiError::Conflict(
            "Run lifecycle changed; refresh before choosing steering or continuation".into(),
        ));
    }
    let workspace = Workspace::find_by_id(pool, target.workspace_id)
        .await?
        .ok_or_else(|| ApiError::Conflict("Workspace missing".into()))?;
    workspace.require_interactive()?;
    if workspace.archived {
        return Err(ApiError::Conflict("Workspace is archived".into()));
    }
    Ok(())
}

pub(super) fn tools() -> Vec<rmcp::model::Tool> {
    let mut tools = vec![
        definition::<ChangeArgs>(
            "steer_execution",
            "Send an explicit new instruction to the selected current running Session. Returns an operation receipt; delivery does not prove implementation. Do not answer a specification question with this tool.",
        ),
        definition::<ChangeArgs>(
            "continue_execution",
            "Continue the selected latest finished run in its existing Session using its saved executor configuration. Never redirects to another Workspace or silently restarts a running run.",
        ),
        definition::<AnswerArgs>(
            "answer_question",
            "Answer exactly one current specification question after reading get_question. Requires its ID, kind, run and revision. Already answered or changed questions conflict; execution approvals require the LVK link.",
        ),
    ];
    for tool in &mut tools {
        let mut value = serde_json::to_value(&*tool).expect("tool schema");
        value["annotations"] = json!({"readOnlyHint":false,"destructiveHint":true,"idempotentHint":true,"openWorldHint":true});
        value["_meta"] =
            json!({"securitySchemes":[{"type":"oauth2","scopes":["lvk:read","lvk:write"]}]});
        *tool = serde_json::from_value(value).expect("runtime tool schema");
    }
    tools
}
fn failure(error: ApiError) -> (&'static str, Value) {
    match error {
        ApiError::Conflict(_) | ApiError::Forbidden(_) => (
            "failed",
            json!({"code":"state_conflict","message":"The selected run or question changed, or the domain service rejected this action; refresh its state in LVK"}),
        ),
        _ => (
            "uncertain",
            json!({"code":"dispatch_outcome_unavailable","message":"Inspect this operation, Session and command delivery before any new request"}),
        ),
    }
}
fn dispatch_failure() -> (&'static str, Value) {
    // The domain may have accepted a command before a later state read failed.
    // A typed API error alone does not prove that no side effect occurred.
    (
        "uncertain",
        json!({"code":"dispatch_outcome_unavailable","message":"Inspect the operation, Session and command delivery before any new request"}),
    )
}
fn text(value: &str) -> anyhow::Result<()> {
    ensure!(
        !value.trim().is_empty() && value.len() <= 32000,
        "Instruction or answer must contain 1–32000 bytes"
    );
    Ok(())
}
fn runtime(remote: &Remote) -> anyhow::Result<Arc<dyn RuntimeActions>> {
    remote
        .runtime
        .clone()
        .ok_or_else(|| anyhow::anyhow!("Runtime dispatch unavailable"))
}

pub(super) async fn call(
    remote: &Remote,
    principal: &Principal,
    name: &str,
    input: Value,
) -> anyhow::Result<Value> {
    ensure!(
        principal.allows("lvk:read") && principal.allows("lvk:write"),
        "Read and write permissions required"
    );
    let dispatch = runtime(remote)?;
    let owned = remote.clone();
    let actor = remote.principal_key(principal);
    let receipt = if name == "answer_question" {
        let args: AnswerArgs = arguments(input.clone())?;
        text(&args.answer)?;
        ensure!(
            args.kind != Kind::Approval,
            "Execution approvals must be handled in LVK"
        );
        ensure!(
            !args.question_id.is_empty() && args.question_id.len() <= 1000,
            "Invalid question ID"
        );
        remote
            .require_workspace(args.target.project_id, args.target.workspace_id)
            .await?;
        remote.operations().submit(actor.clone(),args.request_id,args.target.project_id,name.into(),input,json!({"workspace_id":args.target.workspace_id,"session_id":args.target.session_id,"agent_run_id":args.target.agent_run_id,"question_id":args.question_id}),move |operation| async move {
            if let Err(error)=validate_run(&owned.pool,&args.target,false).await {return failure(error);}
            let pending=owned.pending_questions(args.target.project_id,args.target.workspace_id,args.target.session_id).await;
            let Ok((issue,questions))=pending else {return ("failed",json!({"code":"question_state_unavailable"}));};
            let question = questions.iter().find(|q|q.id==args.question_id && q.run==args.target.agent_run_id && q.kind==args.kind && q.revision()==args.expected_revision && q.delivery_status.is_none());
            let Some(question) = question else {
                return ("failed",json!({"code":"question_conflict","message":"Question changed, expired, already answered or delivery is unconfirmed; refresh its current state"}));
            };
            if args.kind == Kind::BlockingSpecification && question.event_id.is_none() {
                return ("failed",json!({"code":"question_identity_unavailable"}));
            }
            let event = question.event_id;
            let url=owned.config.workspace_url(args.target.project_id,issue,args.target.workspace_id);
            match dispatch.answer(args.target,operation,args.question_id,args.answer,args.kind,format!("mcp:{actor}"),event).await {
                Ok(mut value)=>{value["url"]=json!(url);("succeeded",value)},
                Err(_error)=>dispatch_failure(),
            }
        }).await?
    } else {
        let args: ChangeArgs = arguments(input.clone())?;
        text(&args.prompt)?;
        let kind = match name {
            "steer_execution" => Change::Steer,
            "continue_execution" => Change::Continue,
            _ => anyhow::bail!("Unknown runtime operation"),
        };
        let issue = remote
            .require_workspace(args.target.project_id, args.target.workspace_id)
            .await?;
        remote.operations().submit(actor,args.request_id,args.target.project_id,name.into(),input,json!({"workspace_id":args.target.workspace_id,"session_id":args.target.session_id,"agent_run_id":args.target.agent_run_id}),move |operation| async move {
            if let Err(error)=validate_run(&owned.pool,&args.target,matches!(kind,Change::Continue)).await {return failure(error);}
            let url=owned.config.workspace_url(args.target.project_id,issue,args.target.workspace_id);
            match dispatch.change(args.target,operation,args.prompt,kind).await {
                Ok(mut value)=>{value["url"]=json!(url);("succeeded",value)},
                Err(_error)=>dispatch_failure(),
            }
        }).await?
    };
    Ok(remote.receipt_value(receipt))
}
