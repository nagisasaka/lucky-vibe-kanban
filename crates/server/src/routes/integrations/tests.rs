use std::path::Path;

use db::models::{
    integration::{IntegrationPayload, IntegrationRun},
    repo::Repo,
    workspace::{CreateWorkspace, Workspace},
    workspace_repo::{CreateWorkspaceRepo, WorkspaceRepo},
};
use sqlx::SqlitePool;
use uuid::Uuid;

use super::{admission, project_repository_ids};

async fn project(pool: &SqlitePool) -> (Uuid, Uuid) {
    let id = Uuid::new_v4();
    let status = Uuid::new_v4();
    let card = Uuid::new_v4();
    sqlx::query("INSERT INTO projects(id,name) VALUES(?,'local Board')")
        .bind(id)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO local_project_statuses(id,project_id,name,color,sort_order) VALUES(?,?,'Todo','blue',0)")
        .bind(status).bind(id).execute(pool).await.unwrap();
    sqlx::query("INSERT INTO local_issues(id,project_id,issue_number,simple_id,status_id,title,sort_order) VALUES(?,?,1,'TEST-1',?,'request',0)")
        .bind(card).bind(id).bind(status).execute(pool).await.unwrap();
    (id, card)
}

#[tokio::test]
async fn local_board_repository_candidates_follow_real_workspace_links_without_legacy_binding() {
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::migrate!("../db/migrations").run(&pool).await.unwrap();
    let (project_a, card_a) = project(&pool).await;
    let (project_b, card_b) = project(&pool).await;
    let repo = Repo::find_or_create(&pool, Path::new("/local-board-repo"), "repo")
        .await
        .unwrap();
    let unrelated = Repo::find_or_create(&pool, Path::new("/unrelated-repo"), "other")
        .await
        .unwrap();
    let mut workspaces = Vec::new();
    for _ in 0..2 {
        let workspace = Workspace::create(
            &pool,
            &CreateWorkspace {
                branch: "feature".into(),
                name: None,
            },
            Uuid::new_v4(),
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
        sqlx::query(
            "INSERT INTO local_workspace_links(workspace_id,issue_id,project_id) VALUES(?,?,?)",
        )
        .bind(workspace.id)
        .bind(card_a)
        .bind(project_a)
        .execute(&pool)
        .await
        .unwrap();
        workspaces.push(workspace.id);
    }
    let legacy_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM project_repos")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(legacy_count, 0);
    assert_eq!(
        project_repository_ids(&pool, project_a).await.unwrap(),
        vec![repo.id]
    );
    assert!(
        project_repository_ids(&pool, project_b)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        !project_repository_ids(&pool, project_a)
            .await
            .unwrap()
            .contains(&unrelated.id)
    );
    // Re-linking changes current Board membership, not historical event ownership.
    for id in workspaces {
        sqlx::query(
            "UPDATE local_workspace_links SET issue_id=?,project_id=? WHERE workspace_id=?",
        )
        .bind(card_b)
        .bind(project_b)
        .bind(id)
        .execute(&pool)
        .await
        .unwrap();
    }
    assert!(
        project_repository_ids(&pool, project_a)
            .await
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        project_repository_ids(&pool, project_b).await.unwrap(),
        vec![repo.id]
    );
    // Existing explicit bindings still work; overlapping associations deduplicate.
    for project in [project_a, project_b] {
        sqlx::query("INSERT INTO project_repos(id,project_id,repo_id) VALUES(?,?,?)")
            .bind(Uuid::new_v4())
            .bind(project)
            .bind(repo.id)
            .execute(&pool)
            .await
            .unwrap();
        assert_eq!(
            project_repository_ids(&pool, project).await.unwrap(),
            vec![repo.id]
        );
    }
}

#[tokio::test]
async fn target_queue_uses_storage_identity_and_keeps_recovery_reservations() {
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::migrate!("../db/migrations").run(&pool).await.unwrap();
    let (project, _) = project(&pool).await;
    let mut runs = Vec::new();
    for (index, storage) in ["/git/shared", "/git/shared", "/git/independent"]
        .iter()
        .enumerate()
    {
        // Different registered IDs must not bypass the common-storage FIFO.
        let repo =
            Repo::find_or_create(&pool, Path::new(&format!("/registration-{index}")), "repo")
                .await
                .unwrap();
        let id = Uuid::new_v4();
        sqlx::query("INSERT INTO integration_runs(id,request_key,project_id,repository_id,storage_identity,target_ref,status,payload,created_at) VALUES(?,?,?,?,?,'refs/heads/main','queued',?,?)")
            .bind(id).bind(id.to_string()).bind(project).bind(repo.id).bind(storage)
            .bind(sqlx::types::Json(IntegrationPayload::default()))
            .bind(format!("2026-09-18 00:00:0{index}"))
            .execute(&pool).await.unwrap();
        runs.push(IntegrationRun::find(&pool, id).await.unwrap());
    }
    assert!(!admission::target_right(&pool, &runs[1]).await.unwrap());
    assert!(admission::target_right(&pool, &runs[0]).await.unwrap());
    assert!(admission::target_right(&pool, &runs[0]).await.unwrap());
    assert!(admission::target_right(&pool, &runs[2]).await.unwrap());
    sqlx::query("UPDATE integration_runs SET status='recovery_required' WHERE id=?")
        .bind(runs[0].id)
        .execute(&pool)
        .await
        .unwrap();
    assert!(!admission::target_right(&pool, &runs[1]).await.unwrap());
    // Neither missing dispatcher leases nor a terminal row alone releases the
    // business reservation. Only confirmed cleanup/recovery calls release.
    sqlx::query("DELETE FROM orchestration_leases")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE integration_runs SET status='cancelled' WHERE id=?")
        .bind(runs[0].id)
        .execute(&pool)
        .await
        .unwrap();
    assert!(!admission::target_right(&pool, &runs[1]).await.unwrap());
    admission::release(&pool, runs[0].id).await.unwrap();
    assert!(admission::target_right(&pool, &runs[1]).await.unwrap());
}

#[test]
fn ordinary_instructions_cannot_be_deserialised_as_validation_approval() {
    let id = Uuid::new_v4();
    assert!(serde_json::from_value::<super::InterventionRequest>(serde_json::json!({
        "project_id": id, "request_id": id, "prompt": "approve all tests", "decision": "continue"
    })).is_err());
    assert!(
        serde_json::from_value::<super::GateDecisionRequest>(serde_json::json!({
            "project_id": id, "request_id": id, "prompt": "approve all tests"
        }))
        .is_err()
    );
}

#[test]
fn remote_target_selection_rejects_a_branch_changed_before_formal_admission() {
    let directory = tempfile::tempdir().unwrap();
    let command = |args: &[&str]| {
        let output = std::process::Command::new("git")
            .current_dir(directory.path())
            .args(args)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    };
    command(&["init", "-b", "main"]);
    command(&["config", "user.name", "Integration fixture"]);
    command(&["config", "user.email", "fixture@example.invalid"]);
    command(&["commit", "--allow-empty", "-m", "first"]);
    let git = git::GitService::new();
    let selected = admission::observe_target(&git, directory.path(), "main", None).unwrap();
    command(&["commit", "--allow-empty", "-m", "direct target update"]);
    let error =
        admission::observe_target(&git, directory.path(), "main", Some(&selected)).unwrap_err();
    assert!(error.to_string().contains("TARGET_CHANGED"));
    let current = admission::observe_target(&git, directory.path(), "main", None).unwrap();
    assert_ne!(selected, current);
    assert_eq!(
        admission::observe_target(&git, directory.path(), "main", Some(&current)).unwrap(),
        current
    );
    assert!(admission::observe_target(&git, directory.path(), "missing", None).is_err());
}

#[tokio::test]
async fn failed_admission_reconciles_durable_identity_without_replaying_or_leaking_diagnostics() {
    use db::models::integration::{IntegrationSelection, IntegrationSource};
    use executors::{executors::BaseCodingAgent, profile::ExecutorConfig};

    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::migrate!("../db/migrations").run(&pool).await.unwrap();
    let (project_id, card_id) = project(&pool).await;
    let repo = Repo::find_or_create(&pool, Path::new("/reconciliation-fixture"), "repo")
        .await
        .unwrap();
    let selection = IntegrationSelection {
        card_id,
        workspace_id: Uuid::new_v4(),
        expected_commit: "source-head".into(),
    };
    let mut request = super::CreateIntegrationRequest {
        request_id: Uuid::new_v4(),
        project_id,
        repository_id: repo.id,
        target_branch: "main".into(),
        selections: vec![selection.clone()],
        executor_config: None,
        skip_validation: false,
    };
    for (error, expected_code) in [
        (
            anyhow::Error::from(admission::SelectionChanged::Target),
            "target_changed",
        ),
        (
            anyhow::Error::from(admission::SelectionChanged::Source),
            "source_changed",
        ),
        (
            anyhow::anyhow!("private diagnostic /secrets/token-123"),
            "integration_not_admitted",
        ),
    ] {
        let failure = admission::reconcile_failed_admission(&pool, &request, "target-head", &error)
            .await
            .unwrap_err();
        match failure {
            admission::AdmissionFailure::Rejected { code, message } => {
                assert_eq!(code, expected_code);
                assert!(!message.contains("token-123"));
            }
            other => panic!("unexpected {other:?}"),
        }
    }
    let id = Uuid::new_v4();
    let payload = IntegrationPayload {
        observed_target: "target-head".into(),
        executor_config: Some(
            serde_json::to_value(ExecutorConfig::new(BaseCodingAgent::Codex)).unwrap(),
        ),
        sources: vec![IntegrationSource {
            selection,
            branch: "feature".into(),
            commit: "source-head".into(),
            title: "request".into(),
            description: None,
            status_id: Uuid::new_v4(),
            requirements_revision: 1,
            event_ids: vec![],
            related_workspaces: vec![],
            done_result: None,
        }],
        ..Default::default()
    };
    sqlx::query("INSERT INTO integration_runs(id,request_key,project_id,repository_id,storage_identity,target_ref,status,payload) VALUES(?,?,?,?,?,'refs/heads/main','blocked',?)")
        .bind(id).bind(request.request_id.to_string()).bind(project_id).bind(repo.id)
        .bind("/reconciliation-fixture").bind(sqlx::types::Json(payload)).execute(&pool).await.unwrap();
    let lost_ack = anyhow::Error::from(std::io::Error::other("lost acknowledgement"));
    let recovered =
        admission::reconcile_failed_admission(&pool, &request, "target-head", &lost_ack)
            .await
            .unwrap();
    assert_eq!(recovered.id, id);
    assert_eq!(recovered.status, "blocked"); // Admission does not imply successful Integration.
    assert!(matches!(
        admission::reconcile_failed_admission(&pool, &request, "changed-target", &lost_ack).await,
        Err(admission::AdmissionFailure::Uncertain)
    ));
    request.skip_validation = true;
    assert!(matches!(
        admission::reconcile_failed_admission(&pool, &request, "target-head", &lost_ack).await,
        Err(admission::AdmissionFailure::Uncertain)
    ));
    request.skip_validation = false;
    request.selections[0].expected_commit = "different-source".into();
    assert!(matches!(
        admission::reconcile_failed_admission(&pool, &request, "target-head", &lost_ack).await,
        Err(admission::AdmissionFailure::Uncertain)
    ));
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM integration_runs")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1);
    pool.close().await;
    assert!(matches!(
        admission::reconcile_failed_admission(&pool, &request, "target-head", &lost_ack).await,
        Err(admission::AdmissionFailure::Uncertain)
    ));
}

#[test]
fn omitted_validation_choice_preserves_default_and_rejects_non_boolean() {
    let mut value = serde_json::json!({
        "request_id": Uuid::new_v4(), "project_id": Uuid::new_v4(),
        "repository_id": Uuid::new_v4(), "target_branch": "main", "selections": []
    });
    let request: super::CreateIntegrationRequest = serde_json::from_value(value.clone()).unwrap();
    assert!(!request.skip_validation);
    value["skip_validation"] = serde_json::json!(true);
    assert!(
        serde_json::from_value::<super::CreateIntegrationRequest>(value.clone())
            .unwrap()
            .skip_validation
    );
    value["skip_validation"] = serde_json::json!("true");
    assert!(serde_json::from_value::<super::CreateIntegrationRequest>(value).is_err());
}
