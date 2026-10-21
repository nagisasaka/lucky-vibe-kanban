async fn board_fixture(pool: &SqlitePool, workspace: Uuid) -> Uuid {
    let project = Uuid::new_v4();
    let issue = Uuid::new_v4();
    for (name, color, order) in [
        ("Not Started", "210 80% 52%", 100),
        ("In Progress", "38 92% 50%", 200),
        ("In Review", "265 70% 62%", 300),
        ("Done", "145 63% 42%", 400),
        ("Custom", "x", 600),
    ] {
        sqlx::query("INSERT INTO local_project_statuses VALUES(?,?,?,?,?,0)")
            .bind(Uuid::new_v4())
            .bind(project)
            .bind(name)
            .bind(color)
            .bind(order)
            .execute(pool)
            .await
            .unwrap();
    }
    sqlx::query("INSERT INTO local_issues(id,project_id,status_id) SELECT ?,project_id,id FROM local_project_statuses WHERE project_id=? AND name='Not Started'")
        .bind(issue).bind(project).execute(pool).await.unwrap();
    sqlx::query("INSERT INTO local_workspace_links VALUES(?,?)")
        .bind(workspace)
        .bind(issue)
        .execute(pool)
        .await
        .unwrap();
    issue
}
async fn board_status(pool: &SqlitePool, issue: Uuid) -> String {
    sqlx::query_scalar("SELECT s.name FROM local_issues i JOIN local_project_statuses s ON s.id=i.status_id WHERE i.id=?")
        .bind(issue).fetch_one(pool).await.unwrap()
}
async fn manual_status(pool: &SqlitePool, issue: Uuid, name: &str) {
    sqlx::query("UPDATE local_issues SET agent_status_owner=NULL, status_id=(SELECT id FROM local_project_statuses WHERE project_id=local_issues.project_id AND name=?) WHERE id=?")
        .bind(name).bind(issue).execute(pool).await.unwrap();
}
async fn lifecycle(
    pool: &SqlitePool,
    request: &AgentRunRequestEnvelope,
    attempt: &RunAttemptRequest,
    seq: u64,
    status: AgentRunStatus,
    host: bool,
) {
    if status == AgentRunStatus::Running {
        // Mirror Host Started persistence before canonical projection. Preserve
        // explicit timestamps supplied by delayed/reordered-event fixtures.
        let recorded: Option<DateTime<Utc>> = sqlx::query_scalar(
            "SELECT process_started_at FROM agent_process_registry WHERE run_attempt_id=?",
        )
        .bind(attempt.run_attempt_id)
        .fetch_one(pool)
        .await
        .unwrap();
        if recorded.is_none() {
            record_start(pool, attempt, Utc::now()).await;
        }
    }
    let mut e = event(request, attempt, seq);
    e.payload = AgentEventPayload::LifecycleChanged { status };
    if host {
        AgentEventRecord::append_and_project_host_batch(
            pool,
            &[e],
            &[],
            &[],
            attempt.run_attempt_id,
            seq,
        )
        .await
        .unwrap();
    } else {
        AgentEventRecord::append_and_project(pool, &e)
            .await
            .unwrap();
    }
}
#[tokio::test]
async fn kanban_start_success_followup_and_replay_use_both_persistence_paths() {
    for host in [false, true] {
        let pool = setup_pool().await;
        let (session, workspace) = insert_anchors(&pool).await;
        let issue = board_fixture(&pool, workspace).await;
        assert_eq!(board_status(&pool, issue).await, "Not Started");
        for turn in 0..2 {
            let (request, attempt) = requests(session, workspace, &format!("turn-{turn}"));
            AgentRunRecord::persist_identity_before_launch(&pool, &request, &attempt)
                .await
                .unwrap();
            lifecycle(&pool, &request, &attempt, 1, AgentRunStatus::Starting, host).await;
            assert_eq!(
                board_status(&pool, issue).await,
                if turn == 0 {
                    "Not Started"
                } else {
                    "In Review"
                }
            );
            lifecycle(&pool, &request, &attempt, 2, AgentRunStatus::Running, host).await;
            assert_eq!(board_status(&pool, issue).await, "In Progress");
            lifecycle(
                &pool,
                &request,
                &attempt,
                3,
                AgentRunStatus::Succeeded,
                host,
            )
            .await;
            assert_eq!(board_status(&pool, issue).await, "In Review");
            manual_status(&pool, issue, "Not Started").await;
            lifecycle(&pool, &request, &attempt, 4, AgentRunStatus::Running, host).await;
            lifecycle(
                &pool,
                &request,
                &attempt,
                5,
                AgentRunStatus::Succeeded,
                host,
            )
            .await;
            assert_eq!(board_status(&pool, issue).await, "Not Started");
            manual_status(&pool, issue, "In Review").await;
        }
    }
}
#[tokio::test]
async fn kanban_failures_never_claim_success_or_change_unstarted_cards() {
    for terminal in [
        AgentRunStatus::Failed,
        AgentRunStatus::Cancelled,
        AgentRunStatus::Crashed,
        AgentRunStatus::AuditFailed,
    ] {
        for started in [false, true] {
            let pool = setup_pool().await;
            let (session, workspace) = insert_anchors(&pool).await;
            let issue = board_fixture(&pool, workspace).await;
            let (request, attempt) = requests(session, workspace, "failure");
            AgentRunRecord::persist_identity_before_launch(&pool, &request, &attempt)
                .await
                .unwrap();
            lifecycle(
                &pool,
                &request,
                &attempt,
                1,
                if started {
                    AgentRunStatus::Running
                } else {
                    AgentRunStatus::Starting
                },
                true,
            )
            .await;
            lifecycle(&pool, &request, &attempt, 2, terminal, true).await;
            assert_eq!(
                board_status(&pool, issue).await,
                if started {
                    "In Progress"
                } else {
                    "Not Started"
                }
            );
        }
    }
}
#[tokio::test]
async fn kanban_manual_changes_custom_columns_and_done_survive_completion() {
    for name in ["Not Started", "In Progress", "Custom", "Done"] {
        let pool = setup_pool().await;
        let (session, workspace) = insert_anchors(&pool).await;
        let issue = board_fixture(&pool, workspace).await;
        let (request, attempt) = requests(session, workspace, "manual");
        AgentRunRecord::persist_identity_before_launch(&pool, &request, &attempt)
            .await
            .unwrap();
        lifecycle(&pool, &request, &attempt, 1, AgentRunStatus::Running, false).await;
        manual_status(&pool, issue, name).await;
        lifecycle(&pool, &request, &attempt, 2, AgentRunStatus::Running, false).await;
        lifecycle(
            &pool,
            &request,
            &attempt,
            3,
            AgentRunStatus::Succeeded,
            false,
        )
        .await;
        assert_eq!(board_status(&pool, issue).await, name);
        if matches!(name, "Custom" | "Done") {
            let (request, attempt) = requests(session, workspace, "protected-start");
            AgentRunRecord::persist_identity_before_launch(&pool, &request, &attempt)
                .await
                .unwrap();
            lifecycle(&pool, &request, &attempt, 1, AgentRunStatus::Running, false).await;
            assert_eq!(board_status(&pool, issue).await, name);
        }
    }
}
#[tokio::test]
async fn kanban_old_workspace_cannot_overwrite_newer_run() {
    let pool = setup_pool().await;
    let (session, workspace) = insert_anchors(&pool).await;
    let issue = board_fixture(&pool, workspace).await;
    let (old, old_attempt) = requests(session, workspace, "old");
    AgentRunRecord::persist_identity_before_launch(&pool, &old, &old_attempt)
        .await
        .unwrap();
    lifecycle(&pool, &old, &old_attempt, 1, AgentRunStatus::Running, true).await;
    let (session, workspace) = insert_anchors(&pool).await;
    sqlx::query("INSERT INTO local_workspace_links VALUES(?,?)")
        .bind(workspace)
        .bind(issue)
        .execute(&pool)
        .await
        .unwrap();
    let (new, new_attempt) = requests(session, workspace, "new");
    AgentRunRecord::persist_identity_before_launch(&pool, &new, &new_attempt)
        .await
        .unwrap();
    lifecycle(&pool, &new, &new_attempt, 1, AgentRunStatus::Running, true).await;
    lifecycle(
        &pool,
        &old,
        &old_attempt,
        2,
        AgentRunStatus::Succeeded,
        true,
    )
    .await;
    assert_eq!(board_status(&pool, issue).await, "In Progress");
    lifecycle(
        &pool,
        &new,
        &new_attempt,
        2,
        AgentRunStatus::Succeeded,
        true,
    )
    .await;
    assert_eq!(board_status(&pool, issue).await, "In Review");
}
#[tokio::test]
async fn kanban_status_migration_preserves_ids_links_and_custom_names() {
    let pool = setup_pool().await;
    let (_, workspace) = insert_anchors(&pool).await;
    let issue = board_fixture(&pool, workspace).await;
    let original: Uuid = sqlx::query_scalar("SELECT status_id FROM local_issues WHERE id=?")
        .bind(issue)
        .fetch_one(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE local_project_statuses SET name='Todo' WHERE id=?")
        .bind(original)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE local_project_statuses SET name='Todo' WHERE name='Custom'")
        .execute(&pool)
        .await
        .unwrap();
    let rename = include_str!("../../migrations/20261020190000_kanban_lifecycle.sql")
        .split("-- Ownership")
        .next()
        .unwrap();
    for _ in 0..2 {
        sqlx::raw_sql(rename).execute(&pool).await.unwrap();
    }
    assert_eq!(board_status(&pool, issue).await, "Not Started");
    let after: Uuid = sqlx::query_scalar("SELECT status_id FROM local_issues WHERE id=?")
        .bind(issue)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(original, after);
    let custom: String =
        sqlx::query_scalar("SELECT name FROM local_project_statuses WHERE color='x'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(custom, "Todo");
}

#[tokio::test]
async fn kanban_newest_success_waits_for_older_active_run_to_settle() {
    for older_terminal in [AgentRunStatus::Succeeded, AgentRunStatus::Failed] {
        let pool = setup_pool().await;
        let (session, workspace) = insert_anchors(&pool).await;
        let issue = board_fixture(&pool, workspace).await;
        let (old, old_attempt) = requests(session, workspace, "old");
        AgentRunRecord::persist_identity_before_launch(&pool, &old, &old_attempt)
            .await
            .unwrap();
        lifecycle(&pool, &old, &old_attempt, 1, AgentRunStatus::Running, true).await;
        let (session, workspace) = insert_anchors(&pool).await;
        sqlx::query("INSERT INTO local_workspace_links VALUES(?,?)")
            .bind(workspace)
            .bind(issue)
            .execute(&pool)
            .await
            .unwrap();
        let (new, new_attempt) = requests(session, workspace, "new");
        AgentRunRecord::persist_identity_before_launch(&pool, &new, &new_attempt)
            .await
            .unwrap();
        lifecycle(&pool, &new, &new_attempt, 1, AgentRunStatus::Running, true).await;
        lifecycle(
            &pool,
            &new,
            &new_attempt,
            2,
            AgentRunStatus::Succeeded,
            true,
        )
        .await;
        assert_eq!(board_status(&pool, issue).await, "In Progress");
        lifecycle(&pool, &old, &old_attempt, 2, older_terminal, true).await;
        assert_eq!(board_status(&pool, issue).await, "In Review");
    }
}
#[tokio::test]
async fn kanban_host_batch_and_board_effects_roll_back_together() {
    let pool = setup_pool().await;
    let (session, workspace) = insert_anchors(&pool).await;
    let issue = board_fixture(&pool, workspace).await;
    let (request, attempt) = requests(session, workspace, "atomic");
    AgentRunRecord::persist_identity_before_launch(&pool, &request, &attempt)
        .await
        .unwrap();
    record_start(&pool, &attempt, Utc::now()).await;
    let running = event(&request, &attempt, 1);
    let mut success = event(&request, &attempt, 2);
    success.payload = AgentEventPayload::LifecycleChanged {
        status: AgentRunStatus::Succeeded,
    };
    sqlx::raw_sql("CREATE TRIGGER fail_cursor BEFORE UPDATE OF last_host_event_sequence ON agent_process_registry BEGIN SELECT RAISE(ABORT,'fixture cursor failure'); END;").execute(&pool).await.unwrap();
    let events = [running, success];
    assert!(
        AgentEventRecord::append_and_project_host_batch(
            &pool,
            &events,
            &[],
            &[],
            attempt.run_attempt_id,
            2
        )
        .await
        .is_err()
    );
    assert_eq!(board_status(&pool, issue).await, "Not Started");
    sqlx::query("DROP TRIGGER fail_cursor")
        .execute(&pool)
        .await
        .unwrap();
    AgentEventRecord::append_and_project_host_batch(
        &pool,
        &events,
        &[],
        &[],
        attempt.run_attempt_id,
        2,
    )
    .await
    .unwrap();
    assert_eq!(board_status(&pool, issue).await, "In Review");
    manual_status(&pool, issue, "Not Started").await;
    AgentEventRecord::append_and_project_host_batch(
        &pool,
        &events,
        &[],
        &[],
        attempt.run_attempt_id,
        2,
    )
    .await
    .unwrap();
    assert_eq!(board_status(&pool, issue).await, "Not Started");
}

#[tokio::test]
async fn kanban_delayed_start_and_old_success_cannot_hide_new_failure() {
    let pool = setup_pool().await;
    let (session, workspace) = insert_anchors(&pool).await;
    let issue = board_fixture(&pool, workspace).await;
    let (old, old_attempt) = requests(session, workspace, "old-delayed");
    AgentRunRecord::persist_identity_before_launch(&pool, &old, &old_attempt)
        .await
        .unwrap();
    let (new, new_attempt) = requests(session, workspace, "new-failed");
    AgentRunRecord::persist_identity_before_launch(&pool, &new, &new_attempt)
        .await
        .unwrap();
    record_start(
        &pool,
        &old_attempt,
        Utc::now() - chrono::Duration::seconds(10),
    )
    .await;
    lifecycle(&pool, &new, &new_attempt, 1, AgentRunStatus::Running, false).await;
    lifecycle(&pool, &new, &new_attempt, 2, AgentRunStatus::Failed, false).await;
    lifecycle(&pool, &old, &old_attempt, 1, AgentRunStatus::Running, false).await;
    let owner: Option<Uuid> =
        sqlx::query_scalar("SELECT agent_status_owner FROM local_issues WHERE id=?")
            .bind(issue)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(owner, Some(new_attempt.run_attempt_id));
    lifecycle(
        &pool,
        &old,
        &old_attempt,
        2,
        AgentRunStatus::Succeeded,
        false,
    )
    .await;
    assert_eq!(board_status(&pool, issue).await, "In Progress");
}

async fn record_start(pool: &SqlitePool, attempt: &RunAttemptRequest, at: DateTime<Utc>) {
    let pid: i64 = sqlx::query_scalar(
        "SELECT rowid + 1000 FROM agent_process_registry WHERE run_attempt_id=?",
    )
    .bind(attempt.run_attempt_id)
    .fetch_one(pool)
    .await
    .unwrap();
    AgentRunRecord::mark_process_started(
        pool,
        attempt.run_attempt_id,
        pid as u32,
        Some(pid as u32),
        Some("codex"),
        at,
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn kanban_unstarted_newer_failure_does_not_block_older_actual_start() {
    let pool = setup_pool().await;
    let (session, workspace) = insert_anchors(&pool).await;
    let issue = board_fixture(&pool, workspace).await;
    let (a, aa) = requests(session, workspace, "reserved-a");
    AgentRunRecord::persist_identity_before_launch(&pool, &a, &aa)
        .await
        .unwrap();
    let (session, workspace) = insert_anchors(&pool).await;
    sqlx::query("INSERT INTO local_workspace_links VALUES(?,?)")
        .bind(workspace)
        .bind(issue)
        .execute(&pool)
        .await
        .unwrap();
    let (b, ba) = requests(session, workspace, "failed-before-start-b");
    AgentRunRecord::persist_identity_before_launch(&pool, &b, &ba)
        .await
        .unwrap();
    lifecycle(&pool, &b, &ba, 1, AgentRunStatus::Starting, true).await;
    lifecycle(&pool, &b, &ba, 2, AgentRunStatus::Failed, true).await;
    assert_eq!(board_status(&pool, issue).await, "Not Started");
    lifecycle(&pool, &a, &aa, 1, AgentRunStatus::Running, true).await;
    assert_eq!(board_status(&pool, issue).await, "In Progress");
    lifecycle(&pool, &a, &aa, 2, AgentRunStatus::Succeeded, true).await;
    assert_eq!(board_status(&pool, issue).await, "In Review");
}

#[tokio::test]
async fn kanban_actual_start_order_ignores_admission_delivery_and_completion_order() {
    // A is admitted first but actually starts last. Test both delivery orders,
    // including timestamps within the same SQLite millisecond, and exact ties.
    for tied in [false, true] {
        for latest_delivered_first in [false, true] {
            for latest_finished_first in [false, true] {
                let pool = setup_pool().await;
                let (session, workspace) = insert_anchors(&pool).await;
                let issue = board_fixture(&pool, workspace).await;
                let (a, mut aa) = requests(session, workspace, "a");
                aa.run_attempt_id = Uuid::from_u128(if tied { 2 } else { 1 });
                AgentRunRecord::persist_identity_before_launch(&pool, &a, &aa)
                    .await
                    .unwrap();
                let (session, workspace) = insert_anchors(&pool).await;
                sqlx::query("INSERT INTO local_workspace_links VALUES(?,?)")
                    .bind(workspace)
                    .bind(issue)
                    .execute(&pool)
                    .await
                    .unwrap();
                let (b, mut ba) = requests(session, workspace, "b");
                ba.run_attempt_id = Uuid::from_u128(if tied { 1 } else { 2 });
                AgentRunRecord::persist_identity_before_launch(&pool, &b, &ba)
                    .await
                    .unwrap();
                let base: DateTime<Utc> = "2026-10-20T12:00:00.123456700Z".parse().unwrap();
                record_start(
                    &pool,
                    &aa,
                    base + chrono::Duration::nanoseconds(if tied { 0 } else { 1 }),
                )
                .await;
                record_start(&pool, &ba, base).await;
                let starts = if latest_delivered_first {
                    [(&a, &aa), (&b, &ba)]
                } else {
                    [(&b, &ba), (&a, &aa)]
                };
                for (request, attempt) in starts {
                    lifecycle(&pool, request, attempt, 1, AgentRunStatus::Running, true).await;
                }
                let owner: Option<Uuid> =
                    sqlx::query_scalar("SELECT agent_status_owner FROM local_issues WHERE id=?")
                        .bind(issue)
                        .fetch_one(&pool)
                        .await
                        .unwrap();
                assert_eq!(owner, Some(aa.run_attempt_id));
                let finishes = if latest_finished_first {
                    [
                        (&a, &aa, AgentRunStatus::Succeeded),
                        (&b, &ba, AgentRunStatus::Failed),
                    ]
                } else {
                    [
                        (&b, &ba, AgentRunStatus::Succeeded),
                        (&a, &aa, AgentRunStatus::Succeeded),
                    ]
                };
                for (index, (request, attempt, status)) in finishes.into_iter().enumerate() {
                    lifecycle(&pool, request, attempt, 2, status, true).await;
                    assert_eq!(
                        board_status(&pool, issue).await,
                        if index == 0 {
                            "In Progress"
                        } else {
                            "In Review"
                        }
                    );
                }
                manual_status(&pool, issue, "Not Started").await;
                lifecycle(&pool, &b, &ba, 3, AgentRunStatus::Succeeded, true).await;
                assert_eq!(board_status(&pool, issue).await, "Not Started");
            }
        }
    }
}

#[tokio::test]
async fn kanban_missing_start_evidence_does_not_substitute_event_arrival_time() {
    let pool = setup_pool().await;
    let (session, workspace) = insert_anchors(&pool).await;
    let issue = board_fixture(&pool, workspace).await;
    let (request, attempt) = requests(session, workspace, "no-start-evidence");
    AgentRunRecord::persist_identity_before_launch(&pool, &request, &attempt)
        .await
        .unwrap();
    AgentEventRecord::append_and_project(&pool, &event(&request, &attempt, 1))
        .await
        .unwrap();
    lifecycle(
        &pool,
        &request,
        &attempt,
        2,
        AgentRunStatus::Succeeded,
        false,
    )
    .await;
    assert_eq!(board_status(&pool, issue).await, "Not Started");
}
