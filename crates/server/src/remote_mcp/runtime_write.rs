//! Remote adapters for existing Session and AgentRun control paths. The trait
//! separates domain dispatch from MCP transport and permits isolated port tests.
use std::sync::Arc;

use anyhow::ensure;
use async_trait::async_trait;
use db::models::{session::Session, workspace::Workspace};
use deployment::Deployment;
use executors::{profile::NextRunOverrides, runtime::AgentRunPortCommand};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::{Row, SqlitePool};
use utils::mcp_error::Category;
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
    #[serde(default)]
    attachment_ids: Vec<Uuid>,
    /// Applies only to the next run. Omitted fields preserve saved settings; explicit action=reset removes that override. Null is invalid.
    #[serde(default)]
    overrides: NextRunOverrides,
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
#[derive(Clone)]
pub(super) enum Change {
    Steer,
    Continue(NextRunOverrides),
    PreparedContinue(executors::profile::ExecutorConfig),
}

#[async_trait]
pub(super) trait RuntimeActions: Send + Sync {
    async fn decide_approval(
        &self,
        _target: Target,
        _operation: Uuid,
        _approval: String,
        _approved: bool,
        _reason: Option<String>,
        _event: Uuid,
    ) -> Result<Value, ApiError> {
        Err(ApiError::Conflict("Approval runtime unavailable".into()))
    }

    async fn dev_server_status(&self, _project: Uuid, _workspace: Uuid) -> Result<Value, ApiError> {
        Err(ApiError::Conflict("DevServer runtime unavailable".into()))
    }

    async fn dev_server(
        &self,
        _project: Uuid,
        _workspace: Uuid,
        _start: bool,
    ) -> Result<Value, ApiError> {
        Err(ApiError::Conflict("DevServer runtime unavailable".into()))
    }

    async fn archive_scripts_settled(&self, _workspace: Uuid) -> Result<bool, ApiError> {
        Err(ApiError::Conflict("Archive runtime unavailable".into()))
    }

    async fn goal(
        &self,
        target: Target,
        operation: Uuid,
        action: super::goals::Action,
        revision: String,
    ) -> Result<Value, ApiError>;

    async fn prepare_change(&self, _target: &Target, kind: Change) -> Result<Change, ApiError> {
        Ok(kind)
    }

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

pub(super) fn identity(operation: Uuid, key: String) -> agent_runs::AgentRunControlIdentity {
    agent_runs::AgentRunControlIdentity {
        command_id: operation,
        correlation_id: operation,
        idempotency_key: key,
        created_at: chrono::Utc::now(),
    }
}

#[async_trait]
impl RuntimeActions for ExistingRuntime {
    async fn decide_approval(
        &self,
        target: Target,
        operation: Uuid,
        approval: String,
        approved: bool,
        reason: Option<String>,
        event: Uuid,
    ) -> Result<Value, ApiError> {
        let state = agent_runs::dispatch_control_for_input(
            &self.0,
            target.agent_run_id,
            identity(operation, format!("mcp:{operation}")),
            AgentRunPortCommand::ResolveApproval {
                approval_id: approval,
                approved,
                reason,
            },
            Some(event),
        )
        .await?
        .0
        .into_data();
        Ok(
            json!({"command_id":operation,"decision_accepted":true,"decision_confirmed":false,
            "tool_execution_complete":null,"tool_execution_status":"not_observed","state":state}),
        )
    }

    async fn dev_server_status(&self, project: Uuid, workspace: Uuid) -> Result<Value, ApiError> {
        crate::routes::workspaces::dev_server::status(&self.0, project, workspace).await
    }

    async fn dev_server(
        &self,
        project: Uuid,
        workspace: Uuid,
        start: bool,
    ) -> Result<Value, ApiError> {
        crate::routes::workspaces::dev_server::change(&self.0, project, workspace, start).await
    }

    async fn archive_scripts_settled(&self, workspace: Uuid) -> Result<bool, ApiError> {
        use services::services::container::ContainerService;
        Ok(self.0.container().scripts_settled(workspace).await?)
    }

    async fn goal(
        &self,
        target: Target,
        operation: Uuid,
        action: super::goals::Action,
        revision: String,
    ) -> Result<Value, ApiError> {
        super::goals::dispatch(&self.0, target, operation, action, revision).await
    }

    async fn prepare_change(&self, target: &Target, kind: Change) -> Result<Change, ApiError> {
        if let Change::Continue(overrides) = &kind {
            if self
                .0
                .queued_message_service()
                .has_queued(target.session_id)
            {
                return Err(ApiError::BadRequest(
                    "Resume or clear the follow-up queue before changing next-run settings".into(),
                ));
            }
            let pool = &self.0.db().pool;
            let saved = sessions::executor_config::read_executor_config(pool, target.session_id)
                .await?
                .ok_or_else(|| {
                    ApiError::Conflict("Stored Session configuration unavailable".into())
                })?;
            let workspace = Workspace::find_by_id(pool, target.workspace_id)
                .await?
                .ok_or_else(|| ApiError::Conflict("Workspace missing".into()))?;
            let path = workspace
                .container_ref
                .as_deref()
                .ok_or_else(|| ApiError::Conflict("Workspace path unavailable".into()))?;
            let config = sessions::executor_config::apply_next_run_overrides(
                &saved,
                overrides,
                std::path::Path::new(path),
            )
            .await?;
            let session = Session::find_by_id(pool, target.session_id)
                .await?
                .ok_or_else(|| ApiError::Conflict("Session missing".into()))?;
            sessions::validate_queued_follow_up_profile(pool, &session, &config, None)
                .await.map_err(|_| ApiError::BadRequest(
                    "Session provider/runtime profile or native_adopted fingerprint is fixed; use a new Session for this configuration change".into()
                ))?;
            return Ok(Change::PreparedContinue(config));
        }
        Ok(kind)
    }

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
            Change::Continue(_) => Err(ApiError::BadRequest(
                "Continuation settings were not prepared".into(),
            )),
            Change::PreparedContinue(executor_config) => {
                let session = Session::find_by_id(pool, target.session_id)
                    .await?
                    .ok_or_else(|| ApiError::Conflict("Session missing".into()))?;
                let accepted_config = executor_config.clone();
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
                    Some((
                        target.agent_run_id,
                        (!target.project_id.is_nil()).then_some(target.project_id),
                    )),
                )
                .await?
                .0
                .into_data()
                .ok_or_else(|| ApiError::Conflict("Continuation receipt unavailable".into()))?;
                Ok(
                    json!({"agent_run_id":result.agent_run_id,"session_id":target.session_id,"execution_accepted":true,"implementation_complete":false,"conversation":"existing_session","executor_config":accepted_config,"provider_settings_verified":false}),
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
                    "Use decide_execution_approval for execution decisions".into(),
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
    validate_target(pool, target, terminal, false).await
}

pub(super) async fn validate_approval_target(
    pool: &SqlitePool,
    target: &Target,
) -> Result<(), ApiError> {
    // Execution-only workspaces have legitimate provider approvals too. The
    // shared UI dispatcher validates the live execution owner before dispatch.
    validate_target(pool, target, false, true).await
}

async fn validate_target(
    pool: &SqlitePool,
    target: &Target,
    terminal: bool,
    approval: bool,
) -> Result<(), ApiError> {
    let belongs: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM local_workspace_links l JOIN local_issues i ON i.id=l.issue_id AND i.project_id=l.project_id WHERE l.workspace_id=? AND l.project_id=?)")
        .bind(target.workspace_id).bind(target.project_id).fetch_one(pool).await?;
    let unlinked_internal_target = target.project_id.is_nil()
        && !sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(SELECT 1 FROM local_workspace_links WHERE workspace_id=?)",
        )
        .bind(target.workspace_id)
        .fetch_one(pool)
        .await?;
    // Remote adapters require a real Project first. Only the internal adapter
    // can retain the legacy unlinked Workspace case; this is not a scope grant.
    if !belongs && !unlinked_internal_target {
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
    if !approval {
        workspace.require_interactive()?;
    }
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
            "Continue the selected latest finished run in the same Session/native conversation. overrides apply to the next run only; omitted fields retain saved values, action=reset removes an override, null is invalid. Engine changes require a new Session. Review reports and stops; Code selection grants no implementation/merge/publication permission. Never stops a running run or bypasses a queue.",
        ),
        definition::<AnswerArgs>(
            "answer_question",
            "Answer exactly one current specification question after reading get_question. Requires its ID, kind, run and revision. Already answered or changed questions conflict; execution approvals use decide_execution_approval.",
        ),
    ];
    for tool in &mut tools {
        let mut value = serde_json::to_value(&*tool).expect("tool schema");
        if tool.name == "steer_execution"
            && let Some(properties) = value["inputSchema"]["properties"].as_object_mut()
        {
            properties.remove("overrides");
        }
        value["annotations"] = json!({"readOnlyHint":false,"destructiveHint":true,"idempotentHint":true,"openWorldHint":true});
        value["_meta"] =
            json!({"securitySchemes":[{"type":"oauth2","scopes":["lvk:read","lvk:write"]}]});
        *tool = serde_json::from_value(value).expect("runtime tool schema");
    }
    tools
}
fn failure(error: ApiError) -> (&'static str, Value) {
    // validate_run has not dispatched a command. Its failure is terminal even
    // when the cause is a database outage rather than a state conflict.
    let diagnostic = super::errors::diagnostic(&error.into(), false);
    let code = match diagnostic.category {
        utils::mcp_error::Category::Conflict => "state_conflict",
        utils::mcp_error::Category::Permission => "permission_denied",
        _ => "target_validation_failed",
    };
    (
        "failed",
        json!({"code":code,"message":diagnostic.message,"error":diagnostic.value()}),
    )
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
        super::errors::rejection_with_message(
            utils::mcp_error::Category::Validation,
            "Instruction or answer must contain 1–32000 bytes"
        )
    );
    Ok(())
}
fn runtime(remote: &Remote) -> anyhow::Result<Arc<dyn RuntimeActions>> {
    remote
        .runtime
        .clone()
        .ok_or_else(|| super::errors::rejection(Category::Unavailable))
}

pub(super) async fn call(
    remote: &Remote,
    principal: &Principal,
    name: &str,
    input: Value,
) -> anyhow::Result<Value> {
    ensure!(
        principal.allows("lvk:read") && principal.allows("lvk:write"),
        super::errors::rejection_with_message(
            utils::mcp_error::Category::Permission,
            "Read and write permissions required"
        )
    );
    let dispatch = runtime(remote)?;
    let owned = remote.clone();
    let actor = remote.principal_key(principal);
    let receipt = if name == "answer_question" {
        let args: AnswerArgs = arguments(input.clone())?;
        text(&args.answer)?;
        ensure!(
            args.kind != Kind::Approval,
            super::errors::rejection_with_message(
                utils::mcp_error::Category::Permission,
                "Specification answers cannot grant execution permission; use decide_execution_approval"
            )
        );
        ensure!(
            !args.question_id.is_empty() && args.question_id.len() <= 1000,
            super::errors::rejection_with_message(
                utils::mcp_error::Category::Validation,
                "Invalid question ID"
            )
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
        ensure!(
            name != "steer_execution" || input.get("overrides").is_none(),
            super::errors::rejection_with_message(
                Category::Validation,
                "Steering accepts text only; continue a finished run to change settings"
            )
        );
        let args: ChangeArgs = arguments(input.clone())?;
        text(&args.prompt)?;
        let kind = match name {
            "steer_execution" => Change::Steer,
            "continue_execution" => Change::Continue(args.overrides.clone()),
            _ => return Err(super::errors::rejection(Category::Validation)),
        };
        ensure!(
            !matches!(kind, Change::Steer) || args.overrides.is_empty(),
            super::errors::rejection_with_message(
                Category::Validation,
                "Steering accepts text only; use continuation after the run finishes to change settings"
            )
        );
        let issue = remote
            .require_workspace(args.target.project_id, args.target.workspace_id)
            .await?;
        remote.operations().submit(actor,args.request_id,args.target.project_id,name.into(),input,json!({"workspace_id":args.target.workspace_id,"session_id":args.target.session_id,"agent_run_id":args.target.agent_run_id}),move |operation| async move {
            if let Err(error)=validate_run(&owned.pool,&args.target,matches!(kind,Change::Continue(_))).await {return failure(error);}
            let kind = match dispatch.prepare_change(&args.target, kind).await {
                Ok(kind) => kind,
                Err(error) => {
                    let message = match &error {
                        ApiError::BadRequest(message) => message.as_str(),
                        _ => "Settings validation unavailable; inspect Session settings before retrying",
                    };
                    return ("failed",json!({"code":"configuration_rejected","message":message,"execution_accepted":false}));
                }
            };
            let url=owned.config.workspace_url(args.target.project_id,issue,args.target.workspace_id);
            let mut prompt=args.prompt;
            if !args.attachment_ids.is_empty() {
                let Some(launch)=owned.launch.as_ref() else {return ("failed",json!({"code":"attachment_service_unavailable"}));};
                let placement=launch.place_for_execution(crate::routes::attachment_transfer::Place {project_id:args.target.project_id,workspace_id:args.target.workspace_id,session_id:args.target.session_id,request_id:operation,attachment_ids:args.attachment_ids}).await;
                match placement {
                    Ok(v) if v["all_verified"]==true => {
                        for r in v["results"].as_array().into_iter().flatten() {
                            if let Some(path)=r["attachment"]["file_path"].as_str() { prompt.push_str(&format!("\n\nAttachment: `{path}`")); }
                        }
                    },
                    _=>return ("failed",json!({"code":"attachments_not_verified","execution_accepted":false})),
                }
            }
            match dispatch.change(args.target,operation,prompt,kind).await {
                Ok(mut value)=>{value["url"]=json!(url);("succeeded",value)},
                Err(_error)=>dispatch_failure(),
            }
        }).await?
    };
    Ok(remote.receipt_value(receipt))
}
