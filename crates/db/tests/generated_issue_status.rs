//! Isolated SQLite regression tests for the durable generated-Issue projection.
use serde_json::json;
use sqlx::SqlitePool;
use uuid::Uuid;

struct Fixture {
    pool: SqlitePool,
    project: Uuid,
    issue: Uuid,
    workspace: Uuid,
    repo: Uuid,
    run: Uuid,
    session: Uuid,
    kind: &'static str,
}
impl Fixture {
    async fn new(kind: &'static str) -> Self {
        Self::new_at(kind, false).await
    }
    async fn new_at(kind: &'static str, historical: bool) -> Self {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        let mut migrations = sqlx::migrate!("./migrations");
        if historical {
            migrations.migrations = std::borrow::Cow::Owned(
                migrations
                    .iter()
                    .filter(|m| m.version < 20261027090000)
                    .cloned()
                    .collect(),
            );
        }
        migrations.run(&pool).await.unwrap();
        let f = Self {
            pool,
            project: Uuid::new_v4(),
            issue: Uuid::new_v4(),
            workspace: Uuid::new_v4(),
            repo: Uuid::new_v4(),
            run: Uuid::new_v4(),
            session: Uuid::new_v4(),
            kind,
        };
        sqlx::query("INSERT INTO projects(id,name) VALUES(?,'fixture')")
            .bind(f.project)
            .execute(&f.pool)
            .await
            .unwrap();
        for name in ["Todo", "In Progress", "In Review", "Done", "Cancelled"] {
            sqlx::query("INSERT INTO local_project_statuses(id,project_id,name,color,sort_order) VALUES(?,?,?,'blue',0)")
                .bind(Uuid::new_v4()).bind(f.project).bind(name).execute(&f.pool).await.unwrap();
        }
        sqlx::query(
            "INSERT INTO repos(id,path,name,display_name) VALUES(?,'/fixture','fixture','fixture')",
        )
        .bind(f.repo)
        .execute(&f.pool)
        .await
        .unwrap();
        sqlx::query("INSERT INTO workspaces(id,branch,usage,execution_owner) VALUES(?,'fixture','execution_only',?)")
            .bind(f.workspace).bind(json!({"kind":kind,"repository_id":f.repo,"run_id":f.run}).to_string())
            .execute(&f.pool).await.unwrap();
        sqlx::query("INSERT INTO sessions(id,workspace_id) VALUES(?,?)")
            .bind(f.session)
            .bind(f.workspace)
            .execute(&f.pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO local_issues(id,project_id,issue_number,simple_id,status_id,title,sort_order) SELECT ?,?,1,'TEST-1',id,'system',0 FROM local_project_statuses WHERE project_id=? AND name='Todo'")
            .bind(f.issue).bind(f.project).bind(f.project).execute(&f.pool).await.unwrap();
        sqlx::query(
            "INSERT INTO local_workspace_links(workspace_id,project_id,issue_id) VALUES(?,?,?)",
        )
        .bind(f.workspace)
        .bind(f.project)
        .bind(f.issue)
        .execute(&f.pool)
        .await
        .unwrap();
        if kind == "integration" {
            sqlx::query("INSERT INTO integration_runs(id,request_key,project_id,repository_id,storage_identity,target_ref,status,workspace_id,session_id,payload) VALUES(?,?,?,?,'/fixture','refs/heads/main','queued',?,?,?)")
                .bind(f.run).bind(f.run.to_string()).bind(f.project).bind(f.repo).bind(f.workspace).bind(f.session)
                .bind(serde_json::to_string(&db::models::integration::IntegrationPayload { card_id: Some(f.issue), ..Default::default() }).unwrap()).execute(&f.pool).await.unwrap();
        } else if kind == "openwiki_sync" {
            f.agent(f.run).await;
        } else {
            let workflow = Uuid::new_v4();
            sqlx::query("INSERT INTO workflows(id,name,source,graph_json) VALUES(?,'fixture','system','{}')")
                .bind(workflow).execute(&f.pool).await.unwrap();
            sqlx::query("INSERT INTO workflow_runs(id,workflow_id,repository_id,workspace_id,trigger_source,input_text,status) VALUES(?,?,?,?,'openwiki_bootstrap','','running')")
                .bind(f.run).bind(workflow).bind(f.repo).bind(f.workspace).execute(&f.pool).await.unwrap();
        }
        f.register().await;
        f
    }
    async fn register(&self) {
        sqlx::query("INSERT INTO generated_issue_workspaces(workspace_id,issue_id,project_id,owner_kind,repository_id,owner_run_id) VALUES(?,?,?,?,?,?)")
            .bind(self.workspace).bind(self.issue).bind(self.project).bind(self.kind).bind(self.repo)
            .bind(self.run.to_string()).execute(&self.pool).await.unwrap();
    }
    async fn agent(&self, run: Uuid) {
        sqlx::query("INSERT INTO agent_runs(id,session_id,workspace_id,request_id,idempotency_key,correlation_id,schema_version,payload_version,runtime_profile_id,provider_id,workspace_mode,workspace_path,status,request_envelope) VALUES(?,?,?,?,?,?,1,1,'CODEX','codex','isolated_worktree','/fixture','running','{}')")
            .bind(run).bind(self.session).bind(self.workspace).bind(Uuid::new_v4()).bind(run.to_string())
            .bind(self.run).execute(&self.pool).await.unwrap();
    }
    async fn assert_status(&self, expected: &str) {
        let (name, archived): (String, bool) = sqlx::query_as("SELECT s.name,i.archived FROM local_issues i JOIN local_project_statuses s ON s.id=i.status_id WHERE i.id=?")
            .bind(self.issue).fetch_one(&self.pool).await.unwrap();
        assert_eq!(name, expected);
        assert!(!archived, "status projection must never perform archive");
    }
    async fn product_status(&self, status: &str) {
        let table = if self.kind == "integration" {
            "integration_runs"
        } else {
            "workflow_runs"
        };
        sqlx::query(&format!("UPDATE {table} SET status=? WHERE id=?"))
            .bind(status)
            .bind(self.run)
            .execute(&self.pool)
            .await
            .unwrap();
    }
    async fn agent_status(&self, run: Uuid, status: &str) {
        sqlx::query("UPDATE agent_runs SET status=? WHERE id=?")
            .bind(status)
            .bind(run)
            .execute(&self.pool)
            .await
            .unwrap();
    }
    async fn result(&self, status: &str) {
        sqlx::query("UPDATE workspaces SET execution_owner=json_set(execution_owner,'$.result',json(?)) WHERE id=?")
            .bind(json!({"status":status,"wiki_commit":"published-sha","no_op":false}).to_string())
            .bind(self.workspace).execute(&self.pool).await.unwrap();
    }
}

#[tokio::test]
async fn generated_sync_publication_is_distinct_from_agent_success_and_archive() {
    let f = Fixture::new("openwiki_sync").await;
    f.assert_status("In Progress").await;
    for status in [
        "awaiting_input",
        "awaiting_approval",
        "failed",
        "cancelled",
        "crashed",
        "audit_failed",
    ] {
        f.agent_status(f.run, status).await;
        f.assert_status("In Review").await;
        f.agent_status(f.run, "running").await;
        f.assert_status("In Progress").await;
    }
    f.agent_status(f.run, "succeeded").await;
    f.assert_status("In Progress").await;
    f.result("failed").await; // publication failed despite successful generation
    f.assert_status("In Review").await;
    f.result("succeeded").await;
    f.assert_status("Done").await;
    // Late or duplicate observations never undo publication or rewrite history.
    f.agent_status(f.run, "failed").await;
    f.result("succeeded").await;
    f.assert_status("Done").await;
    let status: String = sqlx::query_scalar("SELECT status FROM agent_runs WHERE id=?")
        .bind(f.run)
        .fetch_one(&f.pool)
        .await
        .unwrap();
    assert_eq!(status, "failed");
}

#[tokio::test]
async fn generated_integration_resume_ignores_old_agent_and_post_publication_failures() {
    let f = Fixture::new("integration").await;
    f.assert_status("In Progress").await;
    for status in [
        "blocked",
        "failed",
        "cancelled",
        "recovery_required",
        "succeeded",
    ] {
        f.product_status(status).await;
        f.assert_status("In Review").await;
        f.product_status("integrating").await;
        f.assert_status("In Progress").await;
    }
    let old = Uuid::new_v4();
    let new = Uuid::new_v4();
    f.agent(old).await;
    f.agent(new).await;
    for agent in [old, new] {
        sqlx::query("UPDATE integration_runs SET agent_run_id=? WHERE id=?")
            .bind(agent)
            .bind(f.run)
            .execute(&f.pool)
            .await
            .unwrap();
        f.agent_status(agent, "awaiting_approval").await;
        f.assert_status("In Review").await;
        f.agent_status(agent, "running").await;
        f.assert_status("In Progress").await;
    }
    f.agent_status(old, "cancelled").await;
    f.assert_status("In Progress").await;
    f.agent_status(new, "failed").await;
    f.assert_status("In Review").await;
    // Host validation is a new phase; a prior conflict Agent is no longer its
    // execution authority, even if its late terminal notification arrives now.
    f.product_status("validating").await;
    f.assert_status("In Progress").await;
    f.agent_status(new, "failed").await;
    f.assert_status("In Progress").await;
    sqlx::query("UPDATE integration_runs SET status='post_processing',payload=json_set(payload,'$.published',json('true')) WHERE id=?")
        .bind(f.run).execute(&f.pool).await.unwrap();
    f.assert_status("Done").await;
    f.product_status("recovery_required").await;
    f.assert_status("Done").await;
}

#[tokio::test]
async fn generated_bootstrap_wait_cancel_resume_and_success() {
    let f = Fixture::new("openwiki_bootstrap").await;
    f.assert_status("In Progress").await;
    for status in ["awaiting_human", "awaiting_arena", "failed", "canceled"] {
        f.product_status(status).await;
        f.assert_status("In Review").await;
        f.product_status("running").await;
        f.assert_status("In Progress").await;
    }
    let child = Uuid::new_v4();
    f.agent(child).await;
    sqlx::query("INSERT INTO node_executions(id,run_id,node_id,node_type,status,agent_run_id) VALUES(?,?,'writer','agent','running',?)")
        .bind(Uuid::new_v4()).bind(f.run).bind(child).execute(&f.pool).await.unwrap();
    f.agent_status(child, "awaiting_approval").await;
    f.assert_status("In Review").await;
    f.agent_status(child, "running").await;
    f.assert_status("In Progress").await;
    let resumed = Uuid::new_v4();
    f.agent(resumed).await;
    sqlx::query("UPDATE node_executions SET agent_run_id=? WHERE run_id=?")
        .bind(resumed)
        .bind(f.run)
        .execute(&f.pool)
        .await
        .unwrap();
    f.agent_status(child, "cancelled").await;
    f.assert_status("In Progress").await;
    f.agent_status(resumed, "succeeded").await;
    f.assert_status("In Progress").await;
    // Repository Workflow success follows its host publication End node.
    f.product_status("succeeded").await;
    f.assert_status("Done").await;
}

#[tokio::test]
async fn generated_status_respects_operator_cancellation_and_duplicate_success() {
    let f = Fixture::new("openwiki_sync").await;
    sqlx::query("UPDATE local_issues SET status_id=(SELECT id FROM local_project_statuses WHERE project_id=? AND name='Cancelled') WHERE id=?")
        .bind(f.project).bind(f.issue).execute(&f.pool).await.unwrap();
    f.result("succeeded").await;
    f.assert_status("Cancelled").await;
    // A separate fixture checks reopening after success, without cancellation.
    let f = Fixture::new("openwiki_sync").await;
    f.result("succeeded").await;
    sqlx::query("UPDATE local_issues SET status_id=(SELECT id FROM local_project_statuses WHERE project_id=? AND name='In Review') WHERE id=?")
        .bind(f.project).bind(f.issue).execute(&f.pool).await.unwrap();
    f.result("succeeded").await;
    f.assert_status("In Review").await;
}

#[tokio::test]
async fn generated_status_requires_provenance_owner_repository_and_link() {
    for mutation in ["ordinary", "owner", "repository", "link", "historical"] {
        let f = Fixture::new("openwiki_sync").await;
        match mutation {
            "ordinary" => {
                sqlx::query("DELETE FROM generated_issue_workspaces")
                    .execute(&f.pool)
                    .await
                    .unwrap();
            }
            "historical" => {
                sqlx::query("DELETE FROM generated_issue_status")
                    .execute(&f.pool)
                    .await
                    .unwrap();
            }
            "owner" => {
                sqlx::query("UPDATE generated_issue_workspaces SET owner_run_id=?")
                    .bind(Uuid::new_v4().to_string())
                    .execute(&f.pool)
                    .await
                    .unwrap();
            }
            "repository" => {
                sqlx::query("UPDATE generated_issue_workspaces SET repository_id=?")
                    .bind(Uuid::new_v4())
                    .execute(&f.pool)
                    .await
                    .unwrap();
            }
            "link" => {
                sqlx::query("DELETE FROM local_workspace_links")
                    .execute(&f.pool)
                    .await
                    .unwrap();
            }
            _ => unreachable!(),
        }
        f.result("succeeded").await;
        f.assert_status("In Progress").await;
    }
}

#[tokio::test]
async fn generated_status_migration_does_not_opt_in_or_repair_existing_issues() {
    let f = Fixture::new_at("openwiki_sync", true).await;
    f.result("succeeded").await;
    sqlx::query("UPDATE workspaces SET usage='interactive' WHERE id=?")
        .bind(f.workspace)
        .execute(&f.pool)
        .await
        .unwrap();
    f.assert_status("Todo").await;
    sqlx::migrate!("./migrations").run(&f.pool).await.unwrap();
    f.result("succeeded").await;
    f.assert_status("Todo").await;
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM generated_issue_status")
        .fetch_one(&f.pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
}

#[tokio::test]
async fn generated_status_missing_destination_never_uses_an_unrelated_column() {
    let f = Fixture::new("openwiki_sync").await;
    sqlx::query("DELETE FROM local_project_statuses WHERE project_id=? AND name='Done'")
        .bind(f.project)
        .execute(&f.pool)
        .await
        .unwrap();
    f.result("succeeded").await;
    f.assert_status("In Progress").await;
    let status: String = sqlx::query_scalar("SELECT projected_status FROM generated_issue_status")
        .fetch_one(&f.pool)
        .await
        .unwrap();
    assert_eq!(status, "in progress");
}

#[tokio::test]
async fn generated_integration_post_processing_cannot_override_manual_cancel_or_reopen() {
    for name in ["Cancelled", "In Review"] {
        let f = Fixture::new("integration").await;
        if name == "In Review" {
            sqlx::query("UPDATE integration_runs SET payload=json_set(payload,'$.published',json('true')) WHERE id=?")
                .bind(f.run).execute(&f.pool).await.unwrap();
            f.assert_status("Done").await;
        }
        sqlx::query("UPDATE local_issues SET status_id=(SELECT id FROM local_project_statuses WHERE project_id=? AND name=?) WHERE id=?")
            .bind(f.project).bind(name).bind(f.issue).execute(&f.pool).await.unwrap();
        sqlx::query("UPDATE integration_runs SET payload=json_set(payload,'$.published',json('true')) WHERE id=?")
            .bind(f.run).execute(&f.pool).await.unwrap();
        db::models::integration::IntegrationRun::complete_cards(&f.pool, f.run)
            .await
            .unwrap();
        db::models::integration::IntegrationRun::complete_cards(&f.pool, f.run)
            .await
            .unwrap();
        f.assert_status(name).await;
    }
}

#[tokio::test]
async fn generated_sync_published_noop_completes_without_a_commit() {
    let f = Fixture::new("openwiki_sync").await;
    f.agent_status(f.run, "succeeded").await;
    sqlx::query("UPDATE workspaces SET execution_owner=json_set(execution_owner,'$.result',json('{\"status\":\"succeeded\",\"wiki_commit\":null,\"no_op\":true}')) WHERE id=?")
        .bind(f.workspace).execute(&f.pool).await.unwrap();
    f.assert_status("Done").await;
}

#[tokio::test]
async fn generated_status_is_independent_of_manual_issue_archive() {
    let f = Fixture::new("openwiki_sync").await;
    sqlx::query("UPDATE local_issues SET archived=1 WHERE id=?")
        .bind(f.issue)
        .execute(&f.pool)
        .await
        .unwrap();
    for (result, expected) in [("failed", "In Review"), ("succeeded", "Done")] {
        f.result(result).await;
        let (status, archived): (String, bool) = sqlx::query_as("SELECT s.name,i.archived FROM local_issues i JOIN local_project_statuses s ON s.id=i.status_id WHERE i.id=?")
            .bind(f.issue).fetch_one(&f.pool).await.unwrap();
        assert_eq!(status, expected);
        assert!(archived);
    }
}

#[tokio::test]
async fn generated_status_upgrade_enrols_active_owner_without_bulk_status_repair() {
    for agent_status in ["running", "succeeded"] {
        let f = Fixture::new_at("openwiki_sync", true).await;
        f.agent_status(f.run, agent_status).await;
        sqlx::migrate!("./migrations").run(&f.pool).await.unwrap();
        f.assert_status("Todo").await;
        if agent_status == "running" {
            f.agent_status(f.run, "awaiting_approval").await;
            f.assert_status("In Review").await;
            f.agent_status(f.run, "running").await;
            f.assert_status("In Progress").await;
        }
        f.result("succeeded").await;
        f.assert_status("Done").await;
    }
}

#[tokio::test]
async fn generated_status_upgrade_preserves_reopened_published_active_issue() {
    let f = Fixture::new_at("integration", true).await;
    sqlx::query("UPDATE integration_runs SET status='post_processing',payload=json_set(payload,'$.published',json('true')) WHERE id=?")
        .bind(f.run).execute(&f.pool).await.unwrap();
    sqlx::query("UPDATE local_issues SET status_id=(SELECT id FROM local_project_statuses WHERE project_id=? AND name='In Review') WHERE id=?")
        .bind(f.project).bind(f.issue).execute(&f.pool).await.unwrap();
    sqlx::migrate!("./migrations").run(&f.pool).await.unwrap();
    f.assert_status("In Review").await;
    f.product_status("succeeded").await;
    db::models::integration::IntegrationRun::complete_cards(&f.pool, f.run)
        .await
        .unwrap();
    f.assert_status("In Review").await;
}

#[tokio::test]
async fn generated_status_legacy_dedicated_card_preserves_whole_issue_cancellation() {
    let f = Fixture::new_at("integration", true).await;
    sqlx::query("UPDATE local_issues SET status_id=(SELECT id FROM local_project_statuses WHERE project_id=? AND name='Cancelled') WHERE id=?")
        .bind(f.project).bind(f.issue).execute(&f.pool).await.unwrap();
    sqlx::migrate!("./migrations").run(&f.pool).await.unwrap();
    sqlx::query("UPDATE integration_runs SET payload=json_set(payload,'$.published',json('true')) WHERE id=?")
        .bind(f.run).execute(&f.pool).await.unwrap();
    db::models::integration::IntegrationRun::complete_cards(&f.pool, f.run)
        .await
        .unwrap();
    f.assert_status("Cancelled").await;
}
