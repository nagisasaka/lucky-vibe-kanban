//! Exercise both normal DB startup paths in a private process/instance.
use db::DBService;
use sqlx::SqlitePool;
use uuid::Uuid;

#[test]
fn startup_preserves_existing_history_and_user_files() {
    let temp = tempfile::tempdir().unwrap();
    for constructor in ["new", "after_connect"] {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "isolated_startup", "--nocapture"])
            .env("LVK_STARTUP_FIXTURE", constructor)
            .env("LVK_INSTANCE_DIR", temp.path().join(constructor))
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{constructor}: {}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

async fn start(constructor: &str) -> DBService {
    match constructor {
        "new" => DBService::new().await.unwrap(),
        "after_connect" => DBService::new_with_after_connect(|_| Box::pin(async { Ok(()) }))
            .await
            .unwrap(),
        _ => panic!("unknown fixture constructor"),
    }
}

async fn history(pool: &SqlitePool) -> Vec<Vec<String>> {
    let mut result = Vec::new();
    for table in [
        "repository_wiki_state",
        "repository_manifest_imports",
        "repository_change_manifests",
        "agent_runs",
        "agent_turns",
        "agent_run_attempts",
        "agent_process_registry",
        "_sqlx_migrations",
    ] {
        let columns: Vec<String> =
            sqlx::query_scalar(&format!("SELECT name FROM pragma_table_info('{table}')"))
                .fetch_all(pool)
                .await
                .unwrap();
        assert!(!columns.is_empty(), "{table}");
        let fields = columns
            .iter()
            .map(|c| format!("quote(\"{c}\")"))
            .collect::<Vec<_>>()
            .join(" || ',' || ");
        result.push(
            sqlx::query_scalar(&format!("SELECT {fields} FROM {table} ORDER BY rowid"))
                .fetch_all(pool)
                .await
                .unwrap(),
        );
    }
    result
}

#[tokio::test]
async fn isolated_startup() {
    let Ok(constructor) = std::env::var("LVK_STARTUP_FIXTURE") else {
        return;
    };
    let db = start(&constructor).await; // Empty DB must start too.
    let repo = Uuid::new_v4();
    sqlx::query("INSERT INTO repos(id,path,name,display_name) VALUES(?,'/fixture','startup-fixture','fixture')")
        .bind(repo).execute(&db.pool).await.unwrap();
    // Include absent, old, current and unknown retired marker values. Startup
    // must neither acknowledge nor interpret any of these payloads.
    for marker in [
        "",
        ",\"artifact_layout\":1",
        ",\"artifact_layout\":2",
        ",\"artifact_layout\":999",
    ] {
        let state_repo = if marker.is_empty() {
            repo
        } else {
            let id = Uuid::new_v4();
            sqlx::query("INSERT INTO repos(id,path,name,display_name) VALUES(?,?,'startup-fixture','fixture')")
                .bind(id).bind(format!("/fixture/{id}")).execute(&db.pool).await.unwrap();
            id
        };
        sqlx::query("INSERT INTO repository_wiki_state(repository_id,payload) VALUES(?,?)")
            .bind(state_repo)
            .bind(format!(
                "{{\"active_run_id\":\"{}\",\"user_field\":\"retain\"{marker}}}",
                Uuid::new_v4()
            ))
            .execute(&db.pool)
            .await
            .unwrap();
    }
    sqlx::query(
        "INSERT INTO repository_manifest_imports(repository_id,imported_count) VALUES(?,387)",
    )
    .bind(repo)
    .execute(&db.pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO repository_change_manifests(repository_id,id,workspace_id,payload) VALUES(?,?,?,'{\"historical\":true}')")
        .bind(repo).bind(Uuid::new_v4()).bind(Uuid::new_v4()).execute(&db.pool).await.unwrap();
    sqlx::raw_sql(r#"
        INSERT INTO workspaces(id,branch) VALUES(randomblob(16),'fixture');
        INSERT INTO workspace_repos(id,workspace_id,repo_id,target_branch)
            SELECT randomblob(16),w.id,r.id,'main' FROM workspaces w,repos r;
        INSERT INTO sessions(id,workspace_id) SELECT randomblob(16),id FROM workspaces;
        INSERT INTO agent_runs(id,session_id,workspace_id,request_id,idempotency_key,correlation_id,
            schema_version,payload_version,runtime_profile_id,provider_id,workspace_mode,workspace_path,status,request_envelope,created_at,updated_at)
            SELECT randomblob(16),id,workspace_id,randomblob(16),lower(hex(randomblob(16))),randomblob(16),1,1,'fixture','codex',
            'isolated_worktree','/fixture','audit_failed','{}','2026-10-26','2026-10-26' FROM sessions CROSS JOIN (SELECT 1 UNION ALL SELECT 2 UNION ALL SELECT 3);
        INSERT INTO agent_turns(id,agent_run_id,request_id,intent,input_message)
            SELECT randomblob(16),id,randomblob(16),'initial','fixture' FROM agent_runs;
        INSERT INTO agent_run_attempts(id,agent_run_id,turn_id,request_id,idempotency_key,attempt_number,mode,
            transport,schema_version,payload_version,capability_snapshot,request_envelope,status)
            SELECT randomblob(16),agent_run_id,id,randomblob(16),lower(hex(randomblob(16))),1,'launch','stdio_cli',1,1,'{}','{}','audit_failed' FROM agent_turns;
        INSERT INTO agent_process_registry(id,run_attempt_id,registry_status,created_at,updated_at)
            SELECT randomblob(16),id,'reserved','2026-10-26','2026-10-26' FROM agent_run_attempts;
    "#).execute(&db.pool).await.unwrap();
    let reserved: i64 = sqlx::query_scalar("SELECT count(*) FROM agent_process_registry WHERE registry_status='reserved' AND host_pid IS NULL AND lease_owner IS NULL")
        .fetch_one(&db.pool).await.unwrap();
    assert_eq!(reserved, 3);
    assert!(
        sqlx::query("PRAGMA foreign_key_check")
            .fetch_all(&db.pool)
            .await
            .unwrap()
            .is_empty()
    );
    let legacy =
        utils::path::shared_resources_dir("startup-fixture", repo).join("persistent/knowledge");
    // Private LVK_INSTANCE_DIR must own this path; never touch shared dev files.
    assert!(legacy.starts_with(std::env::var_os("LVK_INSTANCE_DIR").unwrap()));
    let mut files = Vec::new();
    for kind in [
        "runs",
        "source-publications",
        "wiki-setups",
        "integration-wiki-setups",
        "events",
        "completed-runs",
        "locks",
    ] {
        let dir = legacy.join(kind);
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("user-file");
        let bytes = format!("retained {kind}: not a legacy JSON record").into_bytes();
        std::fs::write(&file, &bytes).unwrap();
        files.push((file, bytes));
    }
    let lock = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(&files.last().unwrap().0)
        .unwrap();
    lock.try_lock().unwrap();
    let before = history(&db.pool).await;
    db.pool.close().await;
    let reopened = start(&constructor).await;
    assert_eq!(history(&reopened.pool).await, before);
    for (file, bytes) in &files {
        assert_eq!(std::fs::read(file).unwrap(), *bytes);
    }
    reopened.pool.close().await;
    lock.unlock().unwrap();
    drop(lock);
    // Missing old storage is equally irrelevant to startup.
    std::fs::rename(&legacy, legacy.with_file_name("retained-user-files")).unwrap();
    let reopened = start(&constructor).await;
    assert_eq!(history(&reopened.pool).await, before);
    assert!(!legacy.exists());
    reopened.pool.close().await;
}
