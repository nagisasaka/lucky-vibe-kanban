use db::models::{
    execution_process::CreateExecutionProcess,
    execution_process_repo_state::CreateExecutionProcessRepoState,
    integration::{IntegrationPayload, IntegrationSelection, IntegrationSource},
    workspace::CreateWorkspace,
};
use serde_json::json;

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
            validation: vec![IntegrationValidation {
                command: "node --test".into(),
                cwd: ".".into(),
                required: true,
                evidence: "package.json test + selected acceptance".into(),
                environment_requirements: "isolated in-memory fixtures; no network".into(),
                execution_process_id: Some(process.id),
                exit_code: Some(0),
                result: Some("passed".into()),
            }],
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
async fn host_proof_requires_after_head_and_matches_plan_not_self_report() {
    let (pool, mut run, process, action) = fixture().await;
    // A zero exit and claimed success are insufficient before finalisation.
    assert!(verify_validation(&pool, &run).await.is_err());
    ExecutionProcessRepoState::update_after_head_commit(
        &pool,
        process.id,
        run.repository_id,
        run.payload.result_commit.as_deref().unwrap(),
    )
    .await
    .unwrap();
    verify_validation(&pool, &run).await.unwrap();
    for replacement in ["b".repeat(40), String::new()] {
        ExecutionProcessRepoState::update_after_head_commit(
            &pool,
            process.id,
            run.repository_id,
            &replacement,
        )
        .await
        .unwrap();
        assert!(verify_validation(&pool, &run).await.is_err());
    }
    ExecutionProcessRepoState::update_after_head_commit(
        &pool,
        process.id,
        run.repository_id,
        run.payload.result_commit.as_deref().unwrap(),
    )
    .await
    .unwrap();
    let mut changed = action.clone();
    let ExecutorActionType::ScriptRequest(script) = &mut changed.typ;
    script.working_dir = Some("wrong-repo".into());
    sqlx::query("UPDATE execution_processes SET executor_action=? WHERE id=?")
        .bind(sqlx::types::Json(&changed))
        .bind(process.id)
        .execute(&pool)
        .await
        .unwrap();
    assert!(verify_validation(&pool, &run).await.is_err());
    changed = action.clone();
    changed.next_action = Some(Box::new(action.clone()));
    sqlx::query("UPDATE execution_processes SET executor_action=? WHERE id=?")
        .bind(sqlx::types::Json(&changed))
        .bind(process.id)
        .execute(&pool)
        .await
        .unwrap();
    assert!(verify_validation(&pool, &run).await.is_err());
    sqlx::query("UPDATE execution_processes SET executor_action=? WHERE id=?")
        .bind(sqlx::types::Json(&action))
        .bind(process.id)
        .execute(&pool)
        .await
        .unwrap();
    run.payload.validation[0].command = "node --test subset.test.js".into();
    assert!(verify_validation(&pool, &run).await.is_err());
    run.payload.validation[0].command = "node --test".into();
    ExecutionProcess::update_completion(
        &pool,
        process.id,
        ExecutionProcessStatus::Completed,
        Some(1),
    )
    .await
    .unwrap();
    assert!(verify_validation(&pool, &run).await.is_err());
    // Replacing mandatory with optional cannot make a plan with no required
    // verification pass; optional checks still need real evidence when present.
    run.payload.validation[0].required = false;
    assert!(verify_validation(&pool, &run).await.is_err());
}

#[tokio::test]
async fn proposal_rejects_changed_selection_empty_plan_and_placeholder_success() {
    let (_pool, run, _, _) = fixture().await;
    let mut value = json!({"outcome":"ready","selected_card_ids":[run.payload.sources[0].selection.card_id],
        "summary":"all selected requirements integrated; host tests pending", "validation":[{"command":"node --test","cwd":".","required":true,"evidence":"package.json","environment_requirements":"no external services"}],"exclusions":[],"semantics":{"goal":"combined request","summary":"compatibility resolution"}});
    proposal(&value.to_string(), &run).unwrap();
    value.as_object_mut().unwrap().remove("semantics");
    let without_memory = proposal(&value.to_string(), &run).unwrap();
    assert!(!without_memory.semantics.unresolved_questions.is_empty());
    value["selected_card_ids"] = json!([]);
    assert!(proposal(&value.to_string(), &run).is_err());
    value["selected_card_ids"] = json!([run.payload.sources[0].selection.card_id]);
    for command in ["true", "exit 0", ":", " "] {
        value["validation"][0]["command"] = json!(command);
        assert!(proposal(&value.to_string(), &run).is_err());
    }
    value["validation"][0]["command"] = json!("node --test");
    value["validation"][0]["cwd"] = json!("../peer");
    assert!(proposal(&value.to_string(), &run).is_err());
    value["validation"] = json!([]);
    assert!(proposal(&value.to_string(), &run).is_err());
}

fn development_proposal(run: &IntegrationRun) -> serde_json::Value {
    json!({
        "outcome": "ready",
        "selected_card_ids": run.payload.sources.iter().map(|source| source.selection.card_id).collect::<Vec<_>>(),
        "summary": "Replay the recorded development checks on this candidate",
        "validation": [{"command": "node --test", "cwd": ".", "required": true,
            "evidence": "A final development verification", "environment_requirements": "node --version checked"}],
        "environment_blockers": [], "exclusions": []
    })
}

#[tokio::test]
async fn cumulative_plan_keeps_distinct_branch_checks_and_deduplicates_without_reusing_results() {
    let (pool, mut run, _, _) = fixture().await;
    let mut value = development_proposal(&run);
    let mut duplicate = value["validation"][0].clone();
    duplicate["required"] = json!(false);
    duplicate["evidence"] = json!("B uses the same suite, including its new regression");
    let mut other = value["validation"][0].clone();
    other["command"] = json!("make test");
    other["cwd"] = json!("native");
    other["evidence"] = json!("B final native checks");
    let mut different_directory = value["validation"][0].clone();
    different_directory["cwd"] = json!("browser");
    value["validation"]
        .as_array_mut()
        .unwrap()
        .extend([duplicate, other, different_directory]);
    let plan = proposal(&value.to_string(), &run).unwrap();
    assert!(!hold_validation_plan(&mut run, &plan));
    replace_validation_plan(&mut run, &plan);
    assert_eq!(run.payload.validation.len(), 3);
    let common = &run.payload.validation[0];
    assert!(common.required);
    assert!(common.evidence.contains("A final"));
    assert!(common.evidence.contains("B uses"));
    assert_eq!(run.payload.validation[1].command, "make test");
    assert_eq!(run.payload.validation[2].cwd, "browser");
    assert!(
        run.payload
            .validation
            .iter()
            .all(|v| v.execution_process_id.is_none()
                && v.exit_code.is_none()
                && v.result.is_none())
    );
    assert!(verify_validation(&pool, &run).await.is_err());
}

#[tokio::test]
async fn revised_plan_does_not_resurrect_ci_only_or_retired_commands_from_completed_steps() {
    for (old, reason) in [
        (
            "pnpm run prepare-db:check",
            "Earlier planning added this solely from CI; neither branch required it during development",
        ),
        (
            "make test-old",
            "A replaced test-old with node --test while preserving the same regression tests",
        ),
    ] {
        let (_, mut run, _, _) = fixture().await;
        run.payload.validation[0].command = old.into();
        super::super::steps::complete(&mut run).unwrap();
        let mut value = development_proposal(&run);
        assert!(
            proposal(&value.to_string(), &run).is_err(),
            "silent omission must need clarification"
        );
        value["exclusions"] = json!([reason]);
        let plan = proposal(&value.to_string(), &run).unwrap();
        replace_validation_plan(&mut run, &plan);
        assert_eq!(run.payload.validation.len(), 1);
        assert_eq!(run.payload.validation[0].command, "node --test");
        assert_eq!(run.payload.exclusions, [reason]);
        assert_eq!(run.payload.steps[0].validation[0].command, old);
        assert_eq!(
            run.payload.history.last().unwrap()["validation"][0]["command"],
            old
        );
    }
}

#[tokio::test]
async fn known_environment_blockers_hold_the_whole_plan_even_when_agent_claims_ready() {
    let (pool, mut run, _, _) = fixture().await;
    let mut value = development_proposal(&run);
    value["environment_blockers"] = json!([
        "node executable missing",
        "Build volume lacks the recorded required space"
    ]);
    let plan = proposal(&value.to_string(), &run).unwrap();
    assert!(hold_validation_plan(&mut run, &plan));
    assert_eq!(run.status, "blocked");
    assert_eq!(
        run.payload.stop.as_ref().unwrap().category,
        "validation_environment"
    );
    assert!(
        run.error
            .as_ref()
            .unwrap()
            .contains("node executable missing")
    );
    assert!(run.error.as_ref().unwrap().contains("Build volume"));
    assert!(run.payload.result_commit.is_none());
    assert!(run.payload.validation[0].execution_process_id.is_none());
    assert!(run.payload.validation[0].result.is_none());
    assert!(
        run.payload.gates.is_empty(),
        "do not ask for one failure exception per missing prerequisite"
    );
    assert!(verify_validation(&pool, &run).await.is_err());
    value["environment_blockers"] = json!([]);
    let repaired = proposal(&value.to_string(), &run).unwrap();
    assert!(!hold_validation_plan(&mut run, &repaired));
    // Removing a known blocker does not turn an unexecuted plan into proof.
    assert!(verify_validation(&pool, &run).await.is_err());
}

#[tokio::test]
async fn ambiguous_development_records_can_be_held_without_inventing_a_validation_plan() {
    let (pool, mut run, _, _) = fixture().await;
    let mut value = development_proposal(&run);
    value["outcome"] = json!("blocked");
    value["validation"] = json!([]);
    value["summary"] =
        json!("Does the recorded manual browser check have a repeatable development procedure?");
    let plan = proposal(&value.to_string(), &run).unwrap();
    assert!(hold_validation_plan(&mut run, &plan));
    assert_eq!(
        run.payload.stop.as_ref().unwrap().category,
        "validation_plan"
    );
    assert!(run.error.as_ref().unwrap().contains("manual browser check"));
    assert!(run.payload.validation.is_empty());
    assert!(verify_validation(&pool, &run).await.is_err());
}

#[tokio::test]
async fn dispatched_prompt_uses_resolved_api_port_and_selected_identity() {
    let (pool, mut run, _, _) = fixture().await;
    let repo = Repo::find_by_id(&pool, run.repository_id)
        .await
        .unwrap()
        .unwrap();
    super::super::steps::complete(&mut run).unwrap();
    run.payload.validation.clear();
    run.payload.exclusions = vec!["Superseded CI-only check".into()];
    run.error = Some("Missing compiler; reassess before replay".into());
    let body = prompt(&run, &repo, Path::new("/test"), Some(4021), &json!([])).unwrap();
    assert!(body.contains(&format!(
        "http://127.0.0.1:4021/api/repos/{}/parallel-context",
        repo.id
    )));
    assert!(body.contains(&run.payload.sources[0].selection.card_id.to_string()));
    assert!(body.contains(&run.payload.sources[0].commit));
    // A resumed/next-step prompt still includes the completed prefix and the
    // previous diagnosis even when the current proposal has no runnable plan.
    assert!(body.contains("package.json test + selected acceptance"));
    assert!(body.contains("node --test"));
    assert!(body.contains("Superseded CI-only check"));
    assert!(body.contains("Missing compiler; reassess before replay"));
    let unavailable = prompt(&run, &repo, Path::new("/test"), None, &json!([])).unwrap();
    assert!(!unavailable.contains("http://127.0.0.1:"));
    assert!(unavailable.contains("do not invent an endpoint"));
}

#[tokio::test]
async fn selected_followup_memory_reaches_prompt_without_importing_other_events() {
    let (pool, mut run, _, _) = fixture().await;
    let repo = Repo::find_by_id(&pool, run.repository_id)
        .await
        .unwrap()
        .unwrap();
    let temp = tempfile::tempdir().unwrap();
    let store = RepositoryMemoryStore::at_persistent(temp.path()).unwrap();
    run.payload.sources[0].description = Some("Improve tool output display".into());
    let event = ChangeManifest {
        version: 1,
        event_id: Uuid::new_v4(),
        repository_id: repo.id,
        workspace_id: run.payload.sources[0].selection.workspace_id,
        task_id: None,
        created_at: chrono::Utc::now(),
        base_commit: run.payload.sources[0].commit.clone(),
        source_commit: run.payload.sources[0].commit.clone(),
        target_branch: Some("main".into()),
        changed_paths: vec!["scripts/preview.sh".into()],
        semantics: SemanticChanges {
            goal: "Follow-up: speed up mobile reloads".into(),
            summary: "Serve built previews and load settings lazily".into(),
            decisions: vec![utils::repository_memory::MemoryDecision {
                decision: "Retain the existing browser URL".into(),
                rationale: Some("Preserve mobile access".into()),
            }],
            tests: vec![
                utils::repository_memory::MemoryTest {
                    command: Some("make test-preview".into()),
                    result: utils::repository_memory::MemoryTestResult::Failed,
                    summary: Some("Initial regression before the fix".into()),
                },
                utils::repository_memory::MemoryTest {
                    command: Some("make test-preview".into()),
                    result: utils::repository_memory::MemoryTestResult::Passed,
                    summary: Some("Final retry after fixing reload".into()),
                },
                utils::repository_memory::MemoryTest {
                    command: None,
                    result: utils::repository_memory::MemoryTestResult::NotRun,
                    summary: Some("Manual browser verification still needed".into()),
                },
            ],
            ..Default::default()
        },
    };
    store.publish_event(&event).unwrap();
    run.payload.sources[0].event_ids = vec![event.event_id];
    let mut later = event.clone();
    later.event_id = Uuid::new_v4();
    later.semantics.goal = "LATER UNSELECTED CHANGE".into();
    store.publish_event(&later).unwrap();
    let mut foreign = later.clone();
    foreign.event_id = Uuid::new_v4();
    foreign.workspace_id = Uuid::new_v4();
    foreign.semantics.goal = "OTHER WORKSPACE CHANGE".into();
    store.publish_event(&foreign).unwrap();

    let memory = integration_memory(&run, Some(&store)).unwrap();
    assert_eq!(memory[0]["events"].as_array().unwrap().len(), 1);
    assert_eq!(memory[0]["events"][0]["event_id"], json!(event.event_id));
    assert_eq!(
        memory[0]["events"][0]["decisions"][0]["rationale"],
        "Preserve mobile access"
    );
    let body = prompt(&run, &repo, Path::new("/test"), None, &memory).unwrap();
    assert!(body.contains("Improve tool output display"));
    assert!(body.contains(&event.semantics.goal));
    for test in &event.semantics.tests {
        assert!(body.contains(test.summary.as_ref().unwrap()));
    }
    assert!(body.contains("make test-preview"));
    assert!(body.contains("TA union TB"));
    assert!(body.contains("do not explore them to expand the plan"));
    assert!(!body.contains(&later.semantics.goal));
    assert!(!body.contains(&foreign.semantics.goal));
    assert!(body.contains("Do not block solely for that omission"));
    assert!(!body.contains("Integrate ALL and ONLY the selected requirements"));
    assert!(body.contains("Do not read peer-private Workspace Memory or conversations"));

    // Absent public memory is represented honestly, not converted into a veto.
    let missing = integration_memory(&run, None).unwrap();
    assert_eq!(missing[0]["unavailable_event_ids"], json!([event.event_id]));
    assert!(missing[0]["events"].as_array().unwrap().is_empty());
    run.payload.sources[0].event_ids = vec![foreign.event_id];
    assert!(integration_memory(&run, Some(&store)).is_err());
    run.payload.sources[0].event_ids = vec![event.event_id];
    run.repository_id = Uuid::new_v4();
    assert!(integration_memory(&run, Some(&store)).is_err());
}

#[tokio::test]
async fn disabled_memory_does_not_publish_events_or_integrations() {
    let (pool, mut run, _, _) = fixture().await;
    let temp = tempfile::tempdir().unwrap();
    let store = RepositoryMemoryStore::at_persistent(temp.path()).unwrap();
    store
        .save_state(&utils::repository_memory::RepositoryMemoryState::default())
        .unwrap();
    let repo = Repo::find_by_id(&pool, run.repository_id)
        .await
        .unwrap()
        .unwrap();
    run.payload.sources.clear();
    run.payload.base_commit = Some("b".repeat(40));
    run.payload.integration_manifest = Some(ChangeManifest {
        version: 1,
        event_id: run.id,
        repository_id: repo.id,
        workspace_id: run.workspace_id.unwrap(),
        task_id: None,
        created_at: chrono::Utc::now(),
        base_commit: run.payload.base_commit.clone().unwrap(),
        source_commit: run.payload.result_commit.clone().unwrap(),
        target_branch: Some("main".into()),
        changed_paths: vec!["source.txt".into()],
        semantics: SemanticChanges {
            goal: "combine selected sources".into(),
            summary: "integrate".into(),
            ..Default::default()
        },
    });
    assert_eq!(
        record_memory_integration_at(&git::GitService::new(), &repo, &run, &store).unwrap(),
        "disabled"
    );
    assert!(
        store.events().unwrap().is_empty(),
        "disabled Memory must not acquire new events"
    );
    assert!(
        store.integrations().unwrap().is_empty(),
        "Git/Done receipts belong to Integration, not disabled Memory"
    );
    // Old absent/corrupt semantic inputs must not hold Git/Card post-processing
    // hostage when the repository explicitly disabled memory.
    run.payload.sources = fixture().await.1.payload.sources.clone();
    run.payload.sources[0].event_ids = vec![Uuid::new_v4()];
    assert_eq!(
        record_memory_integration_at(&git::GitService::new(), &repo, &run, &store).unwrap(),
        "disabled"
    );
}

#[tokio::test]
async fn semantic_outbox_uses_only_frozen_events_and_rejects_changed_or_missing_inputs() {
    let (pool, mut run, _, _) = fixture().await;
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("repo");
    let persistent = temp.path().join("persistent");
    std::fs::create_dir(&root).unwrap();
    std::fs::create_dir(&persistent).unwrap();
    let cmd = |args: &[&str]| {
        let out = std::process::Command::new("git")
            .arg("-C")
            .arg(&root)
            .args(args)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8(out.stdout).unwrap().trim().to_owned()
    };
    cmd(&["init", "-b", "main"]);
    let commit = |name: &str| {
        cmd(&[
            "-c",
            "user.name=Test",
            "-c",
            "user.email=test@example.com",
            "commit",
            "--allow-empty",
            "-m",
            name,
        ]);
        cmd(&["rev-parse", "HEAD"])
    };
    let base = commit("base");
    let selected = commit("selected source");
    let later = commit("later source not selected");
    let mut repo = Repo::find_by_id(&pool, run.repository_id)
        .await
        .unwrap()
        .unwrap();
    repo.path = root;
    let store = RepositoryMemoryStore::at_persistent(&persistent).unwrap();
    let state = utils::repository_memory::RepositoryMemoryState {
        enabled: true,
        target_branch: Some("main".into()),
        ..Default::default()
    };
    store.save_state(&state).unwrap();
    let event = ChangeManifest {
        version: 1,
        event_id: Uuid::new_v4(),
        repository_id: repo.id,
        workspace_id: run.payload.sources[0].selection.workspace_id,
        task_id: None,
        created_at: chrono::Utc::now(),
        base_commit: base.clone(),
        source_commit: selected.clone(),
        target_branch: Some("main".into()),
        changed_paths: vec!["source.txt".into()],
        semantics: SemanticChanges {
            goal: "selected source intent".into(),
            summary: "selected change".into(),
            ..Default::default()
        },
    };
    store.publish_event(&event).unwrap();
    let mut newer = event.clone();
    newer.event_id = Uuid::new_v4();
    newer.source_commit = later.clone();
    store.publish_event(&newer).unwrap();
    let mut peer = newer.clone();
    peer.event_id = Uuid::new_v4();
    peer.workspace_id = Uuid::new_v4();
    peer.target_branch = Some("another-target".into());
    store.publish_event(&peer).unwrap();
    let source = &mut run.payload.sources[0];
    source.commit = selected.clone();
    source.selection.expected_commit = selected.clone();
    source.event_ids = vec![event.event_id, event.event_id];
    run.payload.base_commit = Some(base);
    run.payload.result_commit = Some(selected);
    let git = git::GitService::new();
    for _ in 0..2 {
        assert_eq!(
            record_memory_integration_at(&git, &repo, &run, &store).unwrap(),
            "pending existing OpenWiki reconciliation"
        );
        let records = store.integrations().unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].event_ids, vec![event.event_id]);
        assert_eq!(records[0].target_branch, "main");
    }
    // Later source events cannot silently enter the frozen set, and a peer
    // event cannot be substituted even when its Git object exists here.
    for invalid in [newer.event_id, peer.event_id, Uuid::new_v4()] {
        let mut changed = run.clone();
        changed.payload.sources[0].event_ids = vec![invalid];
        assert!(record_memory_integration_at(&git, &repo, &changed, &store).is_err());
    }
    let mut changed_target = run.clone();
    changed_target.target_ref = "refs/heads/another-target".into();
    assert!(record_memory_integration_at(&git, &repo, &changed_target, &store).is_err());
    // Corrupt a required frozen record: discovery may partially display other
    // records, but strict Integration publication may not ignore this failure.
    std::fs::write(
        persistent
            .join("knowledge/events")
            .join(format!("{}.json", event.event_id)),
        "{invalid",
    )
    .unwrap();
    assert!(record_memory_integration_at(&git, &repo, &run, &store).is_err());
    assert_eq!(
        store.integrations().unwrap()[0].event_ids,
        vec![event.event_id]
    );
}

#[tokio::test]
async fn git_publication_survives_receipt_failure_without_a_second_merge() {
    let (pool, mut run, _, _) = fixture().await;
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    let cmd = |args: &[&str]| {
        let out = std::process::Command::new("git")
            .arg("-C")
            .arg(root)
            .args(args)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8(out.stdout).unwrap().trim().to_owned()
    };
    cmd(&["init", "-b", "main"]);
    std::fs::write(root.join("source.txt"), "base").unwrap();
    cmd(&["add", "source.txt"]);
    cmd(&[
        "-c",
        "user.name=Test",
        "-c",
        "user.email=test@example.com",
        "commit",
        "-m",
        "base",
    ]);
    let base = cmd(&["rev-parse", "HEAD"]);
    cmd(&["checkout", "-b", "result"]);
    std::fs::write(root.join("source.txt"), "validated result").unwrap();
    cmd(&["add", "source.txt"]);
    cmd(&[
        "-c",
        "user.name=Test",
        "-c",
        "user.email=test@example.com",
        "commit",
        "-m",
        "result",
    ]);
    let result = cmd(&["rev-parse", "HEAD"]);
    cmd(&["checkout", "main"]);
    sqlx::query("UPDATE repos SET path=? WHERE id=?")
        .bind(root.to_str().unwrap())
        .bind(run.repository_id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO projects(id,name) VALUES(?,'publication')")
        .bind(run.project_id)
        .execute(&pool)
        .await
        .unwrap();
    run.payload.sources[0].branch = "main".into();
    run.payload.sources[0].commit = base.clone();
    run.payload.sources[0].selection.expected_commit = base.clone();
    run.payload.base_commit = Some(base.clone());
    run.payload.result_commit = Some(result.clone());
    run.payload.publication_intent = true;
    run.status = "publishing".into();
    sqlx::query("INSERT INTO integration_runs(id,request_key,project_id,repository_id,storage_identity,target_ref,status,workspace_id,session_id,payload) VALUES(?,?,?,?,?,?,'publishing',?,?,?)")
        .bind(run.id).bind(&run.request_key).bind(run.project_id).bind(run.repository_id).bind(root.to_str().unwrap()).bind(&run.target_ref)
        .bind(run.workspace_id).bind(run.session_id).bind(&run.payload).execute(&pool).await.unwrap();
    sqlx::query("CREATE TRIGGER fail_publication_receipt BEFORE UPDATE ON integration_runs WHEN json_extract(NEW.payload,'$.published')=1 BEGIN SELECT RAISE(ABORT,'simulated crash before receipt'); END;")
        .execute(&pool).await.unwrap();
    let repo = Repo::find_by_id(&pool, run.repository_id)
        .await
        .unwrap()
        .unwrap();
    let git = git::GitService::new();
    let targets = vec![root.to_path_buf()];
    assert!(
        publish_candidate(&git, &pool, &repo, &mut run, &targets)
            .await
            .is_err()
    );
    assert_eq!(cmd(&["rev-parse", "main"]), result);
    assert_eq!(
        std::fs::read_to_string(root.join("source.txt")).unwrap(),
        "validated result"
    );
    let mut recovered = IntegrationRun::find(&pool, run.id).await.unwrap();
    assert!(recovered.payload.publication_intent);
    assert!(!recovered.payload.published);
    assert!(
        recovered
            .payload
            .sources
            .iter()
            .all(|source| source.done_result.is_none())
    );
    assert_eq!(
        git.inspect_exact_publication(root, &run.target_ref, &base, &result, false, &targets)
            .unwrap(),
        git::publication::ExactPublicationState::Applied
    );
    sqlx::query("DROP TRIGGER fail_publication_receipt")
        .execute(&pool)
        .await
        .unwrap();
    // An unconfirmed publication must still use the frozen source, even when
    // Git has moved. Only the recovered publication receipt enables completion.
    assert_eq!(recovered.payload.sources[0].selection.expected_commit, base);
    recovered.payload.published = true;
    recovered.status = "post_processing".into();
    recovered.save(&pool).await.unwrap();
    assert_eq!(cmd(&["rev-list", "--count", "main"]), "2");
    // Recovery does not invoke the writer a second time, even for a repeated UI request.
    assert!(
        publish_candidate(&git, &pool, &repo, &mut recovered, &targets)
            .await
            .is_err()
    );
    assert_eq!(cmd(&["rev-parse", "result"]), result);
    let persisted = IntegrationRun::find(&pool, run.id).await.unwrap();
    let candidate = persisted.payload.result_commit.as_deref().unwrap();
    assert_eq!(candidate, result);
    assert_eq!(persisted.payload.sources[0].selection.expected_commit, base);
    assert_eq!(persisted.payload.sources[0].commit, base);
    assert!(git.require_clean_source(root, &base).is_err());
    git.require_clean_source(root, candidate).unwrap();

    // Later source edits do not change the immutable candidate or selection.
    std::fs::write(root.join("source.txt"), "later work").unwrap();
    assert!(git.require_clean_source(root, candidate).is_err());
    cmd(&["add", "source.txt"]);
    cmd(&[
        "-c",
        "user.name=Test",
        "-c",
        "user.email=test@example.com",
        "commit",
        "-m",
        "later work",
    ]);
    assert!(git.require_clean_source(root, candidate).is_err());
    assert_eq!(persisted.payload.sources[0].selection.expected_commit, base);
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
async fn exception_preserves_failure_and_never_waives_identity_or_settlement() {
    let (pool, mut run, process, _) = fixture().await;
    ExecutionProcess::update_completion(
        &pool,
        process.id,
        ExecutionProcessStatus::Completed,
        Some(127),
    )
    .await
    .unwrap();
    ExecutionProcessRepoState::update_after_head_commit(
        &pool,
        process.id,
        run.repository_id,
        run.payload.result_commit.as_deref().unwrap(),
    )
    .await
    .unwrap();
    run.payload.validation[0].exit_code = Some(127);
    run.payload.validation[0].result = Some("unavailable".into());
    let gate = db::models::integration::IntegrationGate {
        id: Uuid::new_v4(),
        validation_index: 0,
        scope: run.validation_scope(),
        risk: "unverified behaviour".into(),
        evidence: run.payload.validation[0].clone(),
        decision: None,
        actor: None,
        decided_at: None,
        request_id: None,
    };
    run.payload.gates.push(gate);
    assert!(
        verify_validation(&pool, &run).await.is_err(),
        "unanswered is not approval"
    );
    run.payload.gates[0].decision = Some("cancel".into());
    assert!(
        verify_validation(&pool, &run).await.is_err(),
        "denial is not approval"
    );
    run.payload.gates[0].decision = Some("continue".into());
    run.payload.gates[0].actor = Some(Uuid::from_u128(1));
    run.payload.gates[0].decided_at = Some(chrono::Utc::now().to_rfc3339());
    run.payload.gates[0].request_id = Some(Uuid::new_v4());
    verify_validation(&pool, &run).await.unwrap();
    assert_eq!(
        run.payload.validation[0].result.as_deref(),
        Some("unavailable")
    );
    assert_eq!(run.payload.validation[0].exit_code, Some(127));
    let saved: IntegrationRun =
        serde_json::from_value(serde_json::to_value(&run).unwrap()).unwrap();
    verify_validation(&pool, &saved).await.unwrap();
    for changed in [
        "candidate",
        "base",
        "project",
        "plan",
        "requirements",
        "check-result",
    ] {
        let mut stale = saved.clone();
        match changed {
            "candidate" => stale.payload.result_commit = Some("b".repeat(40)),
            "base" => stale.payload.base_commit = Some("b".repeat(40)),
            "project" => stale.project_id = Uuid::new_v4(),
            "plan" => stale.payload.validation[0].command = "other test".into(),
            "requirements" => stale.payload.sources[0].requirements_revision += 1,
            _ => stale.payload.validation[0].result = Some("passed".into()),
        }
        assert!(
            !stale.validation_excepted(0),
            "{changed} must invalidate the exception"
        );
        assert!(verify_validation(&pool, &stale).await.is_err());
    }
    ExecutionProcessRepoState::update_after_head_commit(
        &pool,
        process.id,
        run.repository_id,
        &"c".repeat(40),
    )
    .await
    .unwrap();
    assert!(
        verify_validation(&pool, &run).await.is_err(),
        "an exception cannot waive a changed checkout"
    );
    ExecutionProcess::update_completion(&pool, process.id, ExecutionProcessStatus::Running, None)
        .await
        .unwrap();
    assert!(
        verify_validation(&pool, &run).await.is_err(),
        "a live writer cannot be waived"
    );
}

#[tokio::test]
async fn approved_failed_check_uses_exact_git_publication_and_rejects_target_races() {
    for target_moves in [false, true] {
        let (pool, mut run, process, _) = fixture().await;
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        let git_cmd = |args: &[&str]| {
            let output = std::process::Command::new("git")
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
            String::from_utf8(output.stdout).unwrap().trim().to_owned()
        };
        git_cmd(&["init", "-b", "main"]);
        let commit = |message: &str| {
            git_cmd(&[
                "-c",
                "user.name=Test",
                "-c",
                "user.email=test@example.invalid",
                "commit",
                "--allow-empty",
                "-m",
                message,
            ]);
            git_cmd(&["rev-parse", "HEAD"])
        };
        let base = commit("base");
        git_cmd(&["checkout", "-b", "candidate"]);
        let candidate = commit("candidate");
        git_cmd(&["checkout", "main"]);
        sqlx::query("UPDATE repos SET path=? WHERE id=?")
            .bind(root.to_str().unwrap())
            .bind(run.repository_id)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("UPDATE execution_process_repo_states SET before_head_commit=?,after_head_commit=? WHERE execution_process_id=?")
            .bind(&candidate).bind(&candidate).bind(process.id).execute(&pool).await.unwrap();
        ExecutionProcess::update_completion(
            &pool,
            process.id,
            ExecutionProcessStatus::Completed,
            Some(1),
        )
        .await
        .unwrap();
        run.status = "blocked".into();
        run.payload.base_commit = Some(base.clone());
        run.payload.result_commit = Some(candidate.clone());
        run.payload.validation[0].result = Some("failed".into());
        run.payload.validation[0].exit_code = Some(1);
        let evidence = run.payload.validation[0].clone();
        let scope = run.validation_scope();
        let gate = Uuid::new_v4();
        run.payload
            .gates
            .push(db::models::integration::IntegrationGate {
                id: gate,
                validation_index: 0,
                scope: scope.clone(),
                risk: "Unverified regression risk".into(),
                evidence,
                decision: None,
                actor: None,
                decided_at: None,
                request_id: None,
            });
        sqlx::query("INSERT INTO projects(id,name) VALUES(?,'exception publication')")
            .bind(run.project_id)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO integration_runs(id,request_key,project_id,repository_id,storage_identity,target_ref,status,workspace_id,session_id,payload) VALUES(?,?,?,?,?,?,'blocked',?,?,?)")
            .bind(run.id).bind(&run.request_key).bind(run.project_id).bind(run.repository_id).bind(&run.storage_identity).bind(&run.target_ref).bind(run.workspace_id).bind(run.session_id).bind(&run.payload).execute(&pool).await.unwrap();
        assert!(verify_validation(&pool, &run).await.is_err());
        assert_eq!(git_cmd(&["rev-parse", "main"]), base);
        IntegrationRun::decide_validation_gate(
            &pool,
            run.id,
            run.project_id,
            gate,
            Uuid::new_v4(),
            "continue",
            &scope,
        )
        .await
        .unwrap();
        run = IntegrationRun::find(&pool, run.id).await.unwrap();
        verify_validation(&pool, &run).await.unwrap();
        if target_moves {
            commit("another writer");
        }
        run.payload.publication_intent = true;
        let mut tx = pool.begin_with("BEGIN IMMEDIATE").await.unwrap();
        IntegrationRun::record_publication_intent(&mut tx, &run)
            .await
            .unwrap();
        tx.commit().await.unwrap();
        let repo = Repo::find_by_id(&pool, run.repository_id)
            .await
            .unwrap()
            .unwrap();
        let git = git::GitService::new();
        let outcome = publish_candidate(&git, &pool, &repo, &mut run, &[root.to_path_buf()]).await;
        if target_moves {
            assert!(outcome.is_err());
            assert_ne!(git_cmd(&["rev-parse", "main"]), candidate);
            assert!(
                !IntegrationRun::find(&pool, run.id)
                    .await
                    .unwrap()
                    .payload
                    .published
            );
        } else {
            outcome.unwrap();
            assert_eq!(git_cmd(&["rev-parse", "main"]), candidate);
            let saved = IntegrationRun::find(&pool, run.id).await.unwrap();
            assert!(saved.payload.published);
            assert_eq!(
                saved.payload.validation[0].result.as_deref(),
                Some("failed")
            );
            assert!(
                publish_candidate(&git, &pool, &repo, &mut run, &[root.to_path_buf()])
                    .await
                    .is_err()
            );
        }
    }
}

#[tokio::test]
async fn sequential_merges_review_only_resolutions_and_retain_cumulative_tests() {
    use crate::routes::integrations::steps;
    let (_, mut run, _, _) = fixture().await;
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    let command = |args: &[&str]| {
        let output = std::process::Command::new("git")
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
        String::from_utf8(output.stdout).unwrap().trim().to_string()
    };
    command(&["init", "-b", "main"]);
    command(&["config", "user.name", "Test"]);
    command(&["config", "user.email", "test@example.invalid"]);
    std::fs::write(root.join("feature.txt"), "base\n").unwrap();
    command(&["add", "."]);
    command(&["commit", "-m", "base"]);
    let base = command(&["rev-parse", "HEAD"]);
    command(&["checkout", "-b", "a"]);
    std::fs::write(root.join("feature.txt"), "A\n").unwrap();
    command(&["commit", "-am", "A"]);
    let a = command(&["rev-parse", "HEAD"]);
    command(&["checkout", "-b", "b", &base]);
    std::fs::write(root.join("feature.txt"), "B\n").unwrap();
    command(&["commit", "-am", "B"]);
    let b = command(&["rev-parse", "HEAD"]);
    command(&["checkout", "-b", "integration", &base]);
    run.payload.sequential = true;
    run.payload.step_base = Some(base.clone());
    run.payload.base_commit = Some(base);
    run.payload.sources[0].commit = a.clone();
    let mut second = run.payload.sources[0].clone();
    second.selection.card_id = Uuid::new_v4();
    second.commit = b.clone();
    run.payload.sources.push(second);
    steps::prepare(root, &mut run).unwrap();
    assert!(!run.payload.merge_conflicted);
    command(&["merge", "--no-ff", "--no-edit", &a]);
    run.payload.result_commit = Some(command(&["rev-parse", "HEAD"]));
    steps::record_review(root, &mut run).unwrap();
    assert!(run.payload.reviews.is_empty());
    // Even without textual conflicts, extra AI compatibility changes need review.
    command(&["checkout", "-b", "extra-edits"]);
    std::fs::write(root.join("compatibility.txt"), "additional fix\n").unwrap();
    command(&["add", "."]);
    command(&["commit", "-m", "compatibility fix"]);
    let mut extra = run.clone();
    extra.payload.result_commit = Some(command(&["rev-parse", "HEAD"]));
    steps::record_review(root, &mut extra).unwrap();
    assert_eq!(extra.payload.reviews.len(), 1);
    assert!(extra.payload.reviews[0].diff.contains("+additional fix"));
    assert!(!steps::review_approved(&extra));
    command(&["checkout", "integration"]);
    steps::complete(&mut run).unwrap();
    steps::complete(&mut run).unwrap(); // receipt replay is harmless
    assert_eq!(run.payload.steps.len(), 1);
    run.payload.step_index = 1;
    run.payload.step_base = run.payload.result_commit.clone();
    steps::prepare(root, &mut run).unwrap();
    assert!(run.payload.merge_conflicted);
    assert!(!steps::review_approved(&run));
    let merge = std::process::Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["merge", "--no-ff", "--no-edit", &b])
        .output()
        .unwrap();
    assert_eq!(merge.status.code(), Some(1));
    std::fs::write(root.join("feature.txt"), "A and B\n").unwrap();
    command(&["add", "."]);
    command(&["commit", "-m", "preserve A and B"]);
    run.payload.result_commit = Some(command(&["rev-parse", "HEAD"]));
    // The host receives the full organised plan; it must execute both branches'
    // checks again on the new cumulative candidate.
    let mut value = development_proposal(&run);
    let mut b_check = value["validation"][0].clone();
    b_check["command"] = json!("make test-b");
    b_check["evidence"] = json!("B development verification");
    value["validation"].as_array_mut().unwrap().push(b_check);
    let plan = proposal(&value.to_string(), &run).unwrap();
    replace_validation_plan(&mut run, &plan);
    assert_eq!(run.payload.validation.len(), 2);
    assert!(run.payload.validation[0].execution_process_id.is_none());
    assert!(run.payload.validation[0].result.is_none());
    steps::record_review(root, &mut run).unwrap();
    let review = run.payload.reviews.last().unwrap();
    assert!(review.diff.contains("+A and B"));
    assert!(review.diff.contains("<<<<<<<"));
    assert!(!steps::review_approved(&run));
    let review = run.payload.reviews.last_mut().unwrap();
    review.decision = Some("continue".into());
    review.request_id = Some(Uuid::new_v4());
    review.actor = Some(Uuid::from_u128(1));
    review.decided_at = Some("now".into());
    assert!(steps::review_approved(&run));
    let mut skipped = run.clone();
    skipped.payload.skip_validation = true;
    skipped.payload.validation.clear();
    // Changing the policy cannot reuse an existing review. A skipped run still
    // needs approval of its actual conflict-resolution diff and candidate.
    assert!(!steps::review_approved(&skipped));
    steps::record_review(root, &mut skipped).unwrap();
    assert!(!steps::review_approved(&skipped));
    let review = skipped.payload.reviews.last_mut().unwrap();
    assert!(review.diff.contains("+A and B"));
    review.decision = Some("continue".into());
    review.request_id = Some(Uuid::new_v4());
    review.actor = Some(Uuid::from_u128(1));
    review.decided_at = Some("now".into());
    assert!(steps::review_approved(&skipped));
    skipped.payload.result_commit = Some("c".repeat(40));
    assert!(!steps::review_approved(&skipped));
    run.payload.result_commit = Some("c".repeat(40));
    assert!(!steps::review_approved(&run));
}

#[tokio::test]
async fn explicit_skip_is_unverified_preserved_and_not_an_agent_selected_policy() {
    let (pool, mut run, _, _) = fixture().await;
    let mut value = development_proposal(&run);
    value["validation"] = json!([]);
    // An agent cannot opt out for a normally admitted run.
    assert!(proposal(&value.to_string(), &run).is_err());
    let old_scope = run.validation_scope();
    run.payload.skip_validation = true;
    assert_ne!(run.validation_scope(), old_scope);
    assert!(verify_validation(&pool, &run).await.is_err());
    run.payload.validation.clear();
    value["semantics"] = json!({"goal":"selected changes", "summary":"unverified merge", "tests":[{"command":"fabricated", "result":"passed"}]});
    let plan = proposal(&value.to_string(), &run).unwrap();
    assert!(plan.summary.contains("Unverified"));
    assert_eq!(plan.semantics.tests.len(), 1);
    assert_eq!(
        plan.semantics.tests[0].result,
        utils::repository_memory::MemoryTestResult::NotRun
    );
    assert!(!hold_validation_plan(&mut run, &plan));
    replace_validation_plan(&mut run, &plan);
    verify_validation(&pool, &run).await.unwrap();
    super::super::steps::complete(&mut run).unwrap();
    let restored: IntegrationRun =
        serde_json::from_value(serde_json::to_value(&run).unwrap()).unwrap();
    assert!(restored.payload.skip_validation);
    assert!(restored.payload.steps[0].validation_skipped);
    assert!(restored.payload.steps[0].validation.is_empty());
    verify_validation(&pool, &restored).await.unwrap();
    // A resumed record cannot turn skipped steps into verified evidence.
    run.payload.skip_validation = false;
    assert!(verify_validation(&pool, &run).await.is_err());
    run.payload.skip_validation = true;
    run.payload.steps[0].validation_skipped = false;
    assert!(verify_validation(&pool, &run).await.is_err());
    run.payload.steps.clear();
    value["validation"] = development_proposal(&run)["validation"].clone();
    assert!(proposal(&value.to_string(), &run).is_err());
    value["validation"] = json!([]);
    value["outcome"] = json!("blocked");
    let blocked = proposal(&value.to_string(), &run).unwrap();
    assert!(hold_validation_plan(&mut run, &blocked));
    assert_eq!(run.status, "blocked");
    assert!(run.payload.result_commit.is_none());
}

#[tokio::test]
async fn skipped_prompt_omits_validation_planning_but_keeps_merge_review() {
    let (pool, mut run, _, _) = fixture().await;
    let repo = Repo::find_by_id(&pool, run.repository_id)
        .await
        .unwrap()
        .unwrap();
    run.payload.skip_validation = true;
    run.payload.validation.clear();
    run.payload.sequential = true;
    let body = prompt(&run, &repo, Path::new("/test"), None, &json!([])).unwrap();
    assert!(body.contains("VALIDATION SKIPPED BY EXPLICIT USER REQUEST"));
    assert!(body.contains("Do not run tests, type checks, lint, builds"));
    assert!(body.contains("retains human approval for conflicts"));
    assert!(body.contains(r#""validation":[],"environment_blockers":[]"#));
    assert!(!body.contains("VALIDATION SCOPE: replay"));
    assert!(!body.contains("Before any heavy validation"));
    assert!(!body.contains("LVK runs tests after each step"));
    run.payload.merge_conflicted = true;
    assert!(!super::super::steps::review_approved(&run));
}

#[test]
fn legacy_payload_and_steps_are_not_relabelled_as_skipped() {
    let mut value = serde_json::to_value(IntegrationPayload::default()).unwrap();
    value.as_object_mut().unwrap().remove("skip_validation");
    value["steps"] = json!([{"source_commit":"source", "result_commit":"result", "validation":[], "gates":[], "summary":"old record"}]);
    let payload: IntegrationPayload = serde_json::from_value(value).unwrap();
    assert!(!payload.skip_validation);
    assert!(!payload.steps[0].validation_skipped);
}

mod cancellation_recovery {
    use chrono::Utc;
    use db::models::orchestration::{
        OrchestrationNodeExecutionRecord, OrchestrationOutboxRecord, OrchestrationRunRecord,
    };
    use executors::runtime::*;

    use super::*;

    #[tokio::test]
    async fn cancellation_distinguishes_unaccepted_identity_from_inflight_or_missing_evidence() {
        let (pool, mut run, _, _) = fixture().await;
        let plan = OrchestrationPlanSnapshot {
            schema_version: 1,
            plan_id: run.id,
            source_definition_id: run.id,
            source_definition_version: "test".into(),
            product_kind: OrchestrationProductKind::Integration,
            workspace_mode: WorkspaceMode::IsolatedWorktree,
            created_at: Utc::now(),
            nodes: vec![OrchestrationPlanNode {
                node_key: "integrate".into(),
                stable_order: 0,
                dependencies: vec![],
                join: OrchestrationJoinPolicy::All,
                failure_policy: OrchestrationFailurePolicy::FailFast,
                remaining_upstreams: RemainingUpstreamsPolicy::Continue,
                each_downstream_execution: EachDownstreamExecution::Serial,
                retry: OrchestrationRetryPolicy::default(),
                runtime_profile_id: None,
                provider_id: None,
                provider_config: None,
                requires_product_validation: true,
            }],
        };
        OrchestrationRunRecord::persist_before_dispatch(
            &pool,
            run.id,
            run.id,
            "cancel-recovery",
            run.id,
            &plan,
        )
        .await
        .unwrap();
        let node = Uuid::new_v4();
        OrchestrationNodeExecutionRecord::persist_identity_before_dispatch(
            &pool,
            node,
            run.id,
            "integrate",
            0,
            0,
        )
        .await
        .unwrap();
        let (mut request, mut attempt) = cancellation_requests(
            run.session_id.unwrap(),
            run.workspace_id.unwrap(),
            "cancel-agent",
        );
        request.correlation_id = run.id;
        attempt.correlation_id = run.id;
        let agent = request.agent_run_id;
        run.agent_run_id = Some(agent);
        assert!(
            cancellation_agent(&pool, &run).await.is_err(),
            "missing command is not proof of non-acceptance"
        );
        let command = AgentRunPortCommandEnvelope {
            schema_version: 1,
            command_id: Uuid::new_v4(),
            idempotency_key: "cancel-create".into(),
            agent_run_id: agent,
            orchestration_run_id: Some(run.id),
            orchestration_node_execution_id: Some(node),
            correlation_id: run.id,
            created_at: Utc::now(),
            command: AgentRunPortCommand::Create {
                request: request.clone(),
                attempt: attempt.clone(),
            },
        };
        let outbox = OrchestrationOutboxRecord::enqueue(&pool, Uuid::new_v4(), &command)
            .await
            .unwrap();
        assert!(
            cancellation_agent(&pool, &run).await.is_err(),
            "live parent can still dispatch"
        );
        sqlx::query("UPDATE orchestration_runs SET status='failed' WHERE id=?")
            .bind(run.id)
            .execute(&pool)
            .await
            .unwrap();
        assert_eq!(cancellation_agent(&pool, &run).await.unwrap(), None);
        sqlx::query("UPDATE orchestration_outbox SET delivery_status='delivering' WHERE id=?")
            .bind(outbox)
            .execute(&pool)
            .await
            .unwrap();
        assert!(
            cancellation_agent(&pool, &run).await.is_err(),
            "inflight create may have been accepted"
        );
        db::models::agent_runtime::AgentRunRecord::persist_identity_before_launch(
            &pool, &request, &attempt,
        )
        .await
        .unwrap();
        assert_eq!(
            cancellation_agent(&pool, &run).await.unwrap(),
            Some(agent),
            "accepted but not yet linked agent must be stopped"
        );
    }
    pub(super) fn cancellation_requests(
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
}

mod instruction_startup {
    use std::sync::Mutex;

    use db::models::agent_runtime::AgentRunRecord;
    use executors::runtime::*;

    use super::*;

    struct StartingPort {
        pool: SqlitePool,
        state: Mutex<Option<RunState>>,
        calls: Mutex<Vec<Uuid>>,
        error: Mutex<Option<&'static str>>,
    }

    #[async_trait::async_trait]
    impl AgentRunPort for StartingPort {
        async fn create(
            &self,
            request: AgentRunRequestEnvelope,
            attempt: RunAttemptRequest,
        ) -> Result<Uuid, AgentRunPortError> {
            AgentRunRecord::persist_identity_before_launch(&self.pool, &request, &attempt)
                .await
                .unwrap();
            let mut state = RunState::pending(&request);
            // Real host registration reports Running before the provider thread exists.
            state.status = AgentRunStatus::Running;
            *self.state.lock().unwrap() = Some(state);
            Ok(request.agent_run_id)
        }

        async fn query(&self, id: Uuid) -> Result<AgentRunPortSnapshot, AgentRunPortError> {
            Ok(AgentRunPortSnapshot {
                agent_run_id: id,
                state: self.state.lock().unwrap().clone().unwrap(),
            })
        }

        async fn subscribe(&self, _: Uuid) -> Result<AgentEventStream, AgentRunPortError> {
            panic!("unexpected subscribe")
        }

        async fn control(
            &self,
            command: AgentRunPortCommandEnvelope,
        ) -> Result<(), AgentRunPortError> {
            assert!(matches!(command.command, AgentRunPortCommand::Steer { .. }));
            self.calls.lock().unwrap().push(command.command_id);
            match *self.error.lock().unwrap() {
                Some(message) => Err(AgentRunPortError::Unavailable(message.into())),
                None => Ok(()),
            }
        }
    }

    #[tokio::test]
    async fn fresh_create_immediate_steer_waits_retries_same_command_and_fences_terminal() {
        let (pool, mut run, _, _) = fixture().await;
        run.payload.skip_validation = true;
        let (request, attempt) = cancellation_recovery::cancellation_requests(
            run.session_id.unwrap(),
            run.workspace_id.unwrap(),
            "startup-instruction",
        );
        let port = StartingPort {
            pool: pool.clone(),
            state: Mutex::new(None),
            calls: Mutex::new(vec![]),
            error: Mutex::new(Some("I/O error: Codex has no active thread")),
        };
        let agent = port.create(request, attempt).await.unwrap();
        let command = AgentRunPortCommandEnvelope {
            schema_version: 1,
            command_id: Uuid::new_v4(),
            idempotency_key: "startup-steer".into(),
            agent_run_id: agent,
            orchestration_run_id: None,
            orchestration_node_execution_id: None,
            correlation_id: run.id,
            created_at: chrono::Utc::now(),
            command: AgentRunPortCommand::Steer {
                content: "operator instruction".into(),
            },
        };
        for error in [
            "I/O error: Codex has no active thread",
            "I/O error: Codex has no active turn",
        ] {
            *port.error.lock().unwrap() = Some(error);
            assert_eq!(
                deliver_intervention_instruction(&pool, &port, command.clone())
                    .await
                    .unwrap(),
                InstructionDelivery::Waiting
            );
            assert_eq!(
                port.state.lock().unwrap().as_ref().unwrap().status,
                AgentRunStatus::Running
            );
        }
        *port.error.lock().unwrap() = None;
        for _ in 0..2 {
            assert_eq!(
                deliver_intervention_instruction(&pool, &port, command.clone())
                    .await
                    .unwrap(),
                InstructionDelivery::Delivered
            );
        }
        assert_eq!(*port.calls.lock().unwrap(), vec![command.command_id; 3]);
        let saved: (String, i64) = sqlx::query_as(
            "SELECT delivery_status,delivery_attempts FROM agent_run_commands WHERE command_id=?",
        )
        .bind(command.command_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(saved, ("delivered".into(), 3));
        // Host acceptance is separate from the child's delivery receipt. A
        // structured ready result completes only the current frozen source.
        run.payload.sequential = true;
        let first_source = run.payload.sources[0].clone();
        for index in 0..3 {
            let mut source = first_source.clone();
            source.selection.card_id = Uuid::new_v4();
            source.commit = format!("{:040x}", index + 2);
            run.payload.sources.push(source);
        }
        run.payload.validation.clear(); // admission chose skip for all four sources
        let mut ready = development_proposal(&run);
        ready["validation"] = json!([]);
        let plan = proposal(&ready.to_string(), &run).unwrap();
        assert!(!hold_validation_plan(&mut run, &plan));
        replace_validation_plan(&mut run, &plan);
        verify_validation(&pool, &run).await.unwrap();
        super::super::super::steps::complete(&mut run).unwrap();
        super::super::super::steps::complete(&mut run).unwrap();
        assert_eq!(run.payload.steps.len(), 1);
        assert!(run.payload.steps[0].validation_skipped);
        assert_eq!(run.payload.sources.len(), 4);
        assert!(!run.payload.published);
        assert!(
            run.payload.steps.len() < run.payload.sources.len(),
            "host must continue to the remaining sources"
        );
        for status in [
            AgentRunStatus::Pending,
            AgentRunStatus::Starting,
            AgentRunStatus::Cancelling,
            AgentRunStatus::Cancelled,
            AgentRunStatus::Failed,
            AgentRunStatus::Succeeded,
        ] {
            port.state.lock().unwrap().as_mut().unwrap().status = status;
            let expected = if status.is_terminal() {
                InstructionDelivery::Terminal
            } else {
                InstructionDelivery::Waiting
            };
            assert_eq!(
                deliver_intervention_instruction(&pool, &port, command.clone())
                    .await
                    .unwrap(),
                expected
            );
        }
        assert_eq!(
            port.calls.lock().unwrap().len(),
            3,
            "terminal/waiting executions must never be steered"
        );
        assert!(run.payload.skip_validation);
    }

    #[tokio::test]
    async fn ambiguous_provider_failure_is_not_a_startup_retry() {
        let (pool, run, _, _) = fixture().await;
        let (request, attempt) = cancellation_recovery::cancellation_requests(
            run.session_id.unwrap(),
            run.workspace_id.unwrap(),
            "startup-error",
        );
        let port = StartingPort {
            pool: pool.clone(),
            state: Mutex::new(None),
            calls: Mutex::new(vec![]),
            error: Mutex::new(Some("transport disconnected")),
        };
        let agent = port.create(request, attempt).await.unwrap();
        let command = AgentRunPortCommandEnvelope {
            schema_version: 1,
            command_id: Uuid::new_v4(),
            idempotency_key: "startup-error-steer".into(),
            agent_run_id: agent,
            orchestration_run_id: None,
            orchestration_node_execution_id: None,
            correlation_id: run.id,
            created_at: chrono::Utc::now(),
            command: AgentRunPortCommand::Steer {
                content: "instruction".into(),
            },
        };
        assert!(
            deliver_intervention_instruction(&pool, &port, command)
                .await
                .unwrap_err()
                .to_string()
                .contains("transport disconnected")
        );
    }
}
