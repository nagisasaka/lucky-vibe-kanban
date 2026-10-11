//! Durable answers to Codex's nonblocking questions. Both the Workspace UI and
//! project conversation use this client of existing steering/follow-up paths.
use axum::{Extension, Json, extract::State};
use db::models::session::Session;
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
    // Questions belong to the attempt that emitted them, not to the whole
    // conversation. Keep the audit event, but expire its answer UI when that
    // attempt ends; a later run/retry must not revive it.
    let events: Vec<(Uuid, Uuid, Uuid, String, String)> = sqlx::query_as(
        "SELECT e.run_attempt_id, e.agent_run_id, r.workspace_id, e.event_envelope, e.created_at
         FROM agent_events e
         JOIN agent_runs r ON r.id=e.agent_run_id
         JOIN agent_run_attempts a ON a.id=e.run_attempt_id AND a.agent_run_id=r.id
         WHERE e.session_id=?
           AND r.status IN ('pending','starting','running','awaiting_input','awaiting_approval')
           AND a.status IN ('pending','starting','running','awaiting_input','awaiting_approval')
           AND NOT EXISTS(SELECT 1 FROM agent_run_attempts newer
                          WHERE newer.agent_run_id=r.id AND newer.attempt_number>a.attempt_number)
           AND json_extract(e.event_envelope,'$.payload.type')='provider_extension'
           AND json_extract(e.event_envelope,'$.payload.data.provider_namespace')='codex'
           AND json_extract(e.event_envelope,'$.payload.data.provider_event')='async_questions'
         ORDER BY e.created_at",
    )
    .bind(session)
    .fetch_all(pool)
    .await?;
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
            let receipt: Option<(Option<String>, Uuid)> = sqlx::query_as(
                "SELECT result,request_id FROM agent_question_answers WHERE question_id=? AND session_id=?",
            )
            .bind(&id)
            .bind(session)
            .fetch_optional(pool)
            .await?;
            if receipt.as_ref().is_some_and(|(result, _)| result.is_some()) {
                continue;
            }
            let delivery_status = match receipt {
                Some((_, request)) => pending_delivery_status(pool, session, request).await?,
                None => None,
            };
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
    let session_id = session.id;
    let pool = &deployment.db().pool;
    let _lock = deployment
        .queued_message_service()
        .lock_session(session_id)
        .await;
    // Reconcile under the same lock as delivery. GET/list must never release a
    // reservation while its sender is still between reservation and dispatch.
    reconcile_answers(pool, session_id).await?;
    release_unstarted_answer(pool, session_id, &request.question_id).await?;
    let existing: Option<(Uuid, String, Option<String>)> = sqlx::query_as(
        "SELECT session_id,answer,result FROM agent_question_answers WHERE question_id=?",
    )
    .bind(&request.question_id)
    .fetch_optional(pool)
    .await?;
    if let Some((answer_session_id, answer, result)) = existing {
        if answer_session_id != session_id || answer != request.answer {
            return Err(ApiError::Conflict("Question already has an answer".into()));
        }
        return result.map(|s|serde_json::from_str(&s).map_err(|_|ApiError::Conflict("Unreadable answer receipt".into()))).unwrap_or_else(||Err(ApiError::Conflict("Answer delivery is unconfirmed. Inspect the Session; do not submit another answer.".into())));
    }
    if deployment.queued_message_service().has_queued(session_id) {
        return Err(ApiError::Conflict(
            "Resume or clear the queued follow-up first".into(),
        ));
    }
    let question = pending(pool, session_id)
        .await?
        .into_iter()
        .find(|q| q.id == request.question_id)
        .ok_or_else(|| {
            ApiError::Conflict(
                "Question has expired, has already been answered, or does not belong to this Session".into(),
            )
        })?;
    // Always steer the run which asked. Never start a follow-up or redirect a
    // late answer to a different run; dispatch revalidates lifecycle as well.
    let run_id = question.agent_run_id;
    sqlx::query("INSERT INTO agent_question_answers(question_id,session_id,request_id,actor,answer) VALUES(?,?,?,?,?)").bind(&request.question_id).bind(session_id).bind(request.request_id).bind(actor).bind(&request.answer).execute(pool).await?;
    let content = format!(
        "Question: {}\nAnswer: {}\n\nLVK question: {}\nAstra operation: {}",
        question.title,
        request.answer.trim(),
        question.id,
        request.request_id
    );
    let delivery: Result<Value, ApiError> = async {
        let response = agent_runs::dispatch_control(
            deployment,
            run_id,
            agent_runs::AgentRunControlIdentity {
                command_id: request.request_id,
                idempotency_key: format!("question:{}", request.question_id),
                correlation_id: request.request_id,
                created_at: chrono::Utc::now(),
            },
            AgentRunPortCommand::Steer { content },
        )
        .await?;
        Ok(json!({"accepted":true,"agent_run_id":run_id,"state":response.0.into_data()}))
    }
    .await;
    let result = match delivery {
        Ok(result) => result,
        Err(error) => {
            // A validation/database failure before dispatch is safe to retry.
            // Once a command or run exists, preserve the receipt: an error may
            // have happened after delivery and must never cause a duplicate.
            release_unstarted_answer(pool, session_id, &request.question_id).await?;
            return Err(error);
        }
    };
    sqlx::query("UPDATE agent_question_answers SET result=? WHERE question_id=?")
        .bind(result.to_string())
        .bind(&request.question_id)
        .execute(pool)
        .await?;
    Ok(result)
}

// A reservation alone must not disable the UI forever after a pre-dispatch
// failure. Reading this status never releases an in-flight reservation: submit
// still serializes with its sender and rechecks dispatch evidence under the lock.
async fn pending_delivery_status(
    pool: &SqlitePool,
    session: Uuid,
    request: Uuid,
) -> Result<Option<String>, ApiError> {
    let started: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM agent_run_commands WHERE command_id=?)
         OR EXISTS(SELECT 1 FROM agent_runs WHERE session_id=? AND instr(request_envelope,?)>0)",
    )
    .bind(request)
    .bind(session)
    .bind(format!("Astra operation: {request}"))
    .fetch_one(pool)
    .await?;
    Ok(started.then(|| "unconfirmed".to_owned()))
}

/// Caller must hold the session queue lock across this check and dispatch.
async fn release_unstarted_answer(
    pool: &SqlitePool,
    session: Uuid,
    question: &str,
) -> Result<(), ApiError> {
    let request: Option<Uuid> = sqlx::query_scalar(
        "SELECT request_id FROM agent_question_answers WHERE session_id=? AND question_id=? AND result IS NULL",
    ).bind(session).bind(question).fetch_optional(pool).await?;
    if let Some(request) = request {
        sqlx::query(
            "DELETE FROM agent_question_answers WHERE session_id=? AND question_id=? AND result IS NULL
             AND NOT EXISTS(SELECT 1 FROM agent_run_commands WHERE command_id=?)
             AND NOT EXISTS(SELECT 1 FROM agent_runs WHERE session_id=? AND instr(request_envelope,?)>0)",
        ).bind(session).bind(question).bind(request).bind(session)
            .bind(format!("Astra operation: {request}"))
            .execute(pool).await?;
    }
    Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;

    async fn database() -> SqlitePool {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        sqlx::raw_sql(
            "CREATE TABLE agent_runs (id BLOB, session_id BLOB, status TEXT, request_envelope TEXT, created_at TEXT, workspace_id BLOB);
             CREATE TABLE agent_run_attempts (id BLOB, agent_run_id BLOB, attempt_number INTEGER, status TEXT);
             CREATE TABLE agent_events (run_attempt_id BLOB, agent_run_id BLOB, session_id BLOB, event_envelope TEXT, created_at TEXT);
             CREATE TABLE agent_run_commands (command_id BLOB, delivery_status TEXT);
             CREATE TABLE agent_question_answers (question_id TEXT PRIMARY KEY, session_id BLOB, request_id BLOB, result TEXT);",
        ).execute(&pool).await.unwrap();
        pool
    }

    #[tokio::test]
    async fn questions_expire_with_their_originating_attempt_without_erasing_history() {
        let pool = database().await;
        let session = Uuid::new_v4();
        let run = Uuid::new_v4();
        let attempt = Uuid::new_v4();
        sqlx::query("INSERT INTO agent_runs VALUES(?,?,'running','{}','2026-10-10',?)")
            .bind(run)
            .bind(session)
            .bind(Uuid::new_v4())
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO agent_run_attempts VALUES(?,?,1,'running')")
            .bind(attempt)
            .bind(run)
            .execute(&pool)
            .await
            .unwrap();
        let envelope = json!({"payload":{"type":"provider_extension","data":{
            "provider_namespace":"codex","provider_event":"async_questions",
            "payload":{"schema_version":1,"message_id":"question-message",
                "questions":[{"title":"Which color?","options":["Blue"]}]}
        }}});
        sqlx::query("INSERT INTO agent_events VALUES(?,?,?,?, '2026-10-10')")
            .bind(attempt)
            .bind(run)
            .bind(session)
            .bind(envelope.to_string())
            .execute(&pool)
            .await
            .unwrap();
        assert_eq!(pending(&pool, session).await.unwrap().len(), 1);
        assert!(pending(&pool, Uuid::new_v4()).await.unwrap().is_empty());
        for status in [
            "pending",
            "starting",
            "running",
            "awaiting_input",
            "awaiting_approval",
            "cancelling",
            "succeeded",
            "failed",
            "cancelled",
            "crashed",
            "audit_failed",
        ] {
            let active = matches!(
                status,
                "pending" | "starting" | "running" | "awaiting_input" | "awaiting_approval"
            );
            sqlx::query("UPDATE agent_runs SET status=?")
                .bind(status)
                .execute(&pool)
                .await
                .unwrap();
            assert_eq!(
                pending(&pool, session).await.unwrap().len(),
                usize::from(active),
                "run {status}"
            );
            sqlx::query("UPDATE agent_runs SET status='running'")
                .execute(&pool)
                .await
                .unwrap();
            sqlx::query("UPDATE agent_run_attempts SET status=?")
                .bind(status)
                .execute(&pool)
                .await
                .unwrap();
            assert_eq!(
                pending(&pool, session).await.unwrap().len(),
                usize::from(active),
                "attempt {status}"
            );
            sqlx::query("UPDATE agent_run_attempts SET status='running'")
                .execute(&pool)
                .await
                .unwrap();
        }
        // Even an incompletely updated old attempt cannot resurface on retry.
        sqlx::query("INSERT INTO agent_run_attempts VALUES(?,?,2,'running')")
            .bind(Uuid::new_v4())
            .bind(run)
            .execute(&pool)
            .await
            .unwrap();
        assert!(pending(&pool, session).await.unwrap().is_empty());
        let count: i64 = sqlx::query_scalar("SELECT count(*) FROM agent_events")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 1);

        // A new conversation turn exposes only its own questions.
        sqlx::query("UPDATE agent_runs SET status='succeeded'")
            .execute(&pool)
            .await
            .unwrap();
        let next_run = Uuid::new_v4();
        let next_attempt = Uuid::new_v4();
        sqlx::query("INSERT INTO agent_runs VALUES(?,?,'running','{}','2026-10-11',?)")
            .bind(next_run)
            .bind(session)
            .bind(Uuid::new_v4())
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO agent_run_attempts VALUES(?,?,1,'running')")
            .bind(next_attempt)
            .bind(next_run)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO agent_events VALUES(?,?,?,?, '2026-10-11')")
            .bind(next_attempt)
            .bind(next_run)
            .bind(session)
            .bind(envelope.to_string())
            .execute(&pool)
            .await
            .unwrap();
        let questions = pending(&pool, session).await.unwrap();
        assert_eq!(questions.len(), 1);
        assert_eq!(questions[0].agent_run_id, next_run);
    }

    #[tokio::test]
    async fn retries_only_answers_without_dispatch_evidence() {
        let pool = database().await;
        let session = Uuid::new_v4();
        for evidence in [
            "none",
            "pending",
            "delivering",
            "failed",
            "delivered",
            "run",
            "receipt",
        ] {
            let request = Uuid::new_v4();
            sqlx::query("INSERT INTO agent_question_answers VALUES(?,?,?,?)")
                .bind(evidence)
                .bind(session)
                .bind(request)
                .bind(if evidence == "receipt" {
                    Some("{}")
                } else {
                    None
                })
                .execute(&pool)
                .await
                .unwrap();
            if matches!(evidence, "pending" | "delivering" | "failed" | "delivered") {
                sqlx::query("INSERT INTO agent_run_commands VALUES(?,?)")
                    .bind(request)
                    .bind(evidence)
                    .execute(&pool)
                    .await
                    .unwrap();
            }
            if evidence == "run" {
                sqlx::query("INSERT INTO agent_runs(id,session_id,status,request_envelope,created_at) VALUES(?,?,'succeeded',?,'2026-10-10')")
                    .bind(Uuid::new_v4())
                    .bind(session)
                    .bind(
                        json!({"input":{"content":format!("Astra operation: {request}")}})
                            .to_string(),
                    )
                    .execute(&pool)
                    .await
                    .unwrap();
            }
            // Listing can offer a retry for an orphan reservation without
            // deleting it, even while its original sender is still running.
            assert_eq!(
                pending_delivery_status(&pool, session, request)
                    .await
                    .unwrap()
                    .is_some(),
                matches!(
                    evidence,
                    "pending" | "delivering" | "failed" | "delivered" | "run"
                ),
                "{evidence}",
            );
            // A different session cannot release this reservation.
            release_unstarted_answer(&pool, Uuid::new_v4(), evidence)
                .await
                .unwrap();
            let before: i64 = sqlx::query_scalar(
                "SELECT count(*) FROM agent_question_answers WHERE question_id=?",
            )
            .bind(evidence)
            .fetch_one(&pool)
            .await
            .unwrap();
            assert_eq!(before, 1);
            release_unstarted_answer(&pool, session, evidence)
                .await
                .unwrap();
            let after: i64 = sqlx::query_scalar(
                "SELECT count(*) FROM agent_question_answers WHERE question_id=?",
            )
            .bind(evidence)
            .fetch_one(&pool)
            .await
            .unwrap();
            assert_eq!(after, i64::from(evidence != "none"), "{evidence}");
        }
        reconcile_answers(&pool, session).await.unwrap();
        for evidence in ["pending", "delivering", "failed", "delivered", "run"] {
            let result: Option<String> =
                sqlx::query_scalar("SELECT result FROM agent_question_answers WHERE question_id=?")
                    .bind(evidence)
                    .fetch_one(&pool)
                    .await
                    .unwrap();
            assert_eq!(result.is_some(), matches!(evidence, "delivered" | "run"));
        }
    }
}
