use std::str::FromStr;

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
    sqlx::raw_sql(include_str!(
        "../../migrations/20261024000000_todo_creation_source.sql"
    ))
    .execute(&pool)
    .await
    .unwrap();
    sqlx::raw_sql(include_str!(
        "../../migrations/20261030000000_todo_numbers.sql"
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
        search: None,
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
    let counter: i64 =
        sqlx::query_scalar("SELECT last_number FROM project_todo_counters WHERE project_id=?")
            .bind(p)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(counter, a.number);
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

#[tokio::test]
async fn creation_source_is_immutable_even_across_retries() {
    let (pool, p, _) = fixture().await;
    for source in [
        TodoCreationSource::Ui,
        TodoCreationSource::Mcp,
        TodoCreationSource::Unknown,
    ] {
        let input = note("original");
        let a = Todo::create_from(&pool, p, input.clone(), source)
            .await
            .unwrap();
        let retry = Todo::create_from(&pool, p, input, TodoCreationSource::Mcp)
            .await
            .unwrap();
        assert_eq!(retry.creation_source, source);
        let updated = Todo::update(&pool, p, a.id, changes(json!({"expected_revision":1,"body":"edited","category":"changed","completed":true}))).await.unwrap();
        assert_eq!(updated.creation_source, source);
        assert_eq!(
            Todo::get(&pool, p, a.id).await.unwrap().creation_source,
            source
        );
        assert!(
            sqlx::query("UPDATE project_todos SET creation_source=? WHERE id=?")
                .bind(if source == TodoCreationSource::Ui {
                    "mcp"
                } else {
                    "ui"
                })
                .bind(a.id)
                .execute(&pool)
                .await
                .is_err()
        );
    }
    assert!(
        serde_json::from_value::<CreateTodo>(
            json!({"id":Uuid::new_v4(),"body":"fake","creation_source":"ui"})
        )
        .is_err()
    );
    assert!(
        serde_json::from_value::<UpdateTodo>(json!({"expected_revision":1,"creation_source":"ui"}))
            .is_err()
    );
}

#[tokio::test]
async fn creation_source_migration_preserves_all_existing_columns_and_order() {
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    sqlx::raw_sql("CREATE TABLE projects(id BLOB PRIMARY KEY); CREATE TABLE local_issues(id BLOB PRIMARY KEY, project_id BLOB);").execute(&pool).await.unwrap();
    sqlx::raw_sql(include_str!(
        "../../migrations/20261018000000_project_todos.sql"
    ))
    .execute(&pool)
    .await
    .unwrap();
    let p = Uuid::new_v4();
    let issue = Uuid::new_v4();
    sqlx::query("INSERT INTO projects VALUES(?)")
        .bind(p)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO local_issues VALUES(?,?)")
        .bind(issue)
        .bind(p)
        .execute(&pool)
        .await
        .unwrap();
    for i in 1..=3 {
        sqlx::query("INSERT INTO project_todos(id,project_id,body,category,completed,issue_id,revision,created_at,updated_at) VALUES(?,?,'line 1\nline 2','category',1,?,7,?, 'preserved')")
            .bind(Uuid::from_u128(i)).bind(p).bind(issue).bind(format!("2026-10-2{i}")).execute(&pool).await.unwrap();
    }
    let snapshot = "SELECT json_array(hex(id),hex(project_id),body,category,completed,hex(issue_id),revision,created_at,updated_at) FROM project_todos ORDER BY created_at DESC,id";
    let before: Vec<String> = sqlx::query_scalar(snapshot).fetch_all(&pool).await.unwrap();
    sqlx::raw_sql(include_str!(
        "../../migrations/20261024000000_todo_creation_source.sql"
    ))
    .execute(&pool)
    .await
    .unwrap();
    sqlx::raw_sql(include_str!(
        "../../migrations/20261030000000_todo_numbers.sql"
    ))
    .execute(&pool)
    .await
    .unwrap();
    let after: Vec<String> = sqlx::query_scalar(snapshot).fetch_all(&pool).await.unwrap();
    assert_eq!(before, after);
    let page = Todo::list(
        &pool,
        p,
        TodoQuery {
            search: None,
            include_completed: true,
            limit: 100,
            offset: 0,
        },
    )
    .await
    .unwrap();
    assert_eq!(page.items.len(), 3);
    assert!(
        page.items
            .iter()
            .all(|t| t.creation_source == TodoCreationSource::Unknown)
    );
}

#[tokio::test]
async fn search_filters_full_notes_before_offset_and_completion() {
    let (pool, p, other) = fixture().await;
    for (n, body, category, completed) in [
        (1, "unrelated", "other", false),
        (2, "Ä body hit", "other", false),
        (3, "different", "Ä CATEGORY", false),
        (4, "ä completed", "other", true),
    ] {
        let mut input = note(body);
        input.id = Uuid::from_u128(n);
        input.category = category.into();
        let todo = Todo::create(&pool, p, input).await.unwrap();
        sqlx::query("UPDATE project_todos SET created_at=?,completed=? WHERE id=?")
            .bind(format!("2026-01-0{n}"))
            .bind(completed)
            .bind(todo.id)
            .execute(&pool)
            .await
            .unwrap();
    }
    Todo::create(&pool, other, note("ä foreign")).await.unwrap();
    let q = |offset, completed, search: &str| TodoQuery {
        search: Some(search.into()),
        include_completed: completed,
        limit: 1,
        offset,
    };
    let first = Todo::list(&pool, p, q(0, false, "ä")).await.unwrap();
    assert_eq!(first.items[0].id, Uuid::from_u128(3));
    assert_eq!(first.next_offset, Some(1));
    let last = Todo::list(&pool, p, q(1, false, "Ä")).await.unwrap();
    assert_eq!(last.items[0].id, Uuid::from_u128(2));
    assert_eq!(last.next_offset, None);
    assert_eq!(
        Todo::list(&pool, p, q(0, true, "ä")).await.unwrap().items[0].id,
        Uuid::from_u128(4)
    );
    assert!(
        Todo::list(&pool, p, q(0, true, " Ä "))
            .await
            .unwrap()
            .items
            .is_empty()
    );
    assert!(
        Todo::list(&pool, p, q(0, true, "%_"))
            .await
            .unwrap()
            .items
            .is_empty()
    );
    let mut all = q(0, true, "");
    all.limit = 100;
    assert_eq!(Todo::list(&pool, p, all).await.unwrap().items.len(), 4);
}

#[tokio::test]
async fn numbers_serialize_writers_persist_and_never_reuse() {
    let dir = tempfile::tempdir().unwrap();
    let url = format!(
        "sqlite:{}?mode=rwc",
        dir.path().join("todos.sqlite").display()
    );
    let options = sqlx::sqlite::SqliteConnectOptions::from_str(&url)
        .unwrap()
        .busy_timeout(std::time::Duration::from_secs(30))
        .foreign_keys(true);
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(8)
        .connect_with(options.clone())
        .await
        .unwrap();
    sqlx::raw_sql("CREATE TABLE projects(id BLOB PRIMARY KEY); CREATE TABLE local_issues(id BLOB PRIMARY KEY,project_id BLOB);").execute(&pool).await.unwrap();
    for migration in [
        include_str!("../../migrations/20261018000000_project_todos.sql"),
        include_str!("../../migrations/20261024000000_todo_creation_source.sql"),
        include_str!("../../migrations/20261030000000_todo_numbers.sql"),
    ] {
        sqlx::raw_sql(migration).execute(&pool).await.unwrap();
    }
    let p = Uuid::new_v4();
    let other = Uuid::new_v4();
    for id in [p, other] {
        sqlx::query("INSERT INTO projects VALUES(?)")
            .bind(id)
            .execute(&pool)
            .await
            .unwrap();
    }
    let mut tasks = tokio::task::JoinSet::new();
    for _ in 0..32 {
        let pool = pool.clone();
        tasks.spawn(async move { Todo::create(&pool, p, note("concurrent")).await.unwrap() });
    }
    let mut numbers = Vec::new();
    while let Some(todo) = tasks.join_next().await {
        numbers.push(todo.unwrap().number);
    }
    numbers.sort();
    assert_eq!(numbers, (1..=32).collect::<Vec<_>>());
    let last = Todo::resolve(&pool, p, "TODO-32").await.unwrap();
    Todo::delete(&pool, p, last.id, 1).await.unwrap();
    assert_eq!(
        Todo::create(&pool, p, note("after tail"))
            .await
            .unwrap()
            .number,
        33
    );
    sqlx::query("DELETE FROM project_todos WHERE project_id=?")
        .bind(p)
        .execute(&pool)
        .await
        .unwrap();
    pool.close().await;
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .connect_with(options)
        .await
        .unwrap();
    let a = Todo::create(&pool, p, note("after reopen")).await.unwrap();
    assert_eq!(a.number, 34);
    assert_eq!(a.display_id, "TODO-34");
    assert_eq!(
        Todo::create(&pool, other, note("independent"))
            .await
            .unwrap()
            .number,
        1
    );
    assert_eq!(
        Todo::resolve(&pool, p, &a.id.to_string()).await.unwrap().id,
        a.id
    );
    assert_eq!(Todo::resolve(&pool, p, "TODO-34").await.unwrap().id, a.id);
    for invalid in [
        "TODO-0",
        "TODO-01",
        "TODO--1",
        "todo-1",
        "TODO-",
        "TODO-1/..",
        "TODO-9223372036854775808",
    ] {
        assert!(matches!(
            Todo::resolve(&pool, p, invalid).await,
            Err(TodoError::InvalidRequest)
        ));
    }
    assert!(matches!(
        Todo::resolve(&pool, other, "TODO-34").await,
        Err(TodoError::NotFound)
    ));
    assert!(matches!(
        Todo::resolve(&pool, other, &a.id.to_string()).await,
        Err(TodoError::NotFound)
    ));
    assert!(matches!(
        Todo::resolve(&pool, p, "TODO-999").await,
        Err(TodoError::NotFound)
    ));
    let updated = Todo::update(
        &pool,
        p,
        a.id,
        changes(json!({"expected_revision":1,"body":"edited","category":"new","completed":true})),
    )
    .await
    .unwrap();
    assert_eq!(updated.display_id, a.display_id);
    assert!(
        sqlx::query("UPDATE project_todos SET number=100 WHERE id=?")
            .bind(a.id)
            .execute(&pool)
            .await
            .is_err()
    );
    assert!(
        sqlx::query("UPDATE project_todo_counters SET last_number=1 WHERE project_id=?")
            .bind(p)
            .execute(&pool)
            .await
            .is_err()
    );
    pool.close().await;
}

#[tokio::test]
async fn number_migration_orders_ties_and_preserves_every_existing_attribute() {
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::raw_sql("PRAGMA foreign_keys=ON; CREATE TABLE projects(id BLOB PRIMARY KEY); CREATE TABLE local_issues(id BLOB PRIMARY KEY,project_id BLOB);").execute(&pool).await.unwrap();
    for migration in [
        include_str!("../../migrations/20261018000000_project_todos.sql"),
        include_str!("../../migrations/20261024000000_todo_creation_source.sql"),
    ] {
        sqlx::raw_sql(migration).execute(&pool).await.unwrap();
    }
    let p = Uuid::new_v4();
    let other = Uuid::new_v4();
    let issue = Uuid::new_v4();
    for id in [p, other] {
        sqlx::query("INSERT INTO projects VALUES(?)")
            .bind(id)
            .execute(&pool)
            .await
            .unwrap();
    }
    sqlx::query("INSERT INTO local_issues VALUES(?,?)")
        .bind(issue)
        .bind(p)
        .execute(&pool)
        .await
        .unwrap();
    for (n, date, source) in [
        (3, "2026-10-01", "ui"),
        (2, "2026-10-02", "mcp"),
        (1, "2026-10-02", "unknown"),
    ] {
        sqlx::query("INSERT INTO project_todos(id,project_id,body,category,completed,issue_id,revision,created_at,updated_at,creation_source) VALUES(?,?,'本文\n保持',' category ',1,?,9,?,'original',?)").bind(Uuid::from_u128(n)).bind(p).bind(issue).bind(date).bind(source).execute(&pool).await.unwrap();
    }
    let snapshot = "SELECT json_array(hex(id),hex(project_id),body,category,completed,hex(issue_id),revision,created_at,updated_at,creation_source) FROM project_todos ORDER BY id";
    let before: Vec<String> = sqlx::query_scalar(snapshot).fetch_all(&pool).await.unwrap();
    sqlx::raw_sql(include_str!(
        "../../migrations/20261030000000_todo_numbers.sql"
    ))
    .execute(&pool)
    .await
    .unwrap();
    let after: Vec<String> = sqlx::query_scalar(snapshot).fetch_all(&pool).await.unwrap();
    assert_eq!(before, after);
    for (number, id) in [(1, 3), (2, 1), (3, 2)] {
        assert_eq!(
            Todo::resolve(&pool, p, &format!("TODO-{number}"))
                .await
                .unwrap()
                .id,
            Uuid::from_u128(id)
        );
    }
    assert_eq!(
        Todo::create(&pool, p, note("next")).await.unwrap().number,
        4
    );
    assert_eq!(
        Todo::create(&pool, other, note("first"))
            .await
            .unwrap()
            .number,
        1
    );
    let a = Todo::resolve(&pool, p, "TODO-1").await.unwrap();
    let a = Todo::update(
        &pool,
        p,
        a.id,
        changes(json!({"expected_revision":9,"clear_issue":true})),
    )
    .await
    .unwrap();
    assert_eq!(a.number, 1);
    let a = Todo::update(
        &pool,
        p,
        a.id,
        changes(json!({"expected_revision":10,"issue_id":issue})),
    )
    .await
    .unwrap();
    assert_eq!(a.number, 1);
}
