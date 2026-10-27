// Isolated upgrade and lifecycle tests; no production data or runtime paths.
async fn reservation_fixture() -> (SqlitePool, AgentRunRequestEnvelope, RunAttemptRequest) {
    let pool = setup_pool().await;
    sqlx::raw_sql(include_str!(
        "../../migrations/20260922010000_host_replay_identity.sql"
    ))
    .execute(&pool)
    .await
    .unwrap();
    let (session, workspace) = insert_anchors(&pool).await;
    let (request, attempt) = requests(session, workspace, "reservation-terminal");
    AgentRunRecord::persist_identity_before_launch(&pool, &request, &attempt)
        .await
        .unwrap();
    (pool, request, attempt)
}

async fn upgrade_reservations(pool: &SqlitePool) {
    let mut tx = pool.begin().await.unwrap();
    sqlx::raw_sql(include_str!(
        "../../migrations/20261027010000_unstarted_reservation_terminal.sql"
    ))
    .execute(&mut *tx)
    .await
    .unwrap();
    tx.commit().await.unwrap();
    let violations = sqlx::query("PRAGMA foreign_key_check")
        .fetch_all(pool)
        .await
        .unwrap();
    assert!(violations.is_empty());
}

#[tokio::test]
async fn reservation_upgrade_retains_historical_terminal_identity_and_idempotency() {
    let (pool, request, attempt) = reservation_fixture().await;
    sqlx::query("UPDATE agent_run_attempts SET status='audit_failed'")
        .execute(&pool)
        .await
        .unwrap();
    let before: (Uuid, Uuid, String, String, String) = sqlx::query_as(
        "SELECT id,run_attempt_id,registry_status,created_at,updated_at FROM agent_process_registry")
        .fetch_one(&pool).await.unwrap();
    upgrade_reservations(&pool).await;
    let after = sqlx::query_as::<_, (Uuid, Uuid, String, String, String)>(
        "SELECT id,run_attempt_id,registry_status,created_at,updated_at FROM agent_process_registry")
        .fetch_one(&pool).await.unwrap();
    assert_eq!(
        before, after,
        "upgrade must not silently repair legacy reservations"
    );
    AgentRunRecord::persist_identity_before_launch(&pool, &request, &attempt)
        .await
        .unwrap();
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM agent_process_registry")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1);
}

#[tokio::test]
async fn reservation_upgrade_preserves_active_host_and_process_evidence() {
    for status in ["reserved", "spawned", "running", "unreachable", "exited"] {
        let (pool, _, _) = reservation_fixture().await;
        sqlx::query("UPDATE agent_process_registry SET registry_status=?,host_endpoint='fixture',host_token='fixture',host_instance_id='fixture',host_pid=12,host_protocol_version=2,host_start_identity='fixture',last_host_event_sequence=7,pid=?,process_started_at=?,observed_exited_at=?,exit_code=?,launch_requested=1,starting_write_attempts=2")
            .bind(status).bind((status != "reserved").then_some(123_i64))
            .bind((status != "reserved").then(Utc::now))
            .bind((status == "exited").then(Utc::now))
            .bind((status == "exited").then_some(0_i64))
            .execute(&pool).await.unwrap();
        // Compare all fields as SQLite values, including identity and receipts.
        let snapshot = "SELECT json_object('id',hex(id),'attempt',hex(run_attempt_id),'status',registry_status,'endpoint',host_endpoint,'token',host_token,'instance',host_instance_id,'host_pid',host_pid,'version',host_protocol_version,'identity',host_start_identity,'cursor',last_host_event_sequence,'pid',pid,'started',process_started_at,'exited',observed_exited_at,'exit_code',exit_code,'launch',launch_requested,'budget',starting_write_attempts,'pending',starting_failure_pending,'created',created_at,'updated',updated_at) FROM agent_process_registry";
        let before: String = sqlx::query_scalar(snapshot).fetch_one(&pool).await.unwrap();
        upgrade_reservations(&pool).await;
        let after: String = sqlx::query_scalar(snapshot).fetch_one(&pool).await.unwrap();
        assert_eq!(before, after, "{status}");
    }
}

#[tokio::test]
async fn reservation_terminal_projection_releases_only_proven_unstarted_attempts() {
    for evidence in [
        "none",
        "success",
        "host",
        "cursor",
        "identity",
        "running",
        "unreachable",
        "audit",
    ] {
        let (pool, request, attempt) = reservation_fixture().await;
        upgrade_reservations(&pool).await;
        match evidence {
            "host" => {
                sqlx::query("UPDATE agent_process_registry SET host_endpoint='fixture',host_token='fixture'")
                .execute(&pool).await.unwrap();
            }
            "cursor" => {
                sqlx::query("UPDATE agent_process_registry SET last_host_event_sequence=1")
                    .execute(&pool)
                    .await
                    .unwrap();
            }
            "identity" => {
                sqlx::query("UPDATE agent_process_registry SET host_start_identity='fixture'")
                    .execute(&pool)
                    .await
                    .unwrap();
            }
            "running" | "unreachable" => {
                sqlx::query("UPDATE agent_process_registry SET registry_status=?,pid=123,process_started_at=?")
                .bind(evidence).bind(Utc::now()).execute(&pool).await.unwrap();
            }
            "audit" => {
                sqlx::query("INSERT INTO native_audit_streams(id,session_id,agent_run_id,run_attempt_id,audit_schema_version,adapter_version,mapper_version,manifest_relative_path,frames_relative_path) VALUES(?,?,?,?,1,'fixture','fixture','fixture','fixture')")
                .bind(Uuid::new_v4()).bind(request.session_id).bind(request.agent_run_id).bind(attempt.run_attempt_id)
                .execute(&pool).await.unwrap();
            }
            _ => {}
        }
        let mut terminal = event(&request, &attempt, 1);
        terminal.payload = AgentEventPayload::LifecycleChanged {
            status: if evidence == "success" {
                AgentRunStatus::Succeeded
            } else {
                AgentRunStatus::Cancelled
            },
        };
        AgentEventRecord::append_and_project(&pool, &terminal)
            .await
            .unwrap();
        AgentEventRecord::append_and_project(&pool, &terminal)
            .await
            .unwrap();
        let status: String =
            sqlx::query_scalar("SELECT registry_status FROM agent_process_registry")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(status == "not_started", evidence == "none", "{evidence}");
        if evidence == "none" {
            assert!(
                AgentRunRecord::reserve_process_host(
                    &pool,
                    attempt.run_attempt_id,
                    "fixture",
                    "fixture",
                    Uuid::new_v4()
                )
                .await
                .is_err()
            );
            assert!(
                sqlx::query("UPDATE agent_process_registry SET pid=123")
                    .execute(&pool)
                    .await
                    .is_err(),
                "not_started cannot invent process identity"
            );
            AgentRunRecord::persist_identity_before_launch(&pool, &request, &attempt)
                .await
                .unwrap();
        }
    }
}

#[tokio::test]
async fn reservation_release_failure_rolls_back_terminal_event_and_projection() {
    let (pool, request, attempt) = reservation_fixture().await;
    upgrade_reservations(&pool).await;
    sqlx::raw_sql("CREATE TRIGGER reject_release BEFORE UPDATE ON agent_process_registry WHEN NEW.registry_status='not_started' BEGIN SELECT RAISE(ABORT,'fixture release failure'); END;")
        .execute(&pool).await.unwrap();
    let mut terminal = event(&request, &attempt, 1);
    terminal.payload = AgentEventPayload::LifecycleChanged {
        status: AgentRunStatus::Failed,
    };
    assert!(
        AgentEventRecord::append_and_project(&pool, &terminal)
            .await
            .is_err()
    );
    let status: String = sqlx::query_scalar("SELECT status FROM agent_run_attempts")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(status, "pending");
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM agent_events")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
    sqlx::raw_sql("DROP TRIGGER reject_release")
        .execute(&pool)
        .await
        .unwrap();
    AgentEventRecord::append_and_project(&pool, &terminal)
        .await
        .unwrap();
    let status: String = sqlx::query_scalar("SELECT registry_status FROM agent_process_registry")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(status, "not_started");
}
