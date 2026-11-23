use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

use super::*;

async fn fixture() -> (tempfile::TempDir, SqlitePool, Uuid, Uuid) {
    let dir = tempfile::tempdir().unwrap();
    let pool = SqlitePoolOptions::new()
        .max_connections(2)
        .connect_with(
            SqliteConnectOptions::new()
                .filename(dir.path().join("filters.db"))
                .create_if_missing(true)
                .foreign_keys(true),
        )
        .await
        .unwrap();
    crate::run_migrations(&pool).await.unwrap();
    let p = Uuid::new_v4();
    let other = Uuid::new_v4();
    for (id, key) in [(p, "ONE"), (other, "TWO")] {
        sqlx::query("INSERT INTO projects(id,name,key) VALUES(?,'filters',?)")
            .bind(id)
            .bind(key)
            .execute(&pool)
            .await
            .unwrap();
    }
    (dir, pool, p, other)
}
fn definition() -> SavedFilterDefinition {
    SavedFilterDefinition {
        name: "Release blockers".into(),
        machine: MachineFilter::default(),
        natural_language: None,
        context: Some("Release 4".into()),
    }
}
fn create() -> SavedFilterWrite {
    SavedFilterWrite::Create(CreateSavedFilterRequest {
        request_id: Uuid::new_v4(),
        definition: definition(),
    })
}
#[tokio::test]
async fn saved_filter_atomic_replay_conflict_revision_and_delete() {
    let (_dir, pool, p, other) = fixture().await;
    let input = create();
    let v = write(&pool, p, "alice", input.clone()).await.unwrap();
    let f: SavedFilter = serde_json::from_value(v.clone()).unwrap();
    assert_eq!(write(&pool, p, "alice", input.clone()).await.unwrap(), v);
    assert!(matches!(
        get(&pool, other, f.id).await,
        Err(SavedFilterError::NotFound)
    ));
    let mut changed = input.clone();
    if let SavedFilterWrite::Create(a) = &mut changed {
        a.definition.name = "changed".into();
    }
    assert!(matches!(
        write(&pool, p, "alice", changed).await,
        Err(SavedFilterError::Conflict)
    ));
    let mut d = definition();
    d.name = "Renamed".into();
    let update = SavedFilterWrite::Update {
        id: f.id,
        input: UpdateSavedFilterRequest {
            request_id: Uuid::new_v4(),
            expected_revision: 1,
            definition: d,
        },
    };
    let renamed = write(&pool, p, "alice", update.clone()).await.unwrap();
    assert_eq!(renamed["id"], v["id"]);
    assert_eq!(renamed["revision"], 2);
    assert_eq!(renamed["created_at"], v["created_at"]);
    assert_eq!(
        write(&pool, p, "alice", input.clone()).await.unwrap(),
        v,
        "replay is pinned to original result, not current revision"
    );
    let stale = SavedFilterWrite::Delete {
        id: f.id,
        input: DeleteSavedFilterRequest {
            request_id: Uuid::new_v4(),
            expected_revision: 1,
        },
    };
    assert!(matches!(
        write(&pool, p, "alice", stale).await,
        Err(SavedFilterError::Conflict)
    ));
    sqlx::query("INSERT INTO saved_filter_semantic_cache VALUES(?,1,'{}')")
        .bind(f.id)
        .execute(&pool)
        .await
        .unwrap();
    let delete = SavedFilterWrite::Delete {
        id: f.id,
        input: DeleteSavedFilterRequest {
            request_id: Uuid::new_v4(),
            expected_revision: 2,
        },
    };
    let deleted = write(&pool, p, "alice", delete.clone()).await.unwrap();
    assert_eq!(write(&pool, p, "alice", delete).await.unwrap(), deleted);
    assert!(matches!(
        get(&pool, p, f.id).await,
        Err(SavedFilterError::NotFound)
    ));
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM saved_filter_semantic_cache")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        write(&pool, p, "alice", update).await.unwrap(),
        renamed,
        "receipts survive definition deletion"
    );
    assert_eq!(
        list(&pool, p, &SavedFilterPage::default())
            .await
            .unwrap()
            .total_count,
        0
    );
}
#[tokio::test]
async fn saved_filter_persistence_paging_and_concurrent_revision() {
    let (dir, pool, p, _) = fixture().await;
    for _ in 0..3 {
        write(&pool, p, "a", create()).await.unwrap();
    }
    let page = list(
        &pool,
        p,
        &SavedFilterPage {
            offset: 1,
            limit: Some(1),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(page.total_count, 3);
    assert_eq!(page.next_offset, Some(2));
    let id = page.saved_filters[0].id;
    let a = SavedFilterWrite::Update {
        id,
        input: UpdateSavedFilterRequest {
            request_id: Uuid::new_v4(),
            expected_revision: 1,
            definition: definition(),
        },
    };
    let b = SavedFilterWrite::Update {
        id,
        input: UpdateSavedFilterRequest {
            request_id: Uuid::new_v4(),
            expected_revision: 1,
            definition: definition(),
        },
    };
    let (a, b) = tokio::join!(write(&pool, p, "a", a), write(&pool, p, "a", b));
    assert!(matches!(
        (&a, &b),
        (Ok(_), Err(SavedFilterError::Conflict)) | (Err(SavedFilterError::Conflict), Ok(_))
    ));
    pool.close().await;
    let reopened = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(SqliteConnectOptions::new().filename(dir.path().join("filters.db")))
        .await
        .unwrap();
    assert_eq!(get(&reopened, p, id).await.unwrap().revision, 2);
    assert_eq!(
        list(&reopened, p, &SavedFilterPage::default())
            .await
            .unwrap()
            .total_count,
        3
    );
}
#[tokio::test]
async fn saved_filter_rejects_foreign_condition_and_invalid_period() {
    let (_dir, pool, p, other) = fixture().await;
    let foreign = Uuid::new_v4();
    sqlx::query("INSERT INTO local_project_statuses(id,project_id,name,color,sort_order) VALUES(?,?,'Todo','blue',0)").bind(foreign).bind(other).execute(&pool).await.unwrap();
    let mut d = definition();
    d.machine.status_ids = vec![foreign];
    assert!(matches!(
        write(
            &pool,
            p,
            "a",
            SavedFilterWrite::Create(CreateSavedFilterRequest {
                request_id: Uuid::new_v4(),
                definition: d
            })
        )
        .await,
        Err(SavedFilterError::Invalid(_))
    ));
    let mut d = definition();
    let now = Utc::now();
    d.machine.created_at = Some(SavedFilterPeriod::Absolute {
        start: Some(now),
        end: Some(now),
    });
    assert!(validate(&d).is_err());
    d.machine.created_at = Some(SavedFilterPeriod::Relative { seconds: 0 });
    assert!(validate(&d).is_err());
    assert_eq!(
        list(&pool, p, &SavedFilterPage::default())
            .await
            .unwrap()
            .total_count,
        0
    );
}
