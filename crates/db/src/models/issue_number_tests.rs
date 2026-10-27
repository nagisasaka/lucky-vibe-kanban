use std::time::Duration;

use sqlx::{SqlitePool, sqlite::SqliteConnectOptions};
use uuid::Uuid;

use super::issue_number::next_issue_number;

async fn open(path: &std::path::Path) -> SqlitePool {
    sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(8)
        .connect_with(
            SqliteConnectOptions::new()
                .filename(path)
                .create_if_missing(true)
                .busy_timeout(Duration::from_secs(30)),
        )
        .await
        .unwrap()
}

async fn project(pool: &SqlitePool, key: &str) -> (Uuid, Uuid) {
    let p = Uuid::new_v4();
    let s = Uuid::new_v4();
    sqlx::query("INSERT INTO projects(id,name,key) VALUES(?,'Number test',?)")
        .bind(p)
        .bind(key)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO local_project_statuses(id,project_id,name,color,sort_order) VALUES(?,?,'Ready','blue',0)").bind(s).bind(p).execute(pool).await.unwrap();
    (p, s)
}

async fn insert(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    p: Uuid,
    s: Uuid,
    n: i32,
    simple: &str,
) -> Result<Uuid, sqlx::Error> {
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO local_issues(id,project_id,issue_number,simple_id,status_id,title,sort_order) VALUES(?,?,?,?,?,'Number test',0)").bind(id).bind(p).bind(n).bind(simple).bind(s).execute(&mut **tx).await?;
    Ok(id)
}

async fn create(pool: &SqlitePool, p: Uuid, s: Uuid) -> (Uuid, i32, String) {
    let mut tx = pool.begin_with("BEGIN IMMEDIATE").await.unwrap();
    let (n, simple) = next_issue_number(&mut tx, p).await.unwrap();
    let id = insert(&mut tx, p, s, n, &simple).await.unwrap();
    tx.commit().await.unwrap();
    (id, n, simple)
}

async fn water(pool: &SqlitePool, p: Uuid) -> i64 {
    sqlx::query_scalar("SELECT issue_number_high_water FROM projects WHERE id=?")
        .bind(p)
        .fetch_one(pool)
        .await
        .unwrap()
}

#[tokio::test]
async fn highest_and_all_deletions_survive_reopen_key_changes_and_archive() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("numbers.sqlite");
    let pool = open(&path).await;
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();
    let (p, s) = project(&pool, "LOCAL").await;
    let (other, os) = project(&pool, "OTHER").await;
    // Reproduce the user-facing LOCAL-205 reference, then delete its UUID.
    sqlx::query("UPDATE projects SET issue_number_high_water=204 WHERE id=?")
        .bind(p)
        .execute(&pool)
        .await
        .unwrap();
    let old = create(&pool, p, s).await;
    assert_eq!(old.2, "LOCAL-205");
    for archived in [1, 0] {
        sqlx::query("UPDATE local_issues SET archived=? WHERE id=?")
            .bind(archived)
            .bind(old.0)
            .execute(&pool)
            .await
            .unwrap();
        assert_eq!(water(&pool, p).await, 205);
    }
    sqlx::query("DELETE FROM local_issues WHERE id=?")
        .bind(old.0)
        .execute(&pool)
        .await
        .unwrap();
    let new = create(&pool, p, s).await;
    assert_eq!(new.2, "LOCAL-206");
    assert_ne!(old.0, new.0);
    sqlx::query("UPDATE projects SET key='RENAMED' WHERE id=?")
        .bind(p)
        .execute(&pool)
        .await
        .unwrap();
    let unchanged: (Uuid, i32, String) =
        sqlx::query_as("SELECT id,issue_number,simple_id FROM local_issues WHERE id=?")
            .bind(new.0)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(new, unchanged);
    assert_eq!(create(&pool, other, os).await.1, 1);
    sqlx::query("DELETE FROM local_issues WHERE project_id=?")
        .bind(p)
        .execute(&pool)
        .await
        .unwrap();
    pool.close().await;
    let pool = open(&path).await;
    assert_eq!(create(&pool, p, s).await.2, "RENAMED-207");
    assert!(
        sqlx::query("UPDATE projects SET issue_number_high_water=0 WHERE id=?")
            .bind(p)
            .execute(&pool)
            .await
            .is_err()
    );
    pool.close().await;
}

#[tokio::test]
async fn concurrent_connections_and_failed_insert_rollback_are_atomic() {
    let dir = tempfile::tempdir().unwrap();
    let pool = open(&dir.path().join("concurrent.sqlite")).await;
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();
    let (p, s) = project(&pool, "PAR").await;
    let mut tasks = tokio::task::JoinSet::new();
    for _ in 0..24 {
        let pool = pool.clone();
        tasks.spawn(async move { create(&pool, p, s).await.1 });
    }
    let mut numbers = Vec::new();
    while let Some(result) = tasks.join_next().await {
        numbers.push(result.unwrap());
    }
    numbers.sort();
    assert_eq!(numbers, (1..=24).collect::<Vec<_>>());
    assert_eq!(water(&pool, p).await, 24);
    let mut tx = pool.begin_with("BEGIN IMMEDIATE").await.unwrap();
    let (n, simple) = next_issue_number(&mut tx, p).await.unwrap();
    assert_eq!(n, 25);
    assert!(
        insert(&mut tx, p, Uuid::new_v4(), n, &simple)
            .await
            .is_err()
    );
    tx.rollback().await.unwrap();
    assert_eq!(water(&pool, p).await, 24);
    assert_eq!(create(&pool, p, s).await.1, 25);
    sqlx::query("UPDATE projects SET issue_number_high_water=2147483646 WHERE id=?")
        .bind(p)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(create(&pool, p, s).await.1, i32::MAX);
    let mut tx = pool.begin_with("BEGIN IMMEDIATE").await.unwrap();
    assert!(
        next_issue_number(&mut tx, p)
            .await
            .unwrap_err()
            .to_string()
            .contains("exhausted")
    );
    tx.rollback().await.unwrap();
    assert_eq!(water(&pool, p).await, i64::from(i32::MAX));
    pool.close().await;
}

#[tokio::test]
async fn migration_seeds_archived_max_but_does_not_invent_deleted_history() {
    let dir = tempfile::tempdir().unwrap();
    let pool = open(&dir.path().join("migration.sqlite")).await;
    let mut old = sqlx::migrate!("./migrations");
    old.migrations = std::borrow::Cow::Owned(
        old.migrations
            .into_owned()
            .into_iter()
            .filter(|m| m.version < 20261027090100)
            .collect(),
    );
    old.run(&pool).await.unwrap();
    let (p, s) = project(&pool, "MIG").await;
    let (empty, es) = project(&pool, "EMPTY").await;
    let mut tx = pool.begin_with("BEGIN IMMEDIATE").await.unwrap();
    let id = insert(&mut tx, p, s, 177, "MIG-177").await.unwrap();
    let gone = insert(&mut tx, empty, es, 205, "EMPTY-205").await.unwrap();
    tx.commit().await.unwrap();
    sqlx::query("UPDATE local_issues SET archived=1 WHERE id=?")
        .bind(id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM local_issues WHERE id=?")
        .bind(gone)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();
    assert_eq!(water(&pool, p).await, 177);
    assert_eq!(water(&pool, empty).await, 0); // No historical-number evidence survives.
    let row: (Uuid, i32, String) =
        sqlx::query_as("SELECT id,issue_number,simple_id FROM local_issues WHERE id=?")
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(row, (id, 177, "MIG-177".into()));
    assert_eq!(create(&pool, p, s).await.1, 178);
    // A verified historical floor may be raised, never lowered.
    sqlx::query("UPDATE projects SET issue_number_high_water=205 WHERE id=?")
        .bind(empty)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(create(&pool, empty, es).await.1, 206);
    pool.close().await;
}
