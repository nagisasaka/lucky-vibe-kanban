//! Project transfer uses only task-owned SQLite databases and Git fixtures.
use std::{collections::BTreeMap, path::Path};

use db::project_transfer::{export_project, import_project};
use sqlx::{
    SqlitePool,
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
};
use uuid::Uuid;

async fn database(path: &Path) -> SqlitePool {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            SqliteConnectOptions::new()
                .filename(path)
                .create_if_missing(true)
                .foreign_keys(true),
        )
        .await
        .unwrap();
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();
    pool
}
// Callers supply only fixed fixture table names; no package data becomes SQL.
async fn count(pool: &SqlitePool, table: &str) -> i64 {
    sqlx::query_scalar(sqlx::AssertSqlSafe(format!("SELECT count(*) FROM {table}")))
        .fetch_one(pool)
        .await
        .unwrap()
}
async fn project(pool: &SqlitePool, key: &str, title: &str) -> (Uuid, Uuid, Uuid) {
    let p = Uuid::new_v4();
    let s = Uuid::new_v4();
    let i = Uuid::new_v4();
    sqlx::query("INSERT INTO projects(id,name,key,issue_number_high_water) VALUES(?,?,?,2)")
        .bind(p)
        .bind(title)
        .bind(key)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO local_project_statuses(id,project_id,name,color,sort_order) VALUES(?,?,'Todo','blue',0)").bind(s).bind(p).execute(pool).await.unwrap();
    sqlx::query("INSERT INTO local_issues(id,project_id,issue_number,simple_id,status_id,title,description,sort_order,extension_metadata) VALUES(?,?,1,?,?,?,'portable body',0,'{\"fixture_only_private_metadata\":true}')")
        .bind(i).bind(p).bind(format!("{key}-1")).bind(s).bind(title).execute(pool).await.unwrap();
    (p, s, i)
}
async fn repo(pool: &SqlitePool, path: &Path) -> Uuid {
    let r = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO repos(id,path,name,display_name) VALUES(?,?,'fixture','fixture repo')",
    )
    .bind(r)
    .bind(path.to_str().unwrap())
    .execute(pool)
    .await
    .unwrap();
    r
}
async fn link(pool: &SqlitePool, p: Uuid, r: Uuid) {
    sqlx::query("INSERT INTO project_repos(id,project_id,repo_id) VALUES(?,?,?)")
        .bind(Uuid::new_v4())
        .bind(p)
        .bind(r)
        .execute(pool)
        .await
        .unwrap();
}

#[tokio::test]
async fn portable_roundtrip_reopens_without_old_root_and_preserves_owned_closure() {
    let source_dir = tempfile::tempdir().unwrap();
    let dest_dir = tempfile::tempdir().unwrap();
    let git = source_dir.path().join("git");
    std::fs::create_dir(&git).unwrap();
    assert!(
        std::process::Command::new("git")
            .args(["init", "--quiet"])
            .arg(&git)
            .status()
            .unwrap()
            .success()
    );
    std::fs::write(git.join("uncommitted.txt"), "git fixture excluded").unwrap();
    let source = database(&source_dir.path().join("source.sqlite")).await;
    let (p, s, i) = project(&source, "SRC", "target project").await;
    let (other, _, foreign) = project(&source, "OTHER", "other project excluded").await;
    let shared = repo(&source, &git).await;
    link(&source, p, shared).await;
    link(&source, other, shared).await;
    let second = Uuid::new_v4();
    sqlx::query("INSERT INTO local_issues(id,project_id,issue_number,simple_id,status_id,title,sort_order,parent_issue_id) VALUES(?,?,2,'SRC-2',?,'child',1,?)").bind(second).bind(p).bind(s).bind(i).execute(&source).await.unwrap();
    sqlx::query("INSERT INTO issue_versions(id,issue_id,name,description) VALUES(?,?,'Reviewed','reviewed body')").bind(Uuid::new_v4()).bind(i).execute(&source).await.unwrap();
    sqlx::query("UPDATE issue_versions SET revision=7 WHERE issue_id=? AND name='Reviewed'")
        .bind(i)
        .execute(&source)
        .await
        .unwrap();
    sqlx::query("UPDATE local_issues SET description='reviewed body',current_version_id=(SELECT id FROM issue_versions WHERE issue_id=? AND name='Reviewed'),current_version_revision=9 WHERE id=?").bind(i).bind(i).execute(&source).await.unwrap();
    let parent = Uuid::new_v4();
    for (id, parent_id, text) in [
        (parent, None, "comment"),
        (Uuid::new_v4(), Some(parent), "reply"),
    ] {
        sqlx::query(
            "INSERT INTO local_issue_comments(id,issue_id,parent_id,message) VALUES(?,?,?,?)",
        )
        .bind(id)
        .bind(i)
        .bind(parent_id)
        .bind(text)
        .execute(&source)
        .await
        .unwrap();
    }
    for related in [second, foreign] {
        sqlx::query("INSERT INTO local_issue_relationships(id,issue_id,related_issue_id,relationship_type) VALUES(?,?,?,'related')").bind(Uuid::new_v4()).bind(i).bind(related).execute(&source).await.unwrap();
    }
    let tag = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO local_tags(id,project_id,name,color) VALUES(?,?,'portable tag','blue')",
    )
    .bind(tag)
    .bind(p)
    .execute(&source)
    .await
    .unwrap();
    sqlx::query("INSERT INTO local_issue_tags(id,issue_id,tag_id) VALUES(?,?,?)")
        .bind(Uuid::new_v4())
        .bind(i)
        .bind(tag)
        .execute(&source)
        .await
        .unwrap();
    let attachment = Uuid::new_v4();
    let attachment_path = source_dir.path().join("private-attachment.txt");
    std::fs::write(&attachment_path, "attachment bytes excluded").unwrap();
    sqlx::query("INSERT INTO attachments(id,file_path,original_name,size_bytes,hash) VALUES(?,?,'missing.txt',25,'fixture-hash')").bind(attachment).bind(attachment_path.to_str().unwrap()).execute(&source).await.unwrap();
    sqlx::query("INSERT INTO project_attachments(project_id,attachment_id) VALUES(?,?)")
        .bind(p)
        .bind(attachment)
        .execute(&source)
        .await
        .unwrap();
    let package = dest_dir.path().join("package.sqlite");
    let export_id = export_project(&source, p, &package).await.unwrap();
    assert!(
        export_project(&source, p, &package).await.is_err(),
        "export must not overwrite"
    );
    let bytes = std::fs::read(&package).unwrap();
    let text = String::from_utf8_lossy(&bytes);
    for excluded in [
        "other project excluded",
        "fixture_only_private_metadata",
        "git fixture excluded",
        "attachment bytes excluded",
        source_dir.path().to_str().unwrap(),
        "fixture-hash",
    ] {
        assert!(
            !text.contains(excluded),
            "unexpected package content: {excluded}"
        );
    }
    let second_package = dest_dir.path().join("same-content.sqlite");
    export_project(&source, p, &second_package).await.unwrap();
    assert_eq!(count(&source, "projects").await, 2);
    source.close().await;
    source_dir.close().unwrap();
    let db_path = dest_dir.path().join("destination.sqlite");
    let dest = database(&db_path).await;
    let (existing, _, _) = project(&dest, "TAKEN", "existing destination").await;
    let prepared_repo = dest_dir.path().join("prepared-repo");
    std::fs::create_dir(&prepared_repo).unwrap();
    assert!(
        std::process::Command::new("git")
            .args(["init", "--quiet"])
            .arg(&prepared_repo)
            .status()
            .unwrap()
            .success()
    );
    std::fs::write(
        prepared_repo.join("keep.txt"),
        "destination Git stays unchanged",
    )
    .unwrap();
    let git_head = std::fs::read(prepared_repo.join(".git/HEAD")).unwrap();
    let destination_repo = repo(&dest, &prepared_repo).await;
    let mapping = BTreeMap::from([(shared, destination_repo)]);
    let org = Uuid::new_v4();
    assert!(
        import_project(&dest, &package, "TAKEN", org, &mapping)
            .await
            .is_err()
    );
    assert!(
        import_project(&dest, &package, "NEW", org, &BTreeMap::new())
            .await
            .is_err()
    );
    let report = import_project(&dest, &package, "NEW", org, &mapping)
        .await
        .unwrap();
    assert_eq!(report.export_id, export_id);
    assert_ne!(report.project_id, p);
    assert_eq!(report.external_reference_count, 1);
    assert_eq!(report.missing_attachments.len(), 1);
    assert_eq!(report.missing_attachments[0].name, "missing.txt");
    assert!(
        import_project(&dest, &package, "DUP", org, &mapping)
            .await
            .is_err()
    );
    assert!(
        import_project(&dest, &second_package, "DUP", org, &mapping)
            .await
            .is_err()
    );
    dest.close().await;
    let dest = database(&db_path).await;
    assert_eq!(count(&dest, "projects").await, 2);
    assert_eq!(count(&dest, "local_issues").await, 3);
    assert_eq!(count(&dest, "local_issue_comments").await, 2);
    assert_eq!(count(&dest, "local_issue_relationships").await, 1);
    assert_eq!(count(&dest, "local_tags").await, 1);
    assert_eq!(count(&dest, "local_issue_tags").await, 1);
    assert_eq!(count(&dest, "issue_versions").await, 4);
    assert_eq!(count(&dest, "attachments").await, 0);
    assert_eq!(count(&dest, "workspaces").await, 0);
    assert_eq!(count(&dest, "agent_runs").await, 0);
    assert_eq!(
        std::fs::read(prepared_repo.join(".git/HEAD")).unwrap(),
        git_head
    );
    assert_eq!(
        std::fs::read_to_string(prepared_repo.join("keep.txt")).unwrap(),
        "destination Git stays unchanged"
    );
    let repo_path: String = sqlx::query_scalar("SELECT path FROM repos WHERE id=?")
        .bind(destination_repo)
        .fetch_one(&dest)
        .await
        .unwrap();
    assert_eq!(repo_path, prepared_repo.to_str().unwrap());
    let old_ids: i64 = sqlx::query_scalar("SELECT count(*) FROM local_issues WHERE id IN (?,?)")
        .bind(i)
        .bind(second)
        .fetch_one(&dest)
        .await
        .unwrap();
    assert_eq!(old_ids, 0);
    let child_parent:(String,String)=sqlx::query_as("SELECT c.simple_id,p.simple_id FROM local_issues c JOIN local_issues p ON p.id=c.parent_issue_id").fetch_one(&dest).await.unwrap();
    assert_eq!(child_parent, ("NEW-2".into(), "NEW-1".into()));
    let original: String = sqlx::query_scalar("SELECT name FROM projects WHERE id=?")
        .bind(existing)
        .fetch_one(&dest)
        .await
        .unwrap();
    assert_eq!(original, "existing destination");
    let version: (String, i64, i64) = sqlx::query_as("SELECT v.name,v.revision,i.current_version_revision FROM local_issues i JOIN issue_versions v ON v.id=i.current_version_id WHERE i.simple_id='NEW-1'").fetch_one(&dest).await.unwrap();
    assert_eq!(version, ("Reviewed".into(), 7, 9));
    let persisted_report: String =
        sqlx::query_scalar("SELECT report FROM project_transfer_receipts WHERE project_id=?")
            .bind(report.project_id)
            .fetch_one(&dest)
            .await
            .unwrap();
    assert!(persisted_report.contains("missing.txt"));
    let violations = sqlx::query("PRAGMA foreign_key_check")
        .fetch_all(&dest)
        .await
        .unwrap();
    assert!(violations.is_empty());
    dest.close().await;
}

#[tokio::test]
async fn invalid_packages_and_mid_transaction_failure_leave_destination_unchanged() {
    let temp = tempfile::tempdir().unwrap();
    let source = database(&temp.path().join("src.sqlite")).await;
    let (p, _, _) = project(&source, "SRC", "portable").await;
    let package = temp.path().join("package.sqlite");
    export_project(&source, p, &package).await.unwrap();
    let dest = database(&temp.path().join("dst.sqlite")).await;
    // Fail after Project, status and Issue insertion; prove all are rolled back.
    sqlx::query("CREATE TRIGGER reject_comments BEFORE INSERT ON local_issue_comments BEGIN SELECT RAISE(ABORT,'fixture failure'); END").execute(&dest).await.unwrap();
    sqlx::query("CREATE TRIGGER reject_versions BEFORE INSERT ON issue_versions WHEN NEW.name='Initial' BEGIN SELECT RAISE(ABORT,'fixture failure'); END").execute(&dest).await.unwrap();
    let org = Uuid::new_v4();
    assert!(
        import_project(&dest, &package, "NEW", org, &BTreeMap::new())
            .await
            .is_err()
    );
    for table in [
        "projects",
        "local_project_statuses",
        "local_issues",
        "issue_versions",
        "project_transfer_receipts",
    ] {
        assert_eq!(count(&dest, table).await, 0, "partial write in {table}");
    }
    sqlx::query("DROP TRIGGER reject_versions")
        .execute(&dest)
        .await
        .unwrap();
    import_project(&dest, &package, "NEW", org, &BTreeMap::new())
        .await
        .unwrap();
    let broken = temp.path().join("broken.sqlite");
    std::fs::write(&broken, b"not a sqlite package").unwrap();
    assert!(
        import_project(&dest, &broken, "BAD", org, &BTreeMap::new())
            .await
            .is_err()
    );
    let tampered = temp.path().join("tampered.sqlite");
    std::fs::copy(&package, &tampered).unwrap();
    let tampered_pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(SqliteConnectOptions::new().filename(&tampered))
        .await
        .unwrap();
    sqlx::query("UPDATE package SET payload=json_set(payload,'$.data.issues[0].status_id',?)")
        .bind(Uuid::new_v4().to_string())
        .execute(&tampered_pool)
        .await
        .unwrap();
    tampered_pool.close().await;
    assert!(
        import_project(&dest, &tampered, "BAD", org, &BTreeMap::new())
            .await
            .is_err()
    );
    let hostile = temp.path().join("hostile.sqlite");
    let hostile_db = database(&hostile).await;
    hostile_db.close().await;
    assert!(
        import_project(&dest, &hostile, "BAD", org, &BTreeMap::new())
            .await
            .is_err()
    );
    assert_eq!(count(&dest, "projects").await, 1);
    source.close().await;
    dest.close().await;
}

#[tokio::test]
async fn rejects_unknown_envelopes_executable_schema_and_invalid_identity() {
    let temp = tempfile::tempdir().unwrap();
    let source = database(&temp.path().join("source.sqlite")).await;
    let (project, _, issue) = project(&source, "SRC", "portable").await;
    sqlx::query("INSERT INTO issue_versions(id,issue_id,name,description) VALUES(?,?,'Alternate','alternative body')").bind(Uuid::new_v4()).bind(issue).execute(&source).await.unwrap();
    let package = temp.path().join("package.sqlite");
    export_project(&source, project, &package).await.unwrap();
    let dest = database(&temp.path().join("destination.sqlite")).await;
    let org = Uuid::new_v4();
    assert!(
        import_project(&dest, &package, "NEW", Uuid::nil(), &BTreeMap::new())
            .await
            .is_err()
    );
    for (index, mutation) in [
        "UPDATE package SET payload=json_set(payload,'$.format','unknown')",
        "UPDATE package SET payload=json_set(payload,'$.version',999)",
        "UPDATE package SET payload=json_set(payload,'$.data.issues[0].created_at','invalid timestamp')",
        "UPDATE package SET payload=json_set(payload,'$.unknown_field',1)",
        "UPDATE package SET payload=json_set(payload,'$.data.issues[0].current_version_id','00000000-0000-0000-0000-000000000000')",
        "CREATE VIEW executable AS SELECT * FROM package",
        "CREATE TRIGGER executable AFTER UPDATE ON package BEGIN DELETE FROM package; END",
    ].iter().enumerate() {
        let path=temp.path().join(format!("invalid-{index}.sqlite"));
        std::fs::copy(&package,&path).unwrap();
        let edit=SqlitePoolOptions::new().max_connections(1).connect_with(SqliteConnectOptions::new().filename(&path)).await.unwrap();
        sqlx::query(*mutation).execute(&edit).await.unwrap(); edit.close().await;
        assert!(import_project(&dest,&path,"NEW",org,&BTreeMap::new()).await.is_err(),"accepted mutation {index}");
    }
    assert_eq!(count(&dest, "projects").await, 0);
    import_project(&dest, &package, "NEW", org, &BTreeMap::new())
        .await
        .unwrap();
    let reordered = temp.path().join("reordered.sqlite");
    std::fs::copy(&package, &reordered).unwrap();
    let edit = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(SqliteConnectOptions::new().filename(&reordered))
        .await
        .unwrap();
    let payload: String = sqlx::query_scalar("SELECT payload FROM package")
        .fetch_one(&edit)
        .await
        .unwrap();
    let mut envelope: serde_json::Value = serde_json::from_str(&payload).unwrap();
    envelope["export_id"] = serde_json::json!(Uuid::new_v4());
    for field in [
        "issues",
        "versions",
        "statuses",
        "comments",
        "relationships",
        "tags",
        "issue_tags",
        "repositories",
        "missing_attachments",
    ] {
        envelope["data"][field].as_array_mut().unwrap().reverse();
    }
    sqlx::query("UPDATE package SET payload=?")
        .bind(serde_json::to_string(&envelope).unwrap())
        .execute(&edit)
        .await
        .unwrap();
    edit.close().await;
    assert!(
        import_project(&dest, &reordered, "DUP", org, &BTreeMap::new())
            .await
            .is_err()
    );
    // The original identity is rejected even if its content has changed.
    let changed_identity = temp.path().join("changed-identity.sqlite");
    std::fs::copy(&package, &changed_identity).unwrap();
    let edit = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(SqliteConnectOptions::new().filename(&changed_identity))
        .await
        .unwrap();
    sqlx::query(
        "UPDATE package SET payload=json_set(payload,'$.data.project.name','identity changed')",
    )
    .execute(&edit)
    .await
    .unwrap();
    edit.close().await;
    assert!(
        import_project(&dest, &changed_identity, "DUP", org, &BTreeMap::new())
            .await
            .is_err()
    );
    // A changed snapshot adds another Project without altering the first import.
    sqlx::query("UPDATE projects SET name='changed snapshot' WHERE id=?")
        .bind(project)
        .execute(&source)
        .await
        .unwrap();
    let changed = temp.path().join("changed.sqlite");
    export_project(&source, project, &changed).await.unwrap();
    import_project(&dest, &changed, "NEXT", org, &BTreeMap::new())
        .await
        .unwrap();
    let names: Vec<String> = sqlx::query_scalar("SELECT name FROM projects ORDER BY key")
        .fetch_all(&dest)
        .await
        .unwrap();
    assert_eq!(names, vec!["portable", "changed snapshot"]);
    source.close().await;
    dest.close().await;
}

async fn completed_run(
    pool: &SqlitePool,
    project: Uuid,
    issue: Uuid,
    repo: Uuid,
    status: &str,
) -> (Uuid, Uuid, Uuid) {
    let w = Uuid::new_v4();
    let s = Uuid::new_v4();
    let r = Uuid::new_v4();
    let t = Uuid::new_v4();
    let a = Uuid::new_v4();
    sqlx::query("INSERT INTO workspaces(id,branch) VALUES(?,'fixture')")
        .bind(w)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO sessions(id,workspace_id) VALUES(?,?)")
        .bind(s)
        .bind(w)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO local_workspace_links(workspace_id,project_id,issue_id) VALUES(?,?,?)",
    )
    .bind(w)
    .bind(project)
    .bind(issue)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO workspace_repos(id,workspace_id,repo_id,target_branch) VALUES(?,?,?,'main')",
    )
    .bind(Uuid::new_v4())
    .bind(w)
    .bind(repo)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO agent_runs(id,session_id,workspace_id,request_id,idempotency_key,correlation_id,schema_version,payload_version,runtime_profile_id,provider_id,workspace_mode,workspace_path,status,request_envelope) VALUES(?,?,?,?,?,?,1,1,'fixture','fixture','shared_workspace','/old/unavailable',?,'{\"auth\":\"fixture-auth-excluded\"}')").bind(r).bind(s).bind(w).bind(Uuid::new_v4()).bind(r.to_string()).bind(Uuid::new_v4()).bind(status).execute(pool).await.unwrap();
    sqlx::query("INSERT INTO agent_turns(id,agent_run_id,request_id,intent,input_message) VALUES(?,?,?,'initial','fixture-injected-input-excluded')").bind(t).bind(r).bind(Uuid::new_v4()).execute(pool).await.unwrap();
    sqlx::query("INSERT INTO agent_run_attempts(id,agent_run_id,turn_id,request_id,idempotency_key,attempt_number,mode,transport,schema_version,payload_version,capability_snapshot,request_envelope) VALUES(?,?,?,?,?,1,'launch','in_process',1,1,'{}','{}')").bind(a).bind(r).bind(t).bind(Uuid::new_v4()).bind(a.to_string()).execute(pool).await.unwrap();
    sqlx::query("UPDATE agent_run_attempts SET status=? WHERE id=?")
        .bind(status)
        .bind(a)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO agent_run_state(agent_run_id,state_schema_version,reducer_version,last_run_attempt_id,last_run_attempt_number,last_event_sequence,status,state_json) VALUES(?,1,1,?,1,1,?,'{}')").bind(r).bind(a).bind(status).execute(pool).await.unwrap();
    use executors::runtime::*;
    let mut last_event = None;
    let mut last_sequence = 0;
    for (seq, payload) in [
        AgentEventPayload::Message {
            message: CanonicalMessage {
                message_id: Uuid::new_v4(),
                role: AgentRuntimeMessageRole::User,
                content: "completed user body".into(),
            },
            final_output: false,
        },
        AgentEventPayload::Message {
            message: CanonicalMessage {
                message_id: Uuid::new_v4(),
                role: AgentRuntimeMessageRole::Assistant,
                content: "completed final body /old/unavailable".into(),
            },
            final_output: true,
        },
        AgentEventPayload::Message {
            message: CanonicalMessage {
                message_id: Uuid::new_v4(),
                role: AgentRuntimeMessageRole::Assistant,
                content: "fixture-provider-commentary-excluded".into(),
            },
            final_output: false,
        },
        AgentEventPayload::Thinking {
            content: "fixture-thinking-excluded".into(),
        },
        AgentEventPayload::ToolCall {
            tool_call_id: None,
            tool_name: "fixture-tool".into(),
            status: AgentRuntimeToolStatus::Succeeded,
            arguments: Some(serde_json::json!({"auth":"fixture-tool-input-excluded"})),
            result: Some(serde_json::json!({"output":"fixture-tool-output-excluded"})),
        },
        AgentEventPayload::ProviderExtension {
            provider_namespace: "fixture".into(),
            provider_event: "raw".into(),
            payload: serde_json::json!({"auth":"fixture-provider-raw-excluded"}),
        },
    ]
    .into_iter()
    .enumerate()
    {
        let e = AgentEventEnvelope {
            schema_version: AGENT_EVENT_SCHEMA_VERSION,
            payload_version: AGENT_EVENT_PAYLOAD_VERSION,
            event_id: Uuid::new_v4(),
            session_id: s,
            agent_run_id: r,
            turn_id: t,
            run_attempt_id: a,
            run_attempt_number: 1,
            sequence: seq as u64 + 1,
            correlation_id: Uuid::new_v4(),
            orchestration_run_id: None,
            orchestration_node_execution_id: None,
            timestamp: chrono::Utc::now(),
            native_refs: Vec::new(),
            payload,
        };
        last_event = Some(e.event_id);
        last_sequence = e.sequence;
        sqlx::query("INSERT INTO agent_events(event_id,session_id,agent_run_id,turn_id,run_attempt_id,run_attempt_number,sequence,correlation_id,schema_version,payload_version,event_envelope) VALUES(?,?,?,?,?,1,?,?,1,1,?)").bind(e.event_id).bind(s).bind(r).bind(t).bind(a).bind(e.sequence as i64).bind(e.correlation_id).bind(serde_json::to_string(&e).unwrap()).execute(pool).await.unwrap();
    }

    let state = RunState {
        state_schema_version: RUN_STATE_SCHEMA_VERSION,
        reducer_version: RUN_STATE_REDUCER_VERSION,
        session_id: s,
        agent_run_id: r,
        turn_id: t,
        status: serde_json::from_value(serde_json::json!(status)).unwrap(),
        projection_status: ProjectionStatus::Current,
        last_run_attempt_id: Some(a),
        last_run_attempt_number: 1,
        last_event_sequence: last_sequence,
        last_event_id: last_event,
        provider_session: None,
        terminal_output: None,
        last_error: None,
        goal: None,
        unknown_event_count: 0,
        updated_at: chrono::Utc::now(),
    };
    sqlx::query("UPDATE agent_run_state SET state_json=?,last_event_sequence=?,last_event_id=? WHERE agent_run_id=?").bind(serde_json::to_string(&state).unwrap()).bind(last_sequence as i64).bind(last_event).bind(r).execute(pool).await.unwrap();
    let manifest = utils::repository_memory::ChangeManifest {
        version: 1,
        event_id: r,
        repository_id: repo,
        workspace_id: w,
        task_id: Some(issue),
        created_at: chrono::Utc::now(),
        base_commit: "a".repeat(40),
        source_commit: "b".repeat(40),
        target_branch: Some("main".into()),
        changed_paths: vec!["src/example.rs".into()],
        semantics: utils::repository_memory::SemanticChanges {
            goal: "portable goal".into(),
            summary: "portable semantic Manifest".into(),
            ..Default::default()
        },
    };
    sqlx::query("INSERT INTO repository_change_manifests(repository_id,id,workspace_id,payload) VALUES(?,?,?,?)").bind(repo).bind(r).bind(w).bind(serde_json::to_string(&manifest).unwrap()).execute(pool).await.unwrap();
    (r, w, s)
}

#[tokio::test]
async fn completed_display_history_is_opt_in_read_only_and_reopens_without_old_root() {
    use db::project_transfer::{
        export_project_with_options, inspect_package, list_imported_history,
    };
    let source_dir = tempfile::tempdir().unwrap();
    let dest_dir = tempfile::tempdir().unwrap();
    let source = database(&source_dir.path().join("source.sqlite")).await;
    let (p, _, issue) = project(&source, "HISTORY", "history source").await;
    let (other, _, other_issue) = project(&source, "OTHER", "foreign project").await;
    let rp = repo(&source, &source_dir.path().join("git")).await;
    link(&source, p, rp).await;
    link(&source, other, rp).await;
    let (run, workspace, session) = completed_run(&source, p, issue, rp, "succeeded").await;
    completed_run(&source, p, issue, rp, "running").await;
    completed_run(&source, other, other_issue, rp, "succeeded").await;
    // Existing Workspace ownership survives removal from the Project preset.
    sqlx::query("DELETE FROM project_repos WHERE project_id=? AND repo_id=?")
        .bind(p)
        .bind(rp)
        .execute(&source)
        .await
        .unwrap();
    let attachment = Uuid::new_v4();
    sqlx::query("INSERT INTO attachments(id,file_path,original_name,size_bytes,hash) VALUES(?,'/old/unavailable/workspace-only.txt','workspace-only.txt',42,'fixture-workspace-attachment')").bind(attachment).execute(&source).await.unwrap();
    sqlx::query("INSERT INTO workspace_attachments(id,workspace_id,attachment_id) VALUES(?,?,?)")
        .bind(Uuid::new_v4())
        .bind(workspace)
        .bind(attachment)
        .execute(&source)
        .await
        .unwrap();
    let package = dest_dir.path().join("history.sqlite");
    export_project_with_options(&source, p, &package, true)
        .await
        .unwrap();
    let preview = inspect_package(&package).await.unwrap();
    assert!(preview.history_included);
    assert_eq!(preview.history_count, 1);
    assert_eq!(preview.manifest_count, 1);
    assert_eq!(preview.repositories[0].source_id, rp);
    assert_eq!(preview.missing_attachments[0].name, "workspace-only.txt");
    let opt_out = dest_dir.path().join("without-conversations.sqlite");
    export_project(&source, p, &opt_out).await.unwrap();
    // Ledger terminal alone is insufficient when latest attempt is still active.
    sqlx::query("UPDATE agent_run_attempts SET status='running' WHERE agent_run_id=?")
        .bind(run)
        .execute(&source)
        .await
        .unwrap();
    let inconsistent = dest_dir.path().join("inconsistent.sqlite");
    export_project_with_options(&source, p, &inconsistent, true)
        .await
        .unwrap();
    assert_eq!(
        inspect_package(&inconsistent).await.unwrap().history_count,
        0
    );
    sqlx::query("UPDATE agent_run_attempts SET status='succeeded' WHERE agent_run_id=?")
        .bind(run)
        .execute(&source)
        .await
        .unwrap();

    for (index,mutation) in [
        "UPDATE agent_run_state SET state_json=json_set(state_json,'$.status','running') WHERE agent_run_id=?",
        "UPDATE agent_run_state SET state_json='{}' WHERE agent_run_id=?",
    ].into_iter().enumerate(){
        let before:String=sqlx::query_scalar("SELECT state_json FROM agent_run_state WHERE agent_run_id=?").bind(run).fetch_one(&source).await.unwrap();
        sqlx::query(mutation).bind(run).execute(&source).await.unwrap();
        let path=dest_dir.path().join(format!("broken-state-{index}.sqlite"));export_project_with_options(&source,p,&path,true).await.unwrap();let preview=inspect_package(&path).await.unwrap();assert_eq!(preview.history_count,0);assert_eq!(preview.manifest_count,0);
        sqlx::query("UPDATE agent_run_state SET state_json=? WHERE agent_run_id=?").bind(before).bind(run).execute(&source).await.unwrap();
    }
    let destination_path = dest_dir.path().join("destination.sqlite");
    let destination = database(&destination_path).await;
    let dest_repo = repo(&destination, &dest_dir.path().join("git")).await;
    let mapping = BTreeMap::from([(rp, dest_repo)]);
    let imported = import_project(&destination, &package, "PORT", Uuid::new_v4(), &mapping)
        .await
        .unwrap();
    let excluded = import_project(&destination, &opt_out, "NOCHAT", Uuid::new_v4(), &mapping)
        .await
        .unwrap();
    assert_eq!(count(&destination, "agent_runs").await, 0);
    assert_eq!(count(&destination, "agent_process_registry").await, 0);
    source.close().await;
    source_dir.close().unwrap();
    destination.close().await;
    let destination = database(&destination_path).await;
    let history = list_imported_history(&destination, imported.project_id)
        .await
        .unwrap();
    assert_eq!(history.len(), 1);
    let h = &history[0];
    assert_ne!(h.run_id, run);
    assert_ne!(h.workspace_id, workspace);
    assert_ne!(h.session_id, session);
    assert_ne!(h.issue_id, issue);
    assert_eq!(h.messages.len(), 2);
    assert_eq!(
        h.messages[1].content,
        "completed final body /old/unavailable"
    );
    assert_eq!(h.manifests[0].id, h.run_id);
    assert_eq!(h.manifests[0].repo_id, dest_repo);
    assert_eq!(
        h.manifests[0].semantics.summary,
        "portable semantic Manifest"
    );
    let json = serde_json::to_string(&history).unwrap();
    for excluded in [
        "fixture-auth-excluded",
        "fixture-injected-input-excluded",
        "fixture-provider-commentary-excluded",
        "fixture-thinking-excluded",
        "fixture-tool-input-excluded",
        "fixture-tool-output-excluded",
        "fixture-provider-raw-excluded",
    ] {
        assert!(!json.contains(excluded));
    }
    let no_chat = list_imported_history(&destination, excluded.project_id)
        .await
        .unwrap();
    assert_eq!(no_chat.len(), 1);
    assert!(no_chat[0].messages.is_empty());
    assert_eq!(no_chat[0].manifests.len(), 1);
    // A re-export remains display-only and can be read without original sources.
    let reexport = dest_dir.path().join("reexport.sqlite");
    export_project_with_options(&destination, imported.project_id, &reexport, true)
        .await
        .unwrap();
    let reexport_preview = inspect_package(&reexport).await.unwrap();
    assert_eq!(reexport_preview.history_count, 1);
    assert_eq!(
        reexport_preview.missing_attachments[0].name,
        "workspace-only.txt"
    );
}

#[tokio::test]
async fn portable_history_rejects_untrusted_closure_and_preserves_v1_duplicate_receipts() {
    use db::project_transfer::export_project_with_options;
    let temp = tempfile::tempdir().unwrap();
    let source = database(&temp.path().join("source.sqlite")).await;
    let (p, _, issue) = project(&source, "OLD", "v1 source").await;
    let rp = repo(&source, &temp.path().join("git")).await;
    link(&source, p, rp).await;
    completed_run(&source, p, issue, rp, "succeeded").await;
    let package = temp.path().join("history.sqlite");
    export_project_with_options(&source, p, &package, true)
        .await
        .unwrap();
    let dest = database(&temp.path().join("destination.sqlite")).await;
    let dr = repo(&dest, &temp.path().join("dest-git")).await;
    let map = BTreeMap::from([(rp, dr)]);
    let org = Uuid::new_v4();
    for (index,mutation) in [
        "UPDATE package SET payload=json_set(payload,'$.data.history[0].issue_id','00000000-0000-0000-0000-000000000001')",
        "UPDATE package SET payload=json_set(payload,'$.data.history[0].run_id',json_extract(payload,'$.data.issues[0].id'))",
        "UPDATE package SET payload=json_set(payload,'$.data.history[0].session_id',json_extract(payload,'$.data.history[0].workspace_id'))",
        "UPDATE package SET payload=json_set(payload,'$.data.history[0].status','running')",
        "UPDATE package SET payload=json_set(payload,'$.data.history[0].messages[0].role','tool')",
        "UPDATE package SET payload=json_set(payload,'$.data.history[0].messages[0].auth','fixture-unknown-auth')",
        "UPDATE package SET payload=json_set(payload,'$.data.history[0].manifests[0].repo_id','00000000-0000-0000-0000-000000000001')",
        "UPDATE package SET payload=json_set(payload,'$.data.history[0].manifests[0].semantics.auth','fixture-unknown-auth')",
        "UPDATE package SET payload=json_set(payload,'$.data.history[0].manifests[0].changed_paths[0]','../escape')",
    ].into_iter().enumerate(){let path=temp.path().join(format!("bad-history-{index}.sqlite"));std::fs::copy(&package,&path).unwrap();let edit=SqlitePoolOptions::new().max_connections(1).connect_with(SqliteConnectOptions::new().filename(&path)).await.unwrap();sqlx::query(mutation).execute(&edit).await.unwrap();edit.close().await;assert!(import_project(&dest,&path,"BAD",org,&map).await.is_err(),"accepted mutation {index}");assert_eq!(count(&dest,"projects").await,0);assert_eq!(count(&dest,"project_transfer_history").await,0);}
    // Failure after the normal Project/Issue inserts still rolls back everything.
    sqlx::query("CREATE TRIGGER fail_history BEFORE INSERT ON project_transfer_history BEGIN SELECT RAISE(ABORT,'fixture history failure'); END").execute(&dest).await.unwrap();
    assert!(
        import_project(&dest, &package, "BAD", org, &map)
            .await
            .is_err()
    );
    assert_eq!(count(&dest, "projects").await, 0);
    assert_eq!(count(&dest, "project_transfer_receipts").await, 0);
    sqlx::query("DROP TRIGGER fail_history")
        .execute(&dest)
        .await
        .unwrap();
    // Build a genuine v1-shaped no-history package and verify the old fingerprint.
    let v1 = temp.path().join("v1.sqlite");
    export_project(&source, p, &v1).await.unwrap();
    let edit = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(SqliteConnectOptions::new().filename(&v1))
        .await
        .unwrap();
    sqlx::query("UPDATE package SET payload=json_remove(json_set(payload,'$.version',1),'$.data.history','$.data.history_included')").execute(&edit).await.unwrap();
    let payload: String = sqlx::query_scalar("SELECT payload FROM package")
        .fetch_one(&edit)
        .await
        .unwrap();
    // json_remove retains the original ordered serialized Snapshot bytes.
    let start = payload.find("\"data\":").unwrap() + 7;
    let legacy_data = &payload[start..payload.len() - 1];
    use sha2::{Digest, Sha256};
    let old_hash = format!("{:x}", Sha256::digest(legacy_data.as_bytes()));
    edit.close().await;
    let report = import_project(&dest, &v1, "OLDNEW", org, &map)
        .await
        .unwrap();
    let stored_hash: String =
        sqlx::query_scalar("SELECT content_hash FROM project_transfer_receipts WHERE export_id=?")
            .bind(report.export_id)
            .fetch_one(&dest)
            .await
            .unwrap();
    assert_eq!(stored_hash, old_hash);
    let retry = temp.path().join("v1-retry.sqlite");
    std::fs::copy(&v1, &retry).unwrap();
    let edit = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(SqliteConnectOptions::new().filename(&retry))
        .await
        .unwrap();
    sqlx::query("UPDATE package SET payload=json_set(payload,'$.export_id',?)")
        .bind(Uuid::new_v4().to_string())
        .execute(&edit)
        .await
        .unwrap();
    edit.close().await;
    assert!(
        import_project(&dest, &retry, "DUP", org, &map)
            .await
            .is_err()
    );
    assert_eq!(count(&dest, "projects").await, 1);
}
