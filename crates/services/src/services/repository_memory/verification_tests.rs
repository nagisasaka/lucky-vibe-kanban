use std::process::Command;

use db::models::{
    agent_runtime::NativeAuditStreamRecord,
    repo::Repo,
    session::{CreateSession, Session},
    workspace::{CreateWorkspace, Workspace},
    workspace_repo::{CreateWorkspaceRepo, WorkspaceRepo},
};
use executors::runtime::{
    AgentEventEnvelope, NativeAuditMetadata, NativeAuditWriter, RunAttemptRequest,
};
use serde_json::{Value, json};
use utils::repository_memory::{MemoryTest, RepositoryWikiStatus, SemanticChanges};
use uuid::Uuid;

use super::*;

struct Fixture {
    _temporary: tempfile::TempDir,
    store: RepositoryMemoryStore,
    run: RepositoryMemoryRun,
}
fn git(root: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap().trim().into()
}
impl Fixture {
    fn new() -> Self {
        let temporary = tempfile::tempdir().unwrap();
        let root = temporary.path().join("repo");
        let persistent = temporary.path().join("persistent");
        std::fs::create_dir(&root).unwrap();
        std::fs::create_dir(&persistent).unwrap();
        git(&root, &["init", "-b", "fixture"]);
        std::fs::write(root.join("source.txt"), "committed source\n").unwrap();
        git(&root, &["add", "source.txt"]);
        git(
            &root,
            &[
                "-c",
                "user.name=Fixture",
                "-c",
                "user.email=fixture@example.invalid",
                "commit",
                "-m",
                "source",
            ],
        );
        let store = RepositoryMemoryStore::at_persistent(&persistent).unwrap();
        let run_id = Uuid::new_v4();
        let workspace_id = Uuid::new_v4();
        let run = RepositoryMemoryRun {
            run_id,
            repository_id: Uuid::new_v4(),
            workspace_id,
            task_id: None,
            created_at: chrono::Utc::now(),
            base_commit: git(&root, &["rev-parse", "HEAD"]),
            target_branch: "main".into(),
            repository_path: root,
            memory_path: store.memory_path(workspace_id),
            draft_path: store.draft_path(run_id),
            wiki_status: RepositoryWikiStatus::Current,
            finalize_source: true,
        };
        store.save_run(&run).unwrap();
        let f = Self {
            _temporary: temporary,
            store,
            run,
        };
        f.draft(vec![test_claim("cargo test", MemoryTestResult::Passed)]);
        f
    }
    fn draft(&self, tests: Vec<MemoryTest>) {
        let draft = SemanticChanges {
            summary: "Verify the existing committed source".into(),
            tests,
            ..Default::default()
        };
        std::fs::write(
            self.store.draft_path(self.run.run_id),
            serde_json::to_vec(&draft).unwrap(),
        )
        .unwrap();
    }
    fn exit(&self, command: &str, exit_code: i64) -> CommandExit {
        CommandExit {
            event_id: Uuid::new_v4(),
            run_attempt_id: Uuid::new_v4(),
            command: command.into(),
            cwd: self.run.repository_path.to_string_lossy().into_owned(),
            exit_code,
        }
    }
    fn outcome(&self) -> VerificationOutcome {
        self.store.verification(self.run.run_id).unwrap().unwrap()
    }
}
fn test_claim(command: &str, result: MemoryTestResult) -> MemoryTest {
    MemoryTest {
        command: Some(command.into()),
        result,
        summary: Some("New Run claim, separately checked against command exits".into()),
    }
}

#[test]
fn clean_new_nochange_records_current_command_facts_without_source_publication() {
    let f = Fixture::new();
    capture_start(&f.store, &f.run).unwrap();
    finish(&f.store, &f.run, vec![f.exit("cargo test", 0)]).unwrap();
    let outcome = f.outcome();
    let record = outcome.record.unwrap();
    assert!(outcome.reason.is_none());
    assert_eq!(record.manifest.source_commit, f.run.base_commit);
    assert_eq!(record.manifest.base_commit, record.manifest.source_commit);
    assert!(record.manifest.changed_paths.is_empty());
    assert!(record.start.source.clean && record.end.clean);
    assert_eq!(
        record.manifest.semantics.tests[0].result,
        MemoryTestResult::Passed
    );
    assert_eq!(record.command_exits[0].exit_code, 0);
    assert!(f.store.event(f.run.run_id).unwrap().is_none());
    assert!(f.store.source_publication(f.run.run_id).unwrap().is_none());
}

#[test]
fn source_moved_before_start_is_rejected_and_same_sha_branch_change_is_declined() {
    let f = Fixture::new();
    let mut stale = f.run.clone();
    stale.base_commit = "0".repeat(40);
    assert!(capture_start(&f.store, &stale).is_err());
    assert!(f.store.verification_start(f.run.run_id).unwrap().is_none());
    capture_start(&f.store, &f.run).unwrap();
    git(
        &f.run.repository_path,
        &["checkout", "-b", "different-branch"],
    );
    finish(&f.store, &f.run, vec![f.exit("cargo test", 0)]).unwrap();
    assert_eq!(f.outcome().reason.as_deref(), Some("source_changed"));
    assert!(f.outcome().record.is_none());
}

#[test]
fn dirty_start_dirty_end_and_changed_sha_freeze_declines() {
    for expected in ["dirty_start", "dirty_end", "source_changed"] {
        let f = Fixture::new();
        if expected == "dirty_start" {
            std::fs::write(f.run.repository_path.join("untracked.txt"), "dirty").unwrap();
        }
        capture_start(&f.store, &f.run).unwrap();
        if expected == "dirty_start" {
            std::fs::remove_file(f.run.repository_path.join("untracked.txt")).unwrap();
        } else if expected == "dirty_end" {
            std::fs::write(f.run.repository_path.join("source.txt"), "changed\n").unwrap();
        } else {
            std::fs::write(
                f.run.repository_path.join("source.txt"),
                "different committed source\n",
            )
            .unwrap();
            git(&f.run.repository_path, &["add", "source.txt"]);
            git(
                &f.run.repository_path,
                &[
                    "-c",
                    "user.name=Fixture",
                    "-c",
                    "user.email=fixture@example.invalid",
                    "commit",
                    "-m",
                    "later source",
                ],
            );
        }
        finish(&f.store, &f.run, vec![f.exit("cargo test", 0)]).unwrap();
        let declined = f.outcome();
        assert!(declined.record.is_none());
        assert_eq!(declined.reason.as_deref(), Some(expected));
        // Restoring a clean matching source and rerunning cannot backfill this Run.
        git(&f.run.repository_path, &["checkout", "--", "source.txt"]);
        finish(&f.store, &f.run, vec![f.exit("cargo test", 0)]).unwrap();
        assert_eq!(f.outcome(), declined);
    }
}

#[test]
fn missing_exit_or_draft_claims_freeze_no_record_and_never_backfill() {
    for expected in ["no_current_command_exit_evidence", "no_current_test_claims"] {
        let f = Fixture::new();
        capture_start(&f.store, &f.run).unwrap();
        if expected == "no_current_test_claims" {
            f.draft(vec![]);
        }
        let exits = if expected == "no_current_command_exit_evidence" {
            vec![]
        } else {
            vec![f.exit("cargo test", 0)]
        };
        finish(&f.store, &f.run, exits).unwrap();
        let original = f.outcome();
        assert_eq!(original.reason.as_deref(), Some(expected));
        assert!(original.record.is_none());
        f.draft(vec![test_claim("cargo test", MemoryTestResult::Passed)]);
        finish(&f.store, &f.run, vec![f.exit("cargo test", 0)]).unwrap();
        assert_eq!(f.outcome(), original);
    }
}

#[test]
fn absent_ambiguous_or_disagreeing_claims_are_not_promoted_and_actual_exits_remain_distinct() {
    let f = Fixture::new();
    f.draft(vec![
        test_claim("missing claim", MemoryTestResult::Passed),
        test_claim("repeated", MemoryTestResult::Passed),
        test_claim("failed command", MemoryTestResult::Passed),
        test_claim("not run", MemoryTestResult::NotRun),
    ]);
    let draft_before = std::fs::read(&f.run.draft_path).unwrap();
    capture_start(&f.store, &f.run).unwrap();
    finish(
        &f.store,
        &f.run,
        vec![
            f.exit("/bin/bash -lc 'arbitrary normal development script'", 0),
            f.exit("repeated", 0),
            f.exit("repeated", 0),
            f.exit("failed command", 1),
        ],
    )
    .unwrap();
    let record = f.outcome().record.unwrap();
    assert!(
        record
            .manifest
            .semantics
            .tests
            .iter()
            .all(|t| t.result == MemoryTestResult::NotRun)
    );
    assert_eq!(record.command_exits.len(), 4);
    assert_eq!(record.command_exits[3].exit_code, 1);
    assert_eq!(std::fs::read(&f.run.draft_path).unwrap(), draft_before);
}

#[test]
fn repeated_and_parallel_completion_preserve_one_immutable_record() {
    let f = Fixture::new();
    capture_start(&f.store, &f.run).unwrap();
    let a = f.exit("cargo test", 0);
    let b = f.exit("cargo test", 1);
    let (a, b) = std::thread::scope(|scope| {
        let a = scope.spawn(|| finish(&f.store, &f.run, vec![a]));
        let b = scope.spawn(|| finish(&f.store, &f.run, vec![b]));
        (a.join().unwrap(), b.join().unwrap())
    });
    assert!(a.is_ok() || b.is_ok());
    let saved = f.outcome();
    finish(&f.store, &f.run, vec![f.exit("cargo test", 7)]).unwrap();
    assert_eq!(f.outcome(), saved);
    let altered = VerificationOutcome {
        record: None,
        reason: Some("replacement".into()),
    };
    assert!(
        f.store
            .publish_verification(f.run.run_id, &altered)
            .is_err()
    );
    assert_eq!(f.outcome(), saved);
}

#[test]
fn existing_source_manifest_and_historical_run_evidence_remain_unchanged() {
    let f = Fixture::new();
    let old_run = Uuid::new_v4();
    let old = ChangeManifest {
        version: 1,
        event_id: old_run,
        repository_id: f.run.repository_id,
        workspace_id: f.run.workspace_id,
        task_id: None,
        created_at: chrono::Utc::now(),
        base_commit: f.run.base_commit.clone(),
        source_commit: f.run.base_commit.clone(),
        target_branch: Some("main".into()),
        changed_paths: vec!["source.txt".into()],
        semantics: SemanticChanges {
            summary: "Historical source manifest".into(),
            ..Default::default()
        },
    };
    f.store.publish_event(&old).unwrap();
    capture_start(&f.store, &f.run).unwrap();
    finish(&f.store, &f.run, vec![f.exit("cargo test", 0)]).unwrap();
    assert_eq!(f.store.event(old_run).unwrap(), Some(old));
    assert!(f.store.verification(old_run).unwrap().is_none());
    assert!(f.store.event(f.run.run_id).unwrap().is_none());
    let g = Fixture::new();
    let mut existing = f.outcome().record.unwrap().manifest;
    existing.event_id = g.run.run_id;
    existing.repository_id = g.run.repository_id;
    existing.workspace_id = g.run.workspace_id;
    existing.base_commit = g.run.base_commit.clone();
    existing.source_commit = g.run.base_commit.clone();
    g.store.publish_event(&existing).unwrap();
    capture_start(&g.store, &g.run).unwrap();
    finish(&g.store, &g.run, vec![g.exit("cargo test", 0)]).unwrap();
    assert_eq!(g.store.event(g.run.run_id).unwrap(), Some(existing));
    assert!(g.store.verification(g.run.run_id).unwrap().is_none());
}

struct DbFixture {
    f: Fixture,
    pool: sqlx::SqlitePool,
    session: Uuid,
    attempt: Uuid,
    turn: Uuid,
    correlation: Uuid,
}
impl DbFixture {
    async fn new() -> Self {
        let f = Fixture::new();
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!("../db/migrations").run(&pool).await.unwrap();
        let workspace = Workspace::create(
            &pool,
            &CreateWorkspace {
                branch: "fixture".into(),
                name: None,
            },
            f.run.workspace_id,
        )
        .await
        .unwrap();
        let session = Uuid::new_v4();
        Session::create(
            &pool,
            &CreateSession {
                executor: Some("CODEX".into()),
                name: None,
            },
            session,
            workspace.id,
        )
        .await
        .unwrap();
        let mut repo = Repo::find_or_create(&pool, &f.run.repository_path, "verification fixture")
            .await
            .unwrap();
        sqlx::query("UPDATE repos SET id=? WHERE id=?")
            .bind(f.run.repository_id)
            .bind(repo.id)
            .execute(&pool)
            .await
            .unwrap();
        repo.id = f.run.repository_id;
        // The fresh identity is recorded before the start stamp, just as host admission does.
        WorkspaceRepo::create_many(
            &pool,
            workspace.id,
            &[CreateWorkspaceRepo {
                repo_id: repo.id,
                target_branch: "main".into(),
            }],
        )
        .await
        .unwrap();
        let now = chrono::Utc::now();
        let turn = Uuid::new_v4();
        let attempt = Uuid::new_v4();
        let correlation = Uuid::new_v4();
        let workspace = json!({"workspace_id":f.run.workspace_id,"mode":"shared_workspace","path":f.run.repository_path});
        let request = json!({"schema_version":1,"payload_version":1,"request_id":Uuid::new_v4(),"idempotency_key":f.run.run_id.to_string(),"session_id":session,"agent_run_id":f.run.run_id,"turn_id":turn,"correlation_id":correlation,"intent":"initial","runtime_profile_id":"CODEX:default","provider_id":"codex","workspace":workspace,"input":{"message_id":Uuid::new_v4(),"role":"user","content":"Verify current source without changing it"},"created_at":now});
        let capabilities = json!({"schema_version":1,"runtime_profile_id":"CODEX:default","provider_id":"codex","adapter_version":"codex-adapter-v1","resolved_at":now,"capabilities":[]});
        let attempt_request: RunAttemptRequest=serde_json::from_value(json!({"schema_version":1,"payload_version":1,"request_id":Uuid::new_v4(),"idempotency_key":attempt.to_string(),"session_id":session,"agent_run_id":f.run.run_id,"turn_id":turn,"run_attempt_id":attempt,"attempt_number":1,"correlation_id":correlation,"mode":"launch","transport":"app_server_jsonrpc","runtime_profile_id":"CODEX:default","provider_id":"codex","workspace":workspace,"capability_snapshot":capabilities,"executor_config":{"executor":"CODEX","variant":"default"},"created_at":now})).unwrap();
        AgentRunRecord::persist_identity_before_launch(
            &pool,
            &serde_json::from_value(request).unwrap(),
            &attempt_request,
        )
        .await
        .unwrap();
        sqlx::query("UPDATE agent_process_registry SET registry_status='exited',pid=111,process_started_at=?,observed_exited_at=?,exit_code=0 WHERE run_attempt_id=?").bind(now).bind(now).bind(attempt).execute(&pool).await.unwrap();
        sqlx::query("UPDATE agent_run_attempts SET status='succeeded' WHERE id=?")
            .bind(attempt)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("UPDATE agent_runs SET status='succeeded' WHERE id=?")
            .bind(f.run.run_id)
            .execute(&pool)
            .await
            .unwrap();
        let assets = f._temporary.path().join("assets");
        std::fs::create_dir(&assets).unwrap();
        let writer = NativeAuditWriter::create_in(
            &assets,
            NativeAuditMetadata {
                session_id: session,
                agent_run_id: f.run.run_id,
                turn_id: turn,
                run_attempt_id: attempt,
                run_attempt_number: 1,
                provider_id: "codex".into(),
                runtime_profile_id: "CODEX:default".into(),
                workspace_path: f.run.repository_path.to_string_lossy().into_owned(),
                runtime_version: None,
                protocol_version: Some("rust-v0.144.1".into()),
                adapter_version: "codex-adapter-v1".into(),
                mapper_version: "codex-mapper-v7".into(),
                created_at: now,
            },
        )
        .unwrap();
        NativeAuditStreamRecord::insert_open(&pool, &writer.close().unwrap())
            .await
            .unwrap();
        capture_start(&f.store, &f.run).unwrap();
        Self {
            f,
            pool,
            session,
            attempt,
            turn,
            correlation,
        }
    }
    async fn event(&self, sequence: i64, payload: Value) {
        let id = Uuid::new_v4();
        let envelope: AgentEventEnvelope=serde_json::from_value(json!({"schema_version":1,"payload_version":1,"event_id":id,"session_id":self.session,"agent_run_id":self.f.run.run_id,"turn_id":self.turn,"run_attempt_id":self.attempt,"run_attempt_number":1,"sequence":sequence,"correlation_id":self.correlation,"timestamp":chrono::Utc::now(),"native_refs":[],"payload":payload})).unwrap();
        sqlx::query("INSERT INTO agent_events(event_id,session_id,agent_run_id,turn_id,run_attempt_id,run_attempt_number,sequence,correlation_id,schema_version,payload_version,event_envelope) VALUES(?,?,?,?,?,1,?,?,1,1,?)").bind(id).bind(self.session).bind(self.f.run.run_id).bind(self.turn).bind(self.attempt).bind(sequence).bind(self.correlation).bind(serde_json::to_string(&envelope).unwrap()).execute(&self.pool).await.unwrap();
    }
    async fn shell(&self) {
        self.event(1,json!({"type":"tool_call","data":{"tool_call_id":"shell-1","tool_name":"Shell","status":"succeeded","arguments":{"command":"cargo test","cwd":self.f.run.repository_path},"result":{"exitCode":0}}})).await;
    }
}

#[tokio::test]
async fn normal_successful_nochange_collects_canonical_exits_and_preserves_failed_history() {
    let f = DbFixture::new().await;
    f.shell().await;
    let old = Uuid::new_v4();
    sqlx::query("INSERT INTO agent_runs(id,session_id,workspace_id,request_id,idempotency_key,correlation_id,schema_version,payload_version,runtime_profile_id,provider_id,workspace_mode,workspace_path,status,request_envelope) VALUES(?,?,?,?,?,?,1,1,'CODEX:default','codex','shared_workspace',?,'failed','{}')").bind(old).bind(f.session).bind(f.f.run.workspace_id).bind(Uuid::new_v4()).bind(old.to_string()).bind(Uuid::new_v4()).bind(f.f.run.repository_path.to_str().unwrap()).execute(&f.pool).await.unwrap();
    sqlx::query("UPDATE agent_runs SET created_at=? WHERE id=?")
        .bind(chrono::Utc::now() - chrono::Duration::days(1))
        .bind(old)
        .execute(&f.pool)
        .await
        .unwrap();
    complete(&f.pool, &f.f.store, &f.f.run).await.unwrap();
    let record = f.f.outcome().record.unwrap();
    assert_eq!(record.command_exits.len(), 1);
    assert_eq!(record.manifest.event_id, f.f.run.run_id);
    let status: String = sqlx::query_scalar("SELECT status FROM agent_runs WHERE id=?")
        .bind(old)
        .fetch_one(&f.pool)
        .await
        .unwrap();
    assert_eq!(status, "failed");
    assert!(f.f.store.verification(old).unwrap().is_none());
    assert!(f.f.store.event(old).unwrap().is_none());
    complete(&f.pool, &f.f.store, &f.f.run).await.unwrap();
    assert_eq!(f.f.outcome().record.unwrap(), record);
}

#[tokio::test]
async fn unknown_child_and_unsettled_process_cannot_publish_verification() {
    let f = DbFixture::new().await;
    f.shell().await;
    f.event(2,json!({"type":"agent_activity","data":{"activity":{"thread_id":"child","parent_thread_id":null,"agent_path":null,"kind":"unknown","content":null}}})).await;
    assert!(complete(&f.pool, &f.f.store, &f.f.run).await.is_err());
    assert!(f.f.store.verification(f.f.run.run_id).unwrap().is_none());
    let g = DbFixture::new().await;
    g.shell().await;
    sqlx::query("UPDATE agent_process_registry SET registry_status='running',observed_exited_at=NULL,exit_code=NULL WHERE run_attempt_id=?").bind(g.attempt).execute(&g.pool).await.unwrap();
    // Active containment defers publication instead of producing a false record.
    complete(&g.pool, &g.f.store, &g.f.run).await.unwrap();
    assert!(g.f.store.verification(g.f.run.run_id).unwrap().is_none());
}

#[tokio::test]
async fn canonical_success_text_without_structured_exit_does_not_establish_a_test() {
    let f = DbFixture::new().await;
    f.event(1,json!({"type":"tool_call","data":{"tool_call_id":"unproven","tool_name":"Shell","status":"succeeded","arguments":{"command":"cargo test","cwd":f.f.run.repository_path},"result":{"aggregatedOutput":"All tests passed; exit code 0"}}})).await;
    complete(&f.pool, &f.f.store, &f.f.run).await.unwrap();
    let outcome = f.f.outcome();
    assert!(outcome.record.is_none());
    assert_eq!(
        outcome.reason.as_deref(),
        Some("no_current_command_exit_evidence")
    );
}

#[test]
fn invalid_or_oversize_current_draft_is_preserved_and_declined_immutably() {
    let oversized =
        serde_json::to_vec(&json!({"summary":"x".repeat(140*1024),"tests":[]})).unwrap();
    for raw in [b"{invalid draft".to_vec(), oversized] {
        let f = Fixture::new();
        capture_start(&f.store, &f.run).unwrap();
        std::fs::write(&f.run.draft_path, &raw).unwrap();
        finish(&f.store, &f.run, vec![f.exit("cargo test", 0)]).unwrap();
        let outcome = f.outcome();
        assert!(outcome.record.is_none());
        assert_eq!(outcome.reason.as_deref(), Some("invalid_current_draft"));
        assert_eq!(std::fs::read(&f.run.draft_path).unwrap(), raw);
        f.draft(vec![test_claim("cargo test", MemoryTestResult::Passed)]);
        finish(&f.store, &f.run, vec![f.exit("cargo test", 0)]).unwrap();
        assert_eq!(f.outcome(), outcome);
    }
}

#[test]
fn oversized_command_snapshot_is_declined_immutably_without_changing_the_draft() {
    let f = Fixture::new();
    capture_start(&f.store, &f.run).unwrap();
    let original = std::fs::read(&f.run.draft_path).unwrap();
    let large_command = "x".repeat(128 * 1024);
    finish(
        &f.store,
        &f.run,
        vec![f.exit("cargo test", 0), f.exit(&large_command, 0)],
    )
    .unwrap();
    let outcome = f.outcome();
    assert!(outcome.record.is_none());
    assert_eq!(
        outcome.reason.as_deref(),
        Some("verification_record_too_large")
    );
    assert_eq!(std::fs::read(&f.run.draft_path).unwrap(), original);
    finish(&f.store, &f.run, vec![f.exit("cargo test", 0)]).unwrap();
    assert_eq!(f.outcome(), outcome);
}

#[tokio::test]
async fn source_reconciliation_ignores_invalid_draft_and_corrupt_verification_sidecar() {
    for corrupt_sidecar in [false, true] {
        let f = DbFixture::new().await;
        f.f.store.mark_no_source_change(f.f.run.run_id).unwrap();
        let invalid = b"{invalid fixture evidence";
        let damaged = if corrupt_sidecar {
            let directory =
                f.f._temporary
                    .path()
                    .join("persistent/knowledge/verifications");
            std::fs::create_dir_all(&directory).unwrap();
            directory.join(format!("{}.json", f.f.run.run_id))
        } else {
            f.f.run.draft_path.clone()
        };
        std::fs::write(&damaged, invalid).unwrap();
        let result = super::super::complete_successful_coding_runs(
            &f.pool,
            &f.f.store,
            f.f.run.workspace_id,
            None,
        )
        .await;
        assert_eq!(result.unwrap(), None);
        assert_eq!(std::fs::read(&damaged).unwrap(), invalid);
        if corrupt_sidecar {
            assert!(f.f.store.verification(f.f.run.run_id).is_err());
            assert!(complete(&f.pool, &f.f.store, &f.f.run).await.is_err());
        } else {
            assert!(f.f.store.read_draft(f.f.run.run_id).is_err());
        }
        assert!(f.f.store.event(f.f.run.run_id).unwrap().is_none());
    }
}

#[tokio::test]
async fn truncated_canonical_evidence_never_promotes_a_valid_prefix_command() {
    let f = DbFixture::new().await;
    f.shell().await;
    for sequence in 2..=1000 {
        f.event(
            sequence,
            json!({"type":"thinking","data":{"content":"Fixture event before the evidence limit"}}),
        )
        .await;
    }
    f.event(1001,json!({"type":"agent_activity","data":{"activity":{"thread_id":"child-beyond-prefix","parent_thread_id":null,"agent_path":null,"kind":"unknown","content":null}}})).await;
    complete(&f.pool, &f.f.store, &f.f.run).await.unwrap();
    let declined = f.f.outcome();
    assert!(declined.record.is_none());
    assert_eq!(
        declined.reason.as_deref(),
        Some("verification_evidence_limit_exceeded")
    );
    assert!(f.f.store.event(f.f.run.run_id).unwrap().is_none());
    // Neither a repeated completion nor a smaller handoff can backfill the prefix.
    complete(&f.pool, &f.f.store, &f.f.run).await.unwrap();
    finish(&f.f.store, &f.f.run, vec![f.f.exit("cargo test", 0)]).unwrap();
    assert_eq!(f.f.outcome(), declined);
    assert_eq!(
        f.f.store.read_draft(f.f.run.run_id).unwrap().unwrap().tests[0].result,
        MemoryTestResult::Passed
    );
}

#[tokio::test]
async fn native_completed_nonzero_exit_retains_failure_with_current_and_legacy_mapping() {
    use executors::{
        executors::provider_adapter::DirectProvider,
        runtime::{
            NativeAuditChannel, NativeAuditDirection, NativeAuditFrame, NativeAuditReplayMapper,
        },
    };
    for legacy in [false, true] {
        for claim in [MemoryTestResult::Failed, MemoryTestResult::Passed] {
            let f = DbFixture::new().await;
            f.f.draft(vec![test_claim("cargo test", claim.clone())]);
            let provider = DirectProvider::Codex;
            let frame = NativeAuditFrame::from_bytes(1, chrono::Utc::now(), NativeAuditDirection::Output, NativeAuditChannel::Stdout, "application/json", f.correlation,
                serde_json::to_vec(&json!({"method":"item/completed","params":{"item":{"type":"commandExecution","id":"failure","command":"cargo test","cwd":f.f.run.repository_path,"status":"completed","exitCode":1}}})).unwrap().as_slice());
            let decoded = provider.decode_native_frame(&frame).unwrap();
            let versions = if legacy {
                provider.v5_mapper().versions()
            } else {
                provider.mapper().versions()
            };
            let writer = NativeAuditWriter::create_in(
                f.f._temporary.path().join("wire-audit"),
                NativeAuditMetadata {
                    session_id: f.session,
                    agent_run_id: f.f.run.run_id,
                    turn_id: f.turn,
                    run_attempt_id: f.attempt,
                    run_attempt_number: 1,
                    provider_id: "codex".into(),
                    runtime_profile_id: "CODEX:default".into(),
                    workspace_path: f.f.run.repository_path.to_string_lossy().into_owned(),
                    runtime_version: None,
                    protocol_version: Some("rust-v0.144.1".into()),
                    adapter_version: versions.adapter_version,
                    mapper_version: versions.mapper_version,
                    created_at: chrono::Utc::now(),
                },
            )
            .unwrap();
            let manifest = writer.close().unwrap();
            let mapped = provider
                .project_provider_event(&decoded, &manifest)
                .unwrap();
            assert!(
                matches!(&mapped.durable_events[0].payload, AgentEventPayload::ToolCall {status,..} if *status == if legacy { AgentRuntimeToolStatus::Succeeded } else { AgentRuntimeToolStatus::Failed })
            );
            f.event(
                1,
                serde_json::to_value(&mapped.durable_events[0].payload).unwrap(),
            )
            .await;
            complete(&f.pool, &f.f.store, &f.f.run).await.unwrap();
            let record = f.f.outcome().record.unwrap();
            assert_eq!(record.command_exits[0].exit_code, 1);
            assert_eq!(
                record.manifest.semantics.tests[0].result,
                if claim == MemoryTestResult::Failed {
                    MemoryTestResult::Failed
                } else {
                    MemoryTestResult::NotRun
                }
            );
            assert_eq!(
                f.f.store.read_draft(f.f.run.run_id).unwrap().unwrap().tests[0].result,
                claim
            );
        }
    }
}
