use std::borrow::Cow;

use sqlx::{migrate::Migrator, sqlite::SqlitePoolOptions};

#[tokio::test]
async fn retires_populated_chat_without_removing_board_or_question_state() {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    let all = sqlx::migrate!("./migrations");
    let previous = Migrator {
        migrations: Cow::Owned(
            all.iter()
                .filter(|m| m.version < 20261016000000)
                .cloned()
                .collect(),
        ),
        ..Migrator::DEFAULT
    };
    previous.run(&pool).await.unwrap();
    sqlx::raw_sql(
        "INSERT INTO projects(id,name) VALUES (X'01','Retained project');
         INSERT INTO workspaces(id,branch) VALUES (X'02','retained');
         INSERT INTO sessions(id,workspace_id) VALUES (X'03',X'02');
         INSERT INTO agent_question_answers(question_id,session_id,request_id,actor,answer)
           VALUES ('question',X'03',X'04','workspace','Keep this answer');
         INSERT INTO local_project_statuses(id,project_id,name,color,sort_order)
           VALUES (X'05',X'01','Todo','blue',0);
         INSERT INTO local_issues(id,project_id,issue_number,simple_id,status_id,title,sort_order)
           VALUES (X'06',X'01',1,'TEST-1',X'05','Retained issue',0);
         INSERT INTO project_conversations(id,project_id,owner_user_id)
           VALUES (X'07',X'01',X'08');
         INSERT INTO project_chat_messages(id,conversation_id,role,content)
           VALUES (X'09',X'07','user','Retired chat');
         INSERT INTO project_chat_operations(id,message_id,call_id,name,arguments)
           VALUES (X'0A',X'09','call','tool','{}');",
    )
    .execute(&pool)
    .await
    .unwrap();
    all.run(&pool).await.unwrap();
    // Reopening an upgraded database must remain safe.
    all.run(&pool).await.unwrap();
    let retired: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM sqlite_master
         WHERE name LIKE 'project_chat_%' OR name = 'project_conversations'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(retired, 0);
    for table in [
        "projects",
        "local_issues",
        "workspaces",
        "sessions",
        "agent_question_answers",
    ] {
        let count: i64 = sqlx::query_scalar(&format!("SELECT count(*) FROM {table}"))
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 1, "{table}");
    }
    let guard: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM sqlite_master WHERE type='trigger' AND name='agent_input_still_pending'",
    ).fetch_one(&pool).await.unwrap();
    assert_eq!(guard, 1);
    // Board writes must no longer try to append to the retired event outbox.
    sqlx::query("UPDATE local_issues SET title='Updated issue' WHERE id=X'06'")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO agent_question_answers(question_id,session_id,request_id,actor,answer)
         VALUES ('new-question',X'03',X'0B','workspace','New answer')",
    )
    .execute(&pool)
    .await
    .unwrap();
    let violations: Vec<(String, Option<i64>, String, i64)> =
        sqlx::query_as("PRAGMA foreign_key_check")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert!(violations.is_empty(), "{violations:?}");
}
