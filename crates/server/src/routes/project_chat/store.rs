use serde::{Deserialize, Serialize};
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

/// Retire legacy automatic turns without deleting their history or checkpoints.
/// Only direct user requests are resumed after a restart.
pub async fn recover_user_queue(pool: &SqlitePool) -> anyhow::Result<()> {
    let mut tx = pool.begin().await?;
    sqlx::query("UPDATE project_chat_messages SET status='done',error=NULL WHERE role='event' AND status!='done'")
        .execute(&mut *tx).await?;
    sqlx::query(
        "UPDATE project_chat_messages SET status='pending' WHERE role='user' AND status='running'",
    )
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(())
}

pub async fn next_user_message(
    pool: &SqlitePool,
    conversation: Uuid,
) -> anyhow::Result<Option<ChatMessage>> {
    Ok(sqlx::query_as("SELECT * FROM project_chat_messages WHERE conversation_id=? AND role='user' AND status!='done' ORDER BY sequence LIMIT 1")
        .bind(conversation).fetch_optional(pool).await?)
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
