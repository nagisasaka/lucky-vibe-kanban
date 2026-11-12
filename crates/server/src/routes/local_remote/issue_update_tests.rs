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
async fn issue_writer_rechecks_revision_and_rejects_unchecked_body() {
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
    assert!(matches!(
        finished(fixture.update(serde_json::json!({"description":"unchecked body"}))).await,
        Err(ApiError::Conflict(_))
    ));
    let observed = fixture.read().await;
    let last = finished(fixture.update(serde_json::json!({
        "description":"checked UI body", "expected_updated_at":observed.updated_at
    })))
    .await
    .unwrap();
    assert_eq!(last.description.as_deref(), Some("checked UI body"));
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
        finished(fixture.update(serde_json::json!({"description":"must roll back", "expected_updated_at":fixture.issue.updated_at})))
            .await
            .is_err()
    );
    let unchanged = fixture.read().await;
    assert_eq!(unchanged.description, fixture.issue.description);
    assert_eq!(unchanged.sort_order, fixture.issue.sort_order);
    assert_eq!(unchanged.updated_at, fixture.issue.updated_at);
}

use super::issue_versions as versions;

#[tokio::test]
async fn named_versions_preserve_bodies_and_compatibility() {
    let f = Fixture::new().await;
    let initial = versions::list(&f.pool, f.issue.id).await.unwrap();
    assert_eq!(initial.versions.len(), 1);
    assert_eq!(initial.versions[0].description, f.issue.description);
    let first = initial.current_version_id;
    let copied = versions::create(
        &f.pool,
        f.issue.id,
        versions::CreateVersion {
            name: "draft1".into(),
            source_version_id: Some(first),
        },
    )
    .await
    .unwrap();
    let draft = copied.versions[1].id;
    assert_eq!(copied.created_version_id, Some(draft));
    let empty = versions::create(
        &f.pool,
        f.issue.id,
        versions::CreateVersion {
            name: "free name".into(),
            source_version_id: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(empty.versions[2].description, None);
    let integration_revision: i64 =
        sqlx::query_scalar("SELECT integration_revision FROM local_issues WHERE id=?")
            .bind(f.issue.id)
            .fetch_one(&f.pool)
            .await
            .unwrap();
    for revision in 0..3 {
        versions::edit(
            &f.pool,
            f.issue.id,
            draft,
            versions::EditVersion {
                expected_revision: revision,
                name: Some("proposal".into()),
                description: Some(Some(format!("draft {revision}"))),
            },
        )
        .await
        .unwrap();
    }
    let edited = versions::list(&f.pool, f.issue.id).await.unwrap();
    assert_eq!(edited.versions.len(), 3);
    assert_eq!(edited.current_version_id, first);
    assert_eq!(f.read().await.description, f.issue.description);
    assert_eq!(f.read().await.updated_at, f.issue.updated_at);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT integration_revision FROM local_issues WHERE id=?")
            .bind(f.issue.id)
            .fetch_one(&f.pool)
            .await
            .unwrap(),
        integration_revision
    );
    let switched = versions::select(
        &f.pool,
        f.issue.id,
        versions::SelectVersion {
            version_id: draft,
            expected_current_revision: 0,
            expected_version_revision: 3,
        },
    )
    .await
    .unwrap();
    assert_eq!(switched.current_revision, 1);
    assert_eq!(switched.versions[0].description, f.issue.description);
    assert_eq!(switched.versions[1].name, "proposal");
    assert_eq!(switched.versions[1].revision, 3);
    assert_eq!(f.read().await.description.as_deref(), Some("draft 2"));
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT integration_revision FROM local_issues WHERE id=?")
            .bind(f.issue.id)
            .fetch_one(&f.pool)
            .await
            .unwrap(),
        integration_revision + 1
    );
    let observed = f.read().await;
    super::update_local_issue(
        &f.pool,
        f.issue.id,
        change(serde_json::json!({"description":null,"expected_updated_at":observed.updated_at})),
    )
    .await
    .unwrap();
    let saved = versions::list(&f.pool, f.issue.id).await.unwrap();
    assert_eq!(saved.versions[1].description, None);
    assert_eq!(saved.versions[1].revision, 4);
    assert_eq!(saved.versions.len(), 3);
    assert_eq!(f.read().await.description, None);
}

#[tokio::test]
async fn named_versions_detect_edit_switch_and_compatibility_conflicts() {
    let f = Fixture::new().await;
    let initial = versions::list(&f.pool, f.issue.id).await.unwrap();
    let first = initial.current_version_id;
    let state = versions::create(
        &f.pool,
        f.issue.id,
        versions::CreateVersion {
            name: "alternate".into(),
            source_version_id: None,
        },
    )
    .await
    .unwrap();
    let alternate = state.versions[1].id;
    versions::edit(
        &f.pool,
        f.issue.id,
        first,
        versions::EditVersion {
            expected_revision: 0,
            name: None,
            description: Some(Some("new body".into())),
        },
    )
    .await
    .unwrap();
    assert!(matches!(
        versions::edit(
            &f.pool,
            f.issue.id,
            first,
            versions::EditVersion {
                expected_revision: 0,
                name: None,
                description: Some(Some("lost update".into()))
            }
        )
        .await,
        Err(ApiError::Conflict(_))
    ));
    assert!(matches!(
        versions::select(
            &f.pool,
            f.issue.id,
            versions::SelectVersion {
                version_id: first,
                expected_current_revision: 0,
                expected_version_revision: 0
            }
        )
        .await,
        Err(ApiError::Conflict(_))
    ));
    let before_switch = f.read().await.updated_at;
    versions::select(
        &f.pool,
        f.issue.id,
        versions::SelectVersion {
            version_id: alternate,
            expected_current_revision: 0,
            expected_version_revision: 0,
        },
    )
    .await
    .unwrap();
    assert!(matches!(
        versions::select(
            &f.pool,
            f.issue.id,
            versions::SelectVersion {
                version_id: first,
                expected_current_revision: 0,
                expected_version_revision: 1
            }
        )
        .await,
        Err(ApiError::Conflict(_))
    ));
    assert!(matches!(
        super::update_local_issue_checked(
            &f.pool,
            f.issue.id,
            change(serde_json::json!({"description":"old request"})),
            Some(before_switch)
        )
        .await,
        Err(ApiError::Conflict(_))
    ));
    versions::select(
        &f.pool,
        f.issue.id,
        versions::SelectVersion {
            version_id: first,
            expected_current_revision: 1,
            expected_version_revision: 1,
        },
    )
    .await
    .unwrap();
    // Even switch-away-and-back invalidates an old body update.
    assert!(matches!(
        super::update_local_issue_checked(
            &f.pool,
            f.issue.id,
            change(serde_json::json!({"description":"old request"})),
            Some(before_switch)
        )
        .await,
        Err(ApiError::Conflict(_))
    ));
    assert_eq!(f.read().await.description.as_deref(), Some("new body"));
    let other = Fixture::new().await;
    assert!(
        versions::edit(
            &f.pool,
            f.issue.id,
            other.issue.id,
            versions::EditVersion {
                expected_revision: 0,
                name: Some("wrong issue".into()),
                description: None
            }
        )
        .await
        .is_err()
    );
}

#[tokio::test]
async fn named_versions_roll_back_when_integration_reserves_card() {
    let f = Fixture::new().await;
    let state = versions::create(
        &f.pool,
        f.issue.id,
        versions::CreateVersion {
            name: "draft".into(),
            source_version_id: None,
        },
    )
    .await
    .unwrap();
    let first = state.current_version_id;
    let alternate = state.versions[1].id;
    let repo = uuid::Uuid::new_v4();
    sqlx::query("INSERT INTO repos(id,path,name,display_name) VALUES(?,'/version-fixture','fixture','fixture')")
        .bind(repo)
        .execute(&f.pool)
        .await
        .unwrap();
    let run = uuid::Uuid::new_v4();
    sqlx::query("INSERT INTO integration_runs(id,request_key,project_id,repository_id,storage_identity,target_ref,status,payload) VALUES(?, 'version-fixture', ?, ?, '/version-fixture', 'refs/heads/main', 'blocked', '{}')").bind(run).bind(f.issue.project_id).bind(repo).execute(&f.pool).await.unwrap();
    sqlx::query("INSERT INTO integration_reservations(resource_kind,resource_key,run_id) VALUES('card',lower(hex(?)),?)").bind(f.issue.id).bind(run).execute(&f.pool).await.unwrap();
    assert!(
        versions::edit(
            &f.pool,
            f.issue.id,
            first,
            versions::EditVersion {
                expected_revision: 0,
                name: Some("must roll back".into()),
                description: Some(Some("blocked".into()))
            }
        )
        .await
        .is_err()
    );
    assert!(
        versions::select(
            &f.pool,
            f.issue.id,
            versions::SelectVersion {
                version_id: alternate,
                expected_current_revision: 0,
                expected_version_revision: 0
            }
        )
        .await
        .is_err()
    );
    assert!(
        super::update_local_issue_checked(
            &f.pool,
            f.issue.id,
            change(serde_json::json!({"description":"blocked"})),
            Some(f.issue.updated_at)
        )
        .await
        .is_err()
    );
    let after = versions::list(&f.pool, f.issue.id).await.unwrap();
    assert_eq!(after.current_version_id, first);
    assert_eq!(after.current_revision, 0);
    assert_eq!(after.versions[0].revision, 0);
    assert_eq!(after.versions[0].name, "Initial");
    assert_eq!(after.versions[0].description, f.issue.description);
    assert_eq!(f.read().await.updated_at, f.issue.updated_at);
    assert_eq!(f.read().await.description, f.issue.description);
    // Display-only version names do not rewrite the reserved current body.
    versions::edit(
        &f.pool,
        f.issue.id,
        first,
        versions::EditVersion {
            expected_revision: 0,
            name: Some("renamed current".into()),
            description: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(f.read().await.description, f.issue.description);
    assert_eq!(f.read().await.updated_at, f.issue.updated_at);
    // A noncurrent draft remains editable during the existing reservation.
    versions::edit(
        &f.pool,
        f.issue.id,
        alternate,
        versions::EditVersion {
            expected_revision: 0,
            name: None,
            description: Some(Some("allowed draft".into())),
        },
    )
    .await
    .unwrap();
    assert_eq!(f.read().await.description, f.issue.description);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM agent_runs")
            .fetch_one(&f.pool)
            .await
            .unwrap(),
        0
    );
}

#[tokio::test]
async fn named_versions_migrate_existing_body_losslessly() {
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    // Minimal historical shape proves the migration preserves nullable bodies,
    // Unicode, whitespace and exact IDs rather than inventing earlier history.
    sqlx::raw_sql(
        "PRAGMA foreign_keys=ON; CREATE TABLE local_issues(id BLOB PRIMARY KEY,description TEXT);",
    )
    .execute(&pool)
    .await
    .unwrap();
    let bodies = [
        None,
        Some(""),
        Some("  合意した仕様\n\ndraft → fix\n末尾  \n"),
    ];
    let mut ids = Vec::new();
    for body in bodies {
        let id = uuid::Uuid::new_v4();
        ids.push(id);
        sqlx::query("INSERT INTO local_issues(id,description) VALUES(?,?)")
            .bind(id)
            .bind(body)
            .execute(&pool)
            .await
            .unwrap();
    }
    sqlx::raw_sql(include_str!(
        "../../../../db/migrations/20261111220000_issue_versions.sql"
    ))
    .execute(&pool)
    .await
    .unwrap();
    for (id, body) in ids.iter().zip(bodies) {
        let state = versions::list(&pool, *id).await.unwrap();
        assert_eq!(state.versions.len(), 1);
        assert_eq!(state.versions[0].description.as_deref(), body);
        assert_eq!(state.versions[0].id, state.current_version_id);
        assert_eq!(state.versions[0].revision, 0);
        assert_eq!(state.current_revision, 0);
    }
    // The circular current-version foreign key must allow ordinary Issue deletion.
    sqlx::query("DELETE FROM local_issues WHERE id=?")
        .bind(ids[0])
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM issue_versions")
            .fetch_one(&pool)
            .await
            .unwrap(),
        2
    );
}

#[tokio::test]
async fn named_versions_parallel_edit_has_one_winner() {
    let f = Fixture::new().await;
    let first = versions::list(&f.pool, f.issue.id)
        .await
        .unwrap()
        .current_version_id;
    let update = |body: &str| versions::EditVersion {
        expected_revision: 0,
        name: None,
        description: Some(Some(body.into())),
    };
    let (a, b) = tokio::join!(
        versions::edit(&f.pool, f.issue.id, first, update("screen A")),
        versions::edit(&f.pool, f.issue.id, first, update("screen B"))
    );
    assert_eq!(usize::from(a.is_ok()) + usize::from(b.is_ok()), 1);
    assert!(matches!(
        if a.is_err() { a } else { b },
        Err(ApiError::Conflict(_))
    ));
    let state = versions::list(&f.pool, f.issue.id).await.unwrap();
    assert_eq!(state.versions[0].revision, 1);
    assert_eq!(state.versions[0].description, f.read().await.description);
}
