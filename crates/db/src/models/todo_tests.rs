use serde_json::json;

use super::*;

async fn fixture() -> (SqlitePool, Uuid, Uuid) {
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::raw_sql("PRAGMA foreign_keys=ON; CREATE TABLE projects(id BLOB PRIMARY KEY); CREATE TABLE local_issues(id BLOB PRIMARY KEY,project_id BLOB NOT NULL REFERENCES projects(id));").execute(&pool).await.unwrap();
    sqlx::raw_sql(include_str!(
        "../../migrations/20261018000000_project_todos.sql"
    ))
    .execute(&pool)
    .await
    .unwrap();
    let project = Uuid::new_v4();
    let foreign = Uuid::new_v4();
    for id in [project, foreign] {
        sqlx::query("INSERT INTO projects VALUES(?)")
            .bind(id)
            .execute(&pool)
            .await
            .unwrap();
    }
    (pool, project, foreign)
}
fn note(body: &str) -> CreateTodo {
    CreateTodo {
        id: Uuid::new_v4(),
        body: body.into(),
        category: " UI ".into(),
        issue_id: None,
    }
}
fn changes(value: serde_json::Value) -> UpdateTodo {
    serde_json::from_value(value).unwrap()
}
fn page(completed: bool, offset: u32) -> TodoQuery {
    TodoQuery {
        include_completed: completed,
        limit: 1,
        offset,
    }
}

#[tokio::test]
async fn lifecycle_preserves_notes_and_rejects_stale_writes() {
    let (pool, p, _) = fixture().await;
    let input = note("改行を含む観察\n原因はまだ不明？");
    let a = Todo::create(&pool, p, input.clone()).await.unwrap();
    assert_eq!(a.category, "UI");
    assert_eq!(a.body, input.body);
    assert_eq!(Todo::create(&pool, p, input).await.unwrap().id, a.id);
    let (first, second) = tokio::join!(
        Todo::update(
            &pool,
            p,
            a.id,
            changes(json!({"expected_revision":1,"completed":true}))
        ),
        Todo::update(
            &pool,
            p,
            a.id,
            changes(json!({"expected_revision":1,"body":"concurrent edit"}))
        )
    );
    assert_eq!(usize::from(first.is_ok()) + usize::from(second.is_ok()), 1);
    assert!(matches!(
        first.err().or(second.err()),
        Some(TodoError::Conflict)
    ));
    let a = Todo::get(&pool, p, a.id).await.unwrap();
    let a = Todo::update(
        &pool,
        p,
        a.id,
        changes(json!({"expected_revision":a.revision,"completed":true})),
    )
    .await
    .unwrap();
    assert!(
        Todo::list(&pool, p, page(false, 0))
            .await
            .unwrap()
            .items
            .is_empty()
    );
    assert_eq!(
        Todo::list(&pool, p, page(true, 0))
            .await
            .unwrap()
            .items
            .len(),
        1
    );
    let a = Todo::update(
        &pool,
        p,
        a.id,
        changes(json!({"expected_revision":a.revision,"completed":false,"category":""})),
    )
    .await
    .unwrap();
    assert!(a.category.is_empty());
    assert!(matches!(
        Todo::delete(&pool, p, a.id, 1).await,
        Err(TodoError::Conflict)
    ));
    Todo::delete(&pool, p, a.id, a.revision).await.unwrap();
    assert!(matches!(
        Todo::get(&pool, p, a.id).await,
        Err(TodoError::NotFound)
    ));
}

#[tokio::test]
async fn project_and_card_boundaries_are_enforced() {
    let (pool, p, foreign) = fixture().await;
    let a = Todo::create(&pool, p, note("original")).await.unwrap();
    assert!(matches!(
        Todo::get(&pool, foreign, a.id).await,
        Err(TodoError::NotFound)
    ));
    assert!(matches!(
        Todo::update(
            &pool,
            foreign,
            a.id,
            changes(json!({"expected_revision":1,"body":"wrong"}))
        )
        .await,
        Err(TodoError::NotFound)
    ));
    assert!(matches!(
        Todo::delete(&pool, foreign, a.id, 1).await,
        Err(TodoError::NotFound)
    ));
    let card = Uuid::new_v4();
    sqlx::query("INSERT INTO local_issues VALUES(?,?)")
        .bind(card)
        .bind(foreign)
        .execute(&pool)
        .await
        .unwrap();
    assert!(matches!(
        Todo::update(
            &pool,
            p,
            a.id,
            changes(json!({"expected_revision":1,"issue_id":card}))
        )
        .await,
        Err(TodoError::InvalidIssue)
    ));
    assert!(
        sqlx::query("UPDATE project_todos SET issue_id=? WHERE id=?")
            .bind(card)
            .bind(a.id)
            .execute(&pool)
            .await
            .is_err()
    );
    let own_card = Uuid::new_v4();
    sqlx::query("INSERT INTO local_issues VALUES(?,?)")
        .bind(own_card)
        .bind(p)
        .execute(&pool)
        .await
        .unwrap();
    let a = Todo::update(
        &pool,
        p,
        a.id,
        changes(json!({"expected_revision":1,"issue_id":own_card})),
    )
    .await
    .unwrap();
    let a = Todo::update(
        &pool,
        p,
        a.id,
        changes(json!({"expected_revision":a.revision,"clear_issue":true})),
    )
    .await
    .unwrap();
    assert!(a.issue_id.is_none());
    assert_eq!(a.body, "original");
}

#[tokio::test]
async fn validation_and_pagination_do_not_lose_or_invent_items() {
    let (pool, p, foreign) = fixture().await;
    for body in ["", " \n\t", &"x".repeat(16001)] {
        assert!(matches!(
            Todo::create(&pool, p, note(body)).await,
            Err(TodoError::InvalidText)
        ));
    }
    let a = Todo::create(&pool, p, note("first")).await.unwrap();
    let b = Todo::create(&pool, p, note("second")).await.unwrap();
    Todo::create(&pool, foreign, note("other project"))
        .await
        .unwrap();
    let first = Todo::list(&pool, p, page(true, 0)).await.unwrap();
    let second = Todo::list(&pool, p, page(true, first.next_offset.unwrap()))
        .await
        .unwrap();
    assert!(second.next_offset.is_none());
    let ids: std::collections::HashSet<_> = first
        .items
        .into_iter()
        .chain(second.items)
        .map(|i| i.id)
        .collect();
    assert_eq!(ids, std::collections::HashSet::from([a.id, b.id]));
    let conflicting = CreateTodo {
        id: a.id,
        ..note("replacement")
    };
    assert!(matches!(
        Todo::create(&pool, p, conflicting).await,
        Err(TodoError::Conflict)
    ));
}
