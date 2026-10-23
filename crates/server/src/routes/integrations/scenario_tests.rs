//! Opt-in isolated host/browser scenarios. No production DB or repository.
//! The deterministic variant supplies provider events; the opt-in real variant uses Codex.
use db::models::{
    agent_runtime::{AgentEventRecord, AgentRunRecord},
    integration::{IntegrationSelection, IntegrationSource},
};
use executors::{executors::BaseCodingAgent, profile::ExecutorConfig, runtime::*};

use super::*;

#[tokio::test]
#[ignore = "requires a fresh LVK_INSTANCE_DIR; run alone in its own process"]
async fn isolated_merge_approval_publication() -> anyhow::Result<()> {
    let instance = PathBuf::from(std::env::var("LVK_INSTANCE_DIR")?);
    ensure!(
        instance
            .file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("local9-scenario-")
    );
    ensure!(
        !instance.join("data/db.v2.sqlite").exists(),
        "Use a fresh isolated instance"
    );
    let deployment = DeploymentImpl::new(tokio_util::sync::CancellationToken::new()).await?;
    merge_scenario(&deployment, &instance, None, None).await
}

// Browser mode uses the real router/access layer and the same deterministic
// provider-event fixture as the host scenario. No production test backdoor.
#[tokio::test]
#[ignore = "requires a fresh isolated instance, built frontend and Chromium"]
async fn isolated_review_browser() -> anyhow::Result<()> {
    let instance = PathBuf::from(std::env::var("LVK_INSTANCE_DIR")?);
    ensure!(
        instance
            .file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("local9-scenario-")
    );
    ensure!(
        !instance.join("data/db.v2.sqlite").exists(),
        "Use a fresh isolated instance"
    );
    ensure!(std::env::var("LVK_ACCESS_DEPLOYMENT").as_deref() == Ok("local"));
    ensure!(std::env::var("LVK_AUTH_MODE").as_deref() == Ok("local"));
    let shutdown = tokio_util::sync::CancellationToken::new();
    let deployment = DeploymentImpl::new(shutdown.clone()).await?;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;
    deployment.client_info().set_server_addr(addr).unwrap();
    let access =
        server_access::Access::from_env(deployment.db().pool.clone(), utils::assets::asset_dir())?;
    let router = crate::routes::router(deployment.clone(), access);
    let stop = shutdown.clone();
    let server = tokio::spawn(async move {
        axum::serve(listener, router)
            .with_graceful_shutdown(async move { stop.cancelled().await })
            .await
    });
    let result = async {
        for mode in ["approve", "cancel", "stale", "empty"] {
            merge_scenario(
                &deployment,
                &instance,
                Some((&format!("http://{addr}"), mode)),
                None,
            )
            .await?;
        }
        anyhow::Ok(())
    }
    .await;
    shutdown.cancel();
    server.await??;
    result
}

async fn merge_scenario(
    deployment: &DeploymentImpl,
    instance: &Path,
    browser: Option<(&str, &str)>,
    mcp: Option<&str>,
) -> anyhow::Result<()> {
    let pool = &deployment.db().pool;
    let root = instance.join(format!(
        "fixture-{}",
        browser.map_or("host", |(_, mode)| mode)
    ));
    std::fs::create_dir_all(&root)?;
    let git = |args: &[&str]| super::super::mechanical::git(&root, args);
    git(&["init", "-b", "main"])?;
    git(&["config", "user.name", "Integration scenario"])?;
    git(&["config", "user.email", "test@localhost"])?;
    std::fs::write(root.join("feature"), "base\n")?;
    std::fs::write(
        root.join("README.md"),
        "Isolated test repository. The feature file lists enabled capabilities, one per line. A and C are independent capabilities; when branches conflict, preserve both A and C and remove base. Only edit and stage the resolution; the Integration host commits it.\n",
    )?;
    git(&["add", "."])?;
    git(&["commit", "-m", "base"])?;
    let base = git(&["rev-parse", "HEAD"])?;
    let repo = Repo::find_or_create(pool, &root, "fixture").await?;
    let project = Uuid::new_v4();
    let status = Uuid::new_v4();
    sqlx::query("INSERT INTO projects(id,name) VALUES(?,'isolated integration')")
        .bind(project)
        .execute(pool)
        .await?;
    sqlx::query("INSERT INTO local_project_metadata(project_id,organization_id) VALUES(?,?)")
        .bind(project)
        .bind(Uuid::from_u128(2))
        .execute(pool)
        .await?;
    sqlx::query("INSERT INTO local_project_statuses(id,project_id,name,color,sort_order) VALUES(?,?,'Done','blue',0)")
        .bind(status).bind(project).execute(pool).await?;
    let mut sources = vec![];
    for (index, (file, text)) in [
        ("feature", "A\n"),
        ("second", "B\n"),
        ("feature", "C\n"),
        ("fourth", "D\n"),
    ]
    .iter()
    .enumerate()
    {
        let branch = format!("source-{index}");
        git(&["checkout", "-b", &branch, &base])?;
        std::fs::write(root.join(file), text)?;
        git(&["add", "."])?;
        git(&["commit", "-m", &branch])?;
        let commit = git(&["rev-parse", "HEAD"])?;
        sources.push(IntegrationSource {
            selection: IntegrationSelection {
                card_id: Uuid::new_v4(),
                workspace_id: Uuid::new_v4(),
                expected_commit: commit.clone(),
            },
            branch,
            commit,
            title: format!("Source {index}"),
            description: None,
            status_id: status,
            requirements_revision: 1,
            event_ids: vec![],
            related_workspaces: vec![],
            done_result: None,
        });
    }
    git(&["checkout", "main"])?;
    let id = Uuid::new_v4();
    let mut run = IntegrationRun {
        id,
        request_key: id.to_string(),
        project_id: project,
        repository_id: repo.id,
        storage_identity: deployment
            .git()
            .storage_identity(&root)?
            .to_string_lossy()
            .into_owned(),
        target_ref: "refs/heads/main".into(),
        status: "queued".into(),
        workspace_id: Some(Uuid::new_v4()),
        session_id: None,
        agent_run_id: None,
        payload: sqlx::types::Json(db::models::integration::IntegrationPayload {
            mechanical: true,
            sequential: true,
            skip_validation: true,
            sources,
            observed_target: base.clone(),
            card_id: Some(Uuid::new_v4()),
            executor_config: Some(serde_json::to_value(ExecutorConfig::new(
                BaseCodingAgent::Codex,
            ))?),
            ..Default::default()
        }),
        cancel_requested: false,
        error: None,
        created_at: String::new(),
        updated_at: String::new(),
    };
    run.create_with_card(pool, "scenario-integration", "Isolated scenario")
        .await?;
    ensure!(admission::target_right(pool, &run).await?);
    advance(deployment, run.clone()).await?;
    let real = browser.is_none()
        && mcp.is_none()
        && std::env::var("LVK_INTEGRATION_REAL_PROVIDER").as_deref() == Ok("1");
    let mut browser_process: Option<tokio::process::Child> = None;
    let mut changed_candidate = false;
    let signal = instance.join("replace-candidate");
    let mut approved = false;
    for _ in 0..1200 {
        run = IntegrationRun::find(pool, id).await?;
        if ["succeeded", "cancelled"].contains(&run.status.as_str()) {
            break;
        }
        if let Some(child) = &mut browser_process
            && let Some(status) = child.try_wait()?
        {
            ensure!(status.success(), "Browser failed: {status}");
        }
        if browser.is_some_and(|(_, mode)| mode == "stale") && signal.exists() && !changed_candidate
        {
            let (_, _, _, checkout) = context(deployment, &run).await?;
            std::fs::write(checkout.join("feature"), "A\nC\nreviewed again\n")?;
            super::super::mechanical::git(&checkout, &["add", "feature"])?;
            super::super::mechanical::git(
                &checkout,
                &[
                    "-c",
                    "user.name=Browser fixture",
                    "-c",
                    "user.email=test@localhost",
                    "commit",
                    "-m",
                    "new candidate",
                ],
            )?;
            run.payload.result_commit = Some(super::super::mechanical::git(
                &checkout,
                &["rev-parse", "HEAD"],
            )?);
            super::super::steps::record_review(&checkout, &mut run)?;
            run.save(pool).await?;
            changed_candidate = true;
        }
        if run.payload.step_index == 2 && run.payload.conflict_dispatch.is_none() && !real {
            let (_, ws, _, checkout) = context(deployment, &run).await?;
            super::super::steps::prepare(&checkout, &mut run)?;
            ensure!(run.payload.merge_conflicted);
            let before = run.payload.step_base.as_deref().unwrap();
            let source = &run.payload.sources[2].commit;
            super::super::mechanical::prepare_conflict(&checkout, before, source)?;
            let dispatch = super::super::mechanical::conflict_request(
                &run,
                ws.container_ref.unwrap(),
                prompt(&run, &repo, &checkout, &serde_json::json!([]))?,
            )?;
            let AgentRunPortCommand::Create { request, attempt } = &dispatch else {
                unreachable!()
            };
            AgentRunRecord::persist_identity_before_launch(pool, request, attempt).await?;
            std::fs::write(checkout.join("feature"), "A\nC\n")?;
            super::super::mechanical::git(&checkout, &["add", "feature"])?;
            for (index, payload) in [
                AgentEventPayload::LifecycleChanged {
                    status: AgentRunStatus::Starting,
                },
                AgentEventPayload::LifecycleChanged {
                    status: AgentRunStatus::Running,
                },
                AgentEventPayload::Message {
                    message: CanonicalMessage {
                        message_id: Uuid::new_v4(),
                        role: AgentRuntimeMessageRole::Assistant,
                        content: serde_json::json!({"outcome":"ready", "summary": "feature の同じ行で A と C の追加が衝突しました。独立した機能なので、片方を失わないよう両方を残して解消しました。意味上の妥当性は人間の確認が必要です。"}).to_string(),
                    },
                    final_output: true,
                },
                AgentEventPayload::LifecycleChanged {
                    status: AgentRunStatus::Succeeded,
                },
            ]
            .into_iter()
            .enumerate()
            {
                AgentEventRecord::append_and_project(
                    pool,
                    &AgentEventEnvelope {
                        schema_version: AGENT_EVENT_SCHEMA_VERSION,
                        payload_version: AGENT_EVENT_PAYLOAD_VERSION,
                        event_id: Uuid::new_v4(),
                        session_id: request.session_id,
                        agent_run_id: request.agent_run_id,
                        turn_id: request.turn_id,
                        run_attempt_id: attempt.run_attempt_id,
                        run_attempt_number: 1,
                        sequence: index as u64 + 1,
                        correlation_id: id,
                        orchestration_run_id: None,
                        orchestration_node_execution_id: None,
                        timestamp: chrono::Utc::now(),
                        native_refs: vec![],
                        payload,
                    },
                )
                .await?;
            }
            run.agent_run_id = Some(request.agent_run_id);
            run.payload.conflict_dispatch = Some(serde_json::to_value(dispatch)?);
            run.save(pool).await?;
        }
        if run.status == "blocked" {
            ensure!(
                run.payload
                    .stop
                    .as_ref()
                    .is_some_and(|s| s.category == "merge_review"),
                "Unexpected blocked result: {:?}",
                run.error
            );
            ensure!(!approved && run.payload.step_index == 2);
            ensure!(git(&["rev-parse", "main"])? == base);
            if let Some((base_url, mode)) = browser {
                if browser_process.is_none() {
                    if mode == "empty" {
                        // Simulate a retained record without an explanation. Current
                        // provider parsing correctly rejects empty ready summaries.
                        run.payload.reviews.last_mut().unwrap().explanation = "  ".into();
                        run.save(pool).await?;
                    }
                    browser_process = Some(
                        tokio::process::Command::new("node")
                            .arg(
                                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                                    .join("../../tests/integration-review/browser.mjs"),
                            )
                            .env("LVK_REVIEW_BASE_URL", base_url)
                            .env("LVK_REVIEW_RUN", id.to_string())
                            .env("LVK_REVIEW_MODE", mode)
                            .env("LVK_REVIEW_SIGNAL", &signal)
                            .spawn()?,
                    );
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
                continue;
            }
            if let Some(mode) = mcp {
                crate::remote_mcp::integration_write_tests::exercise(
                    deployment,
                    &run,
                    mode == "cancel",
                )
                .await?;
                if mode == "cancel" {
                    let current = IntegrationRun::find(pool, id).await?;
                    cancel(deployment, current).await?;
                    let terminal = IntegrationRun::find(pool, id).await?;
                    crate::remote_mcp::integration_write_tests::terminal(
                        deployment, &terminal, true,
                    )
                    .await?;
                    ensure!(git(&["rev-parse", "main"])? == base);
                    return Ok(());
                }
                approved = true;
                continue;
            }
            let review = run.payload.reviews.last().unwrap().id;
            let request_id = Uuid::new_v4();
            for _ in 0..2 {
                decide_gate(
                    deployment,
                    id,
                    super::super::GateDecisionRequest {
                        project_id: project,
                        gate_id: review,
                        request_id,
                        decision: "continue".into(),
                    },
                )
                .await?;
            }
            approved = true;
        } else if let Err(error) = advance(deployment, run.clone()).await {
            cancel(deployment, run).await?;
            return Err(error);
        }
        tokio::time::sleep(Duration::from_millis(if real {
            1000
        } else if browser.is_some() {
            100
        } else {
            10
        }))
        .await;
    }
    run = IntegrationRun::find(pool, id).await?;
    if let Some(mut child) = browser_process {
        let status = tokio::time::timeout(Duration::from_secs(60), child.wait()).await??;
        ensure!(status.success(), "Browser failed: {status}");
        if browser.is_some_and(|(_, mode)| mode != "approve") {
            ensure!(run.status == "cancelled" && !run.payload.published);
            ensure!(git(&["rev-parse", "main"])? == base);
            ensure!(
                run.payload
                    .reviews
                    .iter()
                    .all(|review| review.decision.as_deref() != Some("continue"))
            );
            println!(
                "Browser {:?}: cancelled; target remains {base}; no review approved",
                browser.map(|(_, mode)| mode)
            );
            return Ok(());
        }
        approved = true;
    }
    if run.status != "succeeded" {
        cancel(deployment, run).await?;
        bail!("Scenario timed out; test conflict cancellation requested");
    }
    ensure!(approved && run.payload.published && run.payload.steps.len() == 4);
    let final_commit = run.payload.result_commit.clone().unwrap();
    ensure!(git(&["rev-parse", "main"])? == final_commit);
    for source in &run.payload.sources {
        ensure!(git(&["merge-base", &final_commit, &source.commit])? == source.commit);
        ensure!(git(&["rev-parse", &source.branch])? == source.commit);
    }
    ensure!(run.payload.validation.is_empty() && run.payload.skip_validation);
    ensure!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM agent_runs")
            .fetch_one(pool)
            .await?
            == if browser.is_some() {
                sqlx::query_scalar::<_, i64>("SELECT count(*) FROM integration_runs")
                    .fetch_one(pool)
                    .await?
            } else {
                1
            }
    );
    ensure!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM integration_reservations")
            .fetch_one(pool)
            .await?
            == 0
    );
    if mcp.is_some() {
        crate::remote_mcp::integration_write_tests::terminal(deployment, &run, false).await?;
    }
    request_recovery(deployment, run).await?;
    ensure!(git(&["rev-parse", "main"])? == final_commit);
    println!(
        "Isolated host scenario passed; real_provider={real}, approved step 3, step 4 published exactly once; base={base}, candidate={final_commit}"
    );
    Ok(())
}

#[tokio::test]
#[ignore = "requires a fresh isolated instance and private retained-checkpoint fixture"]
async fn isolated_retained_checkpoint_adoption() -> anyhow::Result<()> {
    use super::super::legacy_checkpoint::{self, Request};
    let instance = PathBuf::from(std::env::var("LVK_INSTANCE_DIR")?);
    ensure!(
        instance
            .file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("local9-scenario-")
    );
    ensure!(
        !instance.join("data/db.v2.sqlite").exists(),
        "Use a fresh isolated instance"
    );
    let fixture: serde_json::Value =
        serde_json::from_slice(&std::fs::read(std::env::var("LVK_LEGACY_FIXTURE")?)?)?;
    let deployment = DeploymentImpl::new(tokio_util::sync::CancellationToken::new()).await?;
    let pool = &deployment.db().pool;
    let root = instance.join("fixture");
    let checkout_parent = instance.join("integration");
    let own_repo = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()?;
    let git = |args: &[&str]| super::super::mechanical::git(&root, args);
    let output = std::process::Command::new("git")
        .args(["clone", "--shared", "--no-checkout"])
        .arg(&own_repo)
        .arg(&root)
        .output()?;
    ensure!(
        output.status.success(),
        "Cannot create isolated object/refs clone"
    );
    git(&[
        "checkout",
        "-B",
        "main",
        "b385a2d2d1a8683fbdcb8d931356d34f4264169c",
    ])?;
    for table in fixture.as_array().context("Invalid fixture")? {
        let name = table["table"].as_str().context("Missing table")?;
        // A local private fixture, not an externally supplied SQL program.
        ensure!(name.bytes().all(|c| c.is_ascii_lowercase() || c == b'_'));
        for row in table["rows"].as_array().context("Missing rows")? {
            let mut row = row.as_object().unwrap().clone();
            if name == "repos" {
                row.insert("path".into(), serde_json::json!(root));
            }
            if name == "workspaces" {
                row.insert("container_ref".into(), serde_json::json!(checkout_parent));
            }
            if name == "integration_runs" {
                row.insert(
                    "storage_identity".into(),
                    serde_json::json!(git(&["rev-parse", "--absolute-git-dir"])?),
                );
            }
            let columns = row.keys().cloned().collect::<Vec<_>>();
            ensure!(
                columns
                    .iter()
                    .all(|s| s.bytes().all(|c| c.is_ascii_lowercase() || c == b'_'))
            );
            let sql = format!(
                "INSERT INTO {name} ({}) VALUES ({})",
                columns.join(","),
                vec!["?"; columns.len()].join(",")
            );
            let mut query = sqlx::query(&sql);
            for value in row.values() {
                query = match value {
                    serde_json::Value::Null => query.bind(None::<String>),
                    serde_json::Value::String(v) => query.bind(v.clone()),
                    serde_json::Value::Number(v) => {
                        if let Some(n) = v.as_i64() {
                            query.bind(n)
                        } else {
                            query.bind(v.as_f64().unwrap())
                        }
                    }
                    serde_json::Value::Object(v) if v.contains_key("blob") => {
                        let s = v["blob"].as_str().unwrap();
                        query.bind(
                            (0..s.len())
                                .step_by(2)
                                .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
                                .collect::<Vec<_>>(),
                        )
                    }
                    _ => bail!("Unsupported fixture value"),
                };
            }
            query
                .execute(pool)
                .await
                .with_context(|| format!("Import {name}"))?;
        }
    }
    let id = Uuid::parse_str("c68ce019-9349-4679-976f-326120191c08")?;
    let original = IntegrationRun::find(pool, id).await?;
    let repo = Repo::find_by_id(pool, original.repository_id)
        .await?
        .unwrap();
    for source in &original.payload.sources {
        git(&["branch", &source.branch, &source.commit])?;
    }
    let ws = Workspace::find_by_id(pool, original.workspace_id.unwrap())
        .await?
        .unwrap();
    let candidate = original.payload.step_base.clone().unwrap();
    git(&[
        "worktree",
        "add",
        "-b",
        &ws.branch,
        checkout_parent.join(&repo.name).to_str().unwrap(),
        &candidate,
    ])?;
    let request_id = Uuid::new_v4();
    let request = |dry_run| Request {
        project_id: original.project_id,
        request_id,
        candidate: candidate.clone(),
        dry_run,
    };
    let raw: String = sqlx::query_scalar("SELECT payload FROM integration_runs WHERE id=?")
        .bind(id)
        .fetch_one(pool)
        .await?;
    let checkout = checkout_parent.join(&repo.name);
    for mutation in 0..5 {
        let mut changed = original.clone();
        match mutation {
            0 => changed.payload.reviews[0].decision = None,
            1 => changed.payload.steps[1].result_commit = candidate.clone(),
            2 => changed.payload.sources[3].commit = candidate.clone(),
            3 => changed.payload.skip_validation = false,
            _ => changed.cancel_requested = true,
        }
        ensure!(legacy_checkpoint::verify_git_and_approval(&changed, &checkout).is_err());
    }
    // Both identity mismatch and dirty candidate must refuse before any adoption.
    let mut wrong = request(false);
    wrong.project_id = Uuid::new_v4();
    ensure!(
        legacy_checkpoint::apply(&deployment, id, wrong)
            .await
            .is_err()
    );
    let dirty = checkout_parent
        .join(&repo.name)
        .join("unexpected-untracked-file");
    std::fs::write(&dirty, "refuse")?;
    ensure!(
        legacy_checkpoint::apply(&deployment, id, request(false))
            .await
            .is_err()
    );
    std::fs::remove_file(dirty)?;
    sqlx::query("UPDATE agent_runs SET status='running'")
        .execute(pool)
        .await?;
    ensure!(
        legacy_checkpoint::apply(&deployment, id, request(false))
            .await
            .is_err()
    );
    sqlx::query("UPDATE agent_runs SET status='succeeded'")
        .execute(pool)
        .await?;
    sqlx::query("UPDATE orchestration_outbox SET delivery_status='pending'")
        .execute(pool)
        .await?;
    ensure!(
        legacy_checkpoint::apply(&deployment, id, request(false))
            .await
            .is_err()
    );
    sqlx::query("UPDATE orchestration_outbox SET delivery_status='delivered'")
        .execute(pool)
        .await?;
    ensure!(
        sqlx::query_scalar::<_, String>("SELECT payload FROM integration_runs WHERE id=?")
            .bind(id)
            .fetch_one(pool)
            .await?
            == raw
    );
    ensure!(legacy_checkpoint::apply(&deployment, id, request(true)).await?["eligible"] == true);
    ensure!(
        sqlx::query_scalar::<_, String>("SELECT payload FROM integration_runs WHERE id=?")
            .bind(id)
            .fetch_one(pool)
            .await?
            == raw
    );
    let service =
        OrchestrationService::new(pool.clone(), Arc::new(deployment.agent_run_port().clone()));
    // Exercise the production startup path, including saved links and child state.
    service.reconcile_startup().await?;
    let settled: (String, String) = sqlx::query_as("SELECT r.status,n.status FROM orchestration_runs r JOIN orchestration_node_executions n ON n.orchestration_run_id=r.id WHERE r.id=?")
        .bind(id).fetch_one(pool).await?;
    ensure!(settled == ("cancelled".into(), "succeeded".into()));
    service.reconcile_startup().await?; // second restart must not append another settlement
    let event_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM orchestration_events WHERE orchestration_run_id=?",
    )
    .bind(id)
    .fetch_one(pool)
    .await?;
    ensure!(event_count == 110);
    // Terminal status alone is insufficient: reject missing settlement evidence.
    sqlx::query("UPDATE orchestration_events SET sequence=111 WHERE orchestration_run_id=? AND sequence=110")
        .bind(id).execute(pool).await?;
    ensure!(
        legacy_checkpoint::apply(&deployment, id, request(true))
            .await
            .is_err()
    );
    sqlx::query("UPDATE orchestration_events SET sequence=110 WHERE orchestration_run_id=? AND sequence=111")
        .bind(id).execute(pool).await?;
    ensure!(legacy_checkpoint::apply(&deployment, id, request(true)).await?["eligible"] == true);
    legacy_checkpoint::apply(&deployment, id, request(false)).await?;
    legacy_checkpoint::apply(&deployment, id, request(false)).await?;
    let mut other = request(false);
    other.request_id = Uuid::new_v4();
    ensure!(
        legacy_checkpoint::apply(&deployment, id, other)
            .await
            .is_err()
    );
    let adopted = IntegrationRun::find(pool, id).await?;
    ensure!(
        adopted.status == "blocked"
            && adopted.payload.mechanical
            && adopted.payload.step_index == 3
    );
    ensure!(
        serde_json::to_value(&adopted.payload.steps)?
            == serde_json::to_value(&original.payload.steps)?
    );
    ensure!(
        adopted.payload.history.last().unwrap()["previous_payload"]
            == serde_json::from_str::<serde_json::Value>(&raw)?
    );
    let agents: i64 = sqlx::query_scalar("SELECT count(*) FROM agent_runs")
        .fetch_one(pool)
        .await?;
    intervene(
        &deployment,
        id,
        super::super::InterventionRequest {
            project_id: original.project_id,
            request_id: Uuid::new_v4(),
            prompt: String::new(),
        },
    )
    .await?;
    advance(&deployment, IntegrationRun::find(pool, id).await?).await?;
    let done = IntegrationRun::find(pool, id).await?;
    ensure!(done.status == "succeeded" && done.payload.published && done.payload.steps.len() == 4);
    ensure!(done.payload.skip_validation && done.payload.validation.is_empty());
    ensure!(git(&["rev-parse", "main"])? == done.payload.result_commit.clone().unwrap());
    ensure!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM agent_runs")
            .fetch_one(pool)
            .await?
            == agents
    );
    ensure!(
        sqlx::query_scalar::<_, String>("SELECT projection_status FROM agent_runs LIMIT 1")
            .fetch_one(pool)
            .await?
            == "projection_degraded"
    );
    let after: (String,String) = sqlx::query_as("SELECT r.status,n.status FROM orchestration_runs r JOIN orchestration_node_executions n ON n.orchestration_run_id=r.id WHERE r.id=?")
        .bind(id).fetch_one(pool).await?;
    ensure!(after == settled);
    ensure!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM orchestration_events WHERE orchestration_run_id=?"
        )
        .bind(id)
        .fetch_one(pool)
        .await?
            == event_count
    );
    println!(
        "Retained checkpoint dry-run/adoption/replay/Resume passed; only step 4 applied, old degraded audit preserved"
    );
    Ok(())
}

// Each invocation owns a fresh instance; no Integration monitor or real provider.
async fn mcp_scenario(mode: &str) -> anyhow::Result<()> {
    let instance = PathBuf::from(std::env::var("LVK_INSTANCE_DIR")?);
    ensure!(
        instance
            .file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("local9-scenario-")
    );
    ensure!(
        !instance.join("data/db.v2.sqlite").exists(),
        "Use a fresh isolated instance"
    );
    let deployment = DeploymentImpl::new(tokio_util::sync::CancellationToken::new()).await?;
    merge_scenario(&deployment, &instance, None, Some(mode)).await
}

#[tokio::test]
#[ignore = "requires a fresh LVK_INSTANCE_DIR; run alone in its own process"]
async fn isolated_mcp_integration_approval() -> anyhow::Result<()> {
    mcp_scenario("approve").await
}

#[tokio::test]
#[ignore = "requires a fresh LVK_INSTANCE_DIR; run alone in its own process"]
async fn isolated_mcp_integration_cancel() -> anyhow::Result<()> {
    mcp_scenario("cancel").await
}
