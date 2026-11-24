//! Copy into crates/db/tests/ in each revision; run only through run.py.
use std::{collections::BTreeMap, path::PathBuf};

use chrono::{DateTime, Utc};
use db::{
    DBService,
    project_transfer::{export_project, import_project},
};
use sqlx::{Row, SqlitePool};
use uuid::Uuid;

fn env(name: &str) -> String {
    std::env::var(name).expect(name)
}
fn stamp() -> DateTime<Utc> {
    "2026-01-02T03:04:05Z".parse().unwrap()
}

async fn seed(pool: &SqlitePool) {
    let p = Uuid::from_u128(1);
    let s = Uuid::from_u128(2);
    let i = Uuid::from_u128(3);
    sqlx::query("INSERT INTO projects(id,name,key,issue_number_high_water) VALUES(?,'Upgrade fixture','UPG',1)").bind(p).execute(pool).await.unwrap();
    sqlx::query("INSERT INTO local_project_statuses(id,project_id,name,color,sort_order) VALUES(?,?,'Todo','blue',0)").bind(s).bind(p).execute(pool).await.unwrap();
    sqlx::query("INSERT INTO local_issues(id,project_id,issue_number,simple_id,status_id,title,description,sort_order,extension_metadata,created_at) VALUES(?,?,1,'UPG-1',?,'original','日本語 and JSON {\"value\":7}',0,?,?)")
        .bind(i).bind(p).bind(s).bind("{\"value\":7,\"nullable\":null}").bind(stamp()).execute(pool).await.unwrap();
    sqlx::query("INSERT INTO local_issue_comments(id,issue_id,parent_id,message) VALUES(?,?,NULL,'retained comment')").bind(Uuid::from_u128(4)).bind(i).execute(pool).await.unwrap();
    sqlx::query("INSERT INTO issue_versions(id,issue_id,name,description,revision) VALUES(?,?,'Reviewed',?,7)").bind(Uuid::from_u128(5)).bind(i).bind("日本語 and JSON {\"value\":7}").execute(pool).await.unwrap();
    sqlx::query(
        "UPDATE local_issues SET current_version_id=?,current_version_revision=9 WHERE id=?",
    )
    .bind(Uuid::from_u128(5))
    .bind(i)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO local_tags(id,project_id,name,color) VALUES(?,?,'fixture tag','blue')",
    )
    .bind(Uuid::from_u128(6))
    .bind(p)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO local_issue_tags(id,issue_id,tag_id) VALUES(?,?,?)")
        .bind(Uuid::from_u128(7))
        .bind(i)
        .bind(Uuid::from_u128(6))
        .execute(pool)
        .await
        .unwrap();
}

async fn verify(pool: &SqlitePool, imported: bool) {
    let counts: (i64, i64, i64, i64, i64) = sqlx::query_as(
        "SELECT (SELECT count(*) FROM projects), (SELECT count(*) FROM local_issues), (SELECT count(*) FROM local_issue_comments), (SELECT count(*) FROM issue_versions), (SELECT count(*) FROM local_issue_tags)",
    ).fetch_one(pool).await.unwrap();
    assert_eq!(counts, (1, 1, 1, 2, 1));
    assert_eq!(
        sqlx::query_scalar::<_, i64>("PRAGMA foreign_keys")
            .fetch_one(pool)
            .await
            .unwrap(),
        1
    );
    let row = sqlx::query("SELECT id,title,description,created_at,target_date,extension_metadata,typeof(id) AS id_type,length(id) AS id_length,current_version_revision FROM local_issues").fetch_one(pool).await.unwrap();
    assert!(!row.get::<Uuid, _>("id").is_nil());
    assert_eq!(row.get::<String, _>("id_type"), "blob");
    assert_eq!(row.get::<i64, _>("id_length"), 16);
    assert_eq!(row.get::<String, _>("title"), env("LVK_UPGRADE_TITLE"));
    assert_eq!(
        row.get::<String, _>("description"),
        "日本語 and JSON {\"value\":7}"
    );
    assert_eq!(row.get::<DateTime<Utc>, _>("created_at"), stamp());
    assert!(row.get::<Option<DateTime<Utc>>, _>("target_date").is_none());
    let metadata: serde_json::Value =
        serde_json::from_str(&row.get::<String, _>("extension_metadata")).unwrap();
    // Project transfer intentionally excludes extension_metadata (the existing contract).
    assert_eq!(
        metadata,
        if imported {
            serde_json::Value::Null
        } else {
            serde_json::json!({"value":7,"nullable":null})
        }
    );
    assert_eq!(row.get::<i64, _>("current_version_revision"), 9);
    let comment: (String, Option<Uuid>) =
        sqlx::query_as("SELECT message,parent_id FROM local_issue_comments")
            .fetch_one(pool)
            .await
            .unwrap();
    assert_eq!(comment, ("retained comment".into(), None));
    let version: (String,i64) = sqlx::query_as("SELECT v.description,v.revision FROM issue_versions v JOIN local_issues i ON i.current_version_id=v.id").fetch_one(pool).await.unwrap();
    assert_eq!(version, ("日本語 and JSON {\"value\":7}".into(), 7));
    let tag: String = sqlx::query_scalar(
        "SELECT t.name FROM local_tags t JOIN local_issue_tags it ON it.tag_id=t.id",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(tag, "fixture tag");
    assert!(
        sqlx::query("PRAGMA foreign_key_check")
            .fetch_all(pool)
            .await
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>("PRAGMA integrity_check")
            .fetch_one(pool)
            .await
            .unwrap(),
        "ok"
    );
}

#[tokio::test]
#[ignore = "requires run.py and explicit task-owned instance"]
async fn sqlite_upgrade_fixture() {
    let root = PathBuf::from(env("LVK_INSTANCE_DIR"));
    assert!(
        root.is_absolute() && root.join(".sqlite-upgrade-fixture").is_file(),
        "fixture marker required"
    );
    let mode = env("LVK_UPGRADE_MODE");
    let db = DBService::new().await.unwrap();
    let version: String = sqlx::query_scalar("SELECT sqlite_version()")
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert_eq!(version, env("LVK_UPGRADE_SQLITE_VERSION"));
    match mode.as_str() {
        "seed" => seed(&db.pool).await,
        "write" => {
            sqlx::query("UPDATE local_issues SET title=?")
                .bind(env("LVK_UPGRADE_TITLE"))
                .execute(&db.pool)
                .await
                .unwrap();
        }
        "export" => {
            let p: Uuid = sqlx::query_scalar("SELECT id FROM projects WHERE key='UPG'")
                .fetch_one(&db.pool)
                .await
                .unwrap();
            export_project(&db.pool, p, &PathBuf::from(env("LVK_UPGRADE_PACKAGE")))
                .await
                .unwrap();
        }
        "import" => {
            import_project(
                &db.pool,
                &PathBuf::from(env("LVK_UPGRADE_PACKAGE")),
                "IMPORTED",
                Uuid::from_u128(100),
                &BTreeMap::new(),
            )
            .await
            .unwrap();
        }
        "verify" => (),
        _ => panic!("unknown fixture mode"),
    }
    verify(&db.pool, env("LVK_UPGRADE_IMPORTED") == "1").await;
    db.pool.close().await;
}
