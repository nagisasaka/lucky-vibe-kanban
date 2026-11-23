use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{
        Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};

use chrono::Duration;
use serde_json::{Value, json};

use super::*;

pub(super) fn request() -> SemanticRequest {
    let a = Uuid::from_u128(10);
    let b = Uuid::from_u128(20);
    SemanticRequest {
        filter_id: Uuid::new_v4(),
        project_id: Uuid::new_v4(),
        definition_revision: 1,
        natural_language: "Work preventing the release".into(),
        context: json!({"release":"v2"}),
        issues: vec![
            SemanticIssue {
                id: a,
                evidence: json!({"body":"release failure","status":"open","tags":[],"relations":[]}),
            },
            SemanticIssue {
                id: b,
                evidence: json!({"body":"documentation","status":"open","tags":[],"relations":[]}),
            },
        ],
        machine_candidate_ids: vec![a, b],
    }
}
fn reply(scope: &str, decisions: &[(Uuid, bool)]) -> String {
    json!({"scope":scope,"time_sensitive":false,"valid_until":null,
        "decisions":decisions.iter().map(|(id, yes)|json!({"issue_id":id,"matches":yes})).collect::<Vec<_>>()}).to_string()
}
fn begin(cache: &mut Cache, request: &SemanticRequest, now: DateTime<Utc>) {
    let plan = cache
        .plan(Evidence::from_request(request).unwrap(), now)
        .unwrap();
    cache.pending = Some(Pending {
        session_id: Uuid::new_v4(),
        workspace_id: Uuid::new_v4(),
        started_at: now,
        plan,
        stream: None,
        dispatched: true,
    });
}
fn successful(request: &SemanticRequest, scope: &str, now: DateTime<Utc>) -> Cache {
    let mut cache = Cache::new(request);
    begin(&mut cache, request, now);
    cache
        .complete(
            &reply(
                scope,
                &[(request.issues[0].id, true), (request.issues[1].id, false)],
            ),
            now,
        )
        .unwrap();
    cache
}

#[test]
fn local_delta_reconsiders_nonmatches_and_can_remove_previous_matches() {
    let now = Utc::now();
    let mut req = request();
    let mut cache = successful(&req, "local", now);
    req.issues[1].evidence["body"] = json!("release blocker");
    let plan = cache
        .plan(Evidence::from_request(&req).unwrap(), now)
        .unwrap();
    assert_eq!(plan.requested_ids, BTreeSet::from([req.issues[1].id]));
    assert_eq!(plan.reused, BTreeMap::from([(req.issues[0].id, true)]));
    begin(&mut cache, &req, now);
    cache
        .complete(&reply("local", &[(req.issues[1].id, true)]), now)
        .unwrap();
    assert_eq!(cache.result(&req, now).unwrap().issue_ids.unwrap().len(), 2);
    req.issues[0].evidence["status"] = json!("done");
    begin(&mut cache, &req, now);
    cache
        .complete(&reply("local", &[(req.issues[0].id, false)]), now)
        .unwrap();
    assert_eq!(
        cache.result(&req, now).unwrap().issue_ids,
        Some(vec![req.issues[1].id])
    );
}

#[test]
fn all_evidence_fields_and_new_issues_invalidate_local_decisions() {
    let now = Utc::now();
    let req = request();
    let cache = successful(&req, "local", now);
    for field in [
        "body",
        "status",
        "tags",
        "assignees",
        "relations",
        "archived",
        "updated_at",
    ] {
        let mut changed = req.clone();
        changed.issues[1].evidence[field] = json!("changed");
        assert_eq!(
            cache
                .plan(Evidence::from_request(&changed).unwrap(), now)
                .unwrap()
                .requested_ids,
            BTreeSet::from([req.issues[1].id]),
            "{field}"
        );
    }
    let mut added = req.clone();
    let id = Uuid::new_v4();
    added.issues.push(SemanticIssue {
        id,
        evidence: json!({"body":"new blocker"}),
    });
    assert_eq!(
        cache
            .plan(Evidence::from_request(&added).unwrap(), now)
            .unwrap()
            .requested_ids,
        BTreeSet::from([id])
    );
}

#[test]
fn global_comparison_invalidates_every_issue_for_changes_additions_and_deletions() {
    let now = Utc::now();
    let req = request();
    let cache = successful(&req, "project", now);
    let mut changed = req.clone();
    changed.issues[1].evidence["body"] = json!("new global best");
    let plan = cache
        .plan(Evidence::from_request(&changed).unwrap(), now)
        .unwrap();
    assert_eq!(plan.requested_ids.len(), 2);
    assert!(plan.reused.is_empty());
    changed.issues.pop();
    assert_eq!(
        cache
            .plan(Evidence::from_request(&changed).unwrap(), now)
            .unwrap()
            .requested_ids
            .len(),
        1
    );
    changed.issues.push(SemanticIssue {
        id: Uuid::new_v4(),
        evidence: Value::Null,
    });
    assert_eq!(
        cache
            .plan(Evidence::from_request(&changed).unwrap(), now)
            .unwrap()
            .requested_ids
            .len(),
        2
    );
}

#[test]
fn machine_time_changes_do_not_invalidate_semantics_but_natural_time_does() {
    let now = Utc::now();
    let mut req = request();
    let mut cache = successful(&req, "local", now);
    req.machine_candidate_ids.clear();
    req.definition_revision += 1;
    assert!(
        cache
            .plan(Evidence::from_request(&req).unwrap(), now)
            .is_none()
    );
    let result = cache.result(&req, now).unwrap();
    assert_eq!(result.status, EvaluationStatus::Complete);
    assert_eq!(result.issue_ids, Some(vec![]));
    cache.success.as_mut().unwrap().valid_until = Some(now + Duration::seconds(10));
    assert!(
        cache
            .plan(
                Evidence::from_request(&req).unwrap(),
                now + Duration::seconds(9)
            )
            .is_none()
    );
    assert_eq!(
        cache
            .plan(
                Evidence::from_request(&req).unwrap(),
                now + Duration::seconds(10)
            )
            .unwrap()
            .requested_ids
            .len(),
        2
    );
}

#[test]
fn definition_context_changes_invalidate_all_and_issue_order_does_not() {
    let now = Utc::now();
    let req = request();
    let cache = successful(&req, "local", now);
    let mut changed = req.clone();
    changed.context = json!({"release":"v3"});
    assert_eq!(
        cache
            .plan(Evidence::from_request(&changed).unwrap(), now)
            .unwrap()
            .requested_ids
            .len(),
        2
    );
    changed = req.clone();
    changed.natural_language = "most urgent work".into();
    assert_eq!(
        cache
            .plan(Evidence::from_request(&changed).unwrap(), now)
            .unwrap()
            .requested_ids
            .len(),
        2
    );
    changed = req.clone();
    changed.issues.reverse();
    assert!(
        cache
            .plan(Evidence::from_request(&changed).unwrap(), now)
            .is_none()
    );
}

#[test]
fn initial_pending_failure_stale_and_real_zero_are_distinct() {
    let now = Utc::now();
    let mut req = request();
    let mut cache = Cache::new(&req);
    let result = cache.result(&req, now).unwrap();
    assert_eq!(result.status, EvaluationStatus::Unevaluated);
    assert_eq!(result.issue_ids, None);
    begin(&mut cache, &req, now);
    assert_eq!(
        cache.result(&req, now).unwrap().status,
        EvaluationStatus::Evaluating
    );
    cache.fail("provider unavailable".into());
    let failed = cache.result(&req, now).unwrap();
    assert_eq!(failed.status, EvaluationStatus::Failed);
    assert_eq!(failed.issue_ids, None);
    assert!(!failed.stale);
    cache = successful(&req, "project", now);
    req.context = json!({"release":"next"});
    begin(&mut cache, &req, now);
    cache.fail("auth required".into());
    let stale = cache.result(&req, now).unwrap();
    assert!(stale.stale);
    assert!(stale.issue_ids.is_some());
    begin(&mut cache, &req, now);
    cache
        .complete(
            &reply(
                "project",
                &[(req.issues[0].id, false), (req.issues[1].id, false)],
            ),
            now,
        )
        .unwrap();
    let zero = cache.result(&req, now).unwrap();
    assert_eq!(zero.status, EvaluationStatus::Complete);
    assert_eq!(zero.issue_ids, Some(vec![]));
    assert!(!zero.stale);
}

#[test]
fn model_must_cover_all_requested_ids_and_global_escalation_covers_project() {
    let now = Utc::now();
    let mut req = request();
    let mut cache = successful(&req, "local", now);
    req.issues[1].evidence["body"] = json!("compare to all others");
    begin(&mut cache, &req, now);
    for output in [
        reply("local", &[]),
        reply("local", &[(Uuid::new_v4(), true)]),
        reply(
            "local",
            &[(req.issues[1].id, true), (req.issues[1].id, false)],
        ),
        reply("project", &[(req.issues[1].id, true)]),
        "not JSON".into(),
    ] {
        assert!(cache.complete(&output, now).is_err());
        assert!(cache.pending.is_some());
    }
    cache
        .complete(
            &reply(
                "project",
                &[(req.issues[0].id, false), (req.issues[1].id, true)],
            ),
            now,
        )
        .unwrap();
    assert_eq!(cache.success.unwrap().scope, engine::Scope::Project);
}

#[test]
fn invalid_temporal_contract_is_rejected_and_delta_preserves_old_expiry() {
    let now = Utc::now();
    let mut req = request();
    let mut cache = Cache::new(&req);
    begin(&mut cache, &req, now);
    let mut output: Value = serde_json::from_str(&reply(
        "local",
        &[(req.issues[0].id, true), (req.issues[1].id, false)],
    ))
    .unwrap();
    output["time_sensitive"] = json!(true);
    assert!(cache.complete(&output.to_string(), now).is_err());
    output["valid_until"] = json!(now);
    assert!(cache.complete(&output.to_string(), now).is_err());
    let expiry = now + Duration::seconds(30);
    output["valid_until"] = json!(expiry);
    cache.complete(&output.to_string(), now).unwrap();
    req.issues[1].evidence = json!({"body":"different"});
    begin(&mut cache, &req, now);
    cache
        .complete(&reply("local", &[(req.issues[1].id, true)]), now)
        .unwrap();
    assert_eq!(cache.success.unwrap().valid_until, Some(expiry));
}

#[derive(Default)]
struct FakeRuntime {
    launches: AtomicUsize,
    output: Mutex<Option<Result<String, String>>>,
    prompts: Mutex<Vec<String>>,
}
#[async_trait]
impl EvaluationRuntime for FakeRuntime {
    async fn launch(&self, _: &Pending, prompt: String) -> Result<(), ApiError> {
        self.launches.fetch_add(1, Ordering::SeqCst);
        self.prompts.lock().unwrap().push(prompt);
        Ok(())
    }
    async fn poll(&self, _: &Pending, _: DateTime<Utc>) -> Result<Progress, ApiError> {
        Ok(match self.output.lock().unwrap().clone() {
            None => Progress::Running,
            Some(Ok(s)) => Progress::Complete(s),
            Some(Err(e)) => Progress::Failed(e),
        })
    }
}
pub(super) async fn schema(pool: &SqlitePool, req: &SemanticRequest) {
    sqlx::raw_sql("CREATE TABLE saved_filters(id BLOB PRIMARY KEY,project_id BLOB NOT NULL,revision INTEGER NOT NULL); CREATE TABLE saved_filter_semantic_cache(filter_id BLOB PRIMARY KEY REFERENCES saved_filters(id) ON DELETE CASCADE,generation INTEGER NOT NULL,state_json TEXT NOT NULL);")
        .execute(pool).await.unwrap();
    sqlx::query("INSERT INTO saved_filters VALUES(?,?,?)")
        .bind(req.filter_id)
        .bind(req.project_id)
        .bind(req.definition_revision)
        .execute(pool)
        .await
        .unwrap();
}
async fn database(req: &SemanticRequest) -> SqlitePool {
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    schema(&pool, req).await;
    pool
}

#[tokio::test]
async fn simultaneous_evaluation_dedupes_and_machine_candidates_are_only_intersected_after_semantics()
 {
    let now = Utc::now();
    let mut req = request();
    let pool = database(&req).await;
    let runtime = FakeRuntime::default();
    req.machine_candidate_ids = vec![req.issues[0].id];
    let (a, b) = tokio::join!(
        evaluate_with(&pool, &runtime, &req, true, false, now),
        evaluate_with(&pool, &runtime, &req, true, false, now)
    );
    assert_eq!(a.unwrap().status, EvaluationStatus::Evaluating);
    assert_eq!(b.unwrap().status, EvaluationStatus::Evaluating);
    assert_eq!(runtime.launches.load(Ordering::SeqCst), 1);
    let prompts = runtime.prompts.lock().unwrap().clone();
    assert!(
        prompts[0].contains(&req.issues[1].id.to_string()),
        "machine-excluded evidence must reach AI"
    );
    *runtime.output.lock().unwrap() = Some(Ok(reply(
        "local",
        &[(req.issues[0].id, true), (req.issues[1].id, true)],
    )));
    assert_eq!(
        evaluate_with(&pool, &runtime, &req, true, false, now)
            .await
            .unwrap()
            .issue_ids,
        Some(vec![req.issues[0].id])
    );
    req.machine_candidate_ids = vec![req.issues[1].id];
    assert_eq!(
        evaluate_with(&pool, &runtime, &req, true, false, now)
            .await
            .unwrap()
            .issue_ids,
        Some(vec![req.issues[1].id])
    );
    assert_eq!(runtime.launches.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn durable_pending_and_success_survive_database_reopen() {
    let now = Utc::now();
    let req = request();
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("cache.sqlite");
    let options = sqlx::sqlite::SqliteConnectOptions::new()
        .filename(&path)
        .create_if_missing(true);
    let pool = SqlitePool::connect_with(options.clone()).await.unwrap();
    schema(&pool, &req).await;
    let runtime = FakeRuntime::default();
    evaluate_with(&pool, &runtime, &req, true, false, now)
        .await
        .unwrap();
    pool.close().await;
    let pool = SqlitePool::connect_with(options.clone()).await.unwrap();
    *runtime.output.lock().unwrap() = Some(Ok(reply(
        "project",
        &[(req.issues[0].id, true), (req.issues[1].id, false)],
    )));
    assert_eq!(
        evaluate_with(&pool, &runtime, &req, false, false, now)
            .await
            .unwrap()
            .status,
        EvaluationStatus::Complete
    );
    pool.close().await;
    let pool = SqlitePool::connect_with(options).await.unwrap();
    assert_eq!(
        evaluate_with(&pool, &runtime, &req, true, false, now)
            .await
            .unwrap()
            .status,
        EvaluationStatus::Complete
    );
    assert_eq!(runtime.launches.load(Ordering::SeqCst), 1);
    pool.close().await;
}

#[tokio::test]
async fn stale_revision_or_competing_generation_cannot_publish() {
    let req = request();
    let pool = database(&req).await;
    let cache = Cache::new(&req);
    assert!(save(&pool, &req, 0, &cache).await.unwrap());
    assert!(!save(&pool, &req, 0, &cache).await.unwrap());
    assert!(save(&pool, &req, 1, &cache).await.unwrap());
    assert!(!save(&pool, &req, 1, &cache).await.unwrap());
    sqlx::query("UPDATE saved_filters SET revision=2")
        .execute(&pool)
        .await
        .unwrap();
    assert!(!save(&pool, &req, 2, &cache).await.unwrap());
    assert!(load(&pool, &req).await.is_err());
    let mut other = req.clone();
    other.definition_revision = 2;
    other.project_id = Uuid::new_v4();
    assert!(load(&pool, &other).await.is_err());
    sqlx::query("DELETE FROM saved_filters")
        .execute(&pool)
        .await
        .unwrap();
    assert!(!save(&pool, &req, 0, &cache).await.unwrap());
}

#[tokio::test]
async fn failed_input_does_not_hot_loop_and_explicit_retry_is_possible() {
    let now = Utc::now();
    let req = request();
    let pool = database(&req).await;
    let runtime = FakeRuntime::default();
    let first = evaluate_with(&pool, &runtime, &req, false, false, now)
        .await
        .unwrap();
    assert_eq!(first.status, EvaluationStatus::Unevaluated);
    evaluate_with(&pool, &runtime, &req, true, false, now)
        .await
        .unwrap();
    *runtime.output.lock().unwrap() = Some(Err("provider authentication unavailable".into()));
    let failed = evaluate_with(&pool, &runtime, &req, true, false, now)
        .await
        .unwrap();
    assert_eq!(failed.status, EvaluationStatus::Failed);
    assert_eq!(failed.issue_ids, None);
    evaluate_with(&pool, &runtime, &req, true, false, now)
        .await
        .unwrap();
    assert_eq!(runtime.launches.load(Ordering::SeqCst), 1);
    evaluate_with(&pool, &runtime, &req, true, true, now)
        .await
        .unwrap();
    assert_eq!(runtime.launches.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn machine_only_empty_project_and_deleted_local_issue_need_no_ai() {
    let now = Utc::now();
    let mut req = request();
    let pool = database(&req).await;
    let runtime = FakeRuntime::default();
    req.natural_language.clear();
    assert_eq!(
        evaluate_with(&pool, &runtime, &req, true, false, now)
            .await
            .unwrap()
            .issue_ids
            .unwrap()
            .len(),
        2
    );
    req.natural_language = "release blockers".into();
    let cache = successful(&req, "local", now);
    assert!(save(&pool, &req, 0, &cache).await.unwrap());
    req.issues.pop();
    evaluate_with(&pool, &runtime, &req, true, false, now)
        .await
        .unwrap();
    req.issues.clear();
    assert_eq!(
        evaluate_with(&pool, &runtime, &req, true, false, now)
            .await
            .unwrap()
            .issue_ids,
        Some(vec![])
    );
    assert_eq!(runtime.launches.load(Ordering::SeqCst), 0);
}

/// Opt-in real-provider connection check. Run this test ALONE in a disposable
/// LVK_INSTANCE_DIR (outside Git), with its own runtime host and no live API URL.
#[tokio::test]
#[ignore = "uses the configured provider; requires an isolated LVK_INSTANCE_DIR and process host"]
async fn live_default_provider_uses_existing_review_lifecycle() {
    let instance = std::env::var_os("LVK_INSTANCE_DIR").expect("isolated LVK_INSTANCE_DIR");
    let instance = std::path::PathBuf::from(instance);
    assert!(
        instance.join("saved-filter-test-instance").is_file(),
        "requires explicit disposable-instance marker"
    );
    assert!(
        !instance.join("data/db.sqlite").exists(),
        "requires a fresh test instance"
    );
    let shutdown = tokio_util::sync::CancellationToken::new();
    let deployment = DeploymentImpl::new(shutdown.clone()).await.unwrap();
    let now = Utc::now();
    let mut req = request();
    req.natural_language = "Tasks that stop customers from successfully paying for an order".into();
    req.context = json!({"product":"online shop"});
    req.issues[0].evidence = json!({"title":"Checkout error","body":"Every credit card payment is rejected by the payment gateway.","status":"open"});
    req.issues[1].evidence = json!({"title":"Logo refresh","body":"Replace the shop logo with our new artwork.","status":"open"});
    // Real migrations + real runtime + real semantic SQL persistence, one chain.
    sqlx::query("INSERT INTO projects(id,name) VALUES(?,'semantic smoke')")
        .bind(req.project_id)
        .execute(&deployment.db().pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO saved_filters(id,project_id,definition,revision,created_at,updated_at) VALUES(?,?,'{}',1,?,?)")
        .bind(req.filter_id).bind(req.project_id).bind(now).bind(now).execute(&deployment.db().pool).await.unwrap();
    let first = evaluate(&deployment, req.clone(), true, false)
        .await
        .unwrap();
    assert_eq!(first.status, EvaluationStatus::Evaluating);
    let (_, cache) = load(&deployment.db().pool, &req).await.unwrap();
    let pending = cache.pending.unwrap();
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(300);
    let outcome = loop {
        let result = evaluate(&deployment, req.clone(), true, false)
            .await
            .unwrap();
        if matches!(
            result.status,
            EvaluationStatus::Complete | EvaluationStatus::Failed
        ) || tokio::time::Instant::now() >= deadline
        {
            break result;
        }
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    };
    let runs = AgentRuntimeReadService::new(&deployment.db().pool)
        .list_for_session(pending.session_id)
        .await
        .unwrap();
    assert_eq!(runs.len(), 1);
    let run = &runs[0];
    // Even a timed-out or permission-blocked live check must settle its own Run.
    if !run.state.status.is_terminal() {
        let command = Uuid::new_v4();
        let cancellation = crate::routes::agent_runs::dispatch_control(
            &deployment,
            run.agent_run_id,
            crate::routes::agent_runs::AgentRunControlIdentity {
                command_id: command,
                idempotency_key: command.to_string(),
                correlation_id: command,
                created_at: Utc::now(),
            },
            executors::runtime::AgentRunPortCommand::Cancel {
                reason: "saved-filter integration test finished".into(),
            },
        )
        .await
        .unwrap();
        assert!(cancellation.0.is_success());
        for _ in 0..30 {
            let state = AgentRuntimeReadService::new(&deployment.db().pool)
                .state(run.agent_run_id)
                .await
                .unwrap();
            if state.status.is_terminal() {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        }
    }
    let mode: String=sqlx::query_scalar("SELECT json_extract(request_envelope,'$.executor_config.execution_mode') FROM agent_run_attempts WHERE agent_run_id=? LIMIT 1")
        .bind(run.agent_run_id).fetch_one(&deployment.db().pool).await.unwrap();
    assert_eq!(mode, "review");
    let issue_count: i64 = sqlx::query_scalar("SELECT count(*) FROM local_issues")
        .fetch_one(&deployment.db().pool)
        .await
        .unwrap();
    assert_eq!(issue_count, 0);
    shutdown.cancel();
    let state = AgentRuntimeReadService::new(&deployment.db().pool)
        .state(run.agent_run_id)
        .await
        .unwrap();
    assert!(state.status.is_terminal());
    let count_before: i64 = sqlx::query_scalar("SELECT count(*) FROM agent_runs")
        .fetch_one(&deployment.db().pool)
        .await
        .unwrap();
    let (generation, persisted) = load(&deployment.db().pool, &req).await.unwrap();
    assert!(generation >= 2, "completion must be SQL persisted");
    let canonical = run
        .state
        .terminal_output
        .as_ref()
        .map(|m| m.content.clone());
    deployment.db().pool.close().await;
    assert_eq!(
        outcome.status,
        EvaluationStatus::Complete,
        "provider outcome: {:?}; canonical: {:?}",
        outcome,
        canonical
    );
    assert_eq!(outcome.issue_ids, Some(vec![req.issues[0].id]));
    assert!(persisted.success.is_some());
    let pool = SqlitePool::connect_with(
        sqlx::sqlite::SqliteConnectOptions::new().filename(instance.join("data/db.sqlite")),
    )
    .await
    .unwrap();
    // Runtime points at a closed original pool: an unexpected launch/poll cannot
    // silently pass. Cache reuse reads the genuinely reopened SQLite connection.
    let reread = evaluate_with(&pool, &Runtime(&deployment), &req, true, false, Utc::now())
        .await
        .unwrap();
    assert_eq!(reread.status, EvaluationStatus::Complete);
    assert_eq!(reread.issue_ids, outcome.issue_ids);
    let count_after: i64 = sqlx::query_scalar("SELECT count(*) FROM agent_runs")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count_before, count_after);
    println!(
        "semantic live chain: Review -> canonical -> SQL generation {generation} -> reopen; runs {count_before} -> {count_after}; matched {:?}",
        reread.issue_ids
    );
    pool.close().await;
}

#[test]
fn local_delta_prompt_omits_unchanged_full_evidence() {
    let now = Utc::now();
    let mut req = request();
    req.issues[0].evidence = json!({"body":"UNCHANGED_SECRET_SENTINEL".repeat(3000)});
    let cache = successful(&req, "local", now);
    req.issues[1].evidence = json!({"body":"changed"});
    let plan = cache
        .plan(Evidence::from_request(&req).unwrap(), now)
        .unwrap();
    let prompt = engine::prompt(&req, &plan, now);
    assert!(!prompt.contains("UNCHANGED_SECRET_SENTINEL"));
    assert!(prompt.contains("changed"));
}

#[tokio::test]
async fn slow_time_relative_success_and_boundary_refresh_are_not_a_hot_loop() {
    let now = Utc::now();
    let req = request();
    let pool = database(&req).await;
    let runtime = FakeRuntime::default();
    evaluate_with(&pool, &runtime, &req, true, false, now)
        .await
        .unwrap();
    let boundary = now + Duration::hours(1);
    let mut output: Value = serde_json::from_str(&reply(
        "local",
        &[(req.issues[0].id, true), (req.issues[1].id, false)],
    ))
    .unwrap();
    output["time_sensitive"] = json!(true);
    output["valid_until"] = json!(boundary);
    *runtime.output.lock().unwrap() = Some(Ok(output.to_string()));
    let result = evaluate_with(
        &pool,
        &runtime,
        &req,
        true,
        false,
        now + Duration::seconds(90),
    )
    .await
    .unwrap();
    assert_eq!(result.status, EvaluationStatus::Complete);
    evaluate_with(
        &pool,
        &runtime,
        &req,
        true,
        false,
        boundary - Duration::seconds(1),
    )
    .await
    .unwrap();
    assert_eq!(runtime.launches.load(Ordering::SeqCst), 1);
    *runtime.output.lock().unwrap() = None;
    evaluate_with(&pool, &runtime, &req, true, false, boundary)
        .await
        .unwrap();
    assert_eq!(runtime.launches.load(Ordering::SeqCst), 2);
    // A genuine short boundary crossed during a long run is a stale SUCCESS,
    // not a failed evidence key which remains stuck as the clock advances.
    output["valid_until"] = json!(boundary + Duration::seconds(30));
    *runtime.output.lock().unwrap() = Some(Ok(output.to_string()));
    let done = boundary + Duration::seconds(90);
    let result = evaluate_with(&pool, &runtime, &req, true, false, done)
        .await
        .unwrap();
    assert!(result.stale);
    assert!(result.issue_ids.is_some());
    for _ in 0..5 {
        evaluate_with(
            &pool,
            &runtime,
            &req,
            true,
            false,
            done + Duration::seconds(1),
        )
        .await
        .unwrap();
    }
    assert_eq!(runtime.launches.load(Ordering::SeqCst), 2);
    let retry = done + Duration::seconds(181);
    *runtime.output.lock().unwrap() = None;
    evaluate_with(&pool, &runtime, &req, true, false, retry)
        .await
        .unwrap();
    assert_eq!(runtime.launches.load(Ordering::SeqCst), 3);
    output["valid_until"] = json!(retry + Duration::seconds(30));
    *runtime.output.lock().unwrap() = Some(Ok(output.to_string()));
    let failed = evaluate_with(
        &pool,
        &runtime,
        &req,
        true,
        false,
        retry + Duration::seconds(90),
    )
    .await
    .unwrap();
    assert_eq!(failed.status, EvaluationStatus::Failed);
    assert!(failed.stale);
    for day in 1..5 {
        evaluate_with(
            &pool,
            &runtime,
            &req,
            true,
            false,
            retry + Duration::days(day),
        )
        .await
        .unwrap();
    }
    assert_eq!(runtime.launches.load(Ordering::SeqCst), 3);
}

#[tokio::test]
async fn failed_temporal_evidence_can_retry_once_when_its_known_boundary_passes() {
    let now = Utc::now();
    let mut req = request();
    let pool = database(&req).await;
    let runtime = FakeRuntime::default();
    let mut cache = successful(&req, "local", now);
    let boundary = now + Duration::minutes(10);
    cache.success.as_mut().unwrap().valid_until = Some(boundary);
    save(&pool, &req, 0, &cache).await.unwrap();
    req.issues[1].evidence = json!({"body":"changed"});
    evaluate_with(&pool, &runtime, &req, true, false, now)
        .await
        .unwrap();
    *runtime.output.lock().unwrap() = Some(Err("temporary failure".into()));
    evaluate_with(
        &pool,
        &runtime,
        &req,
        true,
        false,
        now + Duration::seconds(10),
    )
    .await
    .unwrap();
    evaluate_with(
        &pool,
        &runtime,
        &req,
        true,
        false,
        boundary - Duration::seconds(1),
    )
    .await
    .unwrap();
    assert_eq!(runtime.launches.load(Ordering::SeqCst), 1);
    evaluate_with(&pool, &runtime, &req, true, false, boundary)
        .await
        .unwrap();
    assert_eq!(runtime.launches.load(Ordering::SeqCst), 2);
    evaluate_with(
        &pool,
        &runtime,
        &req,
        true,
        false,
        boundary + Duration::seconds(10),
    )
    .await
    .unwrap();
    evaluate_with(
        &pool,
        &runtime,
        &req,
        true,
        false,
        boundary + Duration::days(3),
    )
    .await
    .unwrap();
    assert_eq!(
        runtime.launches.load(Ordering::SeqCst),
        2,
        "provider failure must not retry forever"
    );
}
