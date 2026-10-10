use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::{FromRow, SqlitePool};
use ts_rs::TS;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, TS)]
pub struct ChatMessage {
    pub sequence: i64,
    pub id: Uuid,
    pub conversation_id: Uuid,
    pub role: String,
    pub content: String,
    pub intent: String,
    pub status: String,
    pub error: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, FromRow)]
pub struct Conversation {
    pub id: Uuid,
    pub project_id: Uuid,
    pub event_cursor: i64,
}

pub async fn conversation(pool: &SqlitePool, project: Uuid) -> anyhow::Result<Conversation> {
    sqlx::query("INSERT OR IGNORE INTO project_conversations(id,project_id,owner_user_id,event_cursor) VALUES(?,?,?,(SELECT COALESCE(MAX(sequence),0) FROM project_chat_events))")
        .bind(Uuid::new_v4()).bind(project).bind(Uuid::from_u128(1)).execute(pool).await?;
    Ok(sqlx::query_as("SELECT * FROM project_conversations WHERE project_id=? AND owner_user_id=? AND name='Astra'")
        .bind(project).bind(Uuid::from_u128(1)).fetch_one(pool).await?)
}

pub async fn messages(
    pool: &SqlitePool,
    conversation: Uuid,
    after: i64,
) -> anyhow::Result<Vec<ChatMessage>> {
    Ok(sqlx::query_as("SELECT * FROM project_chat_messages WHERE conversation_id=? AND sequence>? ORDER BY sequence LIMIT 200")
        .bind(conversation).bind(after).fetch_all(pool).await?)
}

pub async fn enqueue(
    pool: &SqlitePool,
    conversation: Uuid,
    id: Uuid,
    content: &str,
    intent: &str,
) -> anyhow::Result<()> {
    sqlx::query("INSERT OR IGNORE INTO project_chat_messages(id,conversation_id,role,content,intent) VALUES(?,?,'user',?,?)")
        .bind(id).bind(conversation).bind(content).bind(intent).execute(pool).await?;
    let existing: (Uuid, String, String) = sqlx::query_as(
        "SELECT conversation_id,content,intent FROM project_chat_messages WHERE id=?",
    )
    .bind(id)
    .fetch_one(pool)
    .await?;
    anyhow::ensure!(
        existing == (conversation, content.to_owned(), intent.to_owned()),
        "Request ID was already used for another message"
    );
    Ok(())
}

/// Cursor and inbox commit together. A model outage cannot discard events.
pub async fn ingest(pool: &SqlitePool, conversation: &Conversation) -> anyhow::Result<()> {
    let mut tx = pool.begin().await?;
    let events: Vec<(i64,String,String)> = sqlx::query_as("SELECT sequence,source_key,payload FROM project_chat_events WHERE project_id=? AND sequence>? ORDER BY sequence LIMIT 100")
        .bind(conversation.project_id).bind(conversation.event_cursor).fetch_all(&mut *tx).await?;
    // Batch into one read-only model turn, while retaining the original IDs/payloads.
    if let Some((last, _, _)) = events.last() {
        let content = serde_json::to_string(&events.iter().map(|(_,key,payload)| serde_json::json!({"source":key,"data":serde_json::from_str::<Value>(payload).unwrap_or(Value::Null)})).collect::<Vec<_>>())?;
        sqlx::query("INSERT OR IGNORE INTO project_chat_messages(id,conversation_id,role,content,source_key) VALUES(?,?,'event',?,?)")
            .bind(Uuid::new_v4()).bind(conversation.id).bind(content).bind(format!("events:{last}")).execute(&mut *tx).await?;
        sqlx::query("UPDATE project_conversations SET event_cursor=MAX(event_cursor,?) WHERE id=?")
            .bind(last)
            .bind(conversation.id)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    Ok(())
}

pub async fn finish(pool: &SqlitePool, message: &ChatMessage, content: &str) -> anyhow::Result<()> {
    let mut tx = pool.begin().await?;
    sqlx::query("INSERT OR IGNORE INTO project_chat_messages(id,conversation_id,role,content,status,source_key) VALUES(?,?,'assistant',?,'done',?)")
        .bind(Uuid::new_v4()).bind(message.conversation_id).bind(content).bind(format!("reply:{}",message.id)).execute(&mut *tx).await?;
    sqlx::query("UPDATE project_chat_messages SET status='done',error=NULL WHERE id=?")
        .bind(message.id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(())
}
