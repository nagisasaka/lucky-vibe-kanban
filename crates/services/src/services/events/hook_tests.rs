use std::{sync::Arc, time::Duration};

use sqlx::sqlite::SqlitePoolOptions;
use tokio::sync::{RwLock, broadcast};
use utils::{log_msg::LogMsg, msg_store::MsgStore};
use uuid::Uuid;

use super::EventService;

async fn next_patch(receiver: &mut broadcast::Receiver<LogMsg>) -> serde_json::Value {
    let message = tokio::time::timeout(Duration::from_secs(5), receiver.recv())
        .await
        .expect("EventService hook must publish a patch")
        .expect("event receiver must remain open");
    let LogMsg::JsonPatch(patch) = message else {
        panic!("expected a JSON patch");
    };
    serde_json::to_value(patch).unwrap()
}

#[tokio::test]
async fn sqlite_hooks_publish_scratch_insert_update_and_delete() {
    // A single private connection gives the real hook and its asynchronous
    // model lookup the same isolated DB, without touching instance state.
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::raw_sql(include_str!(
        "../../../../db/migrations/20251120000001_refactor_to_scratch.sql"
    ))
    .execute(&pool)
    .await
    .unwrap();
    let store = Arc::new(MsgStore::new());
    let mut receiver = store.get_receiver();
    let hook = EventService::create_hook(
        store,
        Arc::new(RwLock::new(0)),
        db::DBService { pool: pool.clone() },
    );
    {
        let mut connection = pool.acquire().await.unwrap();
        hook(&mut connection).await.unwrap();
    }

    let id = Uuid::new_v4();
    let payload = serde_json::json!({
        "type": "WORKSPACE_NOTES",
        "data": { "content": "original note" }
    });
    sqlx::query("INSERT INTO scratch(id,scratch_type,payload) VALUES(?,'WORKSPACE_NOTES',?)")
        .bind(id)
        .bind(payload.to_string())
        .execute(&pool)
        .await
        .unwrap();
    let inserted = next_patch(&mut receiver).await;
    assert_eq!(inserted[0]["op"], "add");
    assert_eq!(inserted[0]["path"], "/scratch");
    assert_eq!(inserted[0]["value"]["id"], id.to_string());
    assert_eq!(inserted[0]["value"]["payload"], payload);

    let updated_payload = serde_json::json!({
        "type": "WORKSPACE_NOTES",
        "data": { "content": "updated note" }
    });
    sqlx::query("UPDATE scratch SET payload=? WHERE id=? AND scratch_type='WORKSPACE_NOTES'")
        .bind(updated_payload.to_string())
        .bind(id)
        .execute(&pool)
        .await
        .unwrap();
    let updated = next_patch(&mut receiver).await;
    assert_eq!(updated[0]["op"], "replace");
    assert_eq!(updated[0]["value"]["id"], id.to_string());
    assert_eq!(updated[0]["value"]["payload"], updated_payload);

    sqlx::query("DELETE FROM scratch WHERE id=? AND scratch_type='WORKSPACE_NOTES'")
        .bind(id)
        .execute(&pool)
        .await
        .unwrap();
    // This is emitted from preupdate old-column values, exercising both UUID
    // and String decoding before SQLx releases the callback's borrowed values.
    assert_eq!(
        next_patch(&mut receiver).await,
        serde_json::json!([{
            "op": "replace",
            "path": "/scratch",
            "value": {
                "id": id,
                "payload": { "type": "WORKSPACE_NOTES" },
                "deleted": true
            }
        }])
    );
    pool.close().await;
}
