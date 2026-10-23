use super::*;
use crate::remote_mcp::{
    runtime_write_tests::{fixture, insert_run},
    tests::settled_operation,
};

async fn seed(pool: &SqlitePool, target: &Target, status: &str) {
    sqlx::query("INSERT INTO agent_run_state(agent_run_id,state_schema_version,reducer_version,last_run_attempt_number,last_event_sequence,state_json) VALUES(?,1,1,1,3,?) ON CONFLICT(agent_run_id) DO UPDATE SET state_json=excluded.state_json,last_event_sequence=last_event_sequence+1")
        .bind(target.agent_run_id).bind(json!({"goal":{"status":status,"objective":"fixture only","tokens_used":20,"time_used_seconds":9,"token_budget":100}}).to_string()).execute(pool).await.unwrap();
}
async fn args(remote: &Remote, target: &Target) -> Value {
    let state = observation(&remote.pool, target, None).await.unwrap();
    json!({"project_id":target.project_id,"workspace_id":target.workspace_id,"session_id":target.session_id,"agent_run_id":target.agent_run_id,"request_id":Uuid::new_v4(),"expected_revision":state["revision"]})
}
async fn settle(remote: &Remote, principal: &Principal, target: &Target, input: &Value) -> Value {
    settled_operation(
        remote,
        principal,
        target.project_id,
        serde_json::from_value(input["request_id"].clone()).unwrap(),
    )
    .await
}

#[tokio::test]
async fn goal_receipts_are_idempotent_and_delivery_is_not_terminal() {
    let (remote, runtime, principal, target, _) = fixture().await;
    seed(&remote.pool, &target, "active").await;
    let input = args(&remote, &target).await;
    let (a, b) = tokio::join!(
        call(&remote, &principal, "pause_goal", input.clone()),
        call(&remote, &principal, "pause_goal", input.clone())
    );
    assert_eq!(a.unwrap()["operation_id"], b.unwrap()["operation_id"]);
    let receipt = settle(&remote, &principal, &target, &input).await;
    assert_eq!(receipt["status"], "succeeded");
    assert_eq!(runtime.calls.lock().await.len(), 1);
    let command = runtime.calls.lock().await[0]["command"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();
    let state = observation(&remote.pool, &target, Some(command))
        .await
        .unwrap();
    assert_eq!(state["command_delivery_status"], "delivered");
    assert_eq!(state["goal_status"], "active");
    assert_eq!(state["run_terminal"], false);
    assert!(
        call(&remote, &principal, "clear_goal", input)
            .await
            .is_err()
    );
}

#[tokio::test]
async fn goal_actions_reject_stale_parent_revision_projection_and_permission() {
    let (remote, runtime, mut principal, target, _) = fixture().await;
    seed(&remote.pool, &target, "active").await;
    let input = args(&remote, &target).await;
    principal.scopes.remove("lvk:write");
    assert!(
        call(&remote, &principal, "pause_goal", input.clone())
            .await
            .is_err()
    );
    principal.scopes.insert("lvk:write".into());
    seed(&remote.pool, &target, "paused").await;
    call(&remote, &principal, "pause_goal", input.clone())
        .await
        .unwrap();
    assert_eq!(
        settle(&remote, &principal, &target, &input).await["status"],
        "failed"
    );
    let mut wrong = target.clone();
    wrong.session_id = Uuid::new_v4();
    assert!(observation(&remote.pool, &wrong, None).await.is_err());
    wrong = target.clone();
    wrong.workspace_id = Uuid::new_v4();
    assert!(observation(&remote.pool, &wrong, None).await.is_err());
    let input = args(&remote, &target).await;
    insert_run(
        &remote.pool,
        target.session_id,
        target.workspace_id,
        Uuid::new_v4(),
    )
    .await;
    call(&remote, &principal, "resume_goal", input.clone())
        .await
        .unwrap();
    assert_eq!(
        settle(&remote, &principal, &target, &input).await["status"],
        "failed"
    );
    assert_eq!(
        observation(&remote.pool, &target, None).await.unwrap()["is_current_run"],
        false
    );
    assert!(runtime.calls.lock().await.is_empty());
}

#[tokio::test]
async fn goal_transition_matrix_preserves_objective_budget_and_accounts() {
    let (remote, _, _, target, _) = fixture().await;
    for status in [
        "active",
        "paused",
        "blocked",
        "usageLimited",
        "budgetLimited",
        "complete",
    ] {
        seed(&remote.pool, &target, status).await;
        let state = observation(&remote.pool, &target, None).await.unwrap();
        let revision = state["revision"].as_str().unwrap();
        assert_eq!(
            validate(&remote.pool, &target, Action::Pause, revision)
                .await
                .is_ok(),
            status == "active"
        );
        assert_eq!(
            validate(&remote.pool, &target, Action::Resume, revision)
                .await
                .is_ok(),
            !matches!(status, "active" | "complete")
        );
        assert_eq!(state["tokens_used"], 20);
        assert_eq!(state["token_budget"], 100);
    }
    for action in [Action::Pause, Action::Resume] {
        let AgentRunPortCommand::GoalUpdate {
            objective,
            token_budget,
            ..
        } = action.command()
        else {
            panic!("wrong command")
        };
        assert!(objective.is_none() && token_budget.is_none());
    }
    assert!(matches!(
        Action::Cancel.command(),
        AgentRunPortCommand::Cancel { .. }
    ));
    assert!(matches!(
        Action::Clear.command(),
        AgentRunPortCommand::GoalClear
    ));
}

#[tokio::test]
async fn goal_saved_resume_does_not_double_start() {
    let (remote, runtime, principal, target, _) = fixture().await;
    seed(&remote.pool, &target, "blocked").await;
    sqlx::query("UPDATE agent_runs SET status='cancelled' WHERE id=?")
        .bind(target.agent_run_id)
        .execute(&remote.pool)
        .await
        .unwrap();
    let a = args(&remote, &target).await;
    let b = args(&remote, &target).await;
    let (x, y) = tokio::join!(
        call(&remote, &principal, "resume_goal", a.clone()),
        call(&remote, &principal, "resume_goal", b.clone())
    );
    x.unwrap();
    y.unwrap();
    let x = settle(&remote, &principal, &target, &a).await;
    let y = settle(&remote, &principal, &target, &b).await;
    assert_eq!(
        [x["status"] == "succeeded", y["status"] == "succeeded"]
            .into_iter()
            .filter(|v| *v)
            .count(),
        1
    );
    assert_eq!(runtime.calls.lock().await.len(), 1);
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM agent_runs WHERE session_id=?")
        .bind(target.session_id)
        .fetch_one(&remote.pool)
        .await
        .unwrap();
    assert_eq!(count, 2);
}

#[tokio::test]
async fn goal_uncertain_delivery_is_never_replayed() {
    let (remote, runtime, principal, target, _) = fixture().await;
    seed(&remote.pool, &target, "active").await;
    runtime
        .fail_after_delivery
        .store(true, std::sync::atomic::Ordering::SeqCst);
    let input = args(&remote, &target).await;
    call(&remote, &principal, "cancel_goal", input.clone())
        .await
        .unwrap();
    assert_eq!(
        settle(&remote, &principal, &target, &input).await["status"],
        "uncertain"
    );
    call(&remote, &principal, "cancel_goal", input.clone())
        .await
        .unwrap();
    assert_eq!(runtime.calls.lock().await.len(), 1);
}

#[test]
fn goal_catalog_uses_shared_write_scope_and_never_approves_or_marks_success() {
    for tool in tools() {
        assert_eq!(
            crate::remote_mcp::operation_scope(&tool.name),
            if tool.name == "get_goal" {
                None
            } else {
                Some("lvk:write")
            }
        );
        assert!(!matches!(
            tool.name.as_ref(),
            "finish_goal" | "approve_goal" | "start_goal"
        ));
    }
}

#[tokio::test]
async fn goal_missing_degraded_archived_and_wrong_project_are_rejected() {
    let (remote, runtime, principal, target, _) = fixture().await;
    assert!(
        validate(&remote.pool, &target, Action::Resume, "")
            .await
            .is_err()
    );
    seed(&remote.pool, &target, "blocked").await;
    let revision = observation(&remote.pool, &target, None).await.unwrap()["revision"]
        .as_str()
        .unwrap()
        .to_owned();
    sqlx::query("UPDATE agent_run_state SET projection_status='rebuilding' WHERE agent_run_id=?")
        .bind(target.agent_run_id)
        .execute(&remote.pool)
        .await
        .unwrap();
    assert!(
        validate(&remote.pool, &target, Action::Resume, &revision)
            .await
            .is_err()
    );
    sqlx::query("UPDATE agent_run_state SET projection_status='current' WHERE agent_run_id=?")
        .bind(target.agent_run_id)
        .execute(&remote.pool)
        .await
        .unwrap();
    sqlx::query("UPDATE workspaces SET archived=1 WHERE id=?")
        .bind(target.workspace_id)
        .execute(&remote.pool)
        .await
        .unwrap();
    assert!(
        validate(&remote.pool, &target, Action::Resume, &revision)
            .await
            .is_err()
    );
    let mut input = args(&remote, &target).await;
    input["project_id"] = json!(Uuid::new_v4());
    assert!(
        call(&remote, &principal, "resume_goal", input)
            .await
            .is_err()
    );
    assert!(runtime.calls.lock().await.is_empty());
}

#[test]
fn goal_resume_keeps_saved_configuration_and_does_not_auto_approve() {
    for permission in ["AUTO", "SUPERVISED", "PLAN"] {
        let input = json!({"executor":"CODEX","execution_mode":"code","permission_policy":permission,"model_id":"saved-model","goal_token_budget":100});
        let config: ExecutorConfig = serde_json::from_value(input).unwrap();
        let original = config.clone();
        let resumed = resume_config(config).unwrap();
        assert_eq!(resumed.execution_mode, Some(ExecutionMode::Goal));
        assert_eq!(
            resumed.permission_policy,
            Some(if permission == "PLAN" {
                PermissionPolicy::Supervised
            } else {
                original.permission_policy.clone().unwrap()
            })
        );
        assert_eq!(resumed.model_id, original.model_id);
        assert_eq!(resumed.goal_token_budget, original.goal_token_budget);
    }
    let config: ExecutorConfig = serde_json::from_value(json!({"executor":"CLAUDE_CODE"})).unwrap();
    assert!(resume_config(config).is_err());
}
