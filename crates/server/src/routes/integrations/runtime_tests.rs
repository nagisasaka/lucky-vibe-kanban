use db::models::{
    execution_process::{
        CreateExecutionProcess, ExecutionProcessRunReason, ExecutionProcessStatus,
    },
    execution_process_repo_state::{CreateExecutionProcessRepoState, ExecutionProcessRepoState},
    integration::{IntegrationPayload, IntegrationSelection, IntegrationSource},
    session::CreateSession,
    workspace::CreateWorkspace,
    workspace_repo::{CreateWorkspaceRepo, WorkspaceRepo},
};
use executors::actions::{
    ExecutorAction, ExecutorActionType,
    script::{ScriptContext, ScriptRequest, ScriptRequestLanguage},
};

use super::*;

async fn fixture() -> (SqlitePool, IntegrationRun, ExecutionProcess, ExecutorAction) {
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::migrate!("../db/migrations").run(&pool).await.unwrap();
    let repo = Repo::find_or_create(&pool, Path::new("/test-validation"), "source")
        .await
        .unwrap();
    let ws = Workspace::create(
        &pool,
        &CreateWorkspace {
            branch: "test".into(),
            name: None,
        },
        Uuid::new_v4(),
    )
    .await
    .unwrap();
    let session = Session::create(
        &pool,
        &CreateSession {
            executor: None,
            name: None,
        },
        Uuid::new_v4(),
        ws.id,
    )
    .await
    .unwrap();
    let action = ExecutorAction::new(
        ExecutorActionType::ScriptRequest(ScriptRequest {
            script: "node --test".into(),
            language: ScriptRequestLanguage::Bash,
            context: ScriptContext::IntegrationValidation,
            working_dir: Some(format!("{}/.", repo.name)),
        }),
        None,
    );
    let oid = "a".repeat(40);
    let process = ExecutionProcess::create(
        &pool,
        &CreateExecutionProcess {
            session_id: session.id,
            executor_action: action.clone(),
            run_reason: ExecutionProcessRunReason::IntegrationValidation,
        },
        Uuid::new_v4(),
        &[CreateExecutionProcessRepoState {
            repo_id: repo.id,
            before_head_commit: Some(oid.clone()),
            after_head_commit: None,
            merge_commit: None,
        }],
    )
    .await
    .unwrap();
    ExecutionProcess::update_completion(
        &pool,
        process.id,
        ExecutionProcessStatus::Completed,
        Some(0),
    )
    .await
    .unwrap();
    let run = IntegrationRun {
        id: Uuid::new_v4(),
        request_key: "test".into(),
        project_id: Uuid::new_v4(),
        repository_id: repo.id,
        storage_identity: "/git/common".into(),
        target_ref: "refs/heads/main".into(),
        status: "validating".into(),
        workspace_id: Some(ws.id),
        session_id: Some(session.id),
        agent_run_id: None,
        payload: sqlx::types::Json(IntegrationPayload {
            sources: vec![IntegrationSource {
                selection: IntegrationSelection {
                    card_id: Uuid::new_v4(),
                    workspace_id: ws.id,
                    expected_commit: oid.clone(),
                },
                branch: "source".into(),
                commit: oid.clone(),
                title: "request".into(),
                description: None,
                status_id: Uuid::new_v4(),
                requirements_revision: 1,
                event_ids: vec![],
                related_workspaces: vec![ws.id],
                done_result: None,
            }],
            result_commit: Some(oid),
            ..Default::default()
        }),
        cancel_requested: false,
        error: None,
        created_at: String::new(),
        updated_at: String::new(),
    };
    (pool, run, process, action)
}

#[tokio::test]
async fn conversation_evidence_keeps_code_sha_across_wiki_and_rejects_stale_or_forged_success() {
    use db::models::integration::{IntegrationEvidence, IntegrationPublicationRequest};
    let (pool, mut run, process, _) = fixture().await;
    let code = run.payload.result_commit.clone().unwrap();
    run.payload.result_commit = Some("b".repeat(40)); // Wiki-only final commit: tests remain on code.
    run.payload.publication_request = Some(IntegrationPublicationRequest {
        request_id: Uuid::new_v4(),
        agent_run_id: Uuid::new_v4(),
        code_commit: code.clone(),
        wiki_commit: run.payload.result_commit.clone(),
        candidate_commit: "b".repeat(40),
    });
    run.payload.evidence.push(IntegrationEvidence {
        request_id: Uuid::new_v4(),
        agent_run_id: Uuid::new_v4(),
        code_commit: code.clone(),
        command: Some("node --test".into()),
        process_id: Some(process.id),
        result: "passed".into(),
        actor: "host_execution".into(),
        reason: "risk-based code check".into(),
        remaining_risk: String::new(),
    });
    assert!(
        super::super::conversation::verify_evidence(&pool, &run)
            .await
            .is_err()
    ); // after SHA not yet recorded
    ExecutionProcessRepoState::update_after_head_commit(
        &pool,
        process.id,
        run.repository_id,
        &code,
    )
    .await
    .unwrap();
    super::super::conversation::verify_evidence(&pool, &run)
        .await
        .unwrap();
    run.payload
        .publication_request
        .as_mut()
        .unwrap()
        .code_commit = "c".repeat(40);
    assert!(
        super::super::conversation::verify_evidence(&pool, &run)
            .await
            .is_err()
    ); // repaired candidate cannot reuse old pass
    run.payload
        .publication_request
        .as_mut()
        .unwrap()
        .code_commit = code;
    run.payload.evidence[0].command = Some("different test".into());
    assert!(
        super::super::conversation::verify_evidence(&pool, &run)
            .await
            .is_err()
    );
    run.payload.evidence[0].command = Some("node --test".into());
    run.payload.evidence[0].actor = "agent".into();
    assert!(
        super::super::conversation::verify_evidence(&pool, &run)
            .await
            .is_err()
    ); // an agent claim is not a host test event
}

#[tokio::test]
async fn conversation_skip_is_candidate_bound_with_actor_reason_and_risk_without_required_test_plan()
 {
    use db::models::integration::{IntegrationEvidence, IntegrationPublicationRequest};
    let (pool, mut run, _, _) = fixture().await;
    run.payload.validation_suggestions.clear();
    let code = run.payload.result_commit.clone().unwrap();
    run.payload.publication_request = Some(IntegrationPublicationRequest {
        request_id: Uuid::new_v4(),
        agent_run_id: Uuid::new_v4(),
        code_commit: code.clone(),
        wiki_commit: None,
        candidate_commit: code.clone(),
    });
    for actor in ["agent", "user"] {
        run.payload.evidence = vec![IntegrationEvidence {
            request_id: Uuid::new_v4(),
            agent_run_id: Uuid::new_v4(),
            code_commit: code.clone(),
            command: None,
            process_id: None,
            result: "skipped".into(),
            actor: actor.into(),
            reason: "Existing tested source is adopted without a code delta".into(),
            remaining_risk: "No fresh environment verification".into(),
        }];
        super::super::conversation::verify_evidence(&pool, &run)
            .await
            .unwrap();
        run.payload.evidence[0].remaining_risk.clear();
        assert!(
            super::super::conversation::verify_evidence(&pool, &run)
                .await
                .is_err()
        );
        run.payload.evidence[0].remaining_risk = "risk".into();
        run.payload.evidence[0].code_commit = "d".repeat(40);
        assert!(
            super::super::conversation::verify_evidence(&pool, &run)
                .await
                .is_err()
        );
    }
}

#[tokio::test]
async fn conversation_failure_is_preserved_and_requires_repair_or_recorded_skip() {
    use db::models::integration::{IntegrationEvidence, IntegrationPublicationRequest};
    let (pool, mut run, process, _) = fixture().await;
    let code = run.payload.result_commit.clone().unwrap();
    run.payload.publication_request = Some(IntegrationPublicationRequest {
        request_id: Uuid::new_v4(),
        agent_run_id: Uuid::new_v4(),
        code_commit: code.clone(),
        wiki_commit: None,
        candidate_commit: code.clone(),
    });
    ExecutionProcessRepoState::update_after_head_commit(
        &pool,
        process.id,
        run.repository_id,
        &code,
    )
    .await
    .unwrap();
    ExecutionProcess::update_completion(&pool, process.id, ExecutionProcessStatus::Failed, Some(1))
        .await
        .unwrap();
    run.payload.evidence.push(IntegrationEvidence {
        request_id: Uuid::new_v4(),
        agent_run_id: Uuid::new_v4(),
        code_commit: code.clone(),
        command: Some("node --test".into()),
        process_id: Some(process.id),
        result: "failed".into(),
        actor: "host_execution".into(),
        reason: "run test".into(),
        remaining_risk: "failure".into(),
    });
    assert!(
        super::super::conversation::verify_evidence(&pool, &run)
            .await
            .is_err()
    );
    let mut skip = run.payload.evidence[0].clone();
    skip.request_id = Uuid::new_v4();
    skip.process_id = None;
    skip.result = "skipped".into();
    skip.actor = "user".into();
    skip.reason = "Explicit instruction to omit rerunning this check".into();
    run.payload.evidence.push(skip);
    super::super::conversation::verify_evidence(&pool, &run)
        .await
        .unwrap();
    run.payload.evidence[1].command = None; // explicit all-validation omission
    super::super::conversation::verify_evidence(&pool, &run)
        .await
        .unwrap();
    assert_eq!(run.payload.evidence[0].result, "failed");
    let p = ExecutionProcess::find_by_id(&pool, process.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(p.exit_code, Some(1));
}

#[tokio::test]
async fn direct_folder_parent_uses_member_checkout_and_remains_a_target_writer() {
    let (pool, mut run, _, _) = fixture().await;
    let temp = tempfile::tempdir().unwrap();
    let parent = temp.path().join("repos");
    let root = parent.join("nested-repository");
    std::fs::create_dir_all(&root).unwrap();
    let cmd = |args: &[&str]| {
        let output = std::process::Command::new("git")
            .arg("-C")
            .arg(&root)
            .args(args)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap().trim().to_owned()
    };
    cmd(&["init", "-b", "main"]);
    cmd(&[
        "-c",
        "user.name=Test",
        "-c",
        "user.email=test@example.invalid",
        "commit",
        "--allow-empty",
        "-m",
        "base",
    ]);
    let head = cmd(&["rev-parse", "HEAD"]);
    let repo = Repo::find_or_create(&pool, &root, "nested").await.unwrap();
    run.repository_id = repo.id;
    run.target_ref = "refs/heads/main".into();
    let git = git::GitService::new();
    // Reproduce the original failure: the selected folder is not itself a repo.
    assert!(git.require_clean_source(&parent, &head).is_err());
    let mut direct_ids = Vec::new();
    for directory in [&parent, &root] {
        let workspace = Workspace::create_direct_folder(
            &pool,
            &CreateWorkspace {
                branch: "main".into(),
                name: None,
            },
            Uuid::new_v4(),
            directory.to_str().unwrap(),
        )
        .await
        .unwrap();
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
        let checkout = super::super::repository_checkout(directory, true, &repo.path, &repo.name);
        git.require_clean_source(&checkout, &head).unwrap();
        direct_ids.push(workspace.id);
    }
    let container = temp.path().join("managed");
    std::fs::create_dir(&container).unwrap();
    let peer_root = container.join(&repo.name);
    cmd(&["worktree", "add", "-b", "peer", peer_root.to_str().unwrap()]);
    let peer = Workspace::create(
        &pool,
        &CreateWorkspace {
            branch: "peer".into(),
            name: None,
        },
        Uuid::new_v4(),
    )
    .await
    .unwrap();
    Workspace::update_container_ref(&pool, peer.id, container.to_str().unwrap())
        .await
        .unwrap();
    WorkspaceRepo::create_many(
        &pool,
        peer.id,
        &[CreateWorkspaceRepo {
            repo_id: repo.id,
            target_branch: "main".into(),
        }],
    )
    .await
    .unwrap();
    let checkout = super::super::repository_checkout(&container, false, &repo.path, &repo.name);
    assert_eq!(checkout, peer_root);
    git.require_clean_source(&checkout, &head).unwrap();
    std::fs::write(peer_root.join("uncommitted.txt"), "pending work").unwrap();
    assert!(git.require_clean_source(&checkout, &head).is_err());
    git.require_clean_source(&root, &head).unwrap();

    let (targets, writers) = managed_targets(&pool, &repo, &run).await.unwrap();
    assert!(targets.iter().all(|path| path == &root));
    assert_eq!(
        writers.into_iter().collect::<HashSet<_>>(),
        direct_ids.into_iter().collect()
    );
    // A managed peer only blocks publication when its own checkout holds target.
    run.target_ref = "refs/heads/peer".into();
    let (targets, writers) = managed_targets(&pool, &repo, &run).await.unwrap();
    assert!(targets.contains(&peer_root));
    assert_eq!(writers, vec![peer.id]);
}

#[tokio::test]
async fn compact_worker_input_keeps_fixed_identity_and_adopted_references_separate() {
    let (_pool, mut run, _, _) = fixture().await;
    run.payload.sources[0].description = Some("historical: investigation only".into());
    run.payload.reference_materials =
        Some(serde_json::json!({"public_manifests":"historical: do not publish"}));
    run.payload.instructions = "今回の追加指示を原文で保持".into();
    let mut second = run.payload.sources[0].clone();
    second.commit = "b".repeat(40);
    second.selection.workspace_id = Uuid::new_v4();
    run.payload.sources.push(second);
    let identity = services::services::integration_admission::worker_context(
        &run,
        "candidate",
        Path::new("/candidate/repo"),
    );
    assert!(identity.contains("repository作業path=/candidate/repo"));
    assert!(identity.contains("候補branch=refs/heads/candidate"));
    assert!(identity.contains(&run.payload.observed_target));
    let content = super::super::support::worker_content(&run, &identity).unwrap();
    assert!(content.contains(&identity));
    assert!(content.contains(&"a".repeat(40)) && content.contains(&"b".repeat(40)));
    assert!(content.contains(&run.payload.instructions));
    assert!(content.contains("reference_section=sources"));
    assert!(content.contains("相互作用") && content.contains("remaining_risk"));
    assert!(!content.contains("historical:"));
    assert!(!content.contains("---\nname:"));
    assert_eq!(
        run.payload.sources[0].description.as_deref(),
        Some("historical: investigation only")
    );
}
