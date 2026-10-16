//! Adapter integration fixtures replace domain launch, not Git discovery or
//! database admission. Actual formal Integration regression tests remain separate.
use std::{process::Command, sync::Arc};

use async_trait::async_trait;
use db::models::{
    repo::Repo,
    requests::CreateAndStartWorkspaceRequest,
    session::{CreateSession, Session},
    workspace::{CreateWorkspace, Workspace},
    workspace_repo::{CreateWorkspaceRepo, WorkspaceRepo},
};
use executors::{executors::BaseCodingAgent, profile::ExecutorConfig};
use serde_json::{Value, json};
use sqlx::SqlitePool;
use uuid::Uuid;

use super::{
    Remote,
    auth::Principal,
    launch::{self, LaunchActions},
    runtime_write::Target,
    runtime_write_tests,
    tests::settled_operation,
};
use crate::{error::ApiError, routes::integrations::CreateIntegrationRequest};

struct FixtureLaunch {
    pool: SqlitePool,
    starts: tokio::sync::Mutex<Vec<Value>>,
    integrations: tokio::sync::Mutex<Vec<Value>>,
    pause: bool,
    admission_failure: tokio::sync::Mutex<Option<crate::routes::integrations::AdmissionFailure>>,
    entered: tokio::sync::Notify,
    release: tokio::sync::Notify,
}
#[async_trait]
impl LaunchActions for FixtureLaunch {
    async fn defaults(&self) -> ExecutorConfig {
        let mut config = ExecutorConfig::new(BaseCodingAgent::Codex);
        config.variant = Some("SAVED".into());
        config
    }
    async fn start(
        &self,
        request: CreateAndStartWorkspaceRequest,
        workspace_id: Uuid,
    ) -> Result<Value, ApiError> {
        self.starts
            .lock()
            .await
            .push(serde_json::to_value(&request).unwrap());
        self.entered.notify_one();
        if self.pause {
            self.release.notified().await;
        }
        Workspace::create(
            &self.pool,
            &CreateWorkspace {
                branch: format!("fixture-{workspace_id}"),
                name: request.name,
            },
            workspace_id,
        )
        .await?;
        let issue = request.linked_issue.unwrap();
        sqlx::query(
            "INSERT INTO local_workspace_links(workspace_id,project_id,issue_id) VALUES(?,?,?)",
        )
        .bind(workspace_id)
        .bind(issue.remote_project_id)
        .bind(issue.issue_id)
        .execute(&self.pool)
        .await?;
        let session = Uuid::new_v4();
        let run = Uuid::new_v4();
        Session::create(
            &self.pool,
            &CreateSession {
                executor: Some("CODEX".into()),
                name: None,
            },
            session,
            workspace_id,
        )
        .await?;
        runtime_write_tests::insert_run(&self.pool, session, workspace_id, run).await;
        Ok(
            json!({"workspace_id":workspace_id,"session_id":session,"agent_run_id":run,"execution_accepted":true,"implementation_complete":false}),
        )
    }
    async fn integration(
        &self,
        request: CreateIntegrationRequest,
        expected_target: String,
    ) -> Result<Value, crate::routes::integrations::AdmissionFailure> {
        self.integrations.lock().await.push(json!({"request_id":request.request_id,"project_id":request.project_id,"repository_id":request.repository_id,"target_branch":request.target_branch,"expected_target":expected_target,"selections":request.selections,"executor_config":request.executor_config}));
        if let Some(failure) = self.admission_failure.lock().await.take() {
            return Err(failure);
        }
        Ok(
            json!({"integration_id":Uuid::new_v4(),"status":"queued","integration_accepted":true,"published":false}),
        )
    }
}
fn git(path: &std::path::Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .current_dir(path)
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
async fn fixture(
    pause: bool,
) -> (
    tempfile::TempDir,
    Remote,
    Arc<FixtureLaunch>,
    Principal,
    Target,
    Uuid,
    Uuid,
) {
    let (mut remote, _, mut principal, target, issue) = runtime_write_tests::fixture().await;
    principal.scopes.insert("lvk:merge".into());
    let directory = tempfile::tempdir().unwrap();
    git(directory.path(), &["init", "-b", "main"]);
    git(directory.path(), &["config", "user.name", "MCP Fixture"]);
    git(
        directory.path(),
        &["config", "user.email", "fixture@example.invalid"],
    );
    git(
        directory.path(),
        &["commit", "--allow-empty", "-m", "fixture"],
    );
    git(directory.path(), &["branch", "fixture"]);
    let repo = Repo::find_or_create(&remote.pool, directory.path(), "fixture-repository")
        .await
        .unwrap();
    WorkspaceRepo::create_many(
        &remote.pool,
        target.workspace_id,
        &[CreateWorkspaceRepo {
            repo_id: repo.id,
            target_branch: "main".into(),
        }],
    )
    .await
    .unwrap();
    sqlx::query(
        "UPDATE local_issues SET description='Agreed behavior and acceptance criteria' WHERE id=?",
    )
    .bind(issue)
    .execute(&remote.pool)
    .await
    .unwrap();
    let dispatch = Arc::new(FixtureLaunch {
        pool: remote.pool.clone(),
        starts: Default::default(),
        integrations: Default::default(),
        pause,
        admission_failure: Default::default(),
        entered: Default::default(),
        release: Default::default(),
    });
    remote.launch = Some(dispatch.clone());
    (
        directory, remote, dispatch, principal, target, issue, repo.id,
    )
}
async fn start_args(remote: &Remote, target: &Target, issue: Uuid, repo: Uuid) -> Value {
    let revision: String = sqlx::query_scalar("SELECT updated_at FROM local_issues WHERE id=?")
        .bind(issue)
        .fetch_one(&remote.pool)
        .await
        .unwrap();
    json!({"project_id":target.project_id,"issue_id":issue,"request_id":Uuid::new_v4(),"expected_revision":revision,"repositories":[{"repository_id":repo,"target_branch":"main"}],"execution_mode":"goal","goal_token_budget":1234})
}

#[tokio::test]
async fn launch_uses_agreed_revision_native_goal_and_stable_admission_identity() {
    let (_directory, remote, dispatch, principal, target, issue, repo) = fixture(false).await;
    let input = start_args(&remote, &target, issue, repo).await;
    let first = launch::call(&remote, &principal, "start_workspace", input.clone())
        .await
        .unwrap();
    assert_eq!(first["status"], "succeeded");
    assert_eq!(first["result"]["workspace_id"], first["operation_id"]);
    assert!(first["result"]["session_id"].is_string());
    assert!(first["result"]["agent_run_id"].is_string());
    assert_eq!(first["result"]["implementation_complete"], false);
    let second = launch::call(&remote, &principal, "start_workspace", input.clone())
        .await
        .unwrap();
    assert_eq!(first["operation_id"], second["operation_id"]);
    let starts = dispatch.starts.lock().await;
    assert_eq!(starts.len(), 1);
    assert_eq!(starts[0]["executor_config"]["variant"], "SAVED");
    assert_eq!(starts[0]["executor_config"]["execution_mode"], "goal");
    assert_eq!(starts[0]["executor_config"]["goal_token_budget"], 1234);
    assert!(starts[0]["executor_config"].get("model_id").is_none());
    assert!(
        starts[0]["prompt"]
            .as_str()
            .unwrap()
            .contains("Agreed behavior and acceptance criteria")
    );
    assert!(!starts[0]["prompt"].as_str().unwrap().contains("/goal"));
    drop(starts);
    let mut stale = input.clone();
    stale["request_id"] = json!(Uuid::new_v4());
    stale["expected_revision"] = json!("old");
    assert_eq!(
        launch::call(&remote, &principal, "start_workspace", stale)
            .await
            .unwrap()["status"],
        "failed"
    );
    let mut foreign = input;
    foreign["request_id"] = json!(Uuid::new_v4());
    foreign["repositories"][0]["repository_id"] = json!(Uuid::new_v4());
    assert_eq!(
        launch::call(&remote, &principal, "start_workspace", foreign)
            .await
            .unwrap()["status"],
        "failed"
    );
    assert_eq!(dispatch.starts.lock().await.len(), 1);
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
async fn disconnected_start_waiter_does_not_cancel_domain_admission() {
    let (_directory, remote, dispatch, principal, target, issue, repo) = fixture(true).await;
    let input = start_args(&remote, &target, issue, repo).await;
    let request: Uuid = serde_json::from_value(input["request_id"].clone()).unwrap();
    let caller = {
        let remote = remote.clone();
        let principal = principal.clone();
        let input = input.clone();
        tokio::spawn(
            async move { launch::call(&remote, &principal, "start_workspace", input).await },
        )
    };
    tokio::time::timeout(
        std::time::Duration::from_secs(5),
        dispatch.entered.notified(),
    )
    .await
    .unwrap();
    assert!(
        !caller.is_finished(),
        "initial response waits only for domain admission"
    );
    caller.abort();
    assert!(caller.await.unwrap_err().is_cancelled());
    dispatch.release.notify_one();
    let receipt = settled_operation(&remote, &principal, target.project_id, request).await;
    assert_eq!(receipt["status"], "succeeded");
    let replay = launch::call(&remote, &principal, "start_workspace", input)
        .await
        .unwrap();
    assert_eq!(
        replay["result"]["agent_run_id"],
        receipt["result"]["agent_run_id"]
    );
    assert_eq!(dispatch.starts.lock().await.len(), 1);
}

#[tokio::test]
async fn repository_discovery_and_merge_admission_use_scoped_exact_heads() {
    let (directory, remote, dispatch, principal, target, issue, repo) = fixture(false).await;
    let listed = launch::call(
        &remote,
        &principal,
        "list_project_repositories",
        json!({"project_id":target.project_id}),
    )
    .await
    .unwrap();
    assert_eq!(listed["items"][0]["id"], json!(repo));
    assert_eq!(listed["items"][0]["integration_member"], true);
    assert!(
        !listed
            .to_string()
            .contains(&directory.path().display().to_string())
    );
    for index in 0..103 {
        git(directory.path(), &["branch", &format!("branch-{index:03}")]);
    }
    let mut args = json!({"project_id":target.project_id,"repository_id":repo,"limit":20});
    let mut count = 0;
    loop {
        let page = launch::call(
            &remote,
            &principal,
            "list_repository_branches",
            args.clone(),
        )
        .await
        .unwrap();
        let items = page["items"].as_array().unwrap();
        assert!(items.len() <= 20);
        count += items.len();
        if page["has_more"] == false {
            break;
        }
        args["cursor"] = page["next_cursor"].clone();
    }
    assert_eq!(count, 105);
    args["cursor"] = json!(format!("{}:main", Uuid::new_v4()));
    assert!(
        launch::call(&remote, &principal, "list_repository_branches", args)
            .await
            .is_err()
    );
    let candidate=launch::call(&remote,&principal,"get_integration_candidate",json!({"project_id":target.project_id,"repository_id":repo,"workspace_id":target.workspace_id,"target_branch":"main"})).await.unwrap();
    let head = git(directory.path(), &["rev-parse", "main"]);
    assert_eq!(candidate["source_commit"], head);
    assert_eq!(candidate["eligibility"], "not_validated");
    let input = json!({"project_id":target.project_id,"request_id":Uuid::new_v4(),"repository_id":repo,"target_branch":"main","expected_target_commit":candidate["target_commit"],"selections":[{"issue_id":issue,"workspace_id":target.workspace_id,"expected_commit":candidate["source_commit"]}]});
    let mut no_merge = principal.clone();
    no_merge.scopes.remove("lvk:merge");
    assert!(
        launch::call(&remote, &no_merge, "start_integration", input.clone())
            .await
            .is_err()
    );
    let receipt = launch::call(&remote, &principal, "start_integration", input.clone())
        .await
        .unwrap();
    assert_eq!(receipt["status"], "succeeded");
    assert_eq!(receipt["result"]["published"], false);
    launch::call(&remote, &principal, "start_integration", input.clone())
        .await
        .unwrap();
    let requests = dispatch.integrations.lock().await;
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0]["expected_target"], candidate["target_commit"]);
    assert_eq!(
        requests[0]["selections"][0]["expected_commit"],
        candidate["source_commit"]
    );
    assert!(requests[0]["executor_config"].is_null());
    drop(requests);
    let mut bad = input;
    bad["request_id"] = json!(Uuid::new_v4());
    bad["selections"][0]["issue_id"] = json!(Uuid::new_v4());
    assert_eq!(
        launch::call(&remote, &principal, "start_integration", bad)
            .await
            .unwrap()["status"],
        "failed"
    );
    assert_eq!(dispatch.integrations.lock().await.len(), 1);
}

#[tokio::test]
async fn integration_failure_receipts_preserve_rejection_or_uncertainty_on_retry() {
    use crate::routes::integrations::AdmissionFailure;
    let (directory, remote, dispatch, principal, target, issue, repo) = fixture(false).await;
    let head = git(directory.path(), &["rev-parse", "main"]);
    for (failure, status) in [
        (
            AdmissionFailure::Rejected {
                code: "target_changed",
                message: "Target changed",
            },
            "failed",
        ),
        (AdmissionFailure::Uncertain, "uncertain"),
    ] {
        *dispatch.admission_failure.lock().await = Some(failure);
        let input = json!({"project_id":target.project_id,"request_id":Uuid::new_v4(),"repository_id":repo,"target_branch":"main","expected_target_commit":head,"selections":[{"issue_id":issue,"workspace_id":target.workspace_id,"expected_commit":head}]});
        let first = launch::call(&remote, &principal, "start_integration", input.clone())
            .await
            .unwrap();
        assert_eq!(first["status"], status);
        if status == "failed" {
            assert_eq!(first["result"]["integration_accepted"], false);
            assert_eq!(first["result"]["code"], "target_changed");
        }
        let repeated = launch::call(&remote, &principal, "start_integration", input)
            .await
            .unwrap();
        assert_eq!(first, repeated);
    }
    assert_eq!(dispatch.integrations.lock().await.len(), 2);
}
