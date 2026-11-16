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
async fn count(pool: &SqlitePool, table: &str) -> i64 {
    sqlx::query_scalar(&format!("SELECT count(*) FROM {table}"))
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
        sqlx::query(mutation).execute(&edit).await.unwrap(); edit.close().await;
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
