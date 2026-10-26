use sqlx::{Row, SqlitePool};
use uuid::Uuid;

use super::board_tag::*;

#[tokio::test]
async fn classification_migration_preserves_custom_tags_and_backfills_only_owned_issues() {
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    let mut old = sqlx::migrate!("./migrations");
    old.migrations = std::borrow::Cow::Owned(
        old.iter()
            .filter(|m| m.version < 20261025000000)
            .cloned()
            .collect(),
    );
    old.run(&pool).await.unwrap();
    let project = Uuid::new_v4();
    let status = Uuid::new_v4();
    sqlx::query("INSERT INTO projects(id,name) VALUES(?,'migration')")
        .bind(project)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO local_project_statuses(id,project_id,name,color,sort_order) VALUES(?,?,'Todo','blue',0)").bind(status).bind(project).execute(&pool).await.unwrap();
    let custom = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO local_tags(id,project_id,name,color) VALUES(?,?,'enhancement','custom')",
    )
    .bind(custom)
    .bind(project)
    .execute(&pool)
    .await
    .unwrap();
    // Older CRUD permitted duplicate reserved names; never fail startup or
    // delete/rename existing definitions while adopting the reservation.
    for _ in 0..2 {
        sqlx::query(
            "INSERT INTO local_tags(id,project_id,name,color) VALUES(?,?,'system','custom system')",
        )
        .bind(Uuid::new_v4())
        .bind(project)
        .execute(&pool)
        .await
        .unwrap();
    }
    let mut ids = Vec::new();
    for (number, kind) in [
        Some("openwiki_bootstrap"),
        Some("openwiki_sync"),
        Some("integration"),
        None,
        None,
    ]
    .into_iter()
    .enumerate()
    {
        let issue = Uuid::new_v4();
        ids.push(issue);
        sqlx::query("INSERT INTO local_issues(id,project_id,issue_number,simple_id,status_id,title,sort_order) VALUES(?,?,?,?,?,'OpenWiki Integration title is not evidence',0)")
            .bind(issue).bind(project).bind(number as i64).bind(format!("TEST-{number}")).bind(status).execute(&pool).await.unwrap();
        if let Some(kind) = kind {
            let workspace = Uuid::new_v4();
            // Released owners must still backfill.
            sqlx::query("INSERT INTO workspaces(id,branch,usage,execution_owner) VALUES(?,'test','interactive',?)")
                .bind(workspace).bind(serde_json::json!({"kind":kind}).to_string()).execute(&pool).await.unwrap();
            sqlx::query(
                "INSERT INTO local_workspace_links(workspace_id,project_id,issue_id) VALUES(?,?,?)",
            )
            .bind(workspace)
            .bind(project)
            .bind(issue)
            .execute(&pool)
            .await
            .unwrap();
        }
    }
    let repo = Uuid::new_v4();
    sqlx::query("INSERT INTO repos(id,path,name,display_name) VALUES(?,'/classification/test','test','Test')").bind(repo).execute(&pool).await.unwrap();
    // The product receipt also identifies a generated Card after links/owners
    // are unavailable; source Cards in that receipt must stay ordinary.
    sqlx::query("INSERT INTO integration_runs(id,request_key,project_id,repository_id,storage_identity,target_ref,status,payload) VALUES(?,?,?,?, '/classification/test','refs/heads/main','succeeded',?)")
        .bind(Uuid::new_v4()).bind(Uuid::new_v4().to_string()).bind(project).bind(repo)
        .bind(serde_json::json!({"card_id":ids[4],"sources":[{"selection":{"card_id":ids[3]}}]}).to_string()).execute(&pool).await.unwrap();
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();
    let names: Vec<String> =
        sqlx::query_scalar("SELECT name FROM local_tags WHERE project_id=? ORDER BY name")
            .bind(project)
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(
        names,
        vec![
            "bug",
            "enhancement",
            "feature",
            "survey",
            "system",
            "system"
        ]
    );
    let priorities: Vec<String> = sqlx::query_scalar("SELECT priority FROM local_issues")
        .fetch_all(&pool)
        .await
        .unwrap();
    assert_eq!(priorities, vec!["medium"; 5]);
    for (index, issue) in ids.iter().enumerate() {
        let count: i64 =
            sqlx::query_scalar("SELECT count(*) FROM local_issue_tags WHERE issue_id=?")
                .bind(issue)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(count, if index < 3 || index == 4 { 1 } else { 0 });
    }
    sqlx::query("UPDATE local_issues SET priority=NULL WHERE id=?")
        .bind(ids[0])
        .execute(&pool)
        .await
        .unwrap();
    let priority: String = sqlx::query_scalar("SELECT priority FROM local_issues WHERE id=?")
        .bind(ids[0])
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(priority, "medium");
    sqlx::query("DELETE FROM local_tags WHERE name='survey'")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM local_tags WHERE name='survey'")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        count, 0,
        "migration retry must not reseed deleted normal tags"
    );
    assert!(
        sqlx::query("PRAGMA foreign_key_check")
            .fetch_all(&pool)
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn classification_relations_are_project_scoped_idempotent_and_system_protected() {
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();
    let mut issues = Vec::new();
    let mut projects = Vec::new();
    let mut tags = Vec::new();
    for index in 0..2 {
        let project = Uuid::new_v4();
        let status = Uuid::new_v4();
        let issue = Uuid::new_v4();
        let tag = Uuid::new_v4();
        sqlx::query("INSERT INTO projects(id,name,key) VALUES(?,'test',?)")
            .bind(project)
            .bind(format!("PR{index}"))
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO local_project_statuses(id,project_id,name,color,sort_order) VALUES(?,?,'Todo','blue',0)").bind(status).bind(project).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO local_issues(id,project_id,issue_number,simple_id,status_id,title,sort_order) VALUES(?,?,?,'TEST-1',?,'test',0)").bind(issue).bind(project).bind(index).bind(status).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO local_tags(id,project_id,name,color) VALUES(?,?,'bug','red')")
            .bind(tag)
            .bind(project)
            .execute(&pool)
            .await
            .unwrap();
        projects.push(project);
        issues.push(issue);
        tags.push(tag);
    }
    let mut conn = pool.acquire().await.unwrap();
    let first = add(&mut conn, issues[0], tags[0], None).await.unwrap();
    let duplicate = add(&mut conn, issues[0], tags[0], Some(Uuid::new_v4()))
        .await
        .unwrap();
    assert_eq!(first, duplicate);
    assert!(add(&mut conn, issues[0], tags[1], None).await.is_err());
    attach_system(&mut conn, projects[0], issues[0])
        .await
        .unwrap();
    attach_system(&mut conn, projects[0], issues[0])
        .await
        .unwrap();
    let row = sqlx::query("SELECT it.id,it.tag_id FROM local_issue_tags it JOIN local_tags t ON t.id=it.tag_id WHERE it.issue_id=? AND t.name='system'").bind(issues[0]).fetch_one(&mut *conn).await.unwrap();
    let system: Uuid = row.get("tag_id");
    let relation: Uuid = row.get("id");
    assert!(add(&mut conn, issues[0], system, None).await.is_err());
    assert!(remove(&mut conn, relation).await.is_err());
    remove(&mut conn, first).await.unwrap();
    remove(&mut conn, first).await.unwrap();
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM local_issue_tags WHERE issue_id=?")
        .bind(issues[0])
        .fetch_one(&mut *conn)
        .await
        .unwrap();
    assert_eq!(count, 1);
}
