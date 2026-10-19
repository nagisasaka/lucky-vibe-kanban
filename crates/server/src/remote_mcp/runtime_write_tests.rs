use std::sync::Arc;

use async_trait::async_trait;
use db::models::{
    session::{CreateSession, Session},
    workspace::{CreateWorkspace, Workspace},
};
use executors::runtime::AgentRunPortCommand;
use serde_json::{Value, json};
use services::services::queued_message::QueuedMessageService;
use sqlx::SqlitePool;
use uuid::Uuid;

use super::{
    Remote,
    auth::Principal,
    questions::{self, Kind},
    runtime_write::{self, Change, RuntimeActions, Target},
    tests::{config, settled_operation},
};
use crate::{
    error::ApiError,
    routes::{agent_runs, sessions::questions as service},
};

pub(super) struct FixtureRuntime {
    pool: SqlitePool,
    queue: QueuedMessageService,
    calls: tokio::sync::Mutex<Vec<Value>>,
    fail_after_delivery: std::sync::atomic::AtomicBool,
}
impl FixtureRuntime {
    async fn record(&self, run: Uuid, command: Uuid, payload: Value) -> Result<(), ApiError> {
        sqlx::query("INSERT INTO agent_run_commands(command_id,agent_run_id,idempotency_key,command_schema_version,command_envelope,delivery_status) VALUES(?,?,?,1,?,'delivered')")
            .bind(command).bind(run).bind(command.to_string()).bind(json!({"command":payload}).to_string()).execute(&self.pool).await?;
        self.calls
            .lock()
            .await
            .push(json!({"run":run,"command":command}));
        Ok(())
    }
    #[allow(
        clippy::too_many_arguments,
        reason = "Fixture preserves the same explicit question-delivery boundary as RuntimeActions"
    )]
    async fn answer_impl(
        &self,
        target: Target,
        operation: Uuid,
        question: String,
        answer: String,
        kind: Kind,
        actor: String,
        event: Option<Uuid>,
    ) -> Result<Value, ApiError> {
        match kind {
            Kind::AsyncSpecification => {
                let session = Session::find_by_id(&self.pool, target.session_id)
                    .await?
                    .unwrap();
                service::answer_with_dispatch(
                    &self.pool,
                    &self.queue,
                    session,
                    service::AnswerAgentQuestion {
                        request_id: operation,
                        question_id: question,
                        answer,
                    },
                    &actor,
                    |run, identity, command| async move {
                        assert_eq!(run, target.agent_run_id);
                        let AgentRunPortCommand::Steer { content } = command else {
                            panic!("wrong dispatch")
                        };
                        self.record(
                            run,
                            identity.command_id,
                            json!({"type":"steer","data":{"content":content}}),
                        )
                        .await?;
                        Ok(json!({"accepted":true,"agent_run_id":run}))
                    },
                )
                .await
            }
            Kind::BlockingSpecification => {
                let _guard = self.queue.lock_session(target.session_id).await;
                agent_runs::validate_pending_input(
                    &self.pool,
                    target.workspace_id,
                    target.agent_run_id,
                    operation,
                    &question,
                    event,
                )
                .await?;
                self.record(
                    target.agent_run_id,
                    operation,
                    json!({"type":"submit_input","data":{"input_id":question,"content":answer}}),
                )
                .await?;
                Ok(json!({"accepted":true}))
            }
            Kind::Approval => panic!("approvals must never reach dispatch"),
        }
    }
}
#[async_trait]
impl RuntimeActions for FixtureRuntime {
    async fn change(
        &self,
        target: Target,
        operation: Uuid,
        prompt: String,
        kind: Change,
    ) -> Result<Value, ApiError> {
        let _guard = self.queue.lock_session(target.session_id).await;
        runtime_write::validate_run(&self.pool, &target, matches!(kind, Change::Continue)).await?;
        self.record(
            target.agent_run_id,
            operation,
            json!({"type":"steer","data":{"content":prompt}}),
        )
        .await?;
        if self
            .fail_after_delivery
            .swap(false, std::sync::atomic::Ordering::SeqCst)
        {
            return Err(ApiError::Conflict(
                "private diagnostic after dispatch".into(),
            ));
        }
        Ok(
            json!({"agent_run_id":target.agent_run_id,"session_id":target.session_id,"command_id":operation}),
        )
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
        self.answer_impl(target, operation, question, answer, kind, actor, event)
            .await
    }
}

pub(super) async fn fixture() -> (Remote, Arc<FixtureRuntime>, Principal, Target, Uuid) {
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    fixture_with_pool(pool).await
}

async fn fixture_with_pool(
    pool: SqlitePool,
) -> (Remote, Arc<FixtureRuntime>, Principal, Target, Uuid) {
    sqlx::migrate!("../db/migrations").run(&pool).await.unwrap();
    let project = Uuid::from_u128(1);
    let issue = Uuid::new_v4();
    let status = Uuid::new_v4();
    let workspace = Uuid::new_v4();
    let session = Uuid::new_v4();
    let run = Uuid::new_v4();
    sqlx::query("INSERT INTO projects(id,name) VALUES(?,'runtime test')")
        .bind(project)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO local_project_statuses(id,project_id,name,color,sort_order) VALUES(?,?,'Todo','blue',0)").bind(status).bind(project).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO local_issues(id,project_id,issue_number,simple_id,status_id,title,sort_order) VALUES(?,?,1,'TEST-1',?,'agreed',0)").bind(issue).bind(project).bind(status).execute(&pool).await.unwrap();
    Workspace::create(
        &pool,
        &CreateWorkspace {
            branch: "fixture".into(),
            name: None,
        },
        workspace,
    )
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO local_workspace_links(workspace_id,project_id,issue_id) VALUES(?,?,?)",
    )
    .bind(workspace)
    .bind(project)
    .bind(issue)
    .execute(&pool)
    .await
    .unwrap();
    Session::create(
        &pool,
        &CreateSession {
            executor: Some("CODEX".into()),
            name: None,
        },
        session,
        workspace,
    )
    .await
    .unwrap();
    insert_run(&pool, session, workspace, run).await;
    let dispatch = Arc::new(FixtureRuntime {
        pool: pool.clone(),
        queue: QueuedMessageService::new(),
        calls: Default::default(),
        fail_after_delivery: Default::default(),
    });
    let remote = Remote {
        environment: None,
        launch: None,
        pool,
        config: Arc::new(config()),
        process_instance: Uuid::new_v4(),
        runtime: Some(dispatch.clone()),
    };
    let principal = Principal {
        subject: "owner-subject".into(),
        scopes: ["lvk:read".into(), "lvk:write".into()]
            .into_iter()
            .collect(),
    };
    (
        remote,
        dispatch,
        principal,
        Target {
            project_id: project,
            workspace_id: workspace,
            session_id: session,
            agent_run_id: run,
        },
        issue,
    )
}
pub(super) async fn insert_run(pool: &SqlitePool, session: Uuid, workspace: Uuid, run: Uuid) {
    sqlx::query("INSERT INTO agent_runs(id,session_id,workspace_id,request_id,idempotency_key,correlation_id,schema_version,payload_version,runtime_profile_id,provider_id,workspace_mode,workspace_path,status,request_envelope) VALUES(?,?,?,?,?,?,1,1,'CODEX:default','codex','shared_workspace','/fixture','running','{}')")
        .bind(run).bind(session).bind(workspace).bind(Uuid::new_v4()).bind(run.to_string()).bind(Uuid::new_v4()).execute(pool).await.unwrap();
}
fn input(target: &Target) -> Value {
    json!({"project_id":target.project_id,"workspace_id":target.workspace_id,"session_id":target.session_id,"agent_run_id":target.agent_run_id,"request_id":Uuid::new_v4()})
}
async fn settle(remote: &Remote, principal: &Principal, target: &Target, args: &Value) -> Value {
    settled_operation(
        remote,
        principal,
        target.project_id,
        serde_json::from_value(args["request_id"].clone()).unwrap(),
    )
    .await
}
pub(super) async fn emit_questions(pool: &SqlitePool, target: &Target) -> String {
    let turn = Uuid::new_v4();
    let attempt = Uuid::new_v4();
    sqlx::query("INSERT INTO agent_turns(id,agent_run_id,request_id,intent,input_message) VALUES(?,?,?,'initial','{}')").bind(turn).bind(target.agent_run_id).bind(Uuid::new_v4()).execute(pool).await.unwrap();
    sqlx::query("INSERT INTO agent_run_attempts(id,agent_run_id,turn_id,request_id,idempotency_key,attempt_number,mode,transport,schema_version,payload_version,capability_snapshot,request_envelope,status) VALUES(?,?,?,?,?,1,'launch','stdio_rpc',1,1,'{}','{}','running')").bind(attempt).bind(target.agent_run_id).bind(turn).bind(Uuid::new_v4()).bind(attempt.to_string()).execute(pool).await.unwrap();
    sqlx::query("INSERT INTO agent_run_state(agent_run_id,state_schema_version,reducer_version,last_run_attempt_number,last_event_sequence,state_json) VALUES(?,1,1,1,3,'{}')").bind(target.agent_run_id).execute(pool).await.unwrap();
    let payloads = [
        json!({"type":"provider_extension","data":{"provider_namespace":"codex","provider_event":"async_questions","payload":{"schema_version":1,"message_id":"msg","questions":[{"title":"Choose a colour","options":["Blue","Green"]}]}}}),
        json!({"type":"input_requested","data":{"input_id":"blocking","prompt":"Choose A or B"}}),
        json!({"type":"approval_requested","data":{"approval_id":"approval","action":"execute"}}),
    ];
    for (n, payload) in payloads.into_iter().enumerate() {
        sqlx::query("INSERT INTO agent_events(event_id,session_id,agent_run_id,turn_id,run_attempt_id,run_attempt_number,sequence,correlation_id,schema_version,payload_version,event_envelope) VALUES(?,?,?,?,?,1,?,?,1,1,?)")
            .bind(Uuid::new_v4()).bind(target.session_id).bind(target.agent_run_id).bind(turn).bind(attempt).bind(n as i64+1).bind(Uuid::new_v4()).bind(json!({"agent_run_id":target.agent_run_id,"session_id":target.session_id,"run_attempt_id":attempt,"event_id":Uuid::new_v4(),"payload":payload}).to_string()).execute(pool).await.unwrap();
    }
    format!("{attempt}:msg:0")
}

#[tokio::test]
async fn runtime_changes_pin_parent_ids_current_run_and_lifecycle() {
    let (remote, dispatch, principal, target, _) = fixture().await;
    let mut args = input(&target);
    args["prompt"] = json!("Make the agreed adjustment");
    let mut ro = principal.clone();
    ro.scopes.remove("lvk:write");
    assert!(
        runtime_write::call(&remote, &ro, "steer_execution", args.clone())
            .await
            .is_err()
    );
    let (a, b) = tokio::join!(
        runtime_write::call(&remote, &principal, "steer_execution", args.clone()),
        runtime_write::call(&remote, &principal, "steer_execution", args.clone())
    );
    assert_eq!(a.unwrap()["operation_id"], b.unwrap()["operation_id"]);
    assert_eq!(
        settle(&remote, &principal, &target, &args).await["status"],
        "succeeded"
    );
    assert_eq!(dispatch.calls.lock().await.len(), 1);
    let mut stale = args.clone();
    stale["request_id"] = json!(Uuid::new_v4());
    runtime_write::call(&remote, &principal, "continue_execution", stale.clone())
        .await
        .unwrap();
    assert_eq!(
        settle(&remote, &principal, &target, &stale).await["status"],
        "failed"
    );
    sqlx::query("UPDATE agent_runs SET status='succeeded' WHERE id=?")
        .bind(target.agent_run_id)
        .execute(&remote.pool)
        .await
        .unwrap();
    stale["request_id"] = json!(Uuid::new_v4());
    runtime_write::call(&remote, &principal, "continue_execution", stale.clone())
        .await
        .unwrap();
    assert_eq!(
        settle(&remote, &principal, &target, &stale).await["status"],
        "succeeded"
    );
    // A direct new UI run invalidates the previous selected run.
    insert_run(
        &remote.pool,
        target.session_id,
        target.workspace_id,
        Uuid::new_v4(),
    )
    .await;
    stale["request_id"] = json!(Uuid::new_v4());
    runtime_write::call(&remote, &principal, "continue_execution", stale.clone())
        .await
        .unwrap();
    assert_eq!(
        settle(&remote, &principal, &target, &stale).await["status"],
        "failed"
    );
    assert_eq!(dispatch.calls.lock().await.len(), 2);
    stale["request_id"] = json!(Uuid::new_v4());
    stale["session_id"] = json!(Uuid::new_v4());
    runtime_write::call(&remote, &principal, "steer_execution", stale.clone())
        .await
        .unwrap();
    assert_eq!(
        settle(&remote, &principal, &target, &stale).await["status"],
        "failed"
    );
    stale["project_id"] = json!(Uuid::from_u128(2));
    assert!(
        runtime_write::call(&remote, &principal, "steer_execution", stale)
            .await
            .is_err()
    );
    let count: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM sqlite_master WHERE name IN
             ('project_conversations', 'project_chat_messages', 'project_chat_operations', 'project_chat_events')",
        )
        .fetch_one(&remote.pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
}

#[tokio::test]
async fn question_answers_share_real_reservations_and_reject_direct_duplicates() {
    let (remote, dispatch, principal, target, _) = fixture().await;
    let question = emit_questions(&remote.pool, &target).await;
    let mut read = input(&target);
    read.as_object_mut().unwrap().remove("request_id");
    read["question_id"] = json!(question);
    read["kind"] = json!("async_specification");
    let detail = questions::call(&remote, "get_question", read.clone())
        .await
        .unwrap();
    let mut args = read.clone();
    args["request_id"] = json!(Uuid::new_v4());
    args["expected_revision"] = detail["revision"].clone();
    args["answer"] = json!("Blue");
    // Race a direct UI answer against the remote operation. Both reach the real
    // answer service; only its final delivery boundary is replaced here.
    let (remote_result, direct_result) = tokio::join!(
        runtime_write::call(&remote, &principal, "answer_question", args.clone()),
        dispatch.answer_impl(
            target.clone(),
            Uuid::new_v4(),
            question.clone(),
            "Blue".into(),
            Kind::AsyncSpecification,
            "local-user".into(),
            None,
        )
    );
    remote_result.unwrap();
    direct_result.unwrap();
    let receipt = settle(&remote, &principal, &target, &args).await;
    assert!(matches!(
        receipt["status"].as_str(),
        Some("succeeded" | "failed")
    ));
    assert_eq!(dispatch.calls.lock().await.len(), 1);
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM agent_question_answers")
        .fetch_one(&remote.pool)
        .await
        .unwrap();
    assert_eq!(count, 1);
    assert!(
        dispatch
            .answer_impl(
                target.clone(),
                Uuid::new_v4(),
                question,
                "Green".into(),
                Kind::AsyncSpecification,
                "local-user".into(),
                None,
            )
            .await
            .is_err()
    );
    args["request_id"] = json!(Uuid::new_v4());
    runtime_write::call(&remote, &principal, "answer_question", args.clone())
        .await
        .unwrap();
    assert_eq!(
        settle(&remote, &principal, &target, &args).await["status"],
        "failed"
    );
    // Both entry points use the canonical blocking-input guard. A different
    // operation key cannot answer an already delivered input a second time.
    read["question_id"] = json!("blocking");
    read["kind"] = json!("blocking_specification");
    let detail = questions::call(&remote, "get_question", read.clone())
        .await
        .unwrap();
    assert!(
        agent_runs::validate_pending_input(
            &remote.pool,
            target.workspace_id,
            target.agent_run_id,
            Uuid::new_v4(),
            "blocking",
            Some(Uuid::new_v4())
        )
        .await
        .is_err()
    );
    let mut blocking = read;
    blocking["request_id"] = json!(Uuid::new_v4());
    blocking["expected_revision"] = detail["revision"].clone();
    blocking["answer"] = json!("A");
    runtime_write::call(&remote, &principal, "answer_question", blocking.clone())
        .await
        .unwrap();
    assert_eq!(
        settle(&remote, &principal, &target, &blocking).await["status"],
        "succeeded"
    );
    assert!(
        dispatch
            .answer_impl(
                target.clone(),
                Uuid::new_v4(),
                "blocking".into(),
                "B".into(),
                Kind::BlockingSpecification,
                "local-user".into(),
                None,
            )
            .await
            .is_err()
    );
    assert_eq!(dispatch.calls.lock().await.len(), 2);
    blocking["request_id"] = json!(Uuid::new_v4());
    blocking["kind"] = json!("approval");
    assert!(
        runtime_write::call(&remote, &principal, "answer_question", blocking)
            .await
            .is_err()
    );
}

#[tokio::test]
async fn delivered_command_with_lost_domain_response_is_uncertain_and_never_replayed() {
    let (remote, dispatch, principal, target, _) = fixture().await;
    dispatch
        .fail_after_delivery
        .store(true, std::sync::atomic::Ordering::SeqCst);
    let mut args = input(&target);
    args["prompt"] = json!("Agreed adjustment");
    runtime_write::call(&remote, &principal, "steer_execution", args.clone())
        .await
        .unwrap();
    let receipt = settle(&remote, &principal, &target, &args).await;
    assert_eq!(receipt["status"], "uncertain");
    assert!(!receipt.to_string().contains("private diagnostic"));
    let replay = runtime_write::call(&remote, &principal, "steer_execution", args)
        .await
        .unwrap();
    assert_eq!(replay["operation_id"], receipt["operation_id"]);
    assert_eq!(replay["status"], "uncertain");
    assert_eq!(dispatch.calls.lock().await.len(), 1);
    let commands=super::activity::call(&remote,"list_run_commands",json!({"project_id":target.project_id,"workspace_id":target.workspace_id,"agent_run_id":target.agent_run_id})).await.unwrap();
    assert_eq!(commands["items"][0]["delivery_status"], "delivered");
}

#[tokio::test]
async fn reopened_database_preserves_work_questions_and_uncertain_operation_identity() {
    let directory = tempfile::tempdir().unwrap();
    let options = sqlx::sqlite::SqliteConnectOptions::new()
        .filename(directory.path().join("restart.sqlite"))
        .create_if_missing(true);
    let pool = SqlitePool::connect_with(options.clone()).await.unwrap();
    let (remote, dispatch, principal, target, issue) = fixture_with_pool(pool).await;
    let question = emit_questions(&remote.pool, &target).await;
    let request = Uuid::new_v4();
    let store = remote.operations();
    let (receipt, acquired) = store
        .admit(
            &remote.principal_key(&principal),
            request,
            target.project_id,
            "steer_execution",
            &json!({"prompt":"accepted before interruption"}),
            &json!({"workspace_id":target.workspace_id,"agent_run_id":target.agent_run_id}),
        )
        .await
        .unwrap();
    assert!(acquired);
    assert!(store.start(receipt.id).await.unwrap());
    let config = remote.config.clone();
    let old_process = remote.process_instance;
    drop(store);
    drop(dispatch);
    remote.pool.close().await;
    drop(remote);

    // New adapter generation and a newly opened file-backed DB, with no runtime
    // dispatcher installed: these reads cannot resume or create model work.
    let restarted = Remote {
        environment: None,
        pool: SqlitePool::connect_with(options).await.unwrap(),
        process_instance: Uuid::new_v4(),
        config,
        runtime: None,
        launch: None,
    };
    assert_ne!(restarted.process_instance, old_process);
    let work = super::read::call(
        &restarted,
        "list_issue_workspaces",
        json!({"project_id":target.project_id,"issue_id":issue}),
    )
    .await
    .unwrap();
    assert_eq!(work["items"].as_array().unwrap().len(), 1);
    assert_eq!(work["items"][0]["id"], json!(target.workspace_id));
    let runs = super::runtime_read::call(
        &restarted,
        "list_workspace_runs",
        json!({"project_id":target.project_id,"workspace_id":target.workspace_id}),
    )
    .await
    .unwrap();
    assert_eq!(runs["items"][0]["id"], json!(target.agent_run_id));
    assert_eq!(runs["items"][0]["status"], "running");
    let questions = questions::call(&restarted, "list_questions",
        json!({"project_id":target.project_id,"workspace_id":target.workspace_id,"session_id":target.session_id})).await.unwrap();
    assert!(
        questions["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item["question_id"] == question
                && item["agent_run_id"] == json!(target.agent_run_id))
    );
    let operation = super::issue_write::call(
        &restarted,
        &principal,
        "get_operation",
        json!({"project_id":target.project_id,"request_id":request}),
    )
    .await
    .unwrap();
    assert_eq!(operation["status"], "uncertain");
    assert_eq!(operation["operation_id"], json!(receipt.id));
    assert_eq!(
        operation["targets"]["workspace_id"],
        json!(target.workspace_id)
    );
    for (table, expected) in [
        ("agent_runs", 1),
        ("remote_mcp_operations", 1),
        ("agent_run_commands", 0),
    ] {
        let count: i64 = sqlx::query_scalar(&format!("SELECT count(*) FROM {table}"))
            .fetch_one(&restarted.pool)
            .await
            .unwrap();
        assert_eq!(count, expected, "{table}");
    }
    restarted.pool.close().await;
}
