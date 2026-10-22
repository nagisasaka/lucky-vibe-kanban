//! Bounded MCP projection of the same canonical records used by the UI.
//! Follows existing MCP metadata/keyset and Unicode character-page contracts.
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::{Row, SqlitePool};
use uuid::Uuid;

use super::design_note::DesignNoteError;

#[derive(Debug, Default, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DesignNoteRead {
    pub project_id: Uuid,
    pub note_id: Option<Uuid>,
    pub addition_id: Option<Uuid>,
    /// get_design_note: body (default), or additions metadata listing.
    pub section: Option<String>,
    /// Immutable saved revision. Required for text offsets beyond zero.
    pub revision: Option<i32>,
    #[serde(default)]
    pub offset: u32,
    /// Unicode character count, 1..4000 (default 4000).
    pub limit: Option<u32>,
    /// Metadata count, 1..20 (default 20).
    pub page_size: Option<u32>,
    /// UUID keyset cursor for note/addition listings.
    pub cursor: Option<Uuid>,
    /// Exclusive revision cursor for history metadata.
    pub after_revision: Option<i32>,
}

pub async fn read(
    pool: &SqlitePool,
    tool: &str,
    a: DesignNoteRead,
) -> Result<Value, DesignNoteError> {
    let size = a.page_size.unwrap_or(20);
    let limit = a.limit.unwrap_or(4000);
    if !(1..=20).contains(&size)
        || !(1..=4000).contains(&limit)
        || a.after_revision.is_some_and(|r| r < 0)
        || a.revision.is_some_and(|r| r < 1)
    {
        return Err(DesignNoteError::InvalidRead);
    }
    let listing = tool == "list_design_notes"
        || tool == "get_design_note_history"
        || a.section.as_deref() == Some("additions");
    if listing && a.offset != 0 {
        return Err(DesignNoteError::InvalidRead);
    }
    let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM projects WHERE id=?)")
        .bind(a.project_id)
        .fetch_one(pool)
        .await?;
    if !exists {
        return Err(DesignNoteError::NotFound);
    }
    if tool == "list_design_notes" {
        if let Some(cursor) = a.cursor {
            let belongs: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM design_notes WHERE id=? AND project_id=?)",
            )
            .bind(cursor)
            .bind(a.project_id)
            .fetch_one(pool)
            .await?;
            if !belongs {
                return Err(DesignNoteError::NotFound);
            }
        }
        // No body allocation, and edits cannot reorder UUID keyset pages.
        let rows = sqlx::query("SELECT id,title,revision,created_at,updated_at FROM design_notes WHERE project_id=? AND (? IS NULL OR id>?) ORDER BY id LIMIT ?")
            .bind(a.project_id).bind(a.cursor).bind(a.cursor).bind(size+1).fetch_all(pool).await?;
        let items = rows.iter().map(|r| Ok(json!({"id":r.try_get::<Uuid,_>("id")?,"title":r.try_get::<String,_>("title")?,"revision":r.try_get::<i32,_>("revision")?,"created_at":r.try_get::<String,_>("created_at")?,"updated_at":r.try_get::<String,_>("updated_at")?}))).collect::<Result<Vec<_>,sqlx::Error>>()?;
        return Ok(page(items, size, "id", "next_cursor"));
    }
    let note = a.note_id.ok_or(DesignNoteError::InvalidRead)?;
    let exists: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM design_notes WHERE id=? AND project_id=?)")
            .bind(note)
            .bind(a.project_id)
            .fetch_one(pool)
            .await?;
    if !exists {
        return Err(DesignNoteError::NotFound);
    }
    if let Some(addition) = a.addition_id {
        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM design_note_additions WHERE id=? AND note_id=?)",
        )
        .bind(addition)
        .bind(note)
        .fetch_one(pool)
        .await?;
        if !exists {
            return Err(DesignNoteError::NotFound);
        }
    }
    if tool == "get_design_note" && a.section.as_deref() == Some("additions") {
        if let Some(cursor) = a.cursor {
            let belongs: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM design_note_additions WHERE id=? AND note_id=?)",
            )
            .bind(cursor)
            .bind(note)
            .fetch_one(pool)
            .await?;
            if !belongs {
                return Err(DesignNoteError::NotFound);
            }
        }
        let rows = sqlx::query("SELECT id,heading,kind,hidden,revision FROM design_note_additions WHERE note_id=? AND (? IS NULL OR id>?) ORDER BY id LIMIT ?")
            .bind(note).bind(a.cursor).bind(a.cursor).bind(size+1).fetch_all(pool).await?;
        let items = rows.iter().map(|r| Ok(json!({"id":r.try_get::<Uuid,_>("id")?,"heading":r.try_get::<String,_>("heading")?,"kind":r.try_get::<String,_>("kind")?,"hidden":r.try_get::<bool,_>("hidden")?,"revision":r.try_get::<i32,_>("revision")?}))).collect::<Result<Vec<_>,sqlx::Error>>()?;
        return Ok(page(items, size, "id", "next_cursor"));
    }
    if a.section.as_deref().is_some_and(|s| s != "body") {
        return Err(DesignNoteError::InvalidRead);
    }
    // Table and column names are constants, never user-provided SQL.
    let (table, key, id) = match a.addition_id {
        Some(id) => ("design_note_addition_revisions", "addition_id", id),
        None => ("design_note_revisions", "note_id", note),
    };
    if tool == "get_design_note_history" {
        let sql = format!(
            "SELECT revision,json_extract(snapshot,'$.updated_at') AS saved_at FROM {table} WHERE {key}=? AND revision>? ORDER BY revision LIMIT ?"
        );
        let rows = sqlx::query(&sql)
            .bind(id)
            .bind(a.after_revision.unwrap_or(0))
            .bind(size + 1)
            .fetch_all(pool)
            .await?;
        let items = rows.iter().map(|r| Ok(json!({"revision":r.try_get::<i32,_>("revision")?,"saved_at":r.try_get::<String,_>("saved_at")?}))).collect::<Result<Vec<_>,sqlx::Error>>()?;
        return Ok(page(items, size, "revision", "next_after_revision"));
    }
    if tool != "get_design_note" || (a.offset > 0 && a.revision.is_none()) {
        return Err(DesignNoteError::InvalidRead);
    }
    // Read revision, length and chunk in one statement. A concurrent update cannot
    // mix revisions. Following pages pin this immutable snapshot, even after edits.
    let sql = format!(
        "SELECT revision,length(snapshot) AS total,substr(snapshot,?,?) AS content FROM {table} WHERE {key}=? AND (? IS NULL OR revision=?) ORDER BY revision DESC LIMIT 1"
    );
    let r = sqlx::query(&sql)
        .bind(i64::from(a.offset) + 1)
        .bind(limit)
        .bind(id)
        .bind(a.revision)
        .bind(a.revision)
        .fetch_optional(pool)
        .await?
        .ok_or(DesignNoteError::NotFound)?;
    let total: i64 = r.try_get("total")?;
    if i64::from(a.offset) > total {
        return Err(DesignNoteError::InvalidRead);
    }
    let next = i64::from(a.offset) + i64::from(limit);
    Ok(
        json!({"id":id,"note_id":note,"project_id":a.project_id,"revision":r.try_get::<i32,_>("revision")?,"content_page":r.try_get::<String,_>("content")?,"format":"json","offset":a.offset,"total_characters":total,"has_more":next<total,"next_offset":if next<total {Some(next)} else {None},"consistency":"immutable revision; concatenate content_page strings before parsing JSON"}),
    )
}
fn page(mut items: Vec<Value>, size: u32, key: &str, next_key: &str) -> Value {
    let more = items.len() > size as usize;
    items.truncate(size as usize);
    let next = if more {
        items.last().map(|v| v[key].clone())
    } else {
        None
    };
    let mut result = json!({"items":items,"has_more":more,"consistency":"live metadata; restart listing after concurrent additions; use each returned revision for immutable detail"});
    result[next_key] = json!(next);
    result
}
