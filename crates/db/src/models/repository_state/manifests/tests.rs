use files::{ReconciliationResult, SemanticChanges};

use super::*;

async fn fixture() -> (tempfile::TempDir, SqlitePool, RepositoryMemoryStore) {
    let temp = tempfile::tempdir().unwrap();
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(2)
        .connect_with(
            sqlx::sqlite::SqliteConnectOptions::new()
                .filename(temp.path().join("db.sqlite"))
                .create_if_missing(true)
                .busy_timeout(std::time::Duration::from_secs(10)),
        )
        .await
        .unwrap();
    crate::run_migrations(&pool).await.unwrap();
    let files = files::RepositoryMemoryStore::at_persistent(temp.path()).unwrap();
    let store = RepositoryMemoryStore::with_artifacts(&pool, Uuid::new_v4(), files);
    (temp, pool, store)
}

fn event(store: &RepositoryMemoryStore, id: Uuid) -> ChangeManifest {
    ChangeManifest {
        version: 1,
        event_id: id,
        repository_id: store.repository_id,
        workspace_id: Uuid::new_v4(),
        task_id: None,
        created_at: chrono::Utc::now(),
        base_commit: "a".repeat(40),
        source_commit: "b".repeat(40),
        target_branch: Some("main".into()),
        changed_paths: vec!["source.rs".into()],
        semantics: SemanticChanges {
            goal: "Preserve intent".into(),
            summary: "fix: fixture".into(),
            ..Default::default()
        },
    }
}

#[tokio::test]
async fn same_run_in_two_repositories_is_distinct_and_immutable_without_event_files() {
    let (temp, pool, store) = fixture().await;
    let second = RepositoryMemoryStore::with_artifacts(&pool, Uuid::new_v4(), store.files.clone());
    let id = Uuid::new_v4();
    let a = event(&store, id);
    let mut b = event(&second, id);
    b.workspace_id = a.workspace_id;
    b.source_commit = "c".repeat(40);
    let (first, duplicate) = tokio::join!(store.publish_event(&a), store.publish_event(&a));
    first.unwrap();
    duplicate.unwrap();
    second.publish_event(&b).await.unwrap();
    assert_eq!(store.event(id).await.unwrap(), Some(a.clone()));
    let missing = Uuid::new_v4();
    assert_eq!(
        store
            .events_for_runs(a.workspace_id, &[id, id, missing])
            .await
            .unwrap(),
        vec![(id, Some(a.clone())), (missing, None)]
    );
    assert!(store.events_for_runs(Uuid::new_v4(), &[id]).await.is_err());
    assert_eq!(
        RepositoryMemoryStore::read_event(&pool, store.repository_id, id)
            .await
            .unwrap(),
        Some(a.clone())
    );
    assert_eq!(second.event(id).await.unwrap(), Some(b));
    assert!(second.publish_event(&a).await.is_err());
    let mut conflict = a.clone();
    conflict.semantics.summary = "different intent".into();
    assert!(store.publish_event(&conflict).await.is_err());
    let legacy = temp.path().join("knowledge/events");
    std::fs::create_dir_all(&legacy).unwrap();
    let old = legacy.join(format!("{id}.json"));
    std::fs::write(&old, b"not a valid old Manifest").unwrap();
    // Schema startup does not explore or import obsolete files, even malformed ones.
    crate::run_migrations(&pool).await.unwrap();
    assert_eq!(std::fs::read(&old).unwrap(), b"not a valid old Manifest");
    let reopened =
        RepositoryMemoryStore::with_artifacts(&pool, store.repository_id, store.files.clone());
    assert_eq!(reopened.events().await.unwrap(), vec![a.clone()]);
    let page = reopened
        .discover_events(store.repository_id, None, 10)
        .await
        .unwrap();
    assert_eq!(
        page.records[0].url,
        format!("/api/repos/{}/change-manifests/{id}", store.repository_id)
    );
    let receipt = WikiReconciliationReceipt {
        event_id: id,
        reconciled_at: chrono::Utc::now(),
        target_commit: a.source_commit,
        wiki_commit: None,
        result: ReconciliationResult::NoOp,
        error: None,
    };
    reopened.acknowledge(&receipt).await.unwrap();
    reopened
        .save_reconciliation(
            &files::RepositoryMemoryState::default(),
            std::slice::from_ref(&receipt),
        )
        .await
        .unwrap();
    let mut unknown = receipt;
    unknown.event_id = Uuid::new_v4();
    assert!(reopened.acknowledge(&unknown).await.is_err());
}

#[tokio::test]
async fn discovery_pages_past_bad_records_but_strict_readers_reject_them() {
    let (_temp, pool, store) = fixture().await;
    for number in 1..=3 {
        store
            .publish_event(&event(&store, Uuid::from_u128(number)))
            .await
            .unwrap();
    }
    sqlx::query("UPDATE repository_change_manifests SET payload=json_set(payload,'$.summary','') WHERE id=?")
        .bind(Uuid::from_u128(2)).execute(&pool).await.unwrap();
    let first = store
        .discover_events(store.repository_id, None, 1)
        .await
        .unwrap();
    let second = store
        .discover_events(store.repository_id, first.next_after.as_deref(), 1)
        .await
        .unwrap();
    assert_eq!(second.errors.len(), 1);
    let last = store
        .discover_events(store.repository_id, second.next_after.as_deref(), 1)
        .await
        .unwrap();
    assert_eq!(last.records[0].event_id, Uuid::from_u128(3));
    assert!(last.next_after.is_none());
    assert!(store.events().await.is_err());
    assert!(
        store
            .discover_events(Uuid::new_v4(), None, 10)
            .await
            .is_err()
    );
    assert!(
        store
            .discover_events(store.repository_id, Some("../../"), 10)
            .await
            .is_err()
    );
}
