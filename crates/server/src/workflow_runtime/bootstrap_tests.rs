//! No-model tests of the real Workflow planner, repository run reservation,
//! fresh Session bindings, branch skip semantics and existing Git publication.
use std::{
    path::Path,
    sync::atomic::{AtomicUsize, Ordering},
};

use db::models::repository_state::RepositoryMemoryStore;
use services::services::openwiki::{
    self,
    bootstrap::{
        CoverageReview,
        reports::{self, ReportIdentity, ReportReference},
    },
    inventory::{DocumentInventory, InventoryIdentity},
};
use utils::repository_memory::BootstrapReportKind;

use super::*;

fn git(root: &Path, args: &[&str]) -> String {
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
    String::from_utf8(output.stdout).unwrap().trim().into()
}

struct Fixture {
    _temp: tempfile::TempDir,
    pool: SqlitePool,
    root: PathBuf,
    maintenance: PathBuf,
    source: String,
    store: RepositoryMemoryStore,
    repository_id: Uuid,
    workspace_id: Uuid,
}

impl Fixture {
    async fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("repo");
        std::fs::create_dir(&root).unwrap();
        git(&root, &["init", "-b", "main"]);
        git(&root, &["config", "user.name", "LVK fixture"]);
        git(&root, &["config", "user.email", "fixture@example.invalid"]);
        std::fs::write(root.join("source.rs"), "authoritative source").unwrap();
        git(&root, &["add", "."]);
        git(&root, &["commit", "-m", "integrated source"]);
        let source = git(&root, &["rev-parse", "HEAD"]);
        let maintenance = temp.path().join("maintenance/repo");
        git(
            &root,
            &[
                "worktree",
                "add",
                "-b",
                "wiki",
                maintenance.to_str().unwrap(),
            ],
        );
        let persistent = temp.path().join("persistent");
        std::fs::create_dir(&persistent).unwrap();
        let files =
            utils::repository_memory::RepositoryMemoryStore::at_artifacts(&persistent).unwrap();
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!("../db/migrations").run(&pool).await.unwrap();
        let repository_id = Uuid::new_v4();
        let store = RepositoryMemoryStore::with_artifacts(&pool, repository_id, files);
        let workspace_id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO repos (id, path, name, display_name) VALUES (?, ?, 'repo', 'Fixture')",
        )
        .bind(repository_id)
        .bind(root.to_str())
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query("INSERT INTO workspaces (id, branch, container_ref, usage, execution_owner) VALUES (?, 'wiki', ?, 'execution_only', ?)")
            .bind(workspace_id)
            // This fixture imports an already-owned checkout. Publish its root
            // only after its membership/placement has been established.
            .bind(Option::<String>::None)
            .bind(sqlx::types::Json(db::models::workspace_usage::WorkspaceExecutionOwner::new(
                db::models::workspace_usage::OPENWIKI_BOOTSTRAP, repository_id, None,
            )))
            .execute(&pool)
            .await
            .unwrap();
        WorkspaceRepo::create_many(
            &pool,
            workspace_id,
            &[CreateWorkspaceRepo {
                repo_id: repository_id,
                target_branch: "main".into(),
            }],
        )
        .await
        .unwrap();
        db::models::workspace::Workspace::update_container_ref(
            &pool,
            workspace_id,
            maintenance.parent().unwrap().to_str().unwrap(),
        )
        .await
        .unwrap();
        Self {
            _temp: temp,
            pool,
            root,
            maintenance,
            source,
            store,
            repository_id,
            workspace_id,
        }
    }

    async fn link_generated_issue(&self) {
        let project = Uuid::new_v4();
        sqlx::query("INSERT INTO projects(id,name) VALUES(?,'Bootstrap status fixture')")
            .bind(project)
            .execute(&self.pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO project_repos(id,project_id,repo_id) VALUES(?,?,?)")
            .bind(Uuid::new_v4())
            .bind(project)
            .bind(self.repository_id)
            .execute(&self.pool)
            .await
            .unwrap();
        for name in ["In Progress", "In Review", "Done"] {
            sqlx::query("INSERT INTO local_project_statuses(id,project_id,name,color,sort_order) VALUES(?,?,?,'blue',0)")
                .bind(Uuid::new_v4()).bind(project).bind(name).execute(&self.pool).await.unwrap();
        }
        let workspace = Workspace::find_by_id(&self.pool, self.workspace_id)
            .await
            .unwrap()
            .unwrap();
        services::services::workspace_usage::ensure_wiki_board_link(&self.pool, &workspace)
            .await
            .unwrap();
    }

    async fn assert_issue_status(&self, expected: &str) {
        let status: String = sqlx::query_scalar("SELECT s.name FROM local_workspace_links l JOIN local_issues i ON i.id=l.issue_id JOIN local_project_statuses s ON s.id=i.status_id WHERE l.workspace_id=? AND i.archived=0")
            .bind(self.workspace_id).fetch_one(&self.pool).await.unwrap();
        assert_eq!(status, expected);
    }

    async fn reserve(&self) -> Uuid {
        let id = Uuid::new_v4();
        Workspace::bind_execution_owner(
            &self.pool,
            self.workspace_id,
            db::models::workspace_usage::OPENWIKI_BOOTSTRAP,
            id,
        )
        .await
        .unwrap();
        let inventory = DocumentInventory::generate(
            &git::GitService::new(),
            &self.maintenance,
            &self.store,
            InventoryIdentity::new(
                self.repository_id,
                self.workspace_id,
                id,
                self.source.clone(),
            ),
        )
        .unwrap();
        let mut graph = workflow::templates::openwiki_bootstrap().graph;
        for node in &mut graph.nodes {
            if matches!(node.id.as_str(), "review" | "re-review") {
                node.data.prompt_template = Some(
                    openwiki::bootstrap::review_prompt(&self.maintenance, "ja")
                        + &inventory.prompt(&self.store, true),
                );
            } else if node.id == "generate" {
                node.data.prompt_template = Some(
                    openwiki::bootstrap::writer_prompt(&self.maintenance, "ja", None)
                        + &inventory.prompt(&self.store, false),
                );
            }
        }
        reserve_repository_workflow(
            &self.pool,
            id,
            self.repository_id,
            self.workspace_id,
            graph,
            &serde_json::to_string(&inventory).unwrap(),
        )
        .await
        .unwrap();
        id
    }

    async fn inventory(&self, run_id: Uuid) -> Result<DocumentInventory, ApiError> {
        super::super::bootstrap::load_inventory_input(
            &self.pool,
            &self.store,
            InventoryIdentity::new(
                self.repository_id,
                self.workspace_id,
                run_id,
                self.source.clone(),
            ),
        )
        .await
        .map_err(orchestration_api_error)?
        .ok_or_else(|| ApiError::BadRequest("inventory missing".into()))
    }
}

struct FakeBootstrap<'a> {
    fixture: &'a Fixture,
    refine: bool,
    refute: bool,
    fail: Option<&'static str>,
    calls: Mutex<Vec<AgentNodeRequest>>,
    publications: AtomicUsize,
}

#[async_trait]
impl WorkflowAgentExecutor for FakeBootstrap<'_> {
    fn owns_repository_execution(&self) -> bool {
        true
    }

    async fn run_agent(&self, request: AgentNodeRequest) -> Result<AgentNodeExecution, ApiError> {
        let fixture = self.fixture;
        let inventory = fixture.inventory(request.run_id).await?;
        if matches!(request.node_id.as_str(), "generate" | "review") {
            assert_eq!(request.prompt.matches("Knowledge organisation:").count(), 1);
            assert!(request.prompt.contains("canonical explanation home"));
            assert_eq!(
                request.prompt.contains("fictional job-processing product"),
                request.node_id == "generate",
            );
            assert!(
                request
                    .prompt
                    .contains(&inventory.prompt(&fixture.store, request.node_id == "review"))
            );
            assert!(
                !request.prompt.contains("\"digest\":"),
                "host input is not workflow context"
            );
        }
        assert_eq!(
            git(&fixture.maintenance, &["rev-parse", "HEAD"]),
            fixture.source
        );
        assert_eq!(git(&fixture.root, &["rev-parse", "main"]), fixture.source);
        assert_eq!(self.publications.load(Ordering::SeqCst), 0);
        if self.fail == Some(request.node_id.as_str()) {
            return Err(ApiError::BadRequest("fixture phase failure".into()));
        }
        let session_id = request.session_id.unwrap();
        let agent_run_id = Uuid::new_v4();
        let identity = |phase| ReportIdentity {
            source: InventoryIdentity::new(
                fixture.repository_id,
                fixture.workspace_id,
                request.run_id,
                fixture.source.clone(),
            ),
            phase,
            session_id,
            agent_run_id,
        };
        let output = match request.node_id.as_str() {
            "generate" => {
                std::fs::create_dir_all(fixture.maintenance.join("openwiki")).unwrap();
                std::fs::write(
                    fixture.maintenance.join("openwiki/INSTRUCTIONS.md"),
                    include_bytes!("../../../../assets/openwiki-instructions.md"),
                )
                .unwrap();
                std::fs::write(
                    fixture.maintenance.join("openwiki/index.md"),
                    "Generated Wiki",
                )
                .unwrap();
                if self.fail == Some("retention") {
                    std::fs::rename(
                        fixture.maintenance.join("openwiki/moved.md"),
                        fixture.maintenance.join("openwiki/destination.md"),
                    )
                    .unwrap();
                    std::fs::write(
                        fixture.maintenance.join("openwiki/workspace.md"),
                        "DirectFolder is an existing folder.",
                    )
                    .unwrap();
                }
                "SENSITIVE_GENERATOR_CONVERSATION".into()
            }
            "review" if self.fail == Some("missing_review") => {
                json!({"review_missing":true}).to_string()
            }
            "re-review" if self.fail == Some("missing_re_review") => {
                json!({"review_missing":true}).to_string()
            }
            "review" | "re-review" => {
                assert!(!request.prompt.contains("SENSITIVE_GENERATOR_CONVERSATION"));
                assert!(!request.prompt.contains("Direct Upstream Handoff"));
                assert!(
                    request
                        .prompt
                        .contains("unanswered question, inspected evidence")
                );
                assert!(request.prompt.contains(
                    "Missing standalone pages or preferred directory names alone are not material"
                ));
                assert!(request.selected_skills.is_none());
                let findings: Vec<_> = (0..20).map(|index| json!({
                    "severity": if self.refine {"material"} else {"minor"},
                    "title":format!("Coverage gap {index}"),"description":"契約と実装を確認し、知識の不足を具体的に記録。".repeat(40),
                    "evidencePaths":["source.rs"],"recommendedAction":"expand_page"
                })).collect();
                let raw = if self.fail == Some("incomplete") {
                    json!({"version":1,"verdict":"incomplete", "summary":"根拠の確認が一部未完了",
                        "findings": if self.refine {findings} else {vec![]},
                        "checks":[{"path":"openwiki/index.md","conclusion":"現行の根拠と一致", "evidencePaths":["source.rs"]}],
                        "unverified":[{"subject":format!("{}: old/new procedure",request.node_id),"reason":"旧版の残りの手順は未照合"}]}).to_string()
                } else if self.fail == Some("retention") {
                    let parent = tempfile::tempdir().unwrap();
                    let copy = parent.path().join("review");
                    openwiki::bootstrap::copy_review_candidate(
                        &git::GitService::new(),
                        &fixture.maintenance,
                        &copy,
                        &fixture.source,
                    )
                    .unwrap();
                    let focus = openwiki::bootstrap::review_reductions(
                        &git::GitService::new(),
                        &fixture.maintenance,
                        &fixture.source,
                        &copy,
                    )
                    .unwrap();
                    assert_eq!(focus["pages"].as_array().unwrap().len(), 2);
                    assert_eq!(
                        std::fs::read_to_string(copy.join("openwiki/destination.md")).unwrap(),
                        "The procedure survives at its new home."
                    );
                    // Script the semantic verdict; this test exercises the real
                    // report/planner/publication pipeline, not model judgement.
                    json!({"version":1,"verdict":"needs_refinement",
                        "summary":"openwiki/moved.md: moved to openwiki/destination.md, opening paragraph; no knowledge lost.",
                        "findings":[{"severity":"material","title":"DirectFolder auto-merge boundary missing",
                            "description":"openwiki/workspace.md lost the still-current constraint: DirectFolder is excluded from automatic merge. source.rs still rejects it. Restore that explicit constraint in the workspace explanation.",
                            "evidencePaths":["openwiki/workspace.md","source.rs"],"recommendedAction":"expand_page"}]}).to_string()
                } else {
                    let raw = json!({"version":1,"verdict":if self.refine {"needs_refinement"} else {"pass"},
                        "summary":"All findings preserved","findings":findings}).to_string();
                    assert!(raw.len() > 12_000);
                    raw
                };
                let review = CoverageReview::parse(&raw).unwrap();
                let reference = reports::save_review(
                    &fixture.store,
                    identity(if request.node_id == "review" {
                        BootstrapReportKind::Review
                    } else {
                        BootstrapReportKind::ReReview
                    }),
                    &review,
                )
                .unwrap();
                if self.fail == Some("tamper_review_report") {
                    std::fs::write(reference.prompt_path(&fixture.store), "{}").unwrap();
                }
                serde_json::to_string(&reference).unwrap()
            }
            "refine" => {
                assert!(self.refine);
                let (review, reference) = crate::workflow_runtime::bootstrap::load_review_phase(
                    &fixture.pool,
                    &fixture.store,
                    identity(BootstrapReportKind::Review).source,
                )
                .await
                .map_err(orchestration_api_error)?;
                let prompt = openwiki::bootstrap::writer_prompt(
                    &fixture.maintenance,
                    "ja",
                    Some(&reference.prompt_path(&fixture.store)),
                );
                assert!(prompt.contains("force=true"));
                assert_eq!(prompt.matches("Knowledge organisation:").count(), 1);
                assert!(prompt.contains("restructuring is not required"));
                assert!(!prompt.contains("fictional job-processing product"));
                // Full host instructions use developer context, independently
                // of the bounded Goal objective and Workflow result handoff.
                assert!(!prompt.contains("Coverage gap 19"));
                if !self.refute {
                    std::fs::write(
                        fixture.maintenance.join("openwiki/index.md"),
                        "Refined Wiki",
                    )
                    .unwrap();
                }
                let resolutions: Vec<_> = (0..review.findings.len()).map(|index| json!({
                    "findingIndex":index,"disposition":if self.refute {"refuted"} else {"fixed"},
                    "reason":"現在の実装を検証し、根拠とともに処置を記録。".repeat(40),"wikiPaths":["openwiki/index.md"],"evidencePaths":["source.rs"]
                })).collect();
                let report = json!({"version":1,"summary":"Checked every finding","resolutions":resolutions}).to_string();
                if self.fail != Some("retention") {
                    assert!(report.len() > 12_000);
                }
                let report =
                    openwiki::bootstrap::RefinementReport::parse(&report, &review).unwrap();
                report.validate_files(&fixture.maintenance).unwrap();
                let reference = reports::save_refinement(
                    &fixture.store,
                    identity(BootstrapReportKind::Refine),
                    &report,
                    &review,
                )
                .unwrap();
                serde_json::to_string(&reference).unwrap()
            }
            _ => panic!("unexpected agent node"),
        };
        if self.fail == Some(format!("tamper_{}", request.node_id).as_str()) {
            std::fs::write(
                fixture.store.document_inventory_path(request.run_id, None),
                "{}",
            )
            .unwrap();
        }
        fixture.inventory(request.run_id).await?;
        // The fake host supplies durable identities; real host protocol and
        // sandbox behaviour have separate adapter/Native Audit tests.
        sqlx::query("INSERT INTO agent_runs (id, session_id, workspace_id, request_id, idempotency_key, correlation_id, schema_version, payload_version, runtime_profile_id, provider_id, workspace_mode, workspace_path, request_envelope) VALUES (?, ?, ?, ?, ?, ?, 1, 1, 'codex:default', 'codex', 'isolated_worktree', ?, '{}')")
            .bind(agent_run_id).bind(session_id).bind(fixture.workspace_id).bind(Uuid::new_v4()).bind(agent_run_id.to_string()).bind(request.run_id).bind(fixture.maintenance.to_str()).execute(&fixture.pool).await?;
        let node_id = request.orchestration_node_execution_id;
        self.calls.lock().unwrap().push(request);
        Ok(AgentNodeExecution::Completed {
            session_id,
            orchestration_node_execution_id: node_id,
            agent_run_id,
            output_text: output,
        })
    }

    async fn coverage_review(
        &self,
        run: Uuid,
        _workspace: Uuid,
        _raw: &str,
    ) -> Result<Option<openwiki::bootstrap::CoverageVerdict>, ApiError> {
        Ok(crate::workflow_runtime::bootstrap::load_review_phase(
            &self.fixture.pool,
            &self.fixture.store,
            InventoryIdentity::new(
                self.fixture.repository_id,
                self.fixture.workspace_id,
                run,
                self.fixture.source.clone(),
            ),
        )
        .await
        .ok()
        .map(|(review, _)| review.routing_verdict()))
    }
    async fn publish_repository(&self, run_id: Uuid) -> Result<Option<String>, ApiError> {
        if self.fail == Some("publish") {
            return Err(ApiError::BadRequest("fixture publication failure".into()));
        }
        let f = self.fixture;
        if self.fail == Some("tamper_publish") {
            std::fs::write(f.store.document_inventory_path(run_id, None), "{}").unwrap();
        }
        f.inventory(run_id).await?;
        if matches!(
            self.fail,
            Some("tamper_pass_report" | "tamper_refine_report")
        ) {
            let phase = if self.fail == Some("tamper_pass_report") {
                BootstrapReportKind::Review
            } else {
                BootstrapReportKind::Refine
            };
            std::fs::write(f.store.bootstrap_report_path(run_id, phase), "{}").unwrap();
        }
        let warnings = crate::workflow_runtime::bootstrap::publication_warnings(
            &f.pool,
            &f.store,
            InventoryIdentity::new(f.repository_id, f.workspace_id, run_id, f.source.clone()),
        )
        .await
        .map_err(orchestration_api_error)?;
        let result = openwiki::publish_reviewed_wiki(
            &git::GitService::new(),
            &f.store,
            &openwiki::WikiPublicationRequest {
                repository_root: &f.root,
                maintenance_root: &f.maintenance,
                maintenance_branch: "wiki",
                target_branch: "main",
                source_commit: &f.source,
                run_id,
            },
            &warnings,
        )
        .await
        .map_err(orchestration_api_error)?;
        assert!(
            !result.1,
            "New Wiki must not publish as a no-op: status={}, paths={:?}",
            git(&f.maintenance, &["status", "--short"]),
            openwiki::publication_paths(&git::GitService::new(), &f.maintenance, &f.source)
        );
        self.publications.fetch_add(1, Ordering::SeqCst);
        Ok(Some("Published".into()))
    }
}

#[tokio::test]
async fn execution_workspace_sessions_require_bound_repository_owner_not_public_adoption() {
    let f = Fixture::new().await;
    let id = Uuid::new_v4();
    let mut graph = workflow::templates::openwiki_bootstrap().graph;
    assert!(
        crate::routes::workflows::ensure_agent_node_sessions(&f.pool, f.workspace_id, &mut graph,)
            .await
            .is_err()
    );
    assert!(
        crate::routes::workflows::ensure_repository_owner_sessions(
            &f.pool,
            f.workspace_id,
            f.repository_id,
            id,
            &mut graph,
        )
        .await
        .is_err()
    );
    Workspace::bind_execution_owner(
        &f.pool,
        f.workspace_id,
        db::models::workspace_usage::OPENWIKI_BOOTSTRAP,
        id,
    )
    .await
    .unwrap();
    for (repo, run) in [(Uuid::new_v4(), id), (f.repository_id, Uuid::new_v4())] {
        assert!(
            crate::routes::workflows::ensure_repository_owner_sessions(
                &f.pool,
                f.workspace_id,
                repo,
                run,
                &mut graph,
            )
            .await
            .is_err()
        );
    }
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM sessions")
            .fetch_one(&f.pool)
            .await
            .unwrap(),
        0
    );
    crate::routes::workflows::ensure_repository_owner_sessions(
        &f.pool,
        f.workspace_id,
        f.repository_id,
        id,
        &mut graph,
    )
    .await
    .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM sessions")
            .fetch_one(&f.pool)
            .await
            .unwrap(),
        4
    );
    assert!(
        Workspace::find_by_id(&f.pool, f.workspace_id)
            .await
            .unwrap()
            .unwrap()
            .is_execution_only()
    );
}

#[tokio::test]
async fn inventory_preprocessing_failure_before_reservation_releases_owner() {
    use utils::repository_memory::{
        OpenWikiBootstrapOwner, RepositoryMemoryState, RepositoryWikiStatus,
    };
    let f = Fixture::new().await;
    let run_id = Uuid::new_v4();
    // The start path has persisted ownership, but no Workflow row/child exists.
    Workspace::bind_execution_owner(
        &f.pool,
        f.workspace_id,
        db::models::workspace_usage::OPENWIKI_BOOTSTRAP,
        run_id,
    )
    .await
    .unwrap();
    std::fs::write(f.maintenance.join(".openwikiignore"), "docs/**").unwrap();
    let error = DocumentInventory::generate(
        &git::GitService::new(),
        &f.maintenance,
        &f.store,
        InventoryIdentity::new(f.repository_id, f.workspace_id, run_id, f.source.clone()),
    )
    .unwrap_err();
    let state = RepositoryMemoryState {
        enabled: true,
        status: RepositoryWikiStatus::Error,
        maintenance_workspace_id: Some(f.workspace_id),
        active_source_commit: Some(f.source.clone()),
        error: Some(error.to_string()),
        bootstrap: Some(OpenWikiBootstrapOwner {
            workflow_run_id: run_id,
        }),
        ..Default::default()
    };
    f.store.save_state(&state).await.unwrap();
    let repo = db::models::repo::Repo::find_by_id(&f.pool, f.repository_id)
        .await
        .unwrap()
        .unwrap();
    let service = OrchestrationService::new(f.pool.clone(), Arc::new(NoopAgentRunPort));
    super::super::bootstrap::cleanup_with_service(&f.pool, &repo, &f.store, &service)
        .await
        .unwrap();
    let state = f.store.state().await.unwrap();
    assert!(
        state.bootstrap.is_none()
            && state.active_run_id.is_none()
            && state.active_source_commit.is_none()
    );
    assert_eq!(state.status, RepositoryWikiStatus::Error);
    assert!(state.error.unwrap().contains("ignore policy"));
    let runs: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM workflow_runs")
        .fetch_one(&f.pool)
        .await
        .unwrap();
    assert_eq!(runs, 0);
    assert!(f.store.publication(run_id).await.unwrap().is_none());
    assert_eq!(git(&f.root, &["rev-parse", "main"]), f.source);
    assert!(f.store.try_lock().is_ok());
}

#[tokio::test]
async fn bootstrap_cleanup_restores_and_releases_only_after_all_children_exit() {
    check_bootstrap_cleanup_result(false).await;
    check_bootstrap_cleanup_result(true).await;
}

async fn check_bootstrap_cleanup_result(cancelled: bool) {
    use utils::repository_memory::{
        OpenWikiBootstrapOwner, RepositoryMemoryState, RepositoryWikiStatus,
    };
    let f = Fixture::new().await;
    let run_id = f.reserve().await;
    if cancelled {
        update_run_status(
            &f.pool,
            run_id,
            WorkflowRunStatus::Cancelling,
            None,
            None,
            false,
        )
        .await
        .unwrap();
    }
    let repo = db::models::repo::Repo::find_by_id(&f.pool, f.repository_id)
        .await
        .unwrap()
        .unwrap();
    let service = OrchestrationService::new(f.pool.clone(), Arc::new(NoopAgentRunPort));
    let state = RepositoryMemoryState {
        enabled: true,
        status: RepositoryWikiStatus::Initializing,
        maintenance_workspace_id: Some(f.workspace_id),
        active_source_commit: Some(f.source.clone()),
        bootstrap: Some(OpenWikiBootstrapOwner {
            workflow_run_id: run_id,
        }),
        error: Some("fixture failure".into()),
        ..Default::default()
    };
    f.store.save_state(&state).await.unwrap();
    openwiki::setup::prepare(&f.store, f.workspace_id, &f.maintenance, &f.source).unwrap();
    // An unlinked host is still sufficient to prevent instruction restoration.
    let session: Uuid =
        sqlx::query_scalar("SELECT id FROM sessions WHERE workspace_id = ? LIMIT 1")
            .bind(f.workspace_id)
            .fetch_one(&f.pool)
            .await
            .unwrap();
    let (request, attempt_request) = dispatch_envelopes(&f, session, run_id);
    let attempt = attempt_request.run_attempt_id;
    db::models::agent_runtime::AgentRunRecord::persist_identity_before_launch(
        &f.pool,
        &request,
        &attempt_request,
    )
    .await
    .unwrap();
    sqlx::query("UPDATE agent_runs SET status = 'running'")
        .execute(&f.pool)
        .await
        .unwrap();
    sqlx::query("UPDATE agent_run_attempts SET status = 'failed'")
        .execute(&f.pool)
        .await
        .unwrap();
    sqlx::query("UPDATE agent_process_registry SET registry_status = 'running', pid = 456, process_started_at = ? WHERE run_attempt_id = ?")
        .bind(Utc::now()).bind(attempt).execute(&f.pool).await.unwrap();
    assert!(
        super::super::bootstrap::cleanup_with_service(&f.pool, &repo, &f.store, &service)
            .await
            .is_err()
    );
    assert!(f.store.state().await.unwrap().bootstrap.is_some());
    assert_eq!(
        f.store.wiki_setup(f.workspace_id).unwrap().unwrap().phase,
        utils::repository_memory::WikiSetupPhase::Prepared
    );
    sqlx::query("UPDATE agent_runs SET status = 'failed'")
        .execute(&f.pool)
        .await
        .unwrap();
    super::super::bootstrap::cleanup_with_service(&f.pool, &repo, &f.store, &service)
        .await
        .unwrap();
    assert!(
        f.store.state().await.unwrap().bootstrap.is_some(),
        "Terminal projection alone must not release ownership"
    );
    assert_eq!(
        f.store.wiki_setup(f.workspace_id).unwrap().unwrap().phase,
        utils::repository_memory::WikiSetupPhase::Prepared
    );
    sqlx::query(
        "UPDATE agent_process_registry SET registry_status = 'exited', observed_exited_at = ?",
    )
    .bind(chrono::Utc::now())
    .execute(&f.pool)
    .await
    .unwrap();
    super::super::bootstrap::cleanup_with_service(&f.pool, &repo, &f.store, &service)
        .await
        .unwrap();
    let result = f.store.state().await.unwrap();
    assert!(result.bootstrap.is_none() && result.active_run_id.is_none());
    assert_eq!(result.status, RepositoryWikiStatus::Error);
    assert_eq!(
        f.store.wiki_setup(f.workspace_id).unwrap().unwrap().phase,
        utils::repository_memory::WikiSetupPhase::Restored
    );
    assert_eq!(git(&f.maintenance, &["rev-parse", "HEAD"]), f.source);
    assert_eq!(git(&f.root, &["rev-parse", "main"]), f.source);
    assert_eq!(
        get_workflow_run_response(&f.pool, run_id)
            .await
            .unwrap()
            .status,
        if cancelled {
            WorkflowRunStatus::Canceled
        } else {
            WorkflowRunStatus::Failed
        }
    );
    assert!(f.store.publication(run_id).await.unwrap().is_none());
}

#[tokio::test]
async fn bootstrap_validation_gate_survives_generic_recovery_and_does_not_block_cancel() {
    use executors::runtime::{
        AgentEventStream, AgentRunPort, AgentRunPortError, AgentRunPortSnapshot,
        OrchestrationRunStatus, RunState,
    };
    struct CompletedPort {
        pool: SqlitePool,
    }
    #[async_trait]
    impl AgentRunPort for CompletedPort {
        async fn create(
            &self,
            _: AgentRunRequestEnvelope,
            _: RunAttemptRequest,
        ) -> Result<Uuid, AgentRunPortError> {
            panic!("recovery must not create a paid child")
        }
        async fn control(&self, _: AgentRunPortCommandEnvelope) -> Result<(), AgentRunPortError> {
            Ok(())
        }
        async fn subscribe(&self, id: Uuid) -> Result<AgentEventStream, AgentRunPortError> {
            Err(AgentRunPortError::NotFound(id))
        }
        async fn query(&self, id: Uuid) -> Result<AgentRunPortSnapshot, AgentRunPortError> {
            let (session_id, workspace_id): (Uuid, Uuid) =
                sqlx::query_as("SELECT session_id, workspace_id FROM agent_runs WHERE id = ?")
                    .bind(id)
                    .fetch_one(&self.pool)
                    .await
                    .unwrap();
            let mut state = RunState::pending(&AgentRunRequestEnvelope {
                schema_version: AGENT_REQUEST_SCHEMA_VERSION,
                payload_version: AGENT_REQUEST_PAYLOAD_VERSION,
                request_id: id,
                idempotency_key: id.to_string(),
                session_id,
                agent_run_id: id,
                turn_id: id,
                correlation_id: id,
                intent: AgentRunIntent::Initial,
                runtime_profile_id: "codex:default".into(),
                provider_id: "codex".into(),
                workspace: WorkspaceReference {
                    workspace_id,
                    mode: WorkspaceMode::IsolatedWorktree,
                    path: ".".into(),
                },
                input: CanonicalMessage {
                    message_id: id,
                    role: AgentRuntimeMessageRole::User,
                    content: "fixture".into(),
                },
                created_at: Utc::now(),
            });
            state.status = AgentRunStatus::Succeeded;
            Ok(AgentRunPortSnapshot {
                agent_run_id: id,
                state,
            })
        }
    }
    for gated in [true, false] {
        let f = Fixture::new().await;
        let run_id = f.reserve().await;
        let executor = FakeBootstrap {
            fixture: &f,
            refine: false,
            refute: false,
            fail: None,
            calls: Mutex::new(Vec::new()),
            publications: AtomicUsize::new(0),
        };
        drive_repository_workflow(&f.pool, run_id, &executor)
            .await
            .unwrap();
        let run = get_workflow_run_response(&f.pool, run_id).await.unwrap();
        let orchestration_id = run.orchestration_run_id.unwrap();
        if !gated {
            let mut plan: sqlx::types::Json<executors::runtime::OrchestrationPlanSnapshot> =
                sqlx::query_scalar("SELECT plan_snapshot FROM orchestration_runs WHERE id = ?")
                    .bind(orchestration_id)
                    .fetch_one(&f.pool)
                    .await
                    .unwrap();
            for node in &mut plan.nodes {
                node.requires_product_validation = false;
            }
            sqlx::query("UPDATE orchestration_runs SET plan_snapshot = ? WHERE id = ?")
                .bind(plan)
                .bind(orchestration_id)
                .execute(&f.pool)
                .await
                .unwrap();
        }
        for node in run.nodes.iter().filter(|node| node.agent_run_id.is_some()) {
            db::models::orchestration::OrchestrationAgentRunLinkRecord::persist(
                &f.pool,
                Uuid::new_v4(),
                orchestration_id,
                node.orchestration_node_execution_id.unwrap(),
                node.agent_run_id.unwrap(),
                &node.node_id,
            )
            .await
            .unwrap();
        }
        let service = OrchestrationService::new(
            f.pool.clone(),
            Arc::new(CompletedPort {
                pool: f.pool.clone(),
            }),
        );
        service
            .reconcile_run(orchestration_id, orchestration_id)
            .await
            .unwrap();
        let successful: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM orchestration_node_executions WHERE orchestration_run_id = ? AND status = 'succeeded'").bind(orchestration_id).fetch_one(&f.pool).await.unwrap();
        assert_eq!(successful, if gated { 0 } else { 2 });
        service
            .cancel(orchestration_id, orchestration_id)
            .await
            .unwrap();
        service
            .reconcile_run(orchestration_id, orchestration_id)
            .await
            .unwrap();
        let status: OrchestrationRunStatus =
            sqlx::query_scalar("SELECT status FROM orchestration_runs WHERE id = ?")
                .bind(orchestration_id)
                .fetch_one(&f.pool)
                .await
                .unwrap();
        assert_eq!(status, OrchestrationRunStatus::Cancelled);
    }
}

#[tokio::test]
async fn bootstrap_scope_migration_preserves_existing_issue_runs_and_references() {
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    let mut before = sqlx::migrate!("../db/migrations");
    before.migrations = std::borrow::Cow::Owned(
        before
            .iter()
            .filter(|migration| migration.version < 20260913120000)
            .cloned()
            .collect(),
    );
    before.run(&pool).await.unwrap();
    sqlx::raw_sql("INSERT INTO projects (id, name) VALUES (X'00000000000000000000000000000001', 'Old project');
        INSERT INTO local_project_statuses (id, project_id, name, color, sort_order) VALUES (X'00000000000000000000000000000002', X'00000000000000000000000000000001', 'Todo', 'blue', 1);
        INSERT INTO local_issues (id, project_id, issue_number, simple_id, status_id, title, sort_order) VALUES (X'00000000000000000000000000000003', X'00000000000000000000000000000001', 1, 'OLD-1', X'00000000000000000000000000000002', 'Existing Issue', 1);
        INSERT INTO workflows (id, source, name, graph_json) VALUES (X'00000000000000000000000000000004', 'system', 'Old template', '{}');
        INSERT INTO workflow_attempts (id, project_id, issue_id, workflow_id, name) VALUES (X'00000000000000000000000000000005', X'00000000000000000000000000000001', X'00000000000000000000000000000003', X'00000000000000000000000000000004', 'Old attempt');
        INSERT INTO workflow_runs (id, workflow_id, issue_id, input_text, graph_snapshot, attempt_id) VALUES (X'00000000000000000000000000000006', X'00000000000000000000000000000004', X'00000000000000000000000000000003', 'old input', '{\"snapshot\":true}', X'00000000000000000000000000000005');
        UPDATE workflow_attempts SET latest_run_id = X'00000000000000000000000000000006';
        INSERT INTO node_executions (id, run_id, node_id, node_type, output_text) VALUES (X'00000000000000000000000000000007', X'00000000000000000000000000000006', 'old-agent', 'agent', 'preserved artifact');")
        .execute(&pool).await.unwrap();
    sqlx::migrate!("../db/migrations").run(&pool).await.unwrap();
    let preserved: (Uuid, Option<Uuid>, String, String) = sqlx::query_as(
        "SELECT issue_id, repository_id, input_text, graph_snapshot FROM workflow_runs",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        preserved,
        (
            Uuid::from_u128(3),
            None,
            "old input".into(),
            "{\"snapshot\":true}".into()
        )
    );
    let artifact: String = sqlx::query_scalar("SELECT output_text FROM node_executions ne JOIN workflow_attempts wa ON wa.latest_run_id = ne.run_id").fetch_one(&pool).await.unwrap();
    assert_eq!(artifact, "preserved artifact");
    assert!(
        sqlx::query("PRAGMA foreign_key_check")
            .fetch_all(&pool)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        sqlx::query("UPDATE workflow_runs SET issue_id = NULL")
            .execute(&pool)
            .await
            .is_err()
    );
}

#[tokio::test]
async fn bootstrap_pass_and_refine_use_existing_runner_and_publish_once() {
    for (refine, refute) in [(false, false), (true, false), (true, true)] {
        let f = Fixture::new().await;
        f.link_generated_issue().await;
        let run_id = f.reserve().await;
        f.assert_issue_status("In Progress").await;
        let executor = FakeBootstrap {
            fixture: &f,
            refine,
            refute,
            fail: None,
            calls: Mutex::new(Vec::new()),
            publications: AtomicUsize::new(0),
        };
        drive_repository_workflow(&f.pool, run_id, &executor)
            .await
            .unwrap();
        let run = get_workflow_run_response(&f.pool, run_id).await.unwrap();
        assert_eq!(run.status, WorkflowRunStatus::Succeeded, "{:?}", run.nodes);
        f.assert_issue_status("Done").await;
        assert_eq!(run.issue_id, None);
        assert_eq!(run.attempt_id, None);
        assert_eq!(run.repository_id, Some(f.repository_id));
        let calls = executor.calls.lock().unwrap().clone();
        assert_eq!(
            calls
                .iter()
                .map(|call| call.node_id.as_str())
                .collect::<Vec<_>>(),
            if refine {
                vec!["generate", "review", "refine", "re-review"]
            } else {
                vec!["generate", "review"]
            }
        );
        assert!(calls.iter().all(|call| call.workspace_id == f.workspace_id));
        assert_eq!(
            calls
                .iter()
                .map(|call| call.session_id)
                .collect::<HashSet<_>>()
                .len(),
            calls.len()
        );
        assert_eq!(
            run.nodes
                .iter()
                .filter_map(|node| node.agent_run_id)
                .collect::<HashSet<_>>()
                .len(),
            calls.len()
        );
        assert_eq!(executor.publications.load(Ordering::SeqCst), 1);
        for node in run.nodes.iter().filter(|node| {
            matches!(node.node_id.as_str(), "review" | "refine")
                && node.status == DbNodeExecutionStatus::Succeeded
        }) {
            let raw = node.output_text.as_ref().unwrap();
            assert!(raw.len() < 4096);
            let reference = ReportReference::parse(raw).unwrap();
            assert_eq!(reference.finding_count, 20);
            assert_eq!(reference.identity.agent_run_id, node.agent_run_id.unwrap());
            assert!(
                std::fs::metadata(reference.prompt_path(&f.store))
                    .unwrap()
                    .len()
                    > 12_000
            );
        }
        if refute {
            assert_eq!(
                git(&f.root, &["show", "main:openwiki/index.md"]),
                "Generated Wiki"
            );
        }
        assert_eq!(
            git(
                &f.root,
                &["rev-list", "--count", &format!("{}..main", f.source)]
            ),
            "1"
        );
        assert_eq!(
            git(&f.root, &["diff", "--name-only", &f.source, "main"]),
            "openwiki/INSTRUCTIONS.md\nopenwiki/index.md"
        );
        assert_eq!(
            run.nodes.len(),
            8,
            "skip semantics must not manufacture another Publish iteration"
        );
        let snapshot: String = sqlx::query_scalar("SELECT graph_json FROM workflows WHERE id = ?")
            .bind(run.workflow_id)
            .fetch_one(&f.pool)
            .await
            .unwrap();
        assert!(
            !snapshot.contains("session_id"),
            "global system template must stay session-free"
        );
    }
}

#[tokio::test]
async fn bootstrap_phase_failures_never_reach_publication_or_reuse_retry() {
    for phase in [
        "generate",
        "review",
        "refine",
        "publish",
        "tamper_generate",
        "tamper_review",
        "tamper_refine",
        "tamper_publish",
    ] {
        let f = Fixture::new().await;
        let run_id = f.reserve().await;
        let executor = FakeBootstrap {
            fixture: &f,
            refine: true,
            refute: false,
            fail: Some(phase),
            calls: Mutex::new(Vec::new()),
            publications: AtomicUsize::new(0),
        };
        let _ = drive_repository_workflow(&f.pool, run_id, &executor).await;
        assert_eq!(executor.publications.load(Ordering::SeqCst), 0);
        assert_eq!(git(&f.root, &["rev-parse", "main"]), f.source);
        assert_eq!(git(&f.maintenance, &["rev-parse", "HEAD"]), f.source);
        assert!(
            retry_workflow_node(&f.pool, run_id, phase, &executor)
                .await
                .is_err()
        );
    }
}

#[tokio::test]
async fn report_corruption_is_a_quality_warning_after_safe_writes() {
    for (fail, refine) in [
        ("tamper_review_report", true),
        ("tamper_pass_report", false),
        ("tamper_refine_report", true),
    ] {
        let f = Fixture::new().await;
        let run_id = f.reserve().await;
        let executor = FakeBootstrap {
            fixture: &f,
            refine,
            refute: false,
            fail: Some(fail),
            calls: Mutex::new(Vec::new()),
            publications: AtomicUsize::new(0),
        };
        let _ = drive_repository_workflow(&f.pool, run_id, &executor).await;
        let run = get_workflow_run_response(&f.pool, run_id).await.unwrap();
        assert_eq!(run.status, WorkflowRunStatus::Succeeded, "{fail}");
        let publication = f.store.publication(run_id).await.unwrap().unwrap();
        if fail != "tamper_refine_report" {
            assert_eq!(publication.quality_warnings.len(), 1);
        }
        if fail == "tamper_review_report" {
            assert!(
                !executor
                    .calls
                    .lock()
                    .unwrap()
                    .iter()
                    .any(|c| c.node_id == "refine")
            );
        }
    }
}

#[tokio::test]
async fn legacy_review_remains_readable_but_reference_identity_is_checked_against_db() {
    let f = Fixture::new().await;
    let run_id = f.reserve().await;
    let executor = FakeBootstrap {
        fixture: &f,
        refine: false,
        refute: false,
        fail: None,
        calls: Mutex::new(Vec::new()),
        publications: AtomicUsize::new(0),
    };
    drive_repository_workflow(&f.pool, run_id, &executor)
        .await
        .unwrap();
    let source = InventoryIdentity::new(f.repository_id, f.workspace_id, run_id, f.source.clone());
    let (review, reference) =
        crate::workflow_runtime::bootstrap::load_review_phase(&f.pool, &f.store, source.clone())
            .await
            .unwrap();
    // A legacy inline node can use the same report transport without a migration.
    sqlx::query(
        "UPDATE node_executions SET output_text = ? WHERE run_id = ? AND node_id = 'review'",
    )
    .bind(serde_json::to_string(&review).unwrap())
    .bind(run_id)
    .execute(&f.pool)
    .await
    .unwrap();
    assert_eq!(
        crate::workflow_runtime::bootstrap::load_review_phase(&f.pool, &f.store, source.clone())
            .await
            .unwrap()
            .0,
        review
    );
    let mut wrong = reference.clone();
    wrong.identity.source.repository_id = Uuid::new_v4();
    sqlx::query(
        "UPDATE node_executions SET output_text = ? WHERE run_id = ? AND node_id = 'review'",
    )
    .bind(serde_json::to_string(&wrong).unwrap())
    .bind(run_id)
    .execute(&f.pool)
    .await
    .unwrap();
    assert!(
        crate::workflow_runtime::bootstrap::load_review_phase(&f.pool, &f.store, source.clone())
            .await
            .is_err()
    );
    wrong = reference;
    wrong.identity.session_id = Uuid::new_v4();
    sqlx::query(
        "UPDATE node_executions SET output_text = ? WHERE run_id = ? AND node_id = 'review'",
    )
    .bind(serde_json::to_string(&wrong).unwrap())
    .bind(run_id)
    .execute(&f.pool)
    .await
    .unwrap();
    assert!(
        crate::workflow_runtime::bootstrap::load_review_phase(&f.pool, &f.store, source)
            .await
            .is_err()
    );
}

#[tokio::test]
async fn inventory_host_input_identity_and_tampering_fail_before_dispatch() {
    for defect in [
        "changed_manifest",
        "missing_manifest",
        "foreign_run",
        "invalid_input",
    ] {
        let f = Fixture::new().await;
        let run_id = f.reserve().await;
        let inventory = f.inventory(run_id).await.unwrap();
        let manifest = f.store.document_inventory_path(run_id, None);
        match defect {
            "changed_manifest" => {
                let bytes = std::fs::read_to_string(&manifest).unwrap();
                std::fs::write(&manifest, format!("{bytes}\n")).unwrap();
            }
            "missing_manifest" => std::fs::remove_file(&manifest).unwrap(),
            "foreign_run" => {
                let other = DocumentInventory::generate(
                    &git::GitService::new(),
                    &f.maintenance,
                    &f.store,
                    InventoryIdentity::new(
                        f.repository_id,
                        f.workspace_id,
                        Uuid::new_v4(),
                        f.source.clone(),
                    ),
                )
                .unwrap();
                sqlx::query("UPDATE workflow_runs SET input_text = ? WHERE id = ?")
                    .bind(serde_json::to_string(&other).unwrap())
                    .bind(run_id)
                    .execute(&f.pool)
                    .await
                    .unwrap();
            }
            _ => {
                sqlx::query("UPDATE workflow_runs SET input_text = 'not json' WHERE id = ?")
                    .bind(run_id)
                    .execute(&f.pool)
                    .await
                    .unwrap();
            }
        }
        assert!(f.inventory(run_id).await.is_err(), "{defect}");
        let executor = FakeBootstrap {
            fixture: &f,
            refine: false,
            refute: false,
            fail: None,
            calls: Mutex::new(Vec::new()),
            publications: AtomicUsize::new(0),
        };
        let _ = drive_repository_workflow(&f.pool, run_id, &executor).await;
        assert!(executor.calls.lock().unwrap().is_empty());
        assert_eq!(executor.publications.load(Ordering::SeqCst), 0);
        assert!(f.store.publication(run_id).await.unwrap().is_none());
        assert_eq!(git(&f.root, &["rev-parse", "main"]), f.source);
        assert_eq!(inventory.identity().run_id, run_id);
    }
}

#[tokio::test]
async fn inventory_is_frozen_in_host_input_not_global_template_or_upstream() {
    let f = Fixture::new().await;
    let run_id = f.reserve().await;
    let inventory = f.inventory(run_id).await.unwrap();
    let input: String = sqlx::query_scalar("SELECT input_text FROM workflow_runs WHERE id = ?")
        .bind(run_id)
        .fetch_one(&f.pool)
        .await
        .unwrap();
    assert_eq!(
        serde_json::from_str::<DocumentInventory>(&input).unwrap(),
        inventory
    );
    let global: String = sqlx::query_scalar("SELECT graph_json FROM workflows WHERE id = ?")
        .bind(Uuid::parse_str(workflow::templates::OPENWIKI_BOOTSTRAP_ID).unwrap())
        .fetch_one(&f.pool)
        .await
        .unwrap();
    assert!(!global.contains(&run_id.to_string()));
    let graph: String = sqlx::query_scalar("SELECT graph_snapshot FROM workflow_runs WHERE id = ?")
        .bind(run_id)
        .fetch_one(&f.pool)
        .await
        .unwrap();
    let graph: WorkflowGraph = serde_json::from_str(&graph).unwrap();
    for node in graph
        .nodes
        .iter()
        .filter(|node| matches!(node.id.as_str(), "generate" | "review"))
    {
        assert_eq!(node.data.include_workflow_context, Some(false));
        assert!(
            node.data
                .prompt_template
                .as_deref()
                .unwrap()
                .contains(&inventory.prompt(&f.store, node.id == "review"))
        );
    }
    let mut expected = inventory.identity().clone();
    expected.source_sha = "0".repeat(40);
    assert!(
        super::super::bootstrap::load_inventory_input(&f.pool, &f.store, expected)
            .await
            .is_err()
    );
}

#[tokio::test]
async fn missing_review_and_remaining_findings_publish_once_with_exact_warnings() {
    for (fail, refine) in [
        ("missing_review", false),
        ("missing_re_review", true),
        ("remaining", true),
        ("incomplete", false),
        ("incomplete", true),
    ] {
        let f = Fixture::new().await;
        let run = f.reserve().await;
        let executor = FakeBootstrap {
            fixture: &f,
            refine,
            refute: false,
            fail: Some(fail),
            calls: Mutex::new(Vec::new()),
            publications: AtomicUsize::new(0),
        };
        drive_repository_workflow(&f.pool, run, &executor)
            .await
            .unwrap();
        assert_eq!(
            get_workflow_run_response(&f.pool, run)
                .await
                .unwrap()
                .status,
            WorkflowRunStatus::Succeeded
        );
        let published = f.store.publication(run).await.unwrap().unwrap();
        assert_eq!(published.quality_warnings.len(), 1);
        // Receipt recovery replays the same checkpoint, including its quality
        // warning, even when a later caller supplies no review information.
        openwiki::publish_reviewed_wiki(
            &git::GitService::new(),
            &f.store,
            &openwiki::WikiPublicationRequest {
                repository_root: &f.root,
                maintenance_root: &f.maintenance,
                maintenance_branch: "wiki",
                target_branch: "main",
                source_commit: &f.source,
                run_id: run,
            },
            &[],
        )
        .await
        .unwrap();
        assert_eq!(
            f.store
                .publication(run)
                .await
                .unwrap()
                .unwrap()
                .quality_warnings,
            published.quality_warnings
        );
        assert_eq!(
            git(
                &f.root,
                &["rev-list", "--count", &format!("{}..main", f.source)]
            ),
            "1"
        );
        let warning = &published.quality_warnings[0];
        assert_eq!(warning.node_id, if refine { "re-review" } else { "review" });
        if fail == "incomplete" {
            assert!(warning.summary.contains("レビュー未完了"));
            assert_eq!(warning.findings.len(), if refine { 21 } else { 1 });
            let unknown = warning.findings.last().unwrap();
            assert!(
                unknown
                    .title
                    .contains(if refine { "re-review:" } else { "review:" })
            );
            assert!(unknown.description.contains("旧版の残りの手順は未照合"));
            assert!(!warning.summary.contains("レビュー結果なし"));
        } else if fail == "remaining" {
            assert_eq!(warning.findings.len(), 20);
        } else {
            assert!(warning.findings.is_empty());
            assert!(warning.summary.contains("レビュー結果なし"));
        }
        let calls = executor.calls.lock().unwrap();
        assert_eq!(
            calls.iter().filter(|n| n.node_id == "refine").count(),
            usize::from(refine)
        );
        assert_eq!(
            calls.iter().filter(|n| n.node_id == "re-review").count(),
            usize::from(refine)
        );
        for n in calls.iter().filter(|n| {
            matches!(
                n.node_id.as_str(),
                "generate" | "review" | "refine" | "re-review"
            )
        }) {
            assert_eq!(
                n.executor_config.as_ref().unwrap()["execution_mode"],
                "goal"
            );
        }
        assert_eq!(executor.publications.load(Ordering::SeqCst), 1);
    }
}

/// Tiny isolated Git + SQLite fixture, with scripted model output. A move is
/// accounted for in summary; only the actual missing constraint becomes a
/// finding, survives independent re-review and reaches the publication record.
#[tokio::test]
async fn retention_findings_publish_without_turning_page_moves_into_warnings() {
    let mut f = Fixture::new().await;
    std::fs::create_dir(f.root.join("openwiki")).unwrap();
    std::fs::write(
        f.root.join("source.rs"),
        "fn auto_merge(direct_folder: bool) { assert!(!direct_folder); }",
    )
    .unwrap();
    std::fs::write(
        f.root.join("openwiki/INSTRUCTIONS.md"),
        include_bytes!("../../../../assets/openwiki-instructions.md"),
    )
    .unwrap();
    std::fs::write(
        f.root.join("openwiki/moved.md"),
        "The procedure survives at its new home.",
    )
    .unwrap();
    std::fs::write(f.root.join("openwiki/workspace.md"), "DirectFolder is excluded from automatic merge. This constraint applies even when legacy repository rows exist. Repeat explanatory context.").unwrap();
    git(&f.root, &["add", "."]);
    git(&f.root, &["commit", "-m", "published fixture Wiki"]);
    f.source = git(&f.root, &["rev-parse", "HEAD"]);
    git(&f.maintenance, &["merge", "--ff-only", "main"]);
    let run = f.reserve().await;
    let executor = FakeBootstrap {
        fixture: &f,
        refine: true,
        refute: false,
        fail: Some("retention"),
        calls: Mutex::new(Vec::new()),
        publications: AtomicUsize::new(0),
    };
    drive_repository_workflow(&f.pool, run, &executor)
        .await
        .unwrap();
    assert_eq!(
        get_workflow_run_response(&f.pool, run)
            .await
            .unwrap()
            .status,
        WorkflowRunStatus::Succeeded
    );
    let published = f.store.publication(run).await.unwrap().unwrap();
    assert_eq!(published.quality_warnings.len(), 1);
    let warning = &published.quality_warnings[0];
    assert_eq!(warning.node_id, "re-review");
    assert_eq!(warning.findings.len(), 1);
    assert!(
        warning.findings[0]
            .description
            .contains("DirectFolder is excluded from automatic merge")
    );
    assert!(
        warning.findings[0]
            .description
            .contains("openwiki/workspace.md")
    );
    assert!(
        !warning
            .findings
            .iter()
            .any(|f| f.description.contains("moved.md"))
    );
    assert_eq!(
        git(&f.root, &["show", "main:openwiki/destination.md"]),
        "The procedure survives at its new home."
    );
    assert_eq!(
        git(
            &f.root,
            &["rev-list", "--count", &format!("{}..main", f.source)]
        ),
        "1"
    );
    assert_eq!(executor.publications.load(Ordering::SeqCst), 1);
    let calls = executor.calls.lock().unwrap();
    for node in ["generate", "review", "refine", "re-review"] {
        assert_eq!(calls.iter().filter(|n| n.node_id == node).count(), 1);
    }
    let sessions: std::collections::HashSet<_> = calls.iter().map(|n| n.session_id).collect();
    assert_eq!(sessions.len(), 4);
}

fn dispatch_envelopes(
    f: &Fixture,
    session: Uuid,
    run_id: Uuid,
) -> (AgentRunRequestEnvelope, RunAttemptRequest) {
    let agent_run = Uuid::new_v4();
    let turn = Uuid::new_v4();
    let attempt = Uuid::new_v4();
    let executor_config = ExecutorConfig::new(BaseCodingAgent::Codex);
    let request = AgentRunRequestEnvelope {
        schema_version: AGENT_REQUEST_SCHEMA_VERSION,
        payload_version: AGENT_REQUEST_PAYLOAD_VERSION,
        request_id: Uuid::new_v4(),
        idempotency_key: "cleanup-host".into(),
        session_id: session,
        agent_run_id: agent_run,
        turn_id: turn,
        correlation_id: run_id,
        intent: AgentRunIntent::Initial,
        runtime_profile_id: executor_config.profile_id().cache_key(),
        provider_id: "codex".into(),
        workspace: WorkspaceReference {
            workspace_id: f.workspace_id,
            mode: WorkspaceMode::IsolatedWorktree,
            path: f.maintenance.to_string_lossy().into_owned(),
        },
        input: CanonicalMessage {
            message_id: Uuid::new_v4(),
            role: AgentRuntimeMessageRole::User,
            content: "cleanup fixture".into(),
        },
        created_at: Utc::now(),
    };
    let attempt_request = RunAttemptRequest {
        schema_version: request.schema_version,
        payload_version: request.payload_version,
        request_id: request.request_id,
        idempotency_key: "cleanup-attempt".into(),
        session_id: session,
        agent_run_id: agent_run,
        turn_id: turn,
        run_attempt_id: attempt,
        attempt_number: 1,
        correlation_id: run_id,
        mode: RunAttemptMode::Launch,
        transport: DirectProvider::Codex.transport(),
        runtime_profile_id: request.runtime_profile_id.clone(),
        provider_id: request.provider_id.clone(),
        workspace: request.workspace.clone(),
        capability_snapshot: direct_provider_capability_snapshot(
            DirectProvider::Codex,
            request.runtime_profile_id.clone(),
        ),
        executor_config,
        selected_skills: None,
        reset_to_message_id: None,
        provider_session: None,
        created_at: request.created_at,
    };
    (request, attempt_request)
}

#[tokio::test]
async fn bootstrap_dispatch_binds_only_persisted_identity_before_outbox_delivery() {
    use db::models::agent_runtime::AgentRunRecord;
    use executors::runtime::{AgentEventStream, AgentRunPort, AgentRunPortSnapshot, RunState};
    struct CheckingPort(SqlitePool);
    #[async_trait]
    impl AgentRunPort for CheckingPort {
        async fn create(
            &self,
            request: AgentRunRequestEnvelope,
            attempt: RunAttemptRequest,
        ) -> Result<Uuid, AgentRunPortError> {
            let bound: Uuid =
                sqlx::query_scalar("SELECT agent_run_id FROM node_executions WHERE session_id=?")
                    .bind(request.session_id)
                    .fetch_one(&self.0)
                    .await
                    .unwrap();
            assert_eq!(bound, request.agent_run_id);
            let workspace = Workspace::find_by_id(&self.0, request.workspace.workspace_id)
                .await
                .unwrap()
                .unwrap();
            services::services::workspace_usage::validate_agent_owner(
                &self.0, &workspace, &request, false,
            )
            .await
            .unwrap();
            // Exercise the same idempotent canonical persistence boundary used by real Create.
            AgentRunRecord::persist_identity_with_launch(&self.0, &request, &attempt, true)
                .await
                .unwrap();
            Ok(bound)
        }
        async fn query(&self, id: Uuid) -> Result<AgentRunPortSnapshot, AgentRunPortError> {
            let run = AgentRunRecord::find(&self.0, id).await.unwrap().unwrap();
            Ok(AgentRunPortSnapshot {
                agent_run_id: id,
                state: RunState::pending(&run.request_envelope.0),
            })
        }
        async fn control(&self, _: AgentRunPortCommandEnvelope) -> Result<(), AgentRunPortError> {
            panic!("unexpected control")
        }
        async fn subscribe(&self, _: Uuid) -> Result<AgentEventStream, AgentRunPortError> {
            panic!("unexpected subscribe")
        }
    }
    let f = Fixture::new().await;
    let run = f.reserve().await;
    let orchestration: Uuid =
        sqlx::query_scalar("SELECT orchestration_run_id FROM workflow_runs WHERE id=?")
            .bind(run)
            .fetch_one(&f.pool)
            .await
            .unwrap();
    let session = Session::create(
        &f.pool,
        &CreateSession {
            executor: Some("CODEX".into()),
            name: None,
        },
        Uuid::new_v4(),
        f.workspace_id,
    )
    .await
    .unwrap();
    let node:Uuid=sqlx::query_scalar("SELECT orchestration_node_execution_id FROM node_executions WHERE run_id=? AND node_id='generate'").bind(run).fetch_one(&f.pool).await.unwrap();
    sqlx::query("UPDATE node_executions SET session_id=? WHERE orchestration_node_execution_id=?")
        .bind(session.id)
        .bind(node)
        .execute(&f.pool)
        .await
        .unwrap();
    sqlx::query("UPDATE workflow_runs SET status='running' WHERE id=?")
        .bind(run)
        .execute(&f.pool)
        .await
        .unwrap();
    let state = utils::repository_memory::RepositoryMemoryState {
        enabled: true,
        status: utils::repository_memory::RepositoryWikiStatus::Initializing,
        bootstrap: Some(utils::repository_memory::OpenWikiBootstrapOwner {
            workflow_run_id: run,
        }),
        maintenance_workspace_id: Some(f.workspace_id),
        ..Default::default()
    };
    f.store.save_state(&state).await.unwrap();
    let (request, attempt) = dispatch_envelopes(&f, session.id, orchestration);
    // The former order really fails with foreign_keys enabled, not a permissive mock schema.
    let error = sqlx::query(
        "UPDATE node_executions SET agent_run_id=? WHERE orchestration_node_execution_id=?",
    )
    .bind(request.agent_run_id)
    .bind(node)
    .execute(&f.pool)
    .await
    .unwrap_err();
    assert!(error.to_string().contains("FOREIGN KEY"));
    super::super::bootstrap::bind_child_dispatch(&f.pool, node, &request, &attempt)
        .await
        .unwrap();
    super::super::bootstrap::bind_child_dispatch(&f.pool, node, &request, &attempt)
        .await
        .unwrap();
    let eligible: i64 = sqlx::query_scalar(
        "SELECT launch_requested FROM agent_process_registry WHERE run_attempt_id=?",
    )
    .bind(attempt.run_attempt_id)
    .fetch_one(&f.pool)
    .await
    .unwrap();
    assert_eq!(eligible, 0, "binding must not launch a provider");
    let service = OrchestrationService::new(f.pool.clone(), Arc::new(CheckingPort(f.pool.clone())));
    let agent = request.agent_run_id;
    let command = AgentRunPortCommandEnvelope {
        schema_version: ORCHESTRATION_COMMAND_SCHEMA_VERSION,
        command_id: Uuid::new_v4(),
        idempotency_key: "dispatch-regression".into(),
        agent_run_id: agent,
        orchestration_run_id: Some(orchestration),
        orchestration_node_execution_id: Some(node),
        correlation_id: orchestration,
        created_at: request.created_at,
        command: AgentRunPortCommand::Create { request, attempt },
    };
    let outbox = service.enqueue_command(command).await.unwrap();
    service.deliver_create(outbox, agent).await.unwrap();
    let links: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM orchestration_agent_run_links WHERE agent_run_id=?",
    )
    .bind(agent)
    .fetch_one(&f.pool)
    .await
    .unwrap();
    assert_eq!(links, 1);
    let checks: Vec<sqlx::sqlite::SqliteRow> = sqlx::query("PRAGMA foreign_key_check")
        .fetch_all(&f.pool)
        .await
        .unwrap();
    assert!(checks.is_empty());
}
