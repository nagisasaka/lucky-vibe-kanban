use serde_json::{Value, json};

use super::*;
async fn fixture() -> (SqlitePool, Uuid, Uuid) {
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::raw_sql("PRAGMA foreign_keys=ON; CREATE TABLE projects(id BLOB PRIMARY KEY);")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::raw_sql(include_str!(
        "../../migrations/20261023000000_design_notes.sql"
    ))
    .execute(&pool)
    .await
    .unwrap();
    let p = Uuid::new_v4();
    let other = Uuid::new_v4();
    for id in [p, other] {
        sqlx::query("INSERT INTO projects VALUES(?)")
            .bind(id)
            .execute(&pool)
            .await
            .unwrap();
    }
    (pool, p, other)
}
fn write(v: Value) -> DesignNoteWrite {
    DesignNoteWrite {
        request_id: Uuid::new_v4(),
        operation: serde_json::from_value(v).unwrap(),
    }
}
async fn create(pool: &SqlitePool, p: Uuid) -> DesignNote {
    DesignNote::write(
        pool,
        p,
        "human",
        write(json!({"action":"create","title":"初版","body":"人間の理想"})),
    )
    .await
    .unwrap()
    .note
}
#[tokio::test]
async fn revisions_restore_and_retries_preserve_first_record() {
    let (pool, p, other) = fixture().await;
    let initial = write(json!({"action":"create","title":"初版","body":"人間の理想"}));
    let n = DesignNote::write(&pool, p, "human", initial.clone())
        .await
        .unwrap()
        .note;
    let update = write(
        json!({"action":"update","note_id":n.id,"expected_revision":1,"title":"現在","body":"新しい人間の判断"}),
    );
    let updated = DesignNote::write(&pool, p, "human", update.clone())
        .await
        .unwrap()
        .note;
    assert_eq!(updated.revision, 2);
    assert_eq!(
        DesignNote::write(&pool, p, "human", initial)
            .await
            .unwrap()
            .note
            .revision,
        1
    );
    assert_eq!(
        DesignNote::write(&pool, p, "human", update.clone())
            .await
            .unwrap()
            .note
            .revision,
        2
    );
    assert!(matches!(
        DesignNote::write(
            &pool,
            p,
            "human",
            DesignNoteWrite {
                request_id: Uuid::new_v4(),
                ..update.clone()
            }
        )
        .await,
        Err(DesignNoteError::Conflict)
    ));
    assert!(matches!(
        DesignNote::write(&pool, other, "human", update.clone()).await,
        Err(DesignNoteError::NotFound)
    ));
    let mut changed = update;
    changed.operation = DesignNoteAction::Create {
        title: "bad".into(),
        body: "bad".into(),
    };
    assert!(matches!(
        DesignNote::write(&pool, p, "human", changed).await,
        Err(DesignNoteError::Conflict)
    ));
    let restored = DesignNote::write(
        &pool,
        p,
        "human",
        write(json!({"action":"restore","note_id":n.id,"expected_revision":2,"revision":1})),
    )
    .await
    .unwrap()
    .note;
    assert_eq!(restored.revision, 3);
    assert_eq!(restored.body, n.body);
    let history = DesignNote::history(&pool, p, n.id, 0).await.unwrap();
    assert_eq!(history.len(), 3);
    assert_eq!(history[1].body, updated.body);
    assert!(
        sqlx::query("DELETE FROM projects WHERE id=?")
            .bind(p)
            .execute(&pool)
            .await
            .is_err()
    );
}
#[tokio::test]
async fn additions_corrections_hiding_and_explicit_integration_preserve_attribution() {
    let (pool, p, other) = fixture().await;
    let n = create(&pool, p).await;
    let content = json!({"heading":"新しい判断","content":"別ツールで満たせれば終了も合理的","kind":"decision","source":"conversation/message-1","speaker":"user","referenced_body_revision":1,"short_quote":"理想"});
    let append = write(json!({"action":"append","note_id":n.id,"addition":content}));
    let a = DesignNote::write(&pool, p, "AI recorder", append.clone())
        .await
        .unwrap()
        .addition
        .unwrap();
    assert_eq!(
        DesignNote::write(&pool, p, "AI recorder", append)
            .await
            .unwrap()
            .addition
            .unwrap()
            .id,
        a.id
    );
    assert_eq!(DesignNote::get(&pool, p, n.id).await.unwrap().revision, 1);
    assert_eq!(a.recorder, "AI recorder");
    assert_eq!(a.speaker, "user");
    let mut correction = content.clone();
    correction["source"] = json!("conversation/message-2");
    correction["content"] = json!("訂正");
    let corrected = DesignNote::write(&pool,p,"human",write(json!({"action":"correct_addition","note_id":n.id,"addition_id":a.id,"expected_revision":1,"addition":correction}))).await.unwrap().addition.unwrap();
    assert_eq!(corrected.recorder, a.recorder);
    assert_eq!(corrected.editor, "human");
    let hidden = DesignNote::write(&pool,p,"human",write(json!({"action":"set_addition_hidden","note_id":n.id,"addition_id":a.id,"expected_revision":2,"hidden":true}))).await.unwrap().addition.unwrap();
    assert!(hidden.hidden);
    assert!(DesignNote::write(&pool,p,"human",write(json!({"action":"integrate","note_id":n.id,"expected_revision":1,"addition_id":a.id,"expected_addition_revision":3}))).await.is_err());
    DesignNote::write(&pool,p,"human",write(json!({"action":"set_addition_hidden","note_id":n.id,"addition_id":a.id,"expected_revision":3,"hidden":false}))).await.unwrap();
    let integrated = DesignNote::write(&pool,p,"human",write(json!({"action":"integrate","note_id":n.id,"expected_revision":1,"addition_id":a.id,"expected_addition_revision":4}))).await.unwrap().note;
    assert_eq!(integrated.revision, 2);
    assert!(integrated.body.starts_with(&n.body));
    assert!(integrated.body.contains("conversation/message-2"));
    let history = DesignNote::addition_history(&pool, p, n.id, a.id, 0)
        .await
        .unwrap();
    assert_eq!(history.len(), 4);
    assert_eq!(history[0].source, a.source);
    assert_eq!(history[0].content, a.content);
    assert!(
        DesignNote::addition_history(&pool, other, n.id, a.id, 0)
            .await
            .is_err()
    );
    assert_eq!(
        DesignNote::history(&pool, p, n.id, 0).await.unwrap()[0].body,
        n.body
    );
    let mut invalid = content;
    invalid["short_quote"] = json!("存在しない引用");
    assert!(
        DesignNote::write(
            &pool,
            p,
            "AI",
            write(json!({"action":"append","note_id":n.id,"addition":invalid}))
        )
        .await
        .is_err()
    );
}
#[tokio::test]
async fn concurrent_body_writes_have_one_winner() {
    let (pool, p, _) = fixture().await;
    let n = create(&pool, p).await;
    let input = write(
        json!({"action":"update","note_id":n.id,"expected_revision":1,"title":"next","body":"new"}),
    );
    let other = DesignNoteWrite {
        request_id: Uuid::new_v4(),
        ..input.clone()
    };
    let (a, b) = tokio::join!(
        DesignNote::write(&pool, p, "UI", input),
        DesignNote::write(&pool, p, "MCP", other)
    );
    assert_eq!(usize::from(a.is_ok()) + usize::from(b.is_ok()), 1);
    assert_eq!(
        DesignNote::history(&pool, p, n.id, 0).await.unwrap().len(),
        2
    );
}

#[tokio::test]
async fn independent_sqlite_connections_reject_stale_body_and_deduplicate_retries() {
    let directory = tempfile::tempdir().unwrap();
    let options = sqlx::sqlite::SqliteConnectOptions::new()
        .filename(directory.path().join("notes.sqlite"))
        .create_if_missing(true)
        .foreign_keys(true)
        .busy_timeout(std::time::Duration::from_secs(10));
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(4)
        .connect_with(options)
        .await
        .unwrap();
    sqlx::raw_sql("CREATE TABLE projects(id BLOB PRIMARY KEY);")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::raw_sql(include_str!(
        "../../migrations/20261023000000_design_notes.sql"
    ))
    .execute(&pool)
    .await
    .unwrap();
    let p = Uuid::new_v4();
    sqlx::query("INSERT INTO projects VALUES(?)")
        .bind(p)
        .execute(&pool)
        .await
        .unwrap();
    let n = create(&pool, p).await;
    let input = write(
        json!({"action":"update","note_id":n.id,"expected_revision":1,"title":"next","body":"new"}),
    );
    let other = DesignNoteWrite {
        request_id: Uuid::new_v4(),
        ..input.clone()
    };
    let (a, b) = tokio::join!(
        DesignNote::write(&pool, p, "UI", input),
        DesignNote::write(&pool, p, "MCP", other)
    );
    assert_eq!(usize::from(a.is_ok()) + usize::from(b.is_ok()), 1);
    assert!(matches!(
        a.err().or(b.err()),
        Some(DesignNoteError::Conflict)
    ));
    assert_eq!(
        DesignNote::history(&pool, p, n.id, 0).await.unwrap().len(),
        2
    );
    let input = write(
        json!({"action":"update","note_id":n.id,"expected_revision":2,"title":"third","body":"same retry"}),
    );
    let (a, b) = tokio::join!(
        DesignNote::write(&pool, p, "UI", input.clone()),
        DesignNote::write(&pool, p, "UI", input)
    );
    assert_eq!(a.unwrap().note.revision, 3);
    assert_eq!(b.unwrap().note.revision, 3);
    assert_eq!(
        DesignNote::history(&pool, p, n.id, 0).await.unwrap().len(),
        3
    );
    pool.close().await;
}

#[tokio::test]
async fn invalid_references_and_failed_writes_leave_no_partial_history() {
    let (pool, p, other) = fixture().await;
    let note = create(&pool, p).await;
    let foreign = create(&pool, other).await;
    let content = json!({"heading":"idea","content":"proposal","kind":"proposal","source":"message","speaker":"AI","referenced_body_revision":1});
    let addition = DesignNote::write(
        &pool,
        other,
        "AI",
        write(json!({"action":"append","note_id":foreign.id,"addition":content})),
    )
    .await
    .unwrap()
    .addition
    .unwrap();
    for action in [
        json!({"action":"correct_addition","note_id":note.id,"addition_id":addition.id,"expected_revision":1,"addition":content}),
        json!({"action":"integrate","note_id":note.id,"expected_revision":1,"addition_id":addition.id,"expected_addition_revision":1}),
        json!({"action":"restore","note_id":note.id,"expected_revision":1,"revision":99}),
        json!({"action":"update","note_id":note.id,"expected_revision":1,"title":" ","body":"invalid"}),
    ] {
        assert!(
            DesignNote::write(&pool, p, "human", write(action))
                .await
                .is_err()
        );
    }
    for (key, value) in [
        ("kind", json!("approved_by_user")),
        ("referenced_body_revision", json!(99)),
        ("speaker", json!("")),
        ("source", json!("")),
    ] {
        let mut invalid = content.clone();
        invalid[key] = value;
        assert!(
            DesignNote::write(
                &pool,
                p,
                "AI",
                write(json!({"action":"append","note_id":note.id,"addition":invalid}))
            )
            .await
            .is_err()
        );
    }
    assert_eq!(
        DesignNote::history(&pool, p, note.id, 0)
            .await
            .unwrap()
            .len(),
        1
    );
    assert!(
        DesignNote::detail(&pool, p, note.id)
            .await
            .unwrap()
            .additions
            .is_empty()
    );
    assert_eq!(
        DesignNote::addition_history(&pool, other, foreign.id, addition.id, 0)
            .await
            .unwrap()
            .len(),
        1
    );
    let receipts: i64 = sqlx::query_scalar("SELECT count(*) FROM design_note_requests")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(receipts, 3);
}
