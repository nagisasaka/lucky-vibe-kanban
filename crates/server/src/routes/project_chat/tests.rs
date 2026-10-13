use std::{collections::VecDeque, sync::Mutex};

use chrono::Utc;
use db::models::agent_runtime::{AgentEventRecord, AgentRunCommandRecord, AgentRunRecord};
use executors::runtime::*;

use super::*;

async fn database() -> SqlitePool {
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::migrate!("../db/migrations").run(&pool).await.unwrap();
    pool
}

async fn project(pool: &SqlitePool) -> Conversation {
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO projects(id,name) VALUES(?,'Astra test')")
        .bind(id)
        .execute(pool)
        .await
        .unwrap();
    store::conversation(pool, id).await.unwrap()
}

async fn message(pool: &SqlitePool, conversation: &Conversation, intent: &str) -> ChatMessage {
    let id = Uuid::new_v4();
    store::enqueue(pool, conversation.id, id, "A human request", intent)
        .await
        .unwrap();
    sqlx::query_as("SELECT * FROM project_chat_messages WHERE id=?")
        .bind(id)
        .fetch_one(pool)
        .await
        .unwrap()
}

#[derive(Default)]
struct MockServices {
    replies: Mutex<VecDeque<anyhow::Result<Vec<Value>>>>,
    executions: Mutex<Vec<(Uuid, Uuid, String, Value)>>,
    inputs: Mutex<Vec<Vec<Value>>>,
    reconciled: Mutex<usize>,
    definitions: Mutex<Vec<Vec<Value>>>,
    fail_start: bool,
}
impl MockServices {
    fn replies(replies: Vec<Vec<Value>>) -> Self {
        Self {
            replies: Mutex::new(replies.into_iter().map(Ok).collect()),
            ..Default::default()
        }
    }
}

#[async_trait::async_trait]
impl ChatServices for MockServices {
    async fn context(&self, project: Uuid) -> anyhow::Result<Value> {
        Ok(json!({"project_id":project,"source":"current shared LVK state"}))
    }
    async fn respond(
        &self,
        input: &[Value],
        definitions: Vec<Value>,
    ) -> anyhow::Result<Vec<Value>> {
        self.inputs.lock().unwrap().push(input.to_vec());
        self.definitions.lock().unwrap().push(definitions);
        let mut output = self
            .replies
            .lock()
            .unwrap()
            .pop_front()
            .expect("unexpected model call")?;
        for item in &mut output {
            if item["name"] == "start_workspace" {
                let mut args: Value =
                    serde_json::from_str(item["arguments"].as_str().unwrap()).unwrap();
                if args["issue_id"] == "$created_issue" {
                    let receipt = input
                        .iter()
                        .rev()
                        .find(|v| v["type"] == "function_call_output")
                        .unwrap();
                    let receipt: Value =
                        serde_json::from_str(receipt["output"].as_str().unwrap()).unwrap();
                    args["issue_id"] = receipt["result"]["id"].clone();
                    item["arguments"] = json!(args.to_string());
                }
            }
        }
        Ok(output)
    }
    async fn execute(
        &self,
        conversation: &Conversation,
        _: &ChatMessage,
        operation: Uuid,
        name: &str,
        args: &Value,
    ) -> anyhow::Result<Value> {
        self.executions.lock().unwrap().push((
            conversation.project_id,
            operation,
            name.into(),
            args.clone(),
        ));
        if self.fail_start && name == "start_workspace" {
            anyhow::bail!("Workspace admission interrupted");
        }
        // Simulates accepted background work, deliberately never waits for completion.
        Ok(
            json!({"id":operation,"status":"running","issue_id":args["issue_id"],"workspace_id":args["workspace_id"]}),
        )
    }
    async fn reconcile(
        &self,
        _: &Conversation,
        operation: Uuid,
        _: &str,
        _: &Value,
    ) -> anyhow::Result<Value> {
        *self.reconciled.lock().unwrap() += 1;
        Ok(json!({"ok":false,"uncertain":true,"operation_id":operation}))
    }
}

fn call(name: &str, id: &str, args: Value) -> Value {
    json!({"type":"function_call","call_id":id,"name":name,"arguments":args.to_string()})
}
fn reply() -> Vec<Value> {
    vec![
        json!({"type":"message","role":"assistant","content":[{"type":"output_text","text":"acknowledged"}]}),
    ]
}

async fn attempt(
    pool: &SqlitePool,
    conversation: &Conversation,
) -> (AgentRunRequestEnvelope, RunAttemptRequest) {
    use db::models::{
        session::{CreateSession, Session},
        workspace::{CreateWorkspace, Workspace},
    };
    let statuses = local_remote::ensure_default_statuses(pool, conversation.project_id)
        .await
        .unwrap();
    let issue=local_remote::create_local_issue(pool,serde_json::from_value(json!({"project_id":conversation.project_id,"status_id":statuses[0].id,"title":"feature","description":"acceptance criteria","sort_order":0,"extension_metadata":{}})).unwrap()).await.unwrap();
    let workspace = Workspace::create(
        pool,
        &CreateWorkspace {
            branch: format!("test-{}", Uuid::new_v4()),
            name: None,
        },
        Uuid::new_v4(),
    )
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO local_workspace_links(workspace_id,project_id,issue_id) VALUES(?,?,?)",
    )
    .bind(workspace.id)
    .bind(conversation.project_id)
    .bind(issue.id)
    .execute(pool)
    .await
    .unwrap();
    let session = Session::create(
        pool,
        &CreateSession {
            executor: Some("CODEX".into()),
            name: None,
        },
        Uuid::new_v4(),
        workspace.id,
    )
    .await
    .unwrap();
    let (request, attempt) = requests(session.id, workspace.id, &Uuid::new_v4().to_string());
    AgentRunRecord::persist_identity_before_launch(pool, &request, &attempt)
        .await
        .unwrap();
    (request, attempt)
}

async fn event(
    pool: &SqlitePool,
    request: &AgentRunRequestEnvelope,
    attempt: &RunAttemptRequest,
    sequence: u64,
    payload: AgentEventPayload,
) -> AgentEventEnvelope {
    let event = AgentEventEnvelope {
        schema_version: AGENT_EVENT_SCHEMA_VERSION,
        payload_version: AGENT_EVENT_PAYLOAD_VERSION,
        event_id: Uuid::new_v4(),
        session_id: request.session_id,
        agent_run_id: request.agent_run_id,
        turn_id: request.turn_id,
        run_attempt_id: attempt.run_attempt_id,
        run_attempt_number: 1,
        sequence,
        correlation_id: request.correlation_id,
        orchestration_run_id: None,
        orchestration_node_execution_id: None,
        timestamp: Utc::now(),
        native_refs: vec![],
        payload,
    };
    AgentEventRecord::append_and_project(pool, &event)
        .await
        .unwrap();
    event
}

#[tokio::test]
async fn real_runtime_events_preserve_questions_and_exclude_token_noise() {
    let pool = database().await;
    let conversation = project(&pool).await;
    let other = project(&pool).await;
    let (request, attempt) = attempt(&pool, &conversation).await;
    event(
        &pool,
        &request,
        &attempt,
        1,
        AgentEventPayload::LifecycleChanged {
            status: AgentRunStatus::Running,
        },
    )
    .await;
    event(
        &pool,
        &request,
        &attempt,
        2,
        AgentEventPayload::TokenUsage {
            input_tokens: 1,
            output_tokens: 1,
            cached_input_tokens: None,
        },
    )
    .await;
    let question = event(
        &pool,
        &request,
        &attempt,
        3,
        AgentEventPayload::InputRequested {
            input_id: "exact-question-id".into(),
            prompt: "Choose A or B. Preserve these choices.".into(),
        },
    )
    .await;
    AgentEventRecord::append_and_project(&pool, &question)
        .await
        .unwrap();
    let pending = tools::pending_questions(&pool, request.workspace.workspace_id)
        .await
        .unwrap();
    assert_eq!(pending.len(), 1);
    assert_eq!(
        pending[0]["payload"]["data"]["input_id"],
        "exact-question-id"
    );
    assert_eq!(pending[0]["session_id"], json!(request.session_id));
    let events = recorded_events(&pool, conversation.project_id).await;
    assert!(!events.contains("token_usage"));
    assert!(events.contains("Choose A or B"));
    assert_eq!(recorded_events(&pool, other.project_id).await, "[]");
    assert!(
        store::next_user_message(&pool, conversation.id)
            .await
            .unwrap()
            .is_none()
    );
    event(
        &pool,
        &request,
        &attempt,
        4,
        AgentEventPayload::InputResolved {
            input_id: "exact-question-id".into(),
            answered: true,
        },
    )
    .await;
    assert!(
        tools::pending_questions(&pool, request.workspace.workspace_id)
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn direct_answer_and_astra_answer_cannot_both_enter_common_command_queue() {
    let pool = database().await;
    let conversation = project(&pool).await;
    let (request, attempt) = attempt(&pool, &conversation).await;
    event(
        &pool,
        &request,
        &attempt,
        1,
        AgentEventPayload::LifecycleChanged {
            status: AgentRunStatus::Running,
        },
    )
    .await;
    event(
        &pool,
        &request,
        &attempt,
        2,
        AgentEventPayload::InputRequested {
            input_id: "q1".into(),
            prompt: "Question".into(),
        },
    )
    .await;
    let command = AgentRunPortCommandEnvelope {
        schema_version: ORCHESTRATION_COMMAND_SCHEMA_VERSION,
        command_id: Uuid::new_v4(),
        idempotency_key: Uuid::new_v4().to_string(),
        agent_run_id: request.agent_run_id,
        orchestration_run_id: None,
        orchestration_node_execution_id: None,
        correlation_id: Uuid::new_v4(),
        created_at: Utc::now(),
        command: AgentRunPortCommand::SubmitInput {
            input_id: "q1".into(),
            content: "direct user answer".into(),
        },
    };
    assert!(
        AgentRunCommandRecord::enqueue(&pool, &command)
            .await
            .unwrap()
            .1
    );
    assert!(
        !AgentRunCommandRecord::enqueue(&pool, &command)
            .await
            .unwrap()
            .1
    );
    assert!(
        tools::pending_questions(&pool, request.workspace.workspace_id)
            .await
            .unwrap()
            .is_empty()
    );
    let mut other = command.clone();
    other.command_id = Uuid::new_v4();
    other.idempotency_key = Uuid::new_v4().to_string();
    assert!(AgentRunCommandRecord::enqueue(&pool, &other).await.is_err());
    assert!(
        recorded_events(&pool, conversation.project_id)
            .await
            .contains("direct user answer")
    );
}

#[tokio::test]
async fn approval_is_not_a_specification_question_and_old_attempt_questions_expire() {
    let pool = database().await;
    let conversation = project(&pool).await;
    let (request, attempt) = attempt(&pool, &conversation).await;
    event(
        &pool,
        &request,
        &attempt,
        1,
        AgentEventPayload::ApprovalRequested {
            approval_id: "approval".into(),
            tool_call_id: None,
            tool_name: "shell".into(),
        },
    )
    .await;
    let questions = tools::pending_questions(&pool, request.workspace.workspace_id)
        .await
        .unwrap();
    assert_eq!(questions[0]["payload"]["type"], "approval_requested");
    event(
        &pool,
        &request,
        &attempt,
        2,
        AgentEventPayload::LifecycleChanged {
            status: AgentRunStatus::Succeeded,
        },
    )
    .await;
    assert!(
        tools::pending_questions(&pool, request.workspace.workspace_id)
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn codex_nonblocking_questions_keep_options_and_share_answer_receipts() {
    use crate::routes::sessions::questions;
    let pool = database().await;
    let conversation = project(&pool).await;
    let (request, attempt) = attempt(&pool, &conversation).await;
    let message_id = Uuid::new_v4();
    let payload = AgentEventPayload::ProviderExtension {
        provider_namespace: "codex".into(),
        provider_event: "async_questions".into(),
        payload: json!({"schema_version":1,"message_id":message_id,"questions":[{"title":"Which behaviour?","options":["Preserve", "Replace"]},{"title":"Any constraints?","options":[]}]}),
    };
    event(&pool, &request, &attempt, 1, payload).await;
    event(
        &pool,
        &request,
        &attempt,
        2,
        AgentEventPayload::LifecycleChanged {
            status: AgentRunStatus::Running,
        },
    )
    .await;
    let pending = questions::pending(&pool, request.session_id).await.unwrap();
    assert_eq!(pending.len(), 2); // nonblocking questions are answerable while the run is live
    assert_eq!(pending[0].options, vec!["Preserve", "Replace"]);
    assert_eq!(
        pending[0].id,
        format!("{}:{message_id}:0", attempt.run_attempt_id)
    );
    assert!(
        questions::pending(&pool, Uuid::new_v4())
            .await
            .unwrap()
            .is_empty()
    );
    sqlx::query("INSERT INTO agent_question_answers(question_id,session_id,request_id,actor,answer,result) VALUES(?,?,?,'local-user','Preserve','{\"accepted\":true}')").bind(&pending[0].id).bind(request.session_id).bind(Uuid::new_v4()).execute(&pool).await.unwrap();
    let remaining = questions::pending(&pool, request.session_id).await.unwrap();
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0].title, "Any constraints?");
    let events = recorded_events(&pool, conversation.project_id).await;
    assert!(events.contains("async_questions"));
    assert!(events.contains("question_answer_submitted"));
    assert!(events.contains("local-user"));
    event(
        &pool,
        &request,
        &attempt,
        3,
        AgentEventPayload::LifecycleChanged {
            status: AgentRunStatus::Succeeded,
        },
    )
    .await;
    assert!(
        questions::pending(&pool, request.session_id)
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn old_workspace_clients_direct_answers_are_reconciled_from_canonical_input() {
    use crate::routes::sessions::questions;
    let pool = database().await;
    let conversation = project(&pool).await;
    let (request, attempt) = attempt(&pool, &conversation).await;
    event(&pool,&request,&attempt,1,AgentEventPayload::ProviderExtension{provider_namespace:"codex".into(),provider_event:"async_questions".into(),payload:json!({"schema_version":1,"message_id":Uuid::new_v4(),"questions":[{"title":"Which behaviour?","options":["Preserve","Replace"]}]})}).await;
    event(
        &pool,
        &request,
        &attempt,
        2,
        AgentEventPayload::Message {
            message: CanonicalMessage {
                message_id: Uuid::new_v4(),
                role: AgentRuntimeMessageRole::User,
                content: "Question: Which behaviour?\nAnswer: Preserve".into(),
            },
            final_output: false,
        },
    )
    .await;
    assert!(
        questions::pending(&pool, request.session_id)
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn workspace_scope_follows_issue_membership_not_a_supplied_project_id() {
    let pool = database().await;
    let a = project(&pool).await;
    let b = project(&pool).await;
    let (request, _) = attempt(&pool, &a).await;
    assert!(
        tools::scoped_workspace(&pool, a.project_id, request.workspace.workspace_id)
            .await
            .is_ok()
    );
    assert!(
        tools::scoped_workspace(&pool, b.project_id, request.workspace.workspace_id)
            .await
            .is_err()
    );
    sqlx::query("UPDATE local_workspace_links SET project_id=? WHERE workspace_id=?")
        .bind(b.project_id)
        .bind(request.workspace.workspace_id)
        .execute(&pool)
        .await
        .unwrap();
    assert!(
        local_remote::list_project_workspaces(&pool, b.project_id)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        tools::scoped_workspace(&pool, b.project_id, request.workspace.workspace_id)
            .await
            .is_err()
    );
}

#[tokio::test]
async fn discussion_and_events_cannot_mutate_even_if_model_requests_it() {
    let pool = database().await;
    let conversation = project(&pool).await;
    for role in ["user", "event"] {
        let mut m = message(&pool, &conversation, "discuss").await;
        m.role = role.into();
        if role == "event" {
            m.intent = "operate".into();
        } // forged event capability cannot grant writes
        let fake = MockServices::replies(vec![
            vec![
                call("create_issue", "bad", json!({})),
                call("start_workspace", "start", json!({})),
                call("update_issue", "edit", json!({})),
                call("change_workspace", "change", json!({})),
                call("answer_question", "answer", json!({})),
                call("merge", "merge", json!({})),
            ],
            reply(),
        ]);
        run_turn(&pool, &fake, &conversation, &m).await.unwrap();
        assert!(fake.executions.lock().unwrap().is_empty());
        assert!(
            fake.definitions
                .lock()
                .unwrap()
                .iter()
                .flatten()
                .all(|tool| tools::allowed("discuss", tool["name"].as_str().unwrap()))
        );
    }
    let operations: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM project_chat_operations")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        operations, 0,
        "denied model proposals must never enter side-effect execution"
    );
}

#[test]
fn capability_ceiling_distinguishes_creation_start_change_answer_and_merge() {
    assert!(tools::allowed("issue", "create_issue"));
    assert!(!tools::allowed("discuss", "update_issue"));
    assert!(!tools::allowed("issue", "update_issue"));
    assert!(tools::allowed("change", "update_issue"));
    assert!(!tools::allowed("issue", "start_workspace"));
    assert!(tools::allowed("implement", "create_issue"));
    assert!(tools::allowed("implement", "start_workspace"));
    assert!(!tools::allowed("implement", "merge"));
    assert!(!tools::allowed("answer", "resolve_approval"));
    assert!(!tools::allowed("change", "answer_question"));
    assert!(tools::allowed("answer", "answer_question"));
    assert!(tools::allowed("merge", "merge"));
    for intent in ["discuss", "issue", "implement", "change", "answer", "merge"] {
        assert!(!tools::allowed(intent, "shell"));
        assert!(!tools::allowed(intent, "update_configuration"));
    }
}

#[tokio::test]
async fn accepted_implementations_do_not_block_next_conversation_and_keep_targets() {
    let pool = database().await;
    let conversation = project(&pool).await;
    let two = Uuid::new_v4();
    let fake = MockServices::replies(vec![
        vec![call(
            "create_issue",
            "issue",
            json!({"title":"feature","description":"standalone requirements"}),
        )],
        vec![call(
            "start_workspace",
            "start",
            json!({"issue_id":"$created_issue"}),
        )],
        reply(),
        vec![call("start_workspace", "second", json!({"issue_id":two}))],
        reply(),
        reply(),
    ]);
    for intent in ["operate", "operate", "operate"] {
        let m = message(&pool, &conversation, intent).await;
        run_turn(&pool, &fake, &conversation, &m).await.unwrap();
    }
    {
        let calls = fake.executions.lock().unwrap();
        assert_eq!(calls.len(), 3);
        assert_eq!(calls[1].3["issue_id"], json!(calls[0].1));
        assert_eq!(calls[2].3["issue_id"], json!(two));
    }
    let history = store::messages(&pool, conversation.id, 0).await.unwrap();
    assert_eq!(history.iter().filter(|m| m.role == "assistant").count(), 3);
    assert!(history.iter().all(|m| m.status == "done"));
}

#[tokio::test]
async fn reconnect_and_retries_preserve_message_identity_and_detect_collisions() {
    let pool = database().await;
    let a = project(&pool).await;
    let b = project(&pool).await;
    let id = Uuid::new_v4();
    store::enqueue(&pool, a.id, id, "same content", "discuss")
        .await
        .unwrap();
    store::enqueue(&pool, a.id, id, "same content", "discuss")
        .await
        .unwrap();
    assert!(
        store::enqueue(&pool, b.id, id, "same content", "discuss")
            .await
            .is_err()
    );
    assert!(
        store::enqueue(&pool, a.id, id, "different", "discuss")
            .await
            .is_err()
    );
    assert_eq!(store::messages(&pool, a.id, 0).await.unwrap().len(), 1);
    assert!(store::messages(&pool, b.id, 0).await.unwrap().is_empty());
    assert_eq!(
        store::conversation(&pool, a.project_id).await.unwrap().id,
        a.id
    );
}

#[tokio::test]
async fn model_outage_after_side_effect_replays_saved_receipt_without_executing_again() {
    let pool = database().await;
    let conversation = project(&pool).await;
    let m = message(&pool, &conversation, "issue").await;
    let fake = MockServices::replies(vec![vec![call("create_issue", "once", json!({}))]]);
    fake.replies
        .lock()
        .unwrap()
        .push_back(Err(anyhow::anyhow!("mock model outage")));
    assert!(run_turn(&pool, &fake, &conversation, &m).await.is_err());
    assert_eq!(fake.executions.lock().unwrap().len(), 1);
    let restarted = MockServices::replies(vec![reply()]);
    run_turn(&pool, &restarted, &conversation, &m)
        .await
        .unwrap();
    assert!(restarted.executions.lock().unwrap().is_empty());
    let messages = store::messages(&pool, conversation.id, 0).await.unwrap();
    assert_eq!(messages.iter().filter(|m| m.role == "tool").count(), 1);
    let input = restarted.inputs.lock().unwrap();
    assert!(
        input[0]
            .iter()
            .any(|v| v["type"] == "function_call_output" && v["call_id"] == "once")
    );
}

#[tokio::test]
async fn interrupted_tool_is_reconciled_and_new_call_id_cannot_repeat_uncertain_operation() {
    let pool = database().await;
    let conversation = project(&pool).await;
    let m = message(&pool, &conversation, "implement").await;
    let operation = Uuid::new_v4();
    sqlx::query("INSERT INTO project_chat_operations(id,message_id,call_id,name,arguments) VALUES(?,?,'interrupted','start_workspace','{}')").bind(operation).bind(m.id).execute(&pool).await.unwrap();
    let fake = MockServices::default();
    let observed = invoke(
        &pool,
        &fake,
        &conversation,
        &m,
        &call("start_workspace", "interrupted", json!({})),
    )
    .await
    .unwrap();
    assert_eq!(observed["uncertain"], true);
    invoke(
        &pool,
        &fake,
        &conversation,
        &m,
        &call("start_workspace", "new-id", json!({})),
    )
    .await
    .unwrap();
    assert!(fake.executions.lock().unwrap().is_empty());
    assert_eq!(*fake.reconciled.lock().unwrap(), 1);
}

async fn recorded_events(pool: &SqlitePool, project: Uuid) -> String {
    let events: Vec<String> = sqlx::query_scalar(
        "SELECT payload FROM project_chat_events WHERE project_id=? ORDER BY sequence",
    )
    .bind(project)
    .fetch_all(pool)
    .await
    .unwrap();
    serde_json::to_string(&events).unwrap()
}

#[tokio::test]
async fn legacy_events_never_run_or_block_user_requests_after_restart() {
    let pool = database().await;
    let conversation = project(&pool).await;
    let other = project(&pool).await;
    for status in ["pending", "running", "done"] {
        let id = Uuid::new_v4();
        sqlx::query("INSERT INTO project_chat_messages(id,conversation_id,role,content,status,checkpoint,error) VALUES(?,?,'event','legacy workspace event',?,'{}','old error')")
            .bind(id).bind(conversation.id).bind(status).execute(&pool).await.unwrap();
        let event: ChatMessage = sqlx::query_as("SELECT * FROM project_chat_messages WHERE id=?")
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap();
        // A resumed event must not even load context or call the model.
        let fake = MockServices::default();
        run_turn(&pool, &fake, &conversation, &event).await.unwrap();
        assert!(fake.inputs.lock().unwrap().is_empty());
        assert!(fake.executions.lock().unwrap().is_empty());
    }
    assert!(
        store::next_user_message(&pool, conversation.id)
            .await
            .unwrap()
            .is_none()
    );
    let user = message(&pool, &conversation, "discuss").await;
    sqlx::query("UPDATE project_chat_messages SET status='running' WHERE id=?")
        .bind(user.id)
        .execute(&pool)
        .await
        .unwrap();
    // Filtering protects user requests even before startup recovery.
    assert_eq!(
        store::next_user_message(&pool, conversation.id)
            .await
            .unwrap()
            .unwrap()
            .id,
        user.id
    );
    store::recover_user_queue(&pool).await.unwrap();
    store::recover_user_queue(&pool).await.unwrap();
    let next = store::next_user_message(&pool, conversation.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(next.id, user.id);
    assert_eq!(next.status, "pending");
    assert!(
        store::next_user_message(&pool, other.id)
            .await
            .unwrap()
            .is_none()
    );
    let events: Vec<(String, String)> =
        sqlx::query_as("SELECT status,checkpoint FROM project_chat_messages WHERE role='event'")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(events, vec![("done".into(), "{}".into()); 3]);
    let fake = MockServices::replies(vec![reply()]);
    run_turn(&pool, &fake, &conversation, &next).await.unwrap();
    assert_eq!(fake.inputs.lock().unwrap().len(), 1);
    assert!(
        store::next_user_message(&pool, conversation.id)
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn saved_final_model_output_is_finished_after_restart_without_another_inference() {
    let pool = database().await;
    let conversation = project(&pool).await;
    let m = message(&pool, &conversation, "discuss").await;
    save(
        &pool,
        m.id,
        &Checkpoint {
            input: reply(),
            pending: vec![],
            steps: 1,
            state_index: None,
        },
    )
    .await
    .unwrap();
    let fake = MockServices::default();
    run_turn(&pool, &fake, &conversation, &m).await.unwrap();
    assert!(fake.inputs.lock().unwrap().is_empty());
    store::finish(&pool, &m, "duplicate").await.unwrap();
    assert_eq!(
        store::messages(&pool, conversation.id, 0)
            .await
            .unwrap()
            .iter()
            .filter(|m| m.role == "assistant")
            .count(),
        1
    );
}

fn requests(
    session_id: Uuid,
    workspace_id: Uuid,
    idempotency_key: &str,
) -> (AgentRunRequestEnvelope, RunAttemptRequest) {
    let now = Utc::now();
    let workspace = WorkspaceReference {
        workspace_id,
        mode: WorkspaceMode::SharedWorkspace,
        path: "C:/workspace".to_string(),
    };
    let request = AgentRunRequestEnvelope {
        schema_version: AGENT_REQUEST_SCHEMA_VERSION,
        payload_version: AGENT_REQUEST_PAYLOAD_VERSION,
        request_id: Uuid::new_v4(),
        idempotency_key: idempotency_key.to_string(),
        session_id,
        agent_run_id: Uuid::new_v4(),
        turn_id: Uuid::new_v4(),
        correlation_id: Uuid::new_v4(),
        intent: AgentRunIntent::Initial,
        runtime_profile_id: "CODEX:default".to_string(),
        provider_id: "codex".to_string(),
        workspace: workspace.clone(),
        input: CanonicalMessage {
            message_id: Uuid::new_v4(),
            role: AgentRuntimeMessageRole::User,
            content: "Implement WS1".to_string(),
        },
        created_at: now,
    };
    let attempt = RunAttemptRequest {
        schema_version: AGENT_REQUEST_SCHEMA_VERSION,
        payload_version: AGENT_REQUEST_PAYLOAD_VERSION,
        request_id: Uuid::new_v4(),
        idempotency_key: format!("{idempotency_key}:attempt:1"),
        session_id,
        agent_run_id: request.agent_run_id,
        turn_id: request.turn_id,
        run_attempt_id: Uuid::new_v4(),
        attempt_number: 1,
        correlation_id: request.correlation_id,
        mode: RunAttemptMode::Launch,
        transport: AgentTransportKind::AppServerJsonrpc,
        runtime_profile_id: request.runtime_profile_id.clone(),
        provider_id: request.provider_id.clone(),
        workspace,
        capability_snapshot: CapabilitySnapshot {
            schema_version: 1,
            runtime_profile_id: request.runtime_profile_id.clone(),
            provider_id: request.provider_id.clone(),
            runtime_version: Some("1.0.0".to_string()),
            protocol_version: Some("app-server-v2".to_string()),
            adapter_version: "1".to_string(),
            resolved_at: now,
            capabilities: vec![CapabilitySnapshotEntry {
                capability: AgentCapability::SessionResume,
                state: CapabilityState::Native,
                source: CapabilitySource::VersionProbe,
                emulation_policy: None,
                evidence: None,
            }],
        },
        executor_config: executors::profile::ExecutorConfig {
            executor: executors::executors::BaseCodingAgent::Codex,
            variant: Some("default".to_string()),
            model_id: None,
            agent_id: None,
            reasoning_id: None,
            permission_policy: None,
            execution_mode: None,
            goal_token_budget: None,
            goal_max_concurrent_agents: None,
        },
        selected_skills: None,
        reset_to_message_id: None,
        provider_session: None,
        created_at: now,
    };

    (request, attempt)
}

#[tokio::test]
async fn explicit_issue_edits_preserve_identity_and_reject_stale_or_other_project_updates() {
    let pool = database().await;
    let a = project(&pool).await;
    let b = project(&pool).await;
    let (request, _) = attempt(&pool, &a).await;
    let issue_id: Uuid =
        sqlx::query_scalar("SELECT issue_id FROM local_workspace_links WHERE workspace_id=?")
            .bind(request.workspace.workspace_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    let issue = local_remote::get_local_issue(&pool, issue_id)
        .await
        .unwrap();
    let args = json!({"issue_id":issue_id,"title":"Explicit edit","description":"Updated acceptance criteria","expected_updated_at":issue.updated_at});
    let operation = Uuid::new_v4();
    assert!(
        tools::edit_issue(&pool, b.project_id, operation, &args)
            .await
            .is_err()
    );
    let first = tools::edit_issue(&pool, a.project_id, operation, &args)
        .await
        .unwrap();
    assert_eq!(first["id"], json!(issue_id));
    assert_eq!(first["description"], args["description"]);
    assert_eq!(
        tools::edit_issue(&pool, a.project_id, operation, &args)
            .await
            .unwrap(),
        first
    );
    let mut stale = args.clone();
    stale["expected_updated_at"] = json!("2000-01-01T00:00:00Z");
    assert!(
        tools::edit_issue(&pool, a.project_id, Uuid::new_v4(), &stale)
            .await
            .is_err()
    );
    sqlx::query("UPDATE local_issues SET updated_at='2030-01-01 00:00:00.000' WHERE id=?")
        .bind(issue_id)
        .execute(&pool)
        .await
        .unwrap();
    let changes =
        serde_json::from_value(json!({"title":"Must not overwrite a direct edit"})).unwrap();
    assert!(
        local_remote::update_local_issue_checked(&pool, issue_id, changes, Some(issue.updated_at))
            .await
            .is_err()
    );
    assert_eq!(
        local_remote::get_local_issue(&pool, issue_id)
            .await
            .unwrap()
            .title,
        "Explicit edit"
    );
}

#[test]
fn steering_revision_ignores_stream_progress_but_detects_direct_intervention() {
    let original = json!({"runs":[{"state":{"agent_run_id":"run","status":"running","last_event_sequence":1,"last_event_id":"event-1","updated_at":"before","goal":{"objective":"Implement feature","status":"active","tokens_used":1,"time_used_seconds":1}}}],"recent_commands":[],"questions":[]});
    let revision = tools::workspace_revision(&original).unwrap();
    let mut streaming = original.clone();
    streaming["runs"][0]["state"]["last_event_sequence"] = json!(100);
    streaming["runs"][0]["state"]["last_event_id"] = json!("event-100");
    streaming["runs"][0]["state"]["updated_at"] = json!("after");
    streaming["runs"][0]["state"]["goal"]["tokens_used"] = json!(2000);
    streaming["runs"][0]["state"]["goal"]["time_used_seconds"] = json!(30);
    assert_eq!(tools::workspace_revision(&streaming).unwrap(), revision);
    let mut direct = streaming.clone();
    direct["recent_commands"] =
        json!([{"command_id":"direct-command","content":"New direct user instruction"}]);
    assert_ne!(tools::workspace_revision(&direct).unwrap(), revision);
    let mut question = streaming.clone();
    question["questions"] = json!([{"input_id":"new-question"}]);
    assert_ne!(tools::workspace_revision(&question).unwrap(), revision);
    streaming["runs"][0]["state"]["status"] = json!("succeeded");
    assert_ne!(tools::workspace_revision(&streaming).unwrap(), revision);
}

#[test]
fn omitted_mode_defaults_to_ro_and_legacy_requests_keep_their_ceiling() {
    let request = json!({"request_id":Uuid::new_v4(),"content":"Implement this"});
    let parsed: SendChatMessage = serde_json::from_value(request.clone()).unwrap();
    assert!(matches!(parsed.intent, ChatIntent::Discuss));
    for intent in [
        "discuss",
        "operate",
        "issue",
        "implement",
        "change",
        "answer",
        "merge",
    ] {
        let mut request = request.clone();
        request["intent"] = json!(intent);
        let parsed: SendChatMessage = serde_json::from_value(request).unwrap();
        assert_eq!(serde_json::to_value(parsed.intent).unwrap(), json!(intent));
    }
    for name in [
        "create_issue",
        "start_workspace",
        "update_issue",
        "change_workspace",
        "answer_question",
        "merge",
    ] {
        assert!(tools::allowed("operate", name));
        for intent in ["discuss", "", "unknown"] {
            assert!(!tools::allowed(intent, name));
        }
    }
    for name in [
        "resolve_approval",
        "shell",
        "update_configuration",
        "sync_wiki",
    ] {
        assert!(!tools::allowed("operate", name));
    }
}

#[tokio::test]
async fn operate_mock_model_routes_requests_without_operation_mode_changes() {
    let pool = database().await;
    let conversation = project(&pool).await;
    let workspace = Uuid::new_v4();
    let session = Uuid::new_v4();
    let run = Uuid::new_v4();
    // Scripted model selections exercise the server dispatch contract, not language matching.
    let scenarios = vec![
        ("こんにちは。実装するとしたらどんな案がある？", None),
        (
            "Issueに残して",
            Some((
                "create_issue",
                json!({"title":"Agreed feature", "description":"Standalone specification"}),
            )),
        ),
        (
            "さっきの実装をこう変えて",
            Some((
                "change_workspace",
                json!({"workspace_id":workspace,"session_id":session,"revision":"fresh","prompt":"Preserve compatibility"}),
            )),
        ),
        (
            "この質問にはPreserveと回答して",
            Some((
                "answer_question",
                json!({"workspace_id":workspace,"agent_run_id":run,"input_id":"original-question","revision":"fresh","answer":"Preserve"}),
            )),
        ),
        (
            "これをマージして",
            Some((
                "merge",
                json!({"selections":[{"workspace_id":workspace,"expected_commit":"accepted"}]}),
            )),
        ),
        ("対象はまだ決めていない", None),
    ];
    for (content, selected) in scenarios {
        let id = Uuid::new_v4();
        store::enqueue(&pool, conversation.id, id, content, "operate")
            .await
            .unwrap();
        let m = sqlx::query_as("SELECT * FROM project_chat_messages WHERE id=?")
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap();
        let mut responses = Vec::new();
        if let Some((name, args)) = &selected {
            responses.push(vec![call(name, "selected", args.clone())]);
        }
        responses.push(reply());
        let fake = MockServices::replies(responses);
        run_turn(&pool, &fake, &conversation, &m).await.unwrap();
        let executions = fake.executions.lock().unwrap();
        assert_eq!(executions.len(), usize::from(selected.is_some()));
        if let Some((name, args)) = selected {
            assert_eq!(executions[0].0, conversation.project_id);
            assert_eq!(executions[0].2, name);
            assert_eq!(executions[0].3, args);
        }
        for definitions in fake.definitions.lock().unwrap().iter() {
            for name in [
                "create_issue",
                "start_workspace",
                "change_workspace",
                "answer_question",
                "merge",
            ] {
                assert!(definitions.iter().any(|tool| tool["name"] == name));
            }
            assert!(
                !definitions
                    .iter()
                    .any(|tool| tool["name"] == "resolve_approval")
            );
        }
    }
}

#[tokio::test]
async fn later_operate_message_cannot_expand_saved_ro_or_legacy_checkpoint() {
    let pool = database().await;
    let conversation = project(&pool).await;
    for intent in ["discuss", "issue", "implement", "change", "answer"] {
        let old = message(&pool, &conversation, intent).await;
        save(
            &pool,
            old.id,
            &Checkpoint {
                pending: vec![call("merge", "saved-proposal", json!({}))],
                ..Default::default()
            },
        )
        .await
        .unwrap();
        let _new = message(&pool, &conversation, "operate").await;
        assert!(
            store::enqueue(&pool, conversation.id, old.id, &old.content, "operate")
                .await
                .is_err()
        );
        let fake = MockServices::replies(vec![reply()]);
        run_turn(&pool, &fake, &conversation, &old).await.unwrap();
        assert!(fake.executions.lock().unwrap().is_empty());
        let count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM project_chat_operations WHERE message_id=?")
                .bind(old.id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(count, 0);
    }
}

#[tokio::test]
async fn operate_partial_success_retains_issue_and_blocks_duplicate_start() {
    let pool = database().await;
    let conversation = project(&pool).await;
    let m = message(&pool, &conversation, "operate").await;
    let mut fake = MockServices::replies(vec![
        vec![call("create_issue", "created", json!({}))],
        vec![call(
            "start_workspace",
            "failed",
            json!({"issue_id":"$created_issue"}),
        )],
        vec![call("start_workspace", "duplicate", json!({}))],
        reply(),
    ]);
    fake.fail_start = true;
    run_turn(&pool, &fake, &conversation, &m).await.unwrap();
    {
        let calls = fake.executions.lock().unwrap();
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[1].3["issue_id"], json!(calls[0].1));
    }
    let receipts: Vec<String> = sqlx::query_scalar(
        "SELECT result FROM project_chat_operations WHERE message_id=? ORDER BY rowid",
    )
    .bind(m.id)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(receipts.len(), 2);
    let created: Value = serde_json::from_str(&receipts[0]).unwrap();
    let failed: Value = serde_json::from_str(&receipts[1]).unwrap();
    assert_eq!(created["ok"], true);
    assert_eq!(failed["ok"], false);
    assert_eq!(failed["uncertain"], true);
}
