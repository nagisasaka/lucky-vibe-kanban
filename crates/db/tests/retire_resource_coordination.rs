use std::borrow::Cow;

use sqlx::{migrate::Migrator, sqlite::SqlitePoolOptions};

#[tokio::test]
async fn retires_populated_resources_and_runner_preserving_normal_work() {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::query("PRAGMA foreign_keys=ON")
        .execute(&pool)
        .await
        .unwrap();
    let all = sqlx::migrate!("./migrations");
    let previous = Migrator {
        migrations: Cow::Owned(
            all.iter()
                .filter(|m| m.version < 20261122000000)
                .cloned()
                .collect(),
        ),
        ..Migrator::DEFAULT
    };
    previous.run(&pool).await.unwrap();
    sqlx::raw_sql(r#"
        INSERT INTO workspaces(id,branch) VALUES(X'01','normal');
        INSERT INTO sessions(id,workspace_id) VALUES(X'02',X'01');
        INSERT INTO execution_processes(id,session_id,run_reason) VALUES(X'03',X'02','setupscript');
        INSERT INTO execution_processes(id,session_id,run_reason) VALUES(X'04',X'02','resourcecommand');
        INSERT INTO workspaces(id,branch,usage,execution_owner) VALUES(X'05','mediation','execution_only','{"kind":"resource_mediation"}');
        INSERT INTO sessions(id,workspace_id) VALUES(X'06',X'05');
        INSERT INTO agent_runs(id,session_id,workspace_id,request_id,idempotency_key,correlation_id,
            schema_version,payload_version,runtime_profile_id,provider_id,workspace_mode,workspace_path,status,request_envelope)
            VALUES(X'0D',X'06',X'05',X'0E','mediator',X'09',1,1,'CODEX','codex','shared_workspace','/fixture','succeeded','{}');
        INSERT INTO orchestration_runs(id,request_id,idempotency_key,correlation_id,product_kind,source_definition_id,source_definition_version,plan_schema_version,plan_snapshot)
            VALUES(X'09',X'0F','mediation',X'09','resource_mediation',X'09','1',1,'{}');
        INSERT INTO orchestration_node_executions(id,orchestration_run_id,node_key,stable_order) VALUES(X'10',X'09','mediation',0);
        INSERT INTO orchestration_agent_run_links(id,orchestration_run_id,node_execution_id,agent_run_id,dispatch_idempotency_key) VALUES(X'11',X'09',X'10',X'0D','mediation-dispatch');
        INSERT INTO shared_resources(id,resource_key,name,description,state) VALUES(X'07','fixture','Fixture','test','idle');
        INSERT INTO resource_operations(id,workspace_id,session_id,spec,status) VALUES(X'08',X'01',X'02','{}','recovery_required');
        INSERT INTO resource_holders(resource_id,operation_id,fence) VALUES(X'07',X'08',1);
        INSERT INTO resource_mediations(id,trigger_operation_id,snapshot,status,workspace_id,session_id,agent_run_id) VALUES(X'09',X'08','{}','failed',X'05',X'06',X'0D');
        INSERT INTO bridge_runners(id,name,token_hash,execution_resource_id) VALUES(X'0A','runner','hash',X'07');
        INSERT INTO bridge_commands(id,operation_id,kind,script) VALUES(X'0B',X'08','start','true');
        INSERT INTO bridge_settlements(operation_id,runtime_id,evidence) VALUES(X'08',X'0C','done');
    "#).execute(&pool).await.unwrap();
    all.run(&pool).await.unwrap();
    all.run(&pool).await.unwrap();
    let retired: i64 = sqlx::query_scalar("SELECT count(*) FROM sqlite_master WHERE name IN ('shared_resources','resource_operations','resource_holders','resource_events','resource_mediations','bridge_runners','bridge_sources','bridge_commands','bridge_settlements','resource_workspace_change','resource_workspace_delete','resource_session_delete','resource_process_delete')")
        .fetch_one(&pool).await.unwrap();
    assert_eq!(retired, 0);
    for (table, id) in [
        ("workspaces", "01"),
        ("sessions", "02"),
        ("execution_processes", "03"),
    ] {
        let count: i64 = sqlx::query_scalar(&format!(
            "SELECT count(*) FROM {table} WHERE hex(id)='{id}'"
        ))
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(count, 1, "{table}");
    }
    let retired_workspace: i64 =
        sqlx::query_scalar("SELECT count(*) FROM workspaces WHERE id=X'05'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(retired_workspace, 0);
    assert!(sqlx::query("INSERT INTO execution_processes(id,session_id,run_reason) VALUES(X'FF',X'02','resourcecommand')").execute(&pool).await.is_err());
    assert!(sqlx::query("INSERT INTO orchestration_runs(id,request_id,idempotency_key,correlation_id,product_kind,source_definition_id,source_definition_version,plan_schema_version,plan_snapshot) VALUES(X'AA',X'AB','retired',X'AC','resource_mediation',X'AD','1',1,'{}')").execute(&pool).await.is_err());
    let violations = sqlx::query("PRAGMA foreign_key_check")
        .fetch_all(&pool)
        .await
        .unwrap();
    assert!(violations.is_empty());
    let trigger: i64 = sqlx::query_scalar("SELECT count(*) FROM sqlite_master WHERE type='trigger' AND name='integration_script_admission'")
        .fetch_one(&pool).await.unwrap();
    assert_eq!(trigger, 1);
}

#[tokio::test]
async fn new_database_starts_without_retired_tables() {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    db::run_migrations(&pool).await.unwrap();
    let retired: i64 = sqlx::query_scalar("SELECT count(*) FROM sqlite_master WHERE name IN ('shared_resources','resource_operations','resource_mediations','bridge_runners','bridge_commands')")
        .fetch_one(&pool).await.unwrap();
    assert_eq!(retired, 0);
}

#[tokio::test]
async fn instance_upgrade_removes_only_retired_runtime_files() {
    use sqlx::sqlite::SqliteConnectOptions;
    let instance = match std::env::var_os("LVK_RETIRED_FILES_TEST_INSTANCE") {
        Some(path) => std::path::PathBuf::from(path),
        None => {
            let temp = tempfile::tempdir().unwrap();
            let output = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "instance_upgrade_removes_only_retired_runtime_files",
                    "--nocapture",
                ])
                .env("LVK_RETIRED_FILES_TEST_INSTANCE", temp.path())
                .env("LVK_INSTANCE_DIR", temp.path())
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            return;
        }
    };
    let root_buf = instance.join("data");
    std::fs::create_dir_all(&root_buf).unwrap();
    let root = root_buf.as_path();
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            SqliteConnectOptions::new()
                .filename(root.join("db.v2.sqlite"))
                .create_if_missing(true)
                .foreign_keys(true),
        )
        .await
        .unwrap();
    let all = sqlx::migrate!("./migrations");
    let previous = Migrator {
        migrations: Cow::Owned(
            all.iter()
                .filter(|m| m.version < 20261122000000)
                .cloned()
                .collect(),
        ),
        ..Migrator::DEFAULT
    };
    previous.run(&pool).await.unwrap();
    let normal_ws = uuid::Uuid::new_v4();
    let normal_session = uuid::Uuid::new_v4();
    let mediator_ws = uuid::Uuid::new_v4();
    let mediator_session = uuid::Uuid::new_v4();
    let operation = uuid::Uuid::new_v4();
    let mediation = uuid::Uuid::new_v4();
    let inconsistent_mediation = uuid::Uuid::new_v4();
    let interrupted_mediation = uuid::Uuid::new_v4();
    let interrupted_workspace = uuid::Uuid::new_v4();
    let orphan_session = uuid::Uuid::new_v4();
    let orphan_process = uuid::Uuid::new_v4();
    let orphan_operation = uuid::Uuid::new_v4();
    let unattached_operation = uuid::Uuid::new_v4();
    let orphan_mediation = uuid::Uuid::new_v4();
    let retired_process = uuid::Uuid::new_v4();
    let normal_process = uuid::Uuid::new_v4();
    sqlx::query(r#"INSERT INTO workspaces(id,branch,usage,execution_owner) VALUES(?,'normal','interactive',NULL),(?,'mediator','execution_only','{"kind":"resource_mediation"}')"#)
        .bind(normal_ws)
        .bind(mediator_ws)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO sessions(id,workspace_id) VALUES(?,?),(?,?)")
        .bind(normal_session)
        .bind(normal_ws)
        .bind(mediator_session)
        .bind(mediator_ws)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO execution_processes(id,session_id,run_reason) VALUES(?,?,'resourcecommand'),(?,?,'setupscript')")
        .bind(retired_process).bind(normal_session).bind(normal_process).bind(normal_session).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO resource_operations(id,workspace_id,session_id,spec,status) VALUES(?,?,?,'{}','succeeded')")
        .bind(operation).bind(normal_ws).bind(normal_session).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO resource_mediations(id,trigger_operation_id,snapshot,status,workspace_id,session_id) VALUES(?,?,'{}','applied',?,?)")
        .bind(mediation).bind(operation).bind(mediator_ws).bind(mediator_session).execute(&pool).await.unwrap();

    // A stale reference must not turn a normal Workspace into a mediation owner.
    sqlx::query("INSERT INTO resource_mediations(id,trigger_operation_id,snapshot,status,workspace_id,session_id) VALUES(?,?,'{}','failed',?,?)")
        .bind(inconsistent_mediation).bind(operation).bind(normal_ws).bind(normal_session).execute(&pool).await.unwrap();

    // Completed legacy operations can outlive deleted Session/process rows.
    sqlx::query("INSERT INTO resource_operations(id,workspace_id,session_id,spec,status,process_id) VALUES(?,?,?,'{}','succeeded',?)")
        .bind(orphan_operation).bind(normal_ws).bind(normal_session).bind(orphan_process).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO resource_operations(id,workspace_id,session_id,spec,status) VALUES(?,?,?,'{}','succeeded')")
        .bind(unattached_operation).bind(normal_ws).bind(normal_session).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO resource_mediations(id,trigger_operation_id,snapshot,status,session_id) VALUES(?,?,'{}','failed',?)")
        .bind(orphan_mediation).bind(orphan_operation).bind(orphan_session).execute(&pool).await.unwrap();

    let interrupted_dir = root
        .join("resource-mediation")
        .join(interrupted_mediation.to_string());
    sqlx::query("INSERT INTO workspaces(id,branch,workspace_kind,container_ref) VALUES(?,'interrupted','direct_folder',?)")
        .bind(interrupted_workspace).bind(interrupted_dir.to_string_lossy().as_ref()).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO resource_mediations(id,trigger_operation_id,snapshot,status) VALUES(?,?,'{}','preparing')")
        .bind(interrupted_mediation).bind(operation).execute(&pool).await.unwrap();
    let retired_dir = root.join("resource-mediation").join(mediation.to_string());
    let retired_audit = utils::native_audit::session_dir_in_root(root, mediator_session);
    let retired_session =
        utils::execution_logs::process_logs_session_dir_in_root(root, mediator_session);
    let retired_log =
        utils::execution_logs::process_log_file_path_in_root(root, normal_session, retired_process);
    let normal_log =
        utils::execution_logs::process_log_file_path_in_root(root, normal_session, normal_process);
    let normal_audit = utils::native_audit::session_dir_in_root(root, normal_session);
    let unattached_log = utils::execution_logs::process_log_file_path_in_root(
        root,
        normal_session,
        unattached_operation,
    );
    let orphan_audit = utils::native_audit::session_dir_in_root(root, orphan_session);
    let orphan_logs = utils::execution_logs::process_logs_session_dir_in_root(root, orphan_session);
    let orphan_log =
        utils::execution_logs::process_log_file_path_in_root(root, normal_session, orphan_process);
    for dir in [
        &retired_dir,
        &interrupted_dir,
        &retired_audit,
        &retired_session,
        &normal_audit,
        &orphan_audit,
        &orphan_logs,
    ] {
        std::fs::create_dir_all(dir).unwrap();
        std::fs::write(dir.join("fixture"), "data").unwrap();
    }
    for file in [&retired_log, &normal_log, &orphan_log, &unattached_log] {
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(file, "data").unwrap();
    }
    // A checksum mismatch must stop before deleting any runtime evidence.
    sqlx::query("UPDATE _sqlx_migrations SET checksum=X'00' WHERE version=20261006000000")
        .execute(&pool)
        .await
        .unwrap();
    assert!(db::run_migrations(&pool).await.is_err());
    assert!(retired_log.exists() && retired_dir.exists());
    let original = all.iter().find(|m| m.version == 20261006000000).unwrap();
    sqlx::query("UPDATE _sqlx_migrations SET checksum=? WHERE version=20261006000000")
        .bind(original.checksum.as_ref())
        .execute(&pool)
        .await
        .unwrap();

    db::run_migrations(&pool).await.unwrap();
    db::run_migrations(&pool).await.unwrap();
    for path in [
        &retired_dir,
        &retired_audit,
        &retired_session,
        &retired_log,
        &orphan_audit,
        &orphan_logs,
        &orphan_log,
        &unattached_log,
    ] {
        assert!(!path.exists(), "{}", path.display());
    }
    assert!(
        sqlx::query("SELECT id FROM workspaces WHERE id=?")
            .bind(normal_ws)
            .fetch_optional(&pool)
            .await
            .unwrap()
            .is_some()
    );
    assert!(
        sqlx::query("SELECT id FROM workspaces WHERE id=?")
            .bind(interrupted_workspace)
            .fetch_optional(&pool)
            .await
            .unwrap()
            .is_none()
    );
    assert!(!interrupted_dir.exists());
    assert!(normal_log.is_file());
    assert!(normal_audit.join("fixture").is_file());
    assert!(
        sqlx::query("PRAGMA foreign_key_check")
            .fetch_all(&pool)
            .await
            .unwrap()
            .is_empty()
    );
}
