//! File-backed, separate-connection regressions for Issue writer admission.
use std::time::Duration;

use api_types::{Issue, UpdateIssueRequest};
use sqlx::{
    Connection, SqlitePool,
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions},
};
use tokio::{
    task::JoinHandle,
    time::{Instant, timeout},
};

use crate::error::ApiError;

struct Fixture {
    pool: SqlitePool,
    issue: Issue,
    destination: uuid::Uuid,
    _directory: tempfile::TempDir,
}

impl Fixture {
    async fn new() -> Self {
        let directory = tempfile::tempdir().unwrap();
        let pool = SqlitePoolOptions::new()
            .max_connections(2)
            .connect_with(
                SqliteConnectOptions::new()
                    .filename(directory.path().join("issues.sqlite"))
                    .create_if_missing(true)
                    .journal_mode(SqliteJournalMode::Delete)
                    // Match DBService's production budget, including the timeout test.
                    .busy_timeout(Duration::from_secs(5)),
            )
            .await
            .unwrap();
        db::run_migrations(&pool).await.unwrap();
        let project = super::create_local_project(&pool, serde_json::from_value(serde_json::json!({
            "organization_id": super::local_org_id(), "key": "BUSY", "name": "Writer regression", "color": "blue"
        })).unwrap()).await.unwrap();
        let statuses = super::list_project_statuses(&pool, project.id)
            .await
            .unwrap();
        let issue = super::create_local_issue(&pool, serde_json::from_value(serde_json::json!({
            "project_id": project.id, "status_id": statuses[0].id,
            "title": "original", "description": "original body", "sort_order": 0, "extension_metadata": {}
        })).unwrap()).await.unwrap();
        Self {
            pool,
            issue,
            destination: statuses[1].id,
            _directory: directory,
        }
    }

    fn update(&self, changes: serde_json::Value) -> JoinHandle<Result<Issue, ApiError>> {
        let pool = self.pool.clone();
        let id = self.issue.id;
        tokio::spawn(async move { super::update_local_issue(&pool, id, change(changes)).await })
    }

    async fn read(&self) -> Issue {
        super::get_local_issue(&self.pool, self.issue.id)
            .await
            .unwrap()
    }
}

fn change(value: serde_json::Value) -> UpdateIssueRequest {
    serde_json::from_value(value).unwrap()
}

// A writer owns RESERVED before the update begins. DEFERRED can SELECT here,
// then immediately fails its upgrade; IMMEDIATE must wait before reading.
async fn assert_waiting(task: &mut JoinHandle<Result<Issue, ApiError>>) {
    assert!(
        timeout(Duration::from_millis(100), task).await.is_err(),
        "Issue update returned while another connection still reserved the writer"
    );
}

async fn finished(task: JoinHandle<Result<Issue, ApiError>>) -> Result<Issue, ApiError> {
    timeout(Duration::from_secs(3), task)
        .await
        .unwrap()
        .unwrap()
}

#[tokio::test]
async fn issue_writer_waits_before_reading_and_preserves_other_fields() {
    let fixture = Fixture::new().await;
    let mut writer = fixture.pool.begin_with("BEGIN IMMEDIATE").await.unwrap();
    sqlx::query("UPDATE local_issues SET description='other writer body' WHERE id=?")
        .bind(fixture.issue.id)
        .execute(&mut *writer)
        .await
        .unwrap();
    let mut task = fixture.update(serde_json::json!({"title":"saved after wait"}));
    assert_waiting(&mut task).await;
    writer.commit().await.unwrap();
    let saved = finished(task).await.unwrap();
    assert_eq!(saved.title, "saved after wait");
    assert_eq!(saved.description.as_deref(), Some("other writer body"));
    assert_eq!(saved.sort_order, fixture.issue.sort_order);
    assert_eq!(fixture.read().await.title, saved.title);
}

#[tokio::test]
async fn issue_writer_rechecks_revision_and_keeps_unchecked_body_last_write_wins() {
    let fixture = Fixture::new().await;
    let mut writer = fixture.pool.begin_with("BEGIN IMMEDIATE").await.unwrap();
    sqlx::query("UPDATE local_issues SET title='concurrent title', description='concurrent body', updated_at=strftime('%Y-%m-%d %H:%M:%f',julianday(updated_at)+1.0/86400) WHERE id=?")
        .bind(fixture.issue.id).execute(&mut *writer).await.unwrap();
    let pool = fixture.pool.clone();
    let id = fixture.issue.id;
    let revision = fixture.issue.updated_at;
    let mut task = tokio::spawn(async move {
        super::update_local_issue_checked(
            &pool,
            id,
            change(serde_json::json!({"description":"stale checked body"})),
            Some(revision),
        )
        .await
    });
    assert_waiting(&mut task).await;
    writer.commit().await.unwrap();
    assert!(matches!(finished(task).await, Err(ApiError::Conflict(_))));
    assert_eq!(
        fixture.read().await.description.as_deref(),
        Some("concurrent body")
    );
    // The UI does not submit a revision. Do not add optimistic exclusion to it.
    finished(fixture.update(serde_json::json!({"description":"first UI body"})))
        .await
        .unwrap();
    let last = finished(fixture.update(serde_json::json!({"description":"last UI body"})))
        .await
        .unwrap();
    assert_eq!(last.description.as_deref(), Some("last UI body"));
    assert_eq!(last.title, "concurrent title");
}

#[tokio::test]
async fn issue_writer_rechecks_bulk_rank_after_waiting_for_activity() {
    let fixture = Fixture::new().await;
    let mut writer = fixture.pool.begin_with("BEGIN IMMEDIATE").await.unwrap();
    db::models::issue_order::promote(&mut writer, fixture.issue.id)
        .await
        .unwrap();
    let promoted: f64 = sqlx::query_scalar("SELECT sort_order FROM local_issues WHERE id=?")
        .bind(fixture.issue.id)
        .fetch_one(&mut *writer)
        .await
        .unwrap();
    assert_ne!(promoted, fixture.issue.sort_order);
    let pool = fixture.pool.clone();
    let id = fixture.issue.id;
    let expected = fixture.issue.sort_order;
    // Parse exactly the bulk wire shape before calling its shared helper.
    let request: super::BulkUpdateIssuesRequest = serde_json::from_value(serde_json::json!({
        "updates":[{"id":id,"expected_sort_order":expected,"sort_order":99}]
    }))
    .unwrap();
    let mut task = tokio::spawn(async move {
        let update = request.updates.into_iter().next().unwrap();
        super::update_local_issue_with_order_guard(
            &pool,
            update.id,
            update.changes,
            None,
            update.expected_sort_order,
        )
        .await
    });
    assert_waiting(&mut task).await;
    writer.commit().await.unwrap();
    assert_eq!(finished(task).await.unwrap().sort_order, promoted);
    let moved = super::update_local_issue_with_order_guard(
        &fixture.pool,
        id,
        change(serde_json::json!({"status_id":fixture.destination,"sort_order":99})),
        None,
        Some(expected),
    )
    .await
    .unwrap();
    assert_eq!(moved.status_id, fixture.destination);
    assert_ne!(moved.sort_order, 99.0);
}

#[tokio::test]
async fn issue_writer_cancelled_while_waiting_does_not_save_later() {
    let fixture = Fixture::new().await;
    let writer = fixture.pool.begin_with("BEGIN IMMEDIATE").await.unwrap();
    let mut task = fixture.update(serde_json::json!({"title":"cancelled"}));
    assert_waiting(&mut task).await;
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    writer.rollback().await.unwrap();
    // Drain both connection workers, including a cancelled BEGIN and its
    // driver's rollback, before asserting the final state or reusing the pool.
    let mut first = fixture.pool.acquire().await.unwrap();
    let mut second = fixture.pool.acquire().await.unwrap();
    first.ping().await.unwrap();
    second.ping().await.unwrap();
    drop((first, second));
    assert_eq!(fixture.read().await.title, "original");
    assert_eq!(
        finished(fixture.update(serde_json::json!({"title":"next update"})))
            .await
            .unwrap()
            .title,
        "next update"
    );
}

#[tokio::test]
async fn issue_writer_timeout_is_bounded_and_leaves_no_partial_update() {
    let fixture = Fixture::new().await;
    let writer = fixture.pool.begin_with("BEGIN IMMEDIATE").await.unwrap();
    let started = Instant::now();
    let result = timeout(
        Duration::from_secs(10),
        fixture.update(serde_json::json!({"title":"must not save"})),
    )
    .await
    .expect("writer wait exceeded the bounded budget")
    .unwrap();
    assert!(started.elapsed() >= Duration::from_secs(4));
    match result {
        Err(ApiError::Database(sqlx::Error::Database(error))) => {
            assert_eq!(error.code().as_deref(), Some("5"))
        }
        other => panic!("expected SQLite busy after 5s: {other:?}"),
    }
    writer.rollback().await.unwrap();
    assert_eq!(fixture.read().await.title, "original");
    finished(fixture.update(serde_json::json!({"title":"after timeout"})))
        .await
        .unwrap();
}

#[tokio::test]
async fn issue_writer_rolls_back_body_when_activity_promotion_fails() {
    let fixture = Fixture::new().await;
    // Initial body UPDATE retains rank, then promotion changes it and fails.
    sqlx::query("CREATE TRIGGER fail_promotion BEFORE UPDATE OF sort_order ON local_issues WHEN NEW.sort_order != OLD.sort_order BEGIN SELECT RAISE(ABORT,'fixture promotion failure'); END")
        .execute(&fixture.pool).await.unwrap();
    assert!(
        finished(fixture.update(serde_json::json!({"description":"must roll back"})))
            .await
            .is_err()
    );
    let unchanged = fixture.read().await;
    assert_eq!(unchanged.description, fixture.issue.description);
    assert_eq!(unchanged.sort_order, fixture.issue.sort_order);
    assert_eq!(unchanged.updated_at, fixture.issue.updated_at);
}
