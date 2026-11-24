//! New canonical Run observations for a single local board.
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    routing::get,
};
use deployment::Deployment;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::SqlitePool;
use uuid::Uuid;

use crate::{DeploymentImpl, error::ApiError};

pub(super) fn router() -> Router<DeploymentImpl> {
    Router::new().route("/projects/{project_id}/run-observations", get(observations))
}

#[derive(Deserialize)]
struct QueryCursor {
    after: Option<i64>,
}

#[derive(Serialize)]
struct Observation {
    run_id: Uuid,
    attempt: u32,
    issue: String,
    session_number: i64,
    other_active_sessions: i64,
    provider: String,
    payload: Value,
}
#[derive(Serialize)]
struct Pending {
    run_id: Uuid,
    issue: String,
    reference: String,
}
#[derive(Serialize)]
struct Page {
    cursor: i64,
    events: Vec<Observation>,
    pending: Vec<Pending>,
    active: Vec<Uuid>,
}

async fn observations(
    State(d): State<DeploymentImpl>,
    Path(project): Path<Uuid>,
    Query(query): Query<QueryCursor>,
) -> Result<Json<Page>, ApiError> {
    Ok(Json(page(&d.db().pool, project, query.after).await?))
}

async fn page(pool: &SqlitePool, project: Uuid, after: Option<i64>) -> Result<Page, ApiError> {
    let mut tx = pool.begin().await?;
    // Both the baseline and pending snapshot come from the same SQLite snapshot.
    let baseline: i64 = sqlx::query_scalar("SELECT COALESCE(MAX(rowid), 0) FROM agent_events")
        .fetch_one(&mut *tx)
        .await?;
    let after = after.unwrap_or(baseline);
    let active: Vec<Uuid> = sqlx::query_scalar("SELECT r.id FROM agent_runs r JOIN local_workspace_links l ON l.workspace_id=r.workspace_id WHERE l.project_id=? AND r.status NOT IN ('succeeded','failed','cancelled','crashed','audit_failed')")
        .bind(project).fetch_all(&mut *tx).await?;
    let pending_rows: Vec<(i64, Uuid, String, String)> = sqlx::query_as(
        r#"SELECT e.rowid, e.agent_run_id, i.simple_id,
           COALESCE(json_extract(e.event_envelope,'$.payload.data.input_id'),json_extract(e.event_envelope,'$.payload.data.approval_id'))
        FROM agent_events e JOIN agent_runs r ON r.id=e.agent_run_id
        JOIN agent_run_attempts a ON a.id=e.run_attempt_id
        JOIN local_workspace_links l ON l.workspace_id=r.workspace_id
        JOIN local_issues i ON i.id=l.issue_id AND i.project_id=l.project_id
        WHERE l.project_id=? AND r.status IN ('pending','starting','running','awaiting_input','awaiting_approval')
          AND a.status IN ('pending','starting','running','awaiting_input','awaiting_approval')
          AND NOT EXISTS(SELECT 1 FROM agent_run_attempts n WHERE n.agent_run_id=r.id AND n.attempt_number>a.attempt_number)
          AND json_extract(e.event_envelope,'$.payload.type') IN ('input_requested','approval_requested')
          AND NOT EXISTS(SELECT 1 FROM agent_events x WHERE x.agent_run_id=e.agent_run_id AND x.run_attempt_id=e.run_attempt_id AND x.sequence>e.sequence AND json_extract(x.event_envelope,'$.payload.type') IN ('input_resolved','approval_resolved') AND COALESCE(json_extract(x.event_envelope,'$.payload.data.input_id'),json_extract(x.event_envelope,'$.payload.data.approval_id'))=COALESCE(json_extract(e.event_envelope,'$.payload.data.input_id'),json_extract(e.event_envelope,'$.payload.data.approval_id')))
          AND NOT EXISTS(SELECT 1 FROM agent_run_commands c WHERE c.agent_run_id=e.agent_run_id AND json_extract(c.command_envelope,'$.command.type') IN ('submit_input','resolve_approval') AND (json_extract(c.command_envelope,'$.command.type')='resolve_approval' OR c.created_at>=e.created_at) AND COALESCE(json_extract(c.command_envelope,'$.command.data.input_id'),json_extract(c.command_envelope,'$.command.data.approval_id'))=COALESCE(json_extract(e.event_envelope,'$.payload.data.input_id'),json_extract(e.event_envelope,'$.payload.data.approval_id')))
        UNION ALL
        SELECT e.rowid, e.agent_run_id, i.simple_id, json_extract(e.event_envelope,'$.run_attempt_id') || ':' || json_extract(e.event_envelope,'$.payload.data.payload.message_id') || ':' || q.key
        FROM agent_events e JOIN agent_runs r ON r.id=e.agent_run_id
        JOIN agent_run_attempts a ON a.id=e.run_attempt_id
        JOIN local_workspace_links l ON l.workspace_id=r.workspace_id
        JOIN local_issues i ON i.id=l.issue_id AND i.project_id=l.project_id
        JOIN json_each(e.event_envelope,'$.payload.data.payload.questions') q
        WHERE l.project_id=? AND r.status IN ('pending','starting','running','awaiting_input','awaiting_approval')
          AND a.status IN ('pending','starting','running','awaiting_input','awaiting_approval')
          AND NOT EXISTS(SELECT 1 FROM agent_run_attempts n WHERE n.agent_run_id=r.id AND n.attempt_number>a.attempt_number)
          AND json_extract(e.event_envelope,'$.payload.type')='provider_extension'
          AND json_extract(e.event_envelope,'$.payload.data.provider_event')='async_questions'
          AND json_extract(e.event_envelope,'$.payload.data.provider_namespace')='codex'
          AND NOT EXISTS(SELECT 1 FROM agent_question_answers ans WHERE ans.question_id=json_extract(e.event_envelope,'$.run_attempt_id') || ':' || json_extract(e.event_envelope,'$.payload.data.payload.message_id') || ':' || q.key)
        ORDER BY 1
        "#).bind(project).bind(project).fetch_all(&mut *tx).await?;
    let pending = pending_rows
        .into_iter()
        .map(|(_, run_id, issue, reference)| Pending {
            run_id,
            issue,
            reference,
        })
        .collect();
    if after < 0 {
        return Err(ApiError::BadRequest("Invalid cursor".into()));
    }
    // Order by the first canonical start, not tab order or queued Run creation.
    // Keep finished Sessions in the ranking across retries and subsequent Runs.
    // A launch that fails before starting still gets a number at its terminal event.
    let rows: Vec<(i64, Uuid, String, String, bool, bool, String, i64, i64)> = sqlx::query_as(
        "WITH first_runs AS (
          SELECT r.workspace_id, r.session_id, MIN((
            SELECT e.rowid FROM agent_events e WHERE e.agent_run_id=r.id
              AND json_extract(e.event_envelope,'$.payload.type')='lifecycle_changed'
              AND json_extract(e.event_envelope,'$.payload.data.status') IN
                ('starting','succeeded','failed','cancelled','crashed','audit_failed')
            ORDER BY e.run_attempt_number, e.sequence LIMIT 1
          )) AS first_run
          FROM agent_runs r
          JOIN local_workspace_links l ON l.workspace_id=r.workspace_id
          WHERE l.project_id=? GROUP BY r.workspace_id, r.session_id
        ), numbered_sessions AS (
          SELECT workspace_id, session_id,
            ROW_NUMBER() OVER (PARTITION BY workspace_id ORDER BY first_run) AS number
          FROM first_runs WHERE first_run IS NOT NULL
        )
        SELECT e.rowid, e.agent_run_id, i.simple_id, e.event_envelope,
        EXISTS(SELECT 1 FROM agent_events m WHERE m.run_attempt_id=e.run_attempt_id
          AND json_extract(m.event_envelope,'$.payload.type')='provider_extension'
          AND json_extract(m.event_envelope,'$.payload.data.payload.message_id')=json_extract(e.event_envelope,'$.payload.data.message.message_id')
          AND ((json_extract(m.event_envelope,'$.payload.data.provider_namespace')='codex'
                AND json_extract(m.event_envelope,'$.payload.data.provider_event') IN ('agent_message_metadata','async_questions')
                AND json_extract(m.event_envelope,'$.payload.data.payload.native.item.phase')='commentary')
            OR (json_extract(m.event_envelope,'$.payload.data.provider_namespace')='claude_code'
                AND json_extract(m.event_envelope,'$.payload.data.provider_event')='agent_message_metadata'
                AND json_extract(m.event_envelope,'$.payload.data.payload.kind')='commentary')))
        , EXISTS(SELECT 1 FROM agent_events c WHERE c.run_attempt_id=e.run_attempt_id
          AND json_extract(c.event_envelope,'$.payload.type')='provider_extension'
          AND json_extract(c.event_envelope,'$.payload.data.provider_namespace')='claude_code'
          AND json_extract(c.event_envelope,'$.payload.data.provider_event')='agent_message_metadata'
          AND json_extract(c.event_envelope,'$.payload.data.payload.kind')='commentary_candidate'
          AND json_extract(c.event_envelope,'$.payload.data.payload.message_id')=json_extract(e.event_envelope,'$.payload.data.message.message_id'))
        , r.provider_id, ns.number,
        (SELECT COUNT(DISTINCT other.session_id) FROM agent_runs other
          WHERE other.workspace_id=r.workspace_id AND other.session_id<>r.session_id
          AND other.status NOT IN ('succeeded','failed','cancelled','crashed','audit_failed'))
        FROM agent_events e JOIN agent_runs r ON r.id=e.agent_run_id
        JOIN numbered_sessions ns ON ns.workspace_id=r.workspace_id AND ns.session_id=r.session_id
        JOIN local_workspace_links l ON l.workspace_id=r.workspace_id JOIN local_issues i ON i.id=l.issue_id AND i.project_id=l.project_id WHERE l.project_id=? AND e.rowid>? ORDER BY e.rowid LIMIT 256"
    ).bind(project).bind(project).bind(after).fetch_all(&mut *tx).await?;
    // Once this page has drained the project, skip unrelated rows in the
    // same snapshot. Keep the last row only when the page may be incomplete.
    let drained = rows.len() < 256;
    let mut cursor = after;
    let mut events = Vec::new();
    for (
        rowid,
        run_id,
        issue,
        envelope,
        commentary,
        candidate,
        provider,
        session_number,
        other_active_sessions,
    ) in rows
    {
        cursor = rowid;
        if let Ok(envelope) = serde_json::from_str::<Value>(&envelope)
            && let Some(attempt) = envelope
                .get("run_attempt_number")
                .and_then(Value::as_u64)
                .and_then(|n| u32::try_from(n).ok())
                .filter(|n| *n > 0)
            && let Some(payload) = audible_payload(&envelope, commentary, candidate, &provider)
        {
            events.push(Observation {
                run_id,
                attempt,
                issue,
                session_number,
                other_active_sessions,
                provider,
                payload,
            });
        }
    }
    if drained {
        cursor = baseline;
    }
    tx.commit().await?;
    Ok(Page {
        cursor,
        events,
        pending,
        active,
    })
}

// Never expose tool calls, reasoning or user text. A Claude candidate may
// later prove final, so the browser must not speak it without confirmation.
fn audible_payload(
    envelope: &Value,
    commentary: bool,
    candidate: bool,
    provider: &str,
) -> Option<Value> {
    let payload = envelope.get("payload")?;
    let data = payload.get("data")?;
    match payload.get("type")?.as_str()? {
        "lifecycle_changed" | "input_requested" | "input_resolved" | "approval_requested"
        | "approval_resolved" => Some(payload.clone()),
        "provider_extension"
            if data.get("provider_namespace")?.as_str() == Some("codex")
                && data.get("provider_event")?.as_str() == Some("async_questions") =>
        {
            let attempt = envelope.get("run_attempt_id")?.as_str()?;
            let message = data.pointer("/payload/message_id")?.as_str()?;
            let references: Vec<String> = data
                .pointer("/payload/questions")?
                .as_array()?
                .iter()
                .enumerate()
                .map(|(index, _)| format!("{attempt}:{message}:{index}"))
                .collect();
            Some(serde_json::json!({"type":"async_questions","data":{"references":references}}))
        }
        "provider_extension"
            if data.get("provider_namespace")?.as_str() == Some("claude_code")
                && data.get("provider_event")?.as_str() == Some("agent_message_metadata")
                && data.pointer("/payload/kind")?.as_str() == Some("commentary_confirmed") =>
        {
            Some(
                serde_json::json!({"type":"commentary_confirmed", "data":{"message_id":data.pointer("/payload/message_id")?}}),
            )
        }
        "message"
            if provider == "claude_code"
                && candidate
                && !commentary
                && data.get("final_output")?.as_bool() == Some(false)
                && data.pointer("/message/role")?.as_str() == Some("assistant") =>
        {
            Some(
                serde_json::json!({"type":"commentary_candidate", "data":{"message":data.get("message")?}}),
            )
        }
        "message"
            if commentary
                && data.get("final_output")?.as_bool() == Some(false)
                && data.pointer("/message/role")?.as_str() == Some("assistant") =>
        {
            Some(payload.clone())
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::audible_payload;
    #[test]
    fn feed_excludes_non_commentary() {
        for payload in [
            json!({"type":"thinking","data":{"content":"secret"}}),
            json!({"type":"tool_call","data":{}}),
            json!({"type":"message","data":{"final_output":true,"message":{"role":"assistant","content":"final"}}}),
            json!({"type":"message","data":{"final_output":false,"message":{"role":"user","content":"input"}}}),
        ] {
            assert!(audible_payload(&json!({"payload":payload}), true, false, "codex").is_none());
        }
        assert!(audible_payload(&json!({"payload":{"type":"message","data":{"final_output":false,"message":{"role":"assistant","content":"working"}}}}), true, false, "codex").is_some());
        let unmarked = json!({"payload":{"type":"message","data":{"final_output":false,"message":{"role":"assistant","content":"child or old text"}}}});
        assert!(audible_payload(&unmarked, false, false, "claude_code").is_none());
    }
    #[tokio::test]
    async fn project_feed_baselines_and_tracks_questions_without_cross_project_leaks() {
        use sqlx::sqlite::SqlitePoolOptions;
        use uuid::Uuid;
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::raw_sql(r#"
            CREATE TABLE agent_runs(id BLOB PRIMARY KEY, workspace_id BLOB, session_id BLOB, status TEXT, provider_id TEXT DEFAULT 'codex');
            CREATE TABLE agent_run_attempts(id BLOB PRIMARY KEY, agent_run_id BLOB, attempt_number INTEGER, status TEXT);
            CREATE TABLE local_workspace_links(workspace_id BLOB, project_id BLOB, issue_id BLOB);
            CREATE TABLE local_issues(id BLOB, project_id BLOB, simple_id TEXT);
            CREATE TABLE agent_events(agent_run_id BLOB, run_attempt_id BLOB, sequence INTEGER, run_attempt_number INTEGER DEFAULT 1, event_envelope TEXT, created_at TEXT DEFAULT (datetime('now','subsec')));
            CREATE TABLE agent_run_commands(agent_run_id BLOB, command_envelope TEXT, created_at TEXT DEFAULT (datetime('now','subsec')));
            CREATE TABLE agent_question_answers(question_id TEXT);
        "#).execute(&pool).await.unwrap();
        let project = Uuid::new_v4();
        let other = Uuid::new_v4();
        let run = Uuid::new_v4();
        let run2 = Uuid::new_v4();
        let run3 = Uuid::new_v4();
        let attempt3 = Uuid::new_v4();
        let workspace3 = Uuid::new_v4();
        let issue3 = Uuid::new_v4();
        let attempt = Uuid::new_v4();
        let attempt2 = Uuid::new_v4();
        let workspace = Uuid::new_v4();
        let workspace2 = Uuid::new_v4();
        let issue = Uuid::new_v4();
        let issue2 = Uuid::new_v4();
        for (p, r, a, w, i, name) in [
            (project, run, attempt, workspace, issue, "LOCAL-1"),
            (project, run3, attempt3, workspace3, issue3, "LOCAL-2"),
            (other, run2, attempt2, workspace2, issue2, "OTHER-1"),
        ] {
            sqlx::query("INSERT INTO agent_runs(id, workspace_id, session_id, status) VALUES(?,?,?,'running')")
                .bind(r)
                .bind(w)
                .bind(r)
                .execute(&pool)
                .await
                .unwrap();
            sqlx::query("INSERT INTO agent_run_attempts VALUES(?,?,1,'running')")
                .bind(a)
                .bind(r)
                .execute(&pool)
                .await
                .unwrap();
            sqlx::query("INSERT INTO local_workspace_links VALUES(?,?,?)")
                .bind(w)
                .bind(p)
                .bind(i)
                .execute(&pool)
                .await
                .unwrap();
            sqlx::query("INSERT INTO local_issues VALUES(?,?,?)")
                .bind(i)
                .bind(p)
                .bind(name)
                .execute(&pool)
                .await
                .unwrap();
        }
        let insert = |r, a, seq, payload: serde_json::Value| {
            let pool = pool.clone();
            async move {
                let envelope = json!({"run_attempt_id":a,"run_attempt_number":1,"payload":payload})
                    .to_string();
                sqlx::query("INSERT INTO agent_events(agent_run_id,run_attempt_id,sequence,event_envelope) VALUES(?,?,?,?)")
                    .bind(r)
                    .bind(a)
                    .bind(seq)
                    .bind(envelope)
                    .execute(&pool)
                    .await
                    .unwrap();
            }
        };
        insert(
            run,
            attempt,
            1,
            json!({"type":"lifecycle_changed","data":{"status":"starting"}}),
        )
        .await;
        let baseline = super::page(&pool, project, None).await.unwrap();
        assert!(baseline.events.is_empty());
        insert(
            run2,
            attempt2,
            1,
            json!({"type":"lifecycle_changed","data":{"status":"starting"}}),
        )
        .await;
        insert(
            run,
            attempt,
            2,
            json!({"type":"input_requested","data":{"input_id":"q","prompt":"question"}}),
        )
        .await;
        insert(
            run3,
            attempt3,
            1,
            json!({"type":"lifecycle_changed","data":{"status":"starting"}}),
        )
        .await;
        let page = super::page(&pool, project, Some(baseline.cursor))
            .await
            .unwrap();
        assert_eq!(page.events.len(), 2);
        assert_eq!(page.events[0].issue, "LOCAL-1");
        assert_eq!(page.events[1].issue, "LOCAL-2");
        insert(
            run2,
            attempt2,
            2,
            json!({"type":"lifecycle_changed","data":{"status":"running"}}),
        )
        .await;
        let unrelated = super::page(&pool, project, Some(page.cursor))
            .await
            .unwrap();
        assert!(unrelated.events.is_empty());
        assert!(unrelated.cursor > page.cursor);
        assert_eq!(page.pending.len(), 1);
        assert_eq!(page.pending[0].reference, "q");
        sqlx::query("INSERT INTO agent_run_commands(agent_run_id,command_envelope) VALUES(?,?)")
            .bind(run)
            .bind(json!({"command":{"type":"submit_input","data":{"input_id":"q"}}}).to_string())
            .execute(&pool)
            .await
            .unwrap();
        assert!(
            super::page(&pool, project, Some(page.cursor))
                .await
                .unwrap()
                .pending
                .is_empty()
        );
        let message = Uuid::new_v4();
        insert(
            run,
            attempt,
            3,
            json!({"type":"provider_extension","data":{
                "provider_namespace":"codex","provider_event":"async_questions",
                "payload":{"message_id":message,"questions":[{"title":"Choose"}]}
            }}),
        )
        .await;
        let pending = super::page(&pool, project, Some(page.cursor))
            .await
            .unwrap();
        let question_id = format!("{attempt}:{message}:0");
        assert_eq!(pending.pending.len(), 1);
        assert_eq!(pending.pending[0].reference, question_id);
        sqlx::query("INSERT INTO agent_question_answers VALUES(?)")
            .bind(&question_id)
            .execute(&pool)
            .await
            .unwrap();
        let reconnect = super::page(&pool, project, None).await.unwrap();
        assert!(reconnect.events.is_empty());
        assert!(reconnect.pending.is_empty());
        let commentary_id = Uuid::new_v4();
        insert(run, attempt, 4, json!({"type":"message","data":{"final_output":false,"message":{"message_id":commentary_id,"role":"assistant","content":"Checking"}}})).await;
        insert(run, attempt, 5, json!({"type":"provider_extension","data":{"provider_namespace":"codex","provider_event":"agent_message_metadata","payload":{"message_id":commentary_id,"native":{"item":{"phase":"commentary"}}}}})).await;
        insert(run, attempt, 6, json!({"type":"message","data":{"final_output":true,"message":{"message_id":Uuid::new_v4(),"role":"assistant","content":"Final"}}})).await;
        let messages = super::page(&pool, project, Some(reconnect.cursor))
            .await
            .unwrap();
        assert_eq!(messages.events.len(), 1);
        assert_eq!(
            messages.events[0].payload["data"]["message"]["content"],
            "Checking"
        );
        let claude_id = Uuid::new_v4();
        insert(run3, attempt3, 2, json!({"type":"message","data":{"final_output":false,"message":{"message_id":claude_id,"role":"assistant","content":"Reading"}}})).await;
        insert(run3, attempt3, 3, json!({"type":"provider_extension","data":{"provider_namespace":"claude_code","provider_event":"agent_message_metadata","payload":{"message_id":claude_id,"kind":"commentary"}}})).await;
        let claude = super::page(&pool, project, Some(messages.cursor))
            .await
            .unwrap();
        assert_eq!(claude.events.len(), 1);
        assert_eq!(
            claude.events[0].payload["data"]["message"]["content"],
            "Reading"
        );
        sqlx::query("UPDATE agent_runs SET provider_id='claude_code' WHERE id=?")
            .bind(run3)
            .execute(&pool)
            .await
            .unwrap();
        let delayed_id = Uuid::new_v4();
        insert(run3, attempt3, 4, json!({"type":"message","data":{"final_output":false,"message":{"message_id":delayed_id,"role":"assistant","content":"Checking later"}}})).await;
        insert(run3, attempt3, 5, json!({"type":"provider_extension","data":{"provider_namespace":"claude_code","provider_event":"agent_message_metadata","payload":{"message_id":delayed_id,"kind":"commentary_candidate"}}})).await;
        insert(run3, attempt3, 6, json!({"type":"provider_extension","data":{"provider_namespace":"claude_code","provider_event":"agent_message_metadata","payload":{"message_id":delayed_id,"kind":"commentary_confirmed"}}})).await;
        let delayed = super::page(&pool, project, Some(claude.cursor))
            .await
            .unwrap();
        assert_eq!(delayed.events.len(), 2);
        assert_eq!(delayed.events[0].payload["type"], "commentary_candidate");
        assert_eq!(delayed.events[1].payload["type"], "commentary_confirmed");
        assert_eq!(
            delayed.events[1].payload["data"]["message_id"],
            json!(delayed_id)
        );
        sqlx::query("INSERT INTO agent_run_commands(agent_run_id,command_envelope,created_at) VALUES(?,?,'2000-01-01 00:00:00')")
            .bind(run3).bind(json!({"command":{"type":"submit_input","data":{"input_id":"reused"}}}).to_string()).execute(&pool).await.unwrap();
        insert(
            run3,
            attempt3,
            7,
            json!({"type":"input_requested","data":{"input_id":"reused","prompt":"New question"}}),
        )
        .await;
        let reused = super::page(&pool, project, Some(delayed.cursor))
            .await
            .unwrap();
        assert!(
            reused
                .pending
                .iter()
                .any(|question| question.reference == "reused")
        );

        // A finished Session retains its ordinal, even when another Session
        // starts or the original Session gets another Run.
        assert_eq!(messages.events[0].session_number, 1);
        assert_eq!(messages.events[0].other_active_sessions, 0);
        sqlx::query("UPDATE agent_runs SET workspace_id=? WHERE id=?")
            .bind(workspace)
            .bind(run3)
            .execute(&pool)
            .await
            .unwrap();
        let concurrent = super::page(&pool, project, Some(0)).await.unwrap();
        let second = concurrent.events.iter().find(|e| e.run_id == run3).unwrap();
        assert_eq!(second.session_number, 2);
        assert_eq!(second.other_active_sessions, 1);
        assert_eq!(second.provider, "claude_code");
        sqlx::query("UPDATE agent_runs SET status='succeeded' WHERE id=?")
            .bind(run)
            .execute(&pool)
            .await
            .unwrap();
        // The ending run itself is already terminal in this snapshot.
        insert(
            run,
            attempt,
            8,
            json!({"type":"lifecycle_changed","data":{"status":"succeeded"}}),
        )
        .await;
        let ended = super::page(&pool, project, Some(reused.cursor))
            .await
            .unwrap();
        assert_eq!(ended.events[0].session_number, 1);
        assert_eq!(ended.events[0].other_active_sessions, 1);
        let queued = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO agent_runs(id,workspace_id,session_id,status) VALUES(?,?,?,'pending')",
        )
        .bind(queued)
        .bind(workspace)
        .bind(queued)
        .execute(&pool)
        .await
        .unwrap();
        let third = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO agent_runs(id,workspace_id,session_id,status) VALUES(?,?,?,'running')",
        )
        .bind(third)
        .bind(workspace)
        .bind(third)
        .execute(&pool)
        .await
        .unwrap();
        insert(
            third,
            Uuid::new_v4(),
            1,
            json!({"type":"lifecycle_changed","data":{"status":"starting"}}),
        )
        .await;
        let next = super::page(&pool, project, Some(ended.cursor))
            .await
            .unwrap();
        assert_eq!(next.events[0].session_number, 3);
        assert_eq!(next.events[0].other_active_sessions, 2);
        let resumed = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO agent_runs(id,workspace_id,session_id,status) VALUES(?,?,?,'running')",
        )
        .bind(resumed)
        .bind(workspace)
        .bind(run3)
        .execute(&pool)
        .await
        .unwrap();
        insert(
            resumed,
            Uuid::new_v4(),
            1,
            json!({"type":"lifecycle_changed","data":{"status":"starting"}}),
        )
        .await;
        let resume = super::page(&pool, project, Some(next.cursor))
            .await
            .unwrap();
        assert_eq!(resume.events[0].session_number, 2);
        assert_eq!(resume.events[0].other_active_sessions, 2);
        insert(
            third,
            Uuid::new_v4(),
            2,
            json!({"type":"lifecycle_changed","data":{"status":"running"}}),
        )
        .await;
        let distinct = super::page(&pool, project, Some(resume.cursor))
            .await
            .unwrap();
        assert_eq!(distinct.events[0].other_active_sessions, 2); // two Runs in Session 2, plus queued
        sqlx::query("UPDATE agent_runs SET status='failed' WHERE workspace_id=?")
            .bind(workspace)
            .execute(&pool)
            .await
            .unwrap();
        insert(
            third,
            Uuid::new_v4(),
            3,
            json!({"type":"lifecycle_changed","data":{"status":"failed"}}),
        )
        .await;
        let last = super::page(&pool, project, Some(distinct.cursor))
            .await
            .unwrap();
        assert_eq!(last.events[0].session_number, 3);
        assert_eq!(last.events[0].other_active_sessions, 0);
        insert(
            queued,
            Uuid::new_v4(),
            1,
            json!({"type":"lifecycle_changed","data":{"status":"starting"}}),
        )
        .await;
        let delayed_start = super::page(&pool, project, Some(last.cursor))
            .await
            .unwrap();
        assert_eq!(delayed_start.events[0].session_number, 4);
        assert!(
            super::page(&pool, project, None)
                .await
                .unwrap()
                .events
                .is_empty()
        );
    }
}
