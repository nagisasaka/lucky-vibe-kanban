//! Durable answers to Codex's nonblocking questions. Both the Workspace UI and
//! project conversation use this client of existing steering/follow-up paths.
use axum::{Extension, Json, extract::State};
use db::models::{agent_runtime::AgentRunRecord, session::Session};
use deployment::Deployment;
use executors::runtime::AgentRunPortCommand;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::SqlitePool;
use ts_rs::TS;
use uuid::Uuid;

use crate::{DeploymentImpl, error::ApiError, routes::agent_runs};

#[derive(Debug, Serialize, Deserialize, TS)]
pub struct PendingAgentQuestion {
    pub id: String,
    pub session_id: Uuid,
    pub agent_run_id: Uuid,
    pub workspace_id: Uuid,
    pub title: String,
    pub options: Vec<String>,
    pub delivery_status: Option<String>,
}

#[derive(Debug, Deserialize, TS)]
pub struct AnswerAgentQuestion {
    pub request_id: Uuid,
    pub question_id: String,
    pub answer: String,
}

pub(crate) async fn pending(
    pool: &SqlitePool,
    session: Uuid,
) -> Result<Vec<PendingAgentQuestion>, ApiError> {
    reconcile_answers(pool, session).await?;
    let events:Vec<(Uuid,Uuid,Uuid,String,String)>=sqlx::query_as("SELECT e.run_attempt_id,e.agent_run_id,r.workspace_id,e.event_envelope,e.created_at FROM agent_events e JOIN agent_runs r ON r.id=e.agent_run_id WHERE e.session_id=? AND json_extract(e.event_envelope,'$.payload.type')='provider_extension' AND json_extract(e.event_envelope,'$.payload.data.provider_namespace')='codex' AND json_extract(e.event_envelope,'$.payload.data.provider_event')='async_questions' ORDER BY e.created_at").bind(session).fetch_all(pool).await?;
    let mut result = Vec::new();
    for (attempt, run, workspace, envelope, created_at) in events {
        let envelope: Value = serde_json::from_str(&envelope)
            .map_err(|_| ApiError::BadRequest("Invalid question event".into()))?;
        let payload = &envelope["payload"]["data"]["payload"];
        if payload["schema_version"] != 1 {
            continue;
        }
        let Some(message) = payload["message_id"].as_str() else {
            continue;
        };
        let Some(questions) = payload["questions"].as_array() else {
            continue;
        };
        for (index, question) in questions.iter().enumerate() {
            let Some(title) = question["title"].as_str().filter(|s| !s.trim().is_empty()) else {
                continue;
            };
            let id = format!("{attempt}:{message}:{index}");
            let receipt: Option<Option<String>> = sqlx::query_scalar(
                "SELECT result FROM agent_question_answers WHERE question_id=? AND session_id=?",
            )
            .bind(&id)
            .bind(session)
            .fetch_optional(pool)
            .await?;
            if receipt.as_ref().is_some_and(Option::is_some) {
                continue;
            }
            let delivery_status = receipt.map(|_| "unconfirmed".to_owned());
            // Older Workspace clients sent this exact format without an ID.
            // Reconcile their canonical input instead of asking it again.
            let prefix = format!("Question: {title}\nAnswer: ");
            let legacy:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM agent_events WHERE session_id=? AND created_at>=? AND json_extract(event_envelope,'$.payload.type')='message' AND json_extract(event_envelope,'$.payload.data.message.role')='user' AND substr(json_extract(event_envelope,'$.payload.data.message.content'),1,length(?))=?) OR EXISTS(SELECT 1 FROM agent_runs WHERE session_id=? AND created_at>=? AND substr(json_extract(request_envelope,'$.input.content'),1,length(?))=?)")
                .bind(session).bind(&created_at).bind(&prefix).bind(&prefix).bind(session).bind(&created_at).bind(&prefix).bind(&prefix).fetch_one(pool).await?;
            if legacy {
                continue;
            }
            result.push(PendingAgentQuestion {
                id,
                session_id: session,
                agent_run_id: run,
                workspace_id: workspace,
                title: title.into(),
                delivery_status,
                options: question["options"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
                    .map(str::to_owned)
                    .collect(),
            });
        }
    }
    Ok(result)
}

pub async fn list(
    Extension(session): Extension<Session>,
    State(deployment): State<DeploymentImpl>,
) -> Result<Json<utils::response::ApiResponse<Vec<PendingAgentQuestion>>>, ApiError> {
    Ok(Json(utils::response::ApiResponse::success(
        pending(&deployment.db().pool, session.id).await?,
    )))
}

pub async fn submit(
    Extension(session): Extension<Session>,
    State(deployment): State<DeploymentImpl>,
    Json(request): Json<AnswerAgentQuestion>,
) -> Result<Json<utils::response::ApiResponse<Value>>, ApiError> {
    let result = answer(&deployment, session, request, "local-user").await?;
    Ok(Json(utils::response::ApiResponse::success(result)))
}

pub(crate) async fn answer(
    deployment: &DeploymentImpl,
    session: Session,
    request: AnswerAgentQuestion,
    actor: &str,
) -> Result<Value, ApiError> {
    if request.answer.trim().is_empty() || request.answer.len() > 32_000 {
        return Err(ApiError::BadRequest(
            "Answer must contain 1–32000 bytes".into(),
        ));
    }
    let pool = &deployment.db().pool;
    let _lock = deployment
        .queued_message_service()
        .lock_session(session.id)
        .await;
    let existing: Option<(Uuid, String, Option<String>)> = sqlx::query_as(
        "SELECT session_id,answer,result FROM agent_question_answers WHERE question_id=?",
    )
    .bind(&request.question_id)
    .fetch_optional(pool)
    .await?;
    if let Some((session_id, answer, result)) = existing {
        if session_id != session.id || answer != request.answer {
            return Err(ApiError::Conflict("Question already has an answer".into()));
        }
        return result.map(|s|serde_json::from_str(&s).map_err(|_|ApiError::Conflict("Unreadable answer receipt".into()))).unwrap_or_else(||Err(ApiError::Conflict("Answer delivery is unconfirmed. Inspect the Session; do not submit another answer.".into())));
    }
    if deployment.queued_message_service().has_queued(session.id) {
        return Err(ApiError::Conflict(
            "Resume or clear the queued follow-up first".into(),
        ));
    }
    let question = pending(pool, session.id)
        .await?
        .into_iter()
        .find(|q| q.id == request.question_id)
        .ok_or_else(|| {
            ApiError::Conflict(
                "Question has already been answered or does not belong to this Session".into(),
            )
        })?;
    let run: AgentRunRecord = sqlx::query_as(
        "SELECT * FROM agent_runs WHERE session_id=? ORDER BY created_at DESC LIMIT 1",
    )
    .bind(session.id)
    .fetch_one(pool)
    .await?;
    sqlx::query("INSERT INTO agent_question_answers(question_id,session_id,request_id,actor,answer) VALUES(?,?,?,?,?)").bind(&request.question_id).bind(session.id).bind(request.request_id).bind(actor).bind(&request.answer).execute(pool).await?;
    let content = format!(
        "Question: {}\nAnswer: {}\n\nLVK question: {}\nAstra operation: {}",
        question.title,
        request.answer.trim(),
        question.id,
        request.request_id
    );
    let result = if run.status.is_terminal() {
        let executor = super::executor_config::read_executor_config(pool, session.id)
            .await?
            .ok_or_else(|| {
                ApiError::Conflict("Session executor configuration unavailable".into())
            })?;
        let snapshot = super::start_coding_agent_execution_for_session(
            deployment, session, content, None, executor, None, None,
        )
        .await?;
        json!({"accepted":true,"agent_run_id":snapshot.agent_run_id,"state":snapshot.state})
    } else {
        let response = agent_runs::dispatch_control(
            deployment,
            run.id,
            agent_runs::AgentRunControlIdentity {
                command_id: request.request_id,
                idempotency_key: format!("question:{}", request.question_id),
                correlation_id: request.request_id,
                created_at: chrono::Utc::now(),
            },
            AgentRunPortCommand::Steer { content },
        )
        .await?;
        json!({"accepted":true,"agent_run_id":run.id,"state":response.0.into_data()})
    };
    sqlx::query("UPDATE agent_question_answers SET result=? WHERE question_id=?")
        .bind(result.to_string())
        .bind(&request.question_id)
        .execute(pool)
        .await?;
    Ok(result)
}

async fn reconcile_answers(pool: &SqlitePool, session: Uuid) -> Result<(), ApiError> {
    let receipts:Vec<(String,Uuid)>=sqlx::query_as("SELECT question_id,request_id FROM agent_question_answers WHERE session_id=? AND result IS NULL").bind(session).fetch_all(pool).await?;
    for (question, request) in receipts {
        let command: Option<String> =
            sqlx::query_scalar("SELECT delivery_status FROM agent_run_commands WHERE command_id=?")
                .bind(request)
                .fetch_optional(pool)
                .await?;
        let run: Option<Uuid> = sqlx::query_scalar(
            "SELECT id FROM agent_runs WHERE session_id=? AND instr(request_envelope,?)>0 LIMIT 1",
        )
        .bind(session)
        .bind(format!("Astra operation: {request}"))
        .fetch_optional(pool)
        .await?;
        if command.as_deref() == Some("delivered") || run.is_some() {
            let result = json!({"accepted":true,"reconciled":true,"agent_run_id":run,"command_delivery":command,"completion_unverified":true});
            sqlx::query(
                "UPDATE agent_question_answers SET result=? WHERE question_id=? AND result IS NULL",
            )
            .bind(result.to_string())
            .bind(question)
            .execute(pool)
            .await?;
        }
    }
    Ok(())
}
