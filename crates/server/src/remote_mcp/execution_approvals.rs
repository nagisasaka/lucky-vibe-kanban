//! Explicit member decisions, separate from specification answers and publication.
use anyhow::ensure;
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};
use utils::mcp_error::Category;
use uuid::Uuid;

use super::{
    Remote,
    auth::Principal,
    questions::Kind,
    read::{arguments, definition},
    runtime_write::{Target, validate_approval_target},
};

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Decision {
    #[serde(flatten)]
    target: Target,
    request_id: Uuid,
    approval_id: String,
    /// Revision returned by get_question(kind=approval), after all pages are read.
    expected_revision: String,
    approved: bool,
    reason: Option<String>,
}

pub(super) fn tools() -> Vec<rmcp::model::Tool> {
    let mut value = serde_json::to_value(definition::<Decision>("decide_execution_approval",
        "Explicitly approve or deny one pending execution approval after reading get_question(kind=approval). Requires an external authenticated member with read/write permission. Does not grant session permission or approve publication. Read get_operation and command/events to distinguish admission, decision and tool completion; never blindly resend an uncertain delivery.")).expect("tool");
    value["annotations"] = json!({"readOnlyHint":false,"destructiveHint":true,"idempotentHint":true,"openWorldHint":true});
    value["_meta"] =
        json!({"securitySchemes":[{"type":"oauth2","scopes":["lvk:read","lvk:write"]}]});
    vec![
        serde_json::from_value(value).expect("tool"),
        definition::<Status>(
            "get_execution_approval",
            "Read current state and command delivery for one execution approval, including after resolution. Pending details and arguments are read with get_question(kind=approval). Admission, provider decision and tool completion are separate; do not resubmit an uncertain delivery.",
        ),
    ]
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Status {
    #[serde(flatten)]
    target: Target,
    approval_id: String,
}

pub(super) async fn status(remote: &Remote, input: Value) -> anyhow::Result<Value> {
    use sqlx::Row;
    let args: Status = arguments(input)?;
    let issue = remote
        .require_workspace(args.target.project_id, args.target.workspace_id)
        .await?;
    let mut transaction = remote.pool.begin().await?;
    let member: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM agent_runs r JOIN sessions s ON s.id=r.session_id WHERE r.id=? AND r.session_id=? AND r.workspace_id=? AND s.workspace_id=?)")
        .bind(args.target.agent_run_id).bind(args.target.session_id).bind(args.target.workspace_id).bind(args.target.workspace_id).fetch_one(&mut *transaction).await?;
    ensure!(member, super::errors::rejection(Category::NotFound));
    let requested: Option<String> = sqlx::query_scalar("SELECT event_envelope FROM agent_events WHERE agent_run_id=? AND json_extract(event_envelope,'$.payload.type')='approval_requested' AND json_extract(event_envelope,'$.payload.data.approval_id')=? ORDER BY run_attempt_number DESC,sequence DESC LIMIT 1")
        .bind(args.target.agent_run_id).bind(&args.approval_id).fetch_optional(&mut *transaction).await?;
    let requested: Value = serde_json::from_str(
        &requested.ok_or_else(|| super::errors::rejection(Category::NotFound))?,
    )?;
    let resolved: Option<String> = sqlx::query_scalar("SELECT event_envelope FROM agent_events WHERE agent_run_id=? AND run_attempt_id=? AND json_extract(event_envelope,'$.payload.type')='approval_resolved' AND json_extract(event_envelope,'$.payload.data.approval_id')=? ORDER BY sequence DESC LIMIT 1")
        .bind(args.target.agent_run_id).bind(requested["run_attempt_id"].as_str().unwrap_or_default().parse::<Uuid>()?).bind(&args.approval_id).fetch_optional(&mut *transaction).await?;
    let resolved: Option<Value> = resolved.map(|s| serde_json::from_str(&s)).transpose()?;
    let commands = sqlx::query("SELECT command_id,delivery_status FROM agent_run_commands WHERE agent_run_id=? AND json_extract(command_envelope,'$.command.type')='resolve_approval' AND json_extract(command_envelope,'$.command.data.approval_id')=? ORDER BY created_at LIMIT 10")
        .bind(args.target.agent_run_id).bind(&args.approval_id).fetch_all(&mut *transaction).await?;
    let commands: Vec<Value> = commands.iter().map(|r| Ok(json!({"command_id":r.try_get::<Uuid,_>("command_id")?,"delivery_status":r.try_get::<String,_>("delivery_status")?}))).collect::<Result<_,sqlx::Error>>()?;
    let pending = crate::routes::sessions::questions::pending_blocking(
        &mut *transaction,
        args.target.workspace_id,
    )
    .await?
    .iter()
    .any(|e| e["event_id"] == requested["event_id"]);
    transaction.commit().await?;
    Ok(
        json!({"approval_id":args.approval_id,"project_id":args.target.project_id,
        "workspace_id":args.target.workspace_id,"session_id":args.target.session_id,"agent_run_id":args.target.agent_run_id,
        "state":if resolved.is_some() {"resolved"} else if !commands.is_empty() {"decision_reserved"} else if pending {"pending"} else {"inactive"},
        "approved":resolved.as_ref().map(|r| r["payload"]["data"]["approved"].clone()),
        "decision_confirmed":resolved.is_some(),"tool_execution_complete":null,"tool_execution_status":"not_observed","commands":commands,
        "url":remote.config.workspace_url(args.target.project_id,issue,args.target.workspace_id)}),
    )
}

pub(super) async fn call(
    remote: &Remote,
    principal: &Principal,
    input: Value,
) -> anyhow::Result<Value> {
    let member = server_access::ACTOR
        .try_with(|actor| {
            actor.channel == "mcp"
                && actor.subject == principal.subject
                && actor.expires > server_access::now()
                && actor.scopes.contains("lvk:write")
        })
        .unwrap_or(false);
    ensure!(
        member && principal.allows("lvk:read") && principal.allows("lvk:write"),
        super::errors::rejection_with_message(
            Category::Permission,
            "Execution decisions require an external authenticated member with read/write permission; internal agents cannot approve their own operations"
        )
    );
    submit(remote, principal, input).await
}

async fn submit(remote: &Remote, principal: &Principal, input: Value) -> anyhow::Result<Value> {
    let args: Decision = arguments(input.clone())?;
    ensure!(
        !args.approval_id.is_empty()
            && args.approval_id.len() <= 1000
            && !args.expected_revision.is_empty()
            && args.reason.as_ref().is_none_or(|r| r.len() <= 32000),
        super::errors::rejection(Category::Validation)
    );
    let issue = remote
        .require_workspace(args.target.project_id, args.target.workspace_id)
        .await?;
    let runtime = remote
        .runtime
        .clone()
        .ok_or_else(|| super::errors::rejection(Category::Unavailable))?;
    let owned = remote.clone();
    let actor = remote.principal_key(principal);
    let receipt = remote.operations().submit(actor,args.request_id,args.target.project_id,
        "decide_execution_approval".into(),input,
        json!({"workspace_id":args.target.workspace_id,"session_id":args.target.session_id,
            "agent_run_id":args.target.agent_run_id,"approval_id":args.approval_id,"approved":args.approved}),
        move |operation| async move {
            if validate_approval_target(&owned.pool, &args.target).await.is_err() {
                return ("failed",json!({"code":"approval_target_conflict"}));
            }
            let Ok((_, questions)) = owned.pending_questions(args.target.project_id,args.target.workspace_id,args.target.session_id).await else {
                return ("failed",json!({"code":"approval_state_unavailable"}));
            };
            let question = questions.iter().find(|q| q.kind == Kind::Approval && q.run == args.target.agent_run_id
                && q.id == args.approval_id && q.revision() == args.expected_revision);
            let Some(event) = question.and_then(|q| q.event_id) else {
                return ("failed",json!({"code":"approval_conflict","message":"Approval changed, reserved, resolved, cancelled or expired; refresh before deciding"}));
            };
            if args.approved && question.is_some_and(|q| q.content["requires_lvk"] == true) {
                return ("failed",json!({"code":"approval_details_unavailable","requires_lvk":true,
                    "message":"Correlated operation arguments are unavailable; inspect the original approval in LVK"}));
            }
            match runtime.decide_approval(args.target.clone(),operation,args.approval_id,args.approved,args.reason,event).await {
                Ok(mut result) => {
                    result["url"] = json!(owned.config.workspace_url(args.target.project_id,issue,args.target.workspace_id));
                    ("succeeded",result)
                },
                Err(_) => ("uncertain",json!({"code":"approval_delivery_unconfirmed","command_id":operation,
                    "decision_confirmed":false,"tool_execution_complete":null,"tool_execution_status":"not_observed",
                    "message":"Read the operation, command and approval events before any retry"})),
            }
        }).await?;
    Ok(remote.receipt_value(receipt))
}

/// Only the correlated tool-start arguments are exported, never arbitrary
/// provider extensions, tool results or legacy free-form approval payloads.
pub(super) async fn content(pool: &sqlx::SqlitePool, event: &Value) -> anyhow::Result<Value> {
    let data = &event["payload"]["data"];
    let mut args = Value::Null;
    let mut name = data["tool_name"].clone();
    if let (Some(run), Some(attempt), Some(call)) = (
        event["agent_run_id"].as_str(),
        event["run_attempt_id"].as_str(),
        data["tool_call_id"].as_str(),
    ) {
        // Select one earlier argument-bearing frame in the same immutable
        // attempt/call; later calls, results and reused IDs cannot supply args.
        let frame: Option<String> = sqlx::query_scalar("SELECT event_envelope FROM agent_events WHERE agent_run_id=? AND run_attempt_id=? AND json_extract(event_envelope,'$.payload.type')='tool_call' AND json_extract(event_envelope,'$.payload.data.tool_call_id')=? AND json_type(event_envelope,'$.payload.data.arguments') IS NOT NULL AND json_type(event_envelope,'$.payload.data.arguments')!='null' AND sequence<(SELECT sequence FROM agent_events WHERE run_attempt_id=? AND json_extract(event_envelope,'$.event_id')=? LIMIT 1) ORDER BY sequence DESC LIMIT 1")
            .bind(run.parse::<Uuid>()?).bind(attempt.parse::<Uuid>()?).bind(call)
            .bind(attempt.parse::<Uuid>()?).bind(event["event_id"].as_str().unwrap_or_default())
            .fetch_optional(pool).await?;
        if let Some(frame) = frame {
            let frame: Value = serde_json::from_str(&frame)?;
            args = frame["payload"]["data"]["arguments"].clone();
            name = frame["payload"]["data"]["tool_name"].clone();
        }
    }
    redact(&mut args);
    Ok(
        json!({"approval_id":data["approval_id"],"tool_name":data["tool_name"],"operation":name,
        "tool_call_id":data["tool_call_id"],"run_attempt_id":event["run_attempt_id"],
        "state":"pending","scope":"single_request","arguments":args,"requires_lvk":args.is_null(),
        "arguments_status":if args.is_null() {"unavailable"} else if args == json!({}) {"no_arguments"} else {"available"},
        "effect":"Permission for this requested operation only; execution success and publication are separate",
        "expires":"When the original provider waiter expires, is cancelled, or its bound call/attempt ends",
        "redaction_notice":"Credential fields are redacted; unavailable arguments are not empty arguments"}),
    )
}
fn redact(value: &mut Value) {
    match value {
        Value::Object(object) => {
            for (key, value) in object {
                let key = key.to_ascii_lowercase();
                if key.contains("token")
                    || key.contains("secret")
                    || key.contains("password")
                    || matches!(
                        key.as_str(),
                        "authorization" | "cookie" | "api_key" | "apikey" | "private_key"
                    )
                {
                    *value = json!("[REDACTED: credential field]");
                } else {
                    redact(value);
                }
            }
        }
        Value::String(text) if super::run_events::safe_commentary(text).is_none() => {
            *value = json!("[REDACTED: credential-shaped or unsafe text]");
        }
        Value::Array(values) => {
            for value in values {
                redact(value);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::remote_mcp::{
        questions,
        runtime_write_tests::{emit_questions, fixture},
        tests::settled_operation,
    };

    async fn emit_approval(remote: &Remote, target: &Target) {
        emit_questions(&remote.pool, target).await;
        let approval: String = sqlx::query_scalar(
            "SELECT event_envelope FROM agent_events WHERE agent_run_id=? AND sequence=3",
        )
        .bind(target.agent_run_id)
        .fetch_one(&remote.pool)
        .await
        .unwrap();
        let mut approval: Value = serde_json::from_str(&approval).unwrap();
        approval["payload"]["data"]["tool_call_id"] = json!("call");
        approval["payload"]["data"]["tool_name"] =
            json!("MCP fixture/list_shared_resources (one call only)");
        sqlx::query("UPDATE agent_events SET sequence=4,event_envelope=? WHERE agent_run_id=? AND sequence=3")
            .bind(approval.to_string()).bind(target.agent_run_id).execute(&remote.pool).await.unwrap();
        let mut tool = approval.clone();
        tool["event_id"] = json!(Uuid::new_v4());
        tool["payload"] = json!({"type":"tool_call","data":{"tool_call_id":"call","tool_name":"fixture/list_shared_resources","arguments":{},"result":"never export"}});
        sqlx::query("INSERT INTO agent_events(event_id,session_id,agent_run_id,turn_id,run_attempt_id,run_attempt_number,sequence,correlation_id,schema_version,payload_version,event_envelope) SELECT ?,session_id,agent_run_id,turn_id,run_attempt_id,1,3,?,1,1,? FROM agent_events WHERE agent_run_id=? AND sequence=4")
            .bind(Uuid::new_v4()).bind(Uuid::new_v4()).bind(tool.to_string()).bind(target.agent_run_id)
            .execute(&remote.pool).await.unwrap();
    }

    async fn pending(remote: &Remote, target: &Target) -> Value {
        let mut input = serde_json::to_value(json!({"project_id":target.project_id,"workspace_id":target.workspace_id,"session_id":target.session_id})).unwrap();
        input["agent_run_id"] = json!(target.agent_run_id);
        input["question_id"] = json!("approval");
        input["kind"] = json!("approval");
        questions::call(remote, "get_question", input)
            .await
            .unwrap()
    }
    fn decision(target: &Target, revision: &Value, approved: bool) -> Value {
        json!({"project_id":target.project_id,"workspace_id":target.workspace_id,"session_id":target.session_id,
            "agent_run_id":target.agent_run_id,"request_id":Uuid::new_v4(),"approval_id":"approval",
            "expected_revision":revision["revision"],"approved":approved})
    }
    async fn settle(
        remote: &Remote,
        principal: &Principal,
        target: &Target,
        input: &Value,
    ) -> Value {
        settled_operation(
            remote,
            principal,
            target.project_id,
            serde_json::from_value(input["request_id"].clone()).unwrap(),
        )
        .await
    }
    #[tokio::test]
    async fn member_guard_denies_missing_member_context_even_with_write_scope() {
        let (remote, _, principal, target, _) = fixture().await;
        emit_approval(&remote, &target).await;
        let detail = pending(&remote, &target).await;
        let input = decision(&target, &detail, true);
        assert!(call(&remote, &principal, input.clone()).await.is_err());
        let mut reader = principal.clone();
        reader.scopes.remove("lvk:write");
        assert!(call(&remote, &reader, input).await.is_err());
        assert_eq!(
            crate::remote_mcp::operation_scope("decide_execution_approval"),
            Some("lvk:write")
        );
    }
    #[tokio::test]
    async fn approval_decisions_bind_revision_and_target_and_reuse_receipts() {
        for approved in [true, false] {
            let (remote, runtime, principal, target, _) = fixture().await;
            emit_approval(&remote, &target).await;
            let detail = pending(&remote, &target).await;
            let content: Value = serde_json::from_str(detail["content"].as_str().unwrap()).unwrap();
            assert_eq!(content["arguments"], json!({}));
            assert_eq!(content["arguments_status"], "no_arguments");
            assert_eq!(content["scope"], "single_request");
            assert!(!content.to_string().contains("never export"));
            let input = decision(&target, &detail, approved);
            let mut stale = input.clone();
            stale["request_id"] = json!(Uuid::new_v4());
            stale["expected_revision"] = json!("old");
            submit(&remote, &principal, stale.clone()).await.unwrap();
            assert_eq!(
                settle(&remote, &principal, &target, &stale).await["status"],
                "failed"
            );
            let mut wrong = decision(&target, &detail, approved);
            wrong["session_id"] = json!(Uuid::new_v4());
            submit(&remote, &principal, wrong.clone()).await.unwrap();
            assert_eq!(
                settle(&remote, &principal, &target, &wrong).await["status"],
                "failed"
            );
            let first = submit(&remote, &principal, input.clone()).await.unwrap();
            assert_eq!(
                settle(&remote, &principal, &target, &input).await["status"],
                "succeeded"
            );
            let retry = submit(&remote, &principal, input.clone()).await.unwrap();
            assert!(first["operation_id"].is_string());
            assert_eq!(first["operation_id"], retry["operation_id"]);
            assert_eq!(runtime.calls.lock().await.len(), 1);
            let mut changed = input.clone();
            changed["approved"] = json!(!approved);
            assert!(submit(&remote, &principal, changed).await.is_err());
            let duplicate = decision(&target, &detail, approved);
            submit(&remote, &principal, duplicate.clone())
                .await
                .unwrap();
            assert_eq!(
                settle(&remote, &principal, &target, &duplicate).await["status"],
                "failed"
            );
            assert_eq!(runtime.calls.lock().await.len(), 1);
            let observed = status(&remote,json!({"project_id":target.project_id,"workspace_id":target.workspace_id,
                "session_id":target.session_id,"agent_run_id":target.agent_run_id,"approval_id":"approval"})).await.unwrap();
            assert_eq!(observed["state"], "decision_reserved");
            assert_eq!(observed["decision_confirmed"], false);
            let resolved = json!({"type":"approval_resolved","data":{"approval_id":"approval","approved":approved}});
            sqlx::query("INSERT INTO agent_events(event_id,session_id,agent_run_id,turn_id,run_attempt_id,run_attempt_number,sequence,correlation_id,schema_version,payload_version,event_envelope) SELECT ?,session_id,agent_run_id,turn_id,run_attempt_id,1,5,?,1,1,json_set(event_envelope,'$.event_id',?,'$.payload',json(?)) FROM agent_events WHERE agent_run_id=? AND sequence=4")
                .bind(Uuid::new_v4()).bind(Uuid::new_v4()).bind(Uuid::new_v4().to_string()).bind(resolved.to_string()).bind(target.agent_run_id).execute(&remote.pool).await.unwrap();
            let resolved = status(&remote,json!({"project_id":target.project_id,"workspace_id":target.workspace_id,
                "session_id":target.session_id,"agent_run_id":target.agent_run_id,"approval_id":"approval"})).await.unwrap();
            assert_eq!(resolved["state"], "resolved");
            assert_eq!(resolved["approved"], approved);
            assert_eq!(resolved["decision_confirmed"], true);
            assert!(resolved["tool_execution_complete"].is_null());
            let stored: String = sqlx::query_scalar(
                "SELECT command_envelope FROM agent_run_commands WHERE agent_run_id=?",
            )
            .bind(target.agent_run_id)
            .fetch_one(&remote.pool)
            .await
            .unwrap();
            assert_eq!(
                serde_json::from_str::<Value>(&stored).unwrap()["command"]["data"]["approved"],
                approved
            );
        }
    }
    #[tokio::test]
    async fn inactive_attempt_and_unknown_delivery_never_repeat_decisions() {
        let (remote, runtime, principal, target, _) = fixture().await;
        emit_approval(&remote, &target).await;
        let detail = pending(&remote, &target).await;
        let input = decision(&target, &detail, true);
        assert!(
            crate::routes::agent_runs::validate_pending_approval(
                &remote.pool,
                target.workspace_id,
                target.agent_run_id,
                Uuid::new_v4(),
                "approval",
                Some(Uuid::new_v4())
            )
            .await
            .is_err()
        );
        sqlx::query("UPDATE agent_run_attempts SET status='cancelled'")
            .execute(&remote.pool)
            .await
            .unwrap();
        submit(&remote, &principal, input.clone()).await.unwrap();
        assert_eq!(
            settle(&remote, &principal, &target, &input).await["status"],
            "failed"
        );
        assert!(runtime.calls.lock().await.is_empty());
        sqlx::query("UPDATE agent_run_attempts SET status='running'")
            .execute(&remote.pool)
            .await
            .unwrap();
        runtime
            .fail_after_delivery
            .store(true, std::sync::atomic::Ordering::SeqCst);
        let input = decision(&target, &detail, false);
        submit(&remote, &principal, input.clone()).await.unwrap();
        assert_eq!(
            settle(&remote, &principal, &target, &input).await["status"],
            "uncertain"
        );
        submit(&remote, &principal, input.clone()).await.unwrap();
        assert_eq!(runtime.calls.lock().await.len(), 1);
    }
    #[tokio::test]
    async fn competing_decisions_reserve_only_one_domain_command() {
        let (remote, runtime, principal, target, _) = fixture().await;
        emit_approval(&remote, &target).await;
        let detail = pending(&remote, &target).await;
        let approve = decision(&target, &detail, true);
        let deny = decision(&target, &detail, false);
        let (a, b) = tokio::join!(
            submit(&remote, &principal, approve.clone()),
            submit(&remote, &principal, deny.clone())
        );
        a.unwrap();
        b.unwrap();
        let a = settle(&remote, &principal, &target, &approve).await;
        let b = settle(&remote, &principal, &target, &deny).await;
        assert_eq!(
            usize::from(a["status"] == "succeeded") + usize::from(b["status"] == "succeeded"),
            1
        );
        assert_eq!(runtime.calls.lock().await.len(), 1);
    }

    #[test]
    fn credentials_are_withheld_without_erasing_targets_or_empty_arguments() {
        let mut args = json!({"project_id":"project","workspace_id":"workspace","authorization":"secret","nested":[{"api_key":"secret"}],"command":"echo hello","empty":{}});
        redact(&mut args);
        assert!(!args.to_string().contains("\"secret\""));
        assert_eq!(args["project_id"], "project");
        assert_eq!(args["command"], "echo hello");
        assert_eq!(args["empty"], json!({}));
        let mut text = json!({"command":format!("echo Bearer {}","x".repeat(30))});
        redact(&mut text);
        assert!(!text.to_string().contains("Bearer"));
    }
}
