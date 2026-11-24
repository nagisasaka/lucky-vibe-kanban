//! Check the SQLite engine used by LVK, independently of any system sqlite3 CLI.
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};

use db::DBService;
use sqlx::{Connection, Decode, Sqlite, SqliteConnection, sqlite::SqliteOperation};

#[test]
fn sqlite_runtime_preserves_features_and_connection_hooks() {
    let instance = tempfile::tempdir().unwrap();
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "isolated_sqlite_runtime", "--nocapture"])
        .env("LVK_SQLITE_RUNTIME_FIXTURE", "1")
        .env("LVK_INSTANCE_DIR", instance.path())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[tokio::test]
async fn isolated_sqlite_runtime() {
    if std::env::var("LVK_SQLITE_RUNTIME_FIXTURE").as_deref() != Ok("1") {
        return;
    }
    let connection_count = Arc::new(AtomicUsize::new(0));
    let updates = Arc::new(Mutex::new(Vec::new()));
    let deleted_values = Arc::new(Mutex::new(Vec::new()));
    let db = DBService::new_with_after_connect({
        let connection_count = connection_count.clone();
        let updates = updates.clone();
        let deleted_values = deleted_values.clone();
        move |conn| {
            let connection_id = connection_count.fetch_add(1, Ordering::SeqCst);
            let updates = updates.clone();
            let deleted_values = deleted_values.clone();
            Box::pin(async move {
                let mut handle = conn.lock_handle().await?;
                handle.set_update_hook(move |hook| {
                    if hook.table == "lvk_runtime_fixture" {
                        let operation = match hook.operation {
                            SqliteOperation::Insert => "insert",
                            SqliteOperation::Update => "update",
                            SqliteOperation::Delete => "delete",
                            _ => panic!("unexpected SQLite operation"),
                        };
                        updates.lock().unwrap().push((connection_id, operation));
                    }
                });
                handle.set_preupdate_hook(move |hook| {
                    if hook.table == "lvk_runtime_fixture"
                        && hook.operation == SqliteOperation::Delete
                    {
                        // Decode in the callback: SQLite's borrowed value must not
                        // escape the hook into a task on another thread.
                        let value = <String as Decode<Sqlite>>::decode(
                            hook.get_old_column_value(1).unwrap(),
                        )
                        .unwrap();
                        deleted_values.lock().unwrap().push((connection_id, value));
                    }
                });
                Ok(())
            })
        }
    })
    .await
    .unwrap();

    let version: String = sqlx::query_scalar("SELECT sqlite_version()")
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert_eq!(version, "3.54.1");
    let options: Vec<String> = sqlx::query_scalar("PRAGMA compile_options")
        .fetch_all(&db.pool)
        .await
        .unwrap();
    for required in [
        "ENABLE_FTS5",
        "ENABLE_PREUPDATE_HOOK",
        "ENABLE_UNLOCK_NOTIFY",
        "THREADSAFE=1",
    ] {
        assert!(
            options.iter().any(|option| option == required),
            "{options:?}"
        );
    }
    let applied_migrations: i64 =
        sqlx::query_scalar("SELECT count(*) FROM _sqlx_migrations WHERE success = 1")
            .fetch_one(&db.pool)
            .await
            .unwrap();
    assert!(applied_migrations > 0);

    sqlx::raw_sql(
        "CREATE TABLE lvk_runtime_fixture(id INTEGER PRIMARY KEY, value TEXT NOT NULL);
         CREATE VIRTUAL TABLE lvk_runtime_fts USING fts5(body);
         INSERT INTO lvk_runtime_fts(body) VALUES ('alpha beta'), ('alpha'), ('beta'), ('gamma');",
    )
    .execute(&db.pool)
    .await
    .unwrap();
    for (expression, expected) in [("alpha AND beta", 1_i64), ("alpha OR beta", 3)] {
        let count: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM lvk_runtime_fts WHERE lvk_runtime_fts MATCH ?",
        )
        .bind(expression)
        .fetch_one(&db.pool)
        .await
        .unwrap();
        assert_eq!(count, expected);
    }

    // Hold both leases simultaneously so these cannot be the same connection.
    let mut first = db.pool.acquire().await.unwrap();
    let mut second = db.pool.acquire().await.unwrap();
    for (index, conn) in [&mut first, &mut second].into_iter().enumerate() {
        let journal: String = sqlx::query_scalar("PRAGMA journal_mode")
            .fetch_one(&mut **conn)
            .await
            .unwrap();
        assert_eq!(journal, "delete");
        let busy: i64 = sqlx::query_scalar("PRAGMA busy_timeout")
            .fetch_one(&mut **conn)
            .await
            .unwrap();
        assert_eq!(busy, 5000);
        let foreign_keys: i64 = sqlx::query_scalar("PRAGMA foreign_keys")
            .fetch_one(&mut **conn)
            .await
            .unwrap();
        assert_eq!(foreign_keys, 1);
        sqlx::query("INSERT INTO lvk_runtime_fixture(id, value) VALUES (?, 'original')")
            .bind(index as i64)
            .execute(&mut **conn)
            .await
            .unwrap();
        sqlx::query("UPDATE lvk_runtime_fixture SET value = ? WHERE id = ?")
            .bind(format!("updated-{index}"))
            .bind(index as i64)
            .execute(&mut **conn)
            .await
            .unwrap();
        sqlx::query("DELETE FROM lvk_runtime_fixture WHERE id = ?")
            .bind(index as i64)
            .execute(&mut **conn)
            .await
            .unwrap();
    }
    {
        let updates = updates.lock().unwrap();
        let deleted_values = deleted_values.lock().unwrap();
        assert_eq!(updates.len(), 6);
        assert_eq!(deleted_values.len(), 2);
        assert_ne!(deleted_values[0].0, deleted_values[1].0);
        for (index, (connection_id, value)) in deleted_values.iter().enumerate() {
            assert_eq!(value, &format!("updated-{index}"));
            let operations: Vec<_> = updates
                .iter()
                .filter(|(id, _)| id == connection_id)
                .map(|(_, operation)| *operation)
                .collect();
            assert_eq!(operations, ["insert", "update", "delete"]);
        }
    }
    transaction_recovery(&mut first).await;
    drop(first);
    drop(second);
    workspace_returning_preserves_nulls(&db.pool).await;
    db.pool.close().await;
}

async fn workspace_returning_preserves_nulls(pool: &sqlx::SqlitePool) {
    use db::models::workspace::{CreateWorkspace, Workspace};

    let data = CreateWorkspace {
        branch: "sqlite-upgrade-fixture".into(),
        name: None,
    };
    let managed = Workspace::create(pool, &data, uuid::Uuid::new_v4())
        .await
        .unwrap();
    let direct = Workspace::create_direct_folder(
        pool,
        &data,
        uuid::Uuid::new_v4(),
        "/sqlite-upgrade-fixture",
    )
    .await
    .unwrap();
    assert!(managed.container_ref.is_none());
    assert_eq!(
        direct.container_ref.as_deref(),
        Some("/sqlite-upgrade-fixture")
    );
    for workspace in [managed, direct] {
        assert!(workspace.execution_owner.is_none());
        assert!(workspace.arena_group_id.is_none());
        assert!(workspace.task_id.is_none());
        assert!(workspace.setup_completed_at.is_none());
        assert!(workspace.name.is_none());
    }
}

async fn transaction_recovery(conn: &mut SqliteConnection) {
    sqlx::query("CREATE TABLE lvk_transaction_fixture(id INTEGER PRIMARY KEY)")
        .execute(&mut *conn)
        .await
        .unwrap();
    let mut tx = conn.begin_with("BEGIN IMMEDIATE").await.unwrap();
    sqlx::query("INSERT INTO lvk_transaction_fixture VALUES (1)")
        .execute(&mut *tx)
        .await
        .unwrap();
    // SQLite itself rolls back the entire transaction, rather than leaving it
    // open after a normal constraint error. SQLx must synchronize its depth.
    let error = sqlx::query("INSERT OR ROLLBACK INTO lvk_transaction_fixture VALUES (1)")
        .execute(&mut *tx)
        .await
        .unwrap_err();
    assert!(error.as_database_error().unwrap().is_unique_violation());
    drop(tx);
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM lvk_transaction_fixture")
        .fetch_one(&mut *conn)
        .await
        .unwrap();
    assert_eq!(count, 0, "engine rollback must undo earlier statements");

    // Reuse the same connection, including SQLx's ordinary rollback-on-drop.
    let mut tx = conn.begin_with("BEGIN IMMEDIATE").await.unwrap();
    sqlx::query("INSERT INTO lvk_transaction_fixture VALUES (2)")
        .execute(&mut *tx)
        .await
        .unwrap();
    drop(tx);
    let mut tx = conn.begin_with("BEGIN IMMEDIATE").await.unwrap();
    sqlx::query("INSERT INTO lvk_transaction_fixture VALUES (3)")
        .execute(&mut *tx)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    let ids: Vec<i64> = sqlx::query_scalar("SELECT id FROM lvk_transaction_fixture ORDER BY id")
        .fetch_all(&mut *conn)
        .await
        .unwrap();
    assert_eq!(ids, [3], "dropped transaction must not leak its write");
}
