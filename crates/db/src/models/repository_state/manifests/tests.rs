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
    assert!(
        store.files.legacy_events().unwrap().is_empty(),
        "no dual write"
    );
    std::fs::remove_dir_all(temp.path().join("knowledge/events")).unwrap();
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
async fn legacy_import_is_atomic_retryable_and_keeps_events_without_run_metadata() {
    let (temp, pool, store) = fixture().await;
    // More than one page, none with auxiliary Run files. Pre-existing identical
    // DB content represents a prior partial/manual attempt and is not duplicated.
    let mut expected = Vec::new();
    for number in 1..=105 {
        let value = event(&store, Uuid::from_u128(number));
        store.files.legacy_publish_event(&value).unwrap();
        expected.push(value);
    }
    store.publish_event(&expected[0]).await.unwrap();
    let broken = temp
        .path()
        .join("knowledge/events/ffffffff-ffff-ffff-ffff-ffffffffffff.json");
    std::fs::write(&broken, "{").unwrap();
    let error = store.import_legacy_manifests().await.unwrap_err();
    assert!(error.to_string().contains("ffffffff"));
    assert_eq!(
        store.events().await.unwrap().len(),
        1,
        "all imported rows roll back"
    );
    let markers: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM repository_manifest_imports")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(markers, 0);
    std::fs::remove_file(&broken).unwrap();
    // A failed DB write simulates interruption after reading and inserting pages.
    sqlx::query("CREATE TRIGGER stop_import BEFORE INSERT ON repository_change_manifests WHEN NEW.id=x'00000000000000000000000000000069' BEGIN SELECT RAISE(ABORT,'simulated interruption'); END")
        .execute(&pool).await.unwrap();
    assert!(store.import_legacy_manifests().await.is_err());
    assert_eq!(store.events().await.unwrap().len(), 1);
    sqlx::query("DROP TRIGGER stop_import")
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(store.import_legacy_manifests().await.unwrap(), 105);
    assert_eq!(store.events().await.unwrap().len(), 105);
    for value in &expected {
        assert_eq!(
            store.event(value.event_id).await.unwrap().as_ref(),
            Some(value)
        );
    }
    assert_eq!(store.import_legacy_manifests().await.unwrap(), 0);
    assert_eq!(
        store.files.legacy_events().unwrap().len(),
        105,
        "old files retained"
    );
    // Completed cutover and all normal readers have no dependency on legacy files.
    std::fs::remove_dir_all(temp.path().join("knowledge/events")).unwrap();
    assert_eq!(store.import_legacy_manifests().await.unwrap(), 0);
    assert_eq!(store.events().await.unwrap().len(), 105);
}

#[tokio::test]
async fn legacy_conflict_and_foreign_or_misnamed_records_are_reported_without_overwrite() {
    let (temp, _, store) = fixture().await;
    let original = event(&store, Uuid::new_v4());
    store.publish_event(&original).await.unwrap();
    let mut other = original.clone();
    other.source_commit = "c".repeat(40);
    store.files.legacy_publish_event(&other).unwrap();
    assert!(
        store
            .import_legacy_manifests()
            .await
            .unwrap_err()
            .to_string()
            .contains("conflict")
    );
    assert_eq!(
        store.event(original.event_id).await.unwrap(),
        Some(original.clone())
    );
    let path = temp
        .path()
        .join(format!("knowledge/events/{}.json", original.event_id));
    other.repository_id = Uuid::new_v4();
    std::fs::write(&path, serde_json::to_vec(&other).unwrap()).unwrap();
    assert!(
        store
            .import_legacy_manifests()
            .await
            .unwrap_err()
            .to_string()
            .contains("another registered repository")
    );
    other = original.clone();
    other.event_id = Uuid::new_v4();
    std::fs::write(&path, serde_json::to_vec(&other).unwrap()).unwrap();
    assert!(
        store
            .import_legacy_manifests()
            .await
            .unwrap_err()
            .to_string()
            .contains("identity")
    );
    assert_eq!(store.events().await.unwrap(), vec![original]);
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

#[tokio::test]
async fn import_checks_available_run_identity_but_does_not_infer_project_membership() {
    let (temp, _pool, store) = fixture().await;
    let value = event(&store, Uuid::new_v4());
    store.files.legacy_publish_event(&value).unwrap();
    let mut run = files::RepositoryMemoryRun {
        run_id: value.event_id,
        repository_id: store.repository_id,
        workspace_id: Uuid::new_v4(),
        task_id: None,
        created_at: value.created_at,
        base_commit: value.base_commit.clone(),
        target_branch: "main".into(),
        repository_path: temp.path().join("missing-worktree"),
        draft_path: store.draft_path(value.event_id),
        wiki_status: files::RepositoryWikiStatus::Current,
        finalize_source: true,
    };
    store.save_run(&run).unwrap();
    assert!(
        store
            .import_legacy_manifests()
            .await
            .unwrap_err()
            .to_string()
            .contains("Run context mismatch")
    );
    assert!(store.events().await.unwrap().is_empty());
    run.workspace_id = value.workspace_id;
    std::fs::write(
        temp.path()
            .join(format!("knowledge/runs/{}.json", value.event_id)),
        serde_json::to_vec(&run).unwrap(),
    )
    .unwrap();
    assert_eq!(store.import_legacy_manifests().await.unwrap(), 1);
    assert_eq!(store.event(value.event_id).await.unwrap(), Some(value));
}

#[tokio::test]
async fn missing_legacy_storage_is_not_acknowledged_as_an_empty_import() {
    let (temp, pool, store) = fixture().await;
    std::fs::remove_dir(temp.path().join("knowledge/events")).unwrap();
    assert!(store.import_legacy_manifests().await.is_err());
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM repository_manifest_imports")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
    std::fs::create_dir(temp.path().join("knowledge/events")).unwrap();
    assert_eq!(store.import_legacy_manifests().await.unwrap(), 0);
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM repository_manifest_imports")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1);
}
