//! Human-authored bodies and independent, attributed additions. Every mutation
//! and its retry receipt commit together; no history is deleted or rewritten.
use chrono::Utc;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqliteConnection, SqlitePool};
use ts_rs::TS;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, TS)]
pub struct DesignNote {
    pub id: Uuid,
    pub project_id: Uuid,
    pub title: String,
    pub body: String,
    pub revision: i32,
    pub editor: String,
    pub change_reason: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, FromRow, TS)]
pub struct DesignNoteAddition {
    pub id: Uuid,
    pub note_id: Uuid,
    pub heading: String,
    pub content: String,
    pub kind: String,
    pub recorder: String,
    pub source: String,
    pub speaker: String,
    pub referenced_body_revision: i32,
    pub short_quote: Option<String>,
    pub hidden: bool,
    pub revision: i32,
    pub editor: String,
    pub created_at: String,
    pub updated_at: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct AdditionContent {
    pub heading: String,
    pub content: String,
    /// decision = attributed human decision; proposal = suggestion; open_question = unresolved.
    pub kind: String,
    /// Conversation/message reference, not an assertion of approval.
    pub source: String,
    /// Who expressed the idea; distinct from the authenticated recorder.
    pub speaker: String,
    pub referenced_body_revision: i32,
    pub short_quote: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub enum DesignNoteAction {
    Create {
        title: String,
        body: String,
    },
    Update {
        note_id: Uuid,
        expected_revision: i32,
        title: String,
        body: String,
        change_reason: Option<String>,
    },
    Restore {
        note_id: Uuid,
        expected_revision: i32,
        revision: i32,
        change_reason: Option<String>,
    },
    Append {
        note_id: Uuid,
        addition: AdditionContent,
    },
    CorrectAddition {
        note_id: Uuid,
        addition_id: Uuid,
        expected_revision: i32,
        addition: AdditionContent,
    },
    SetAdditionHidden {
        note_id: Uuid,
        addition_id: Uuid,
        expected_revision: i32,
        hidden: bool,
    },
    /// Explicit integration only. Appends the selected addition to a new body revision.
    Integrate {
        note_id: Uuid,
        expected_revision: i32,
        addition_id: Uuid,
        expected_addition_revision: i32,
        change_reason: Option<String>,
    },
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct DesignNoteWrite {
    pub request_id: Uuid,
    pub operation: DesignNoteAction,
}
#[derive(Debug, Serialize, Deserialize, TS)]
pub struct DesignNoteResult {
    pub note: DesignNote,
    pub addition: Option<DesignNoteAddition>,
}
/// MCP success evidence only. Revisions describe the committed operation result,
/// not the latest state. Full immutable snapshots are available via bounded reads.
#[derive(Debug, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct DesignNoteReceipt {
    pub note: DesignNoteVersion,
    pub addition: Option<DesignNoteVersion>,
}
#[derive(Debug, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct DesignNoteVersion {
    pub id: Uuid,
    pub revision: i32,
}
impl DesignNoteResult {
    pub fn mcp_receipt(&self) -> DesignNoteReceipt {
        DesignNoteReceipt {
            note: DesignNoteVersion {
                id: self.note.id,
                revision: self.note.revision,
            },
            addition: self.addition.as_ref().map(|a| DesignNoteVersion {
                id: a.id,
                revision: a.revision,
            }),
        }
    }
}
#[derive(Debug, Serialize, Deserialize, TS)]
pub struct DesignNoteDetail {
    pub note: DesignNote,
    /// Includes hidden additions so the user can inspect and undo hiding.
    pub additions: Vec<DesignNoteAddition>,
}
#[derive(Debug, thiserror::Error)]
pub enum DesignNoteError {
    #[error("Design note, addition, revision or Project not found")]
    NotFound,
    #[error("Revision or request_id conflict; keep your draft and refresh before retrying")]
    Conflict,
    #[error(
        "Invalid design note input (title/heading 1–300, body/content up to 100000, source/speaker 1–2000, quote/reason up to 2000 characters)"
    )]
    Invalid,
    #[error(
        "Invalid design note read: page_size 1–20, limit 1–4000 Unicode characters; use cursor/after_revision for metadata and a saved revision with next_offset for text pages"
    )]
    InvalidRead,
    #[error(transparent)]
    Database(#[from] sqlx::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}
fn text(value: &str, max: usize, required: bool) -> Result<(), DesignNoteError> {
    if value.chars().count() > max || (required && value.trim().is_empty()) {
        return Err(DesignNoteError::Invalid);
    }
    Ok(())
}
fn body(title: &str, value: &str, reason: Option<&str>) -> Result<(), DesignNoteError> {
    text(title, 300, true)?;
    text(value, 100_000, false)?;
    text(reason.unwrap_or(""), 2000, false)
}
impl DesignNote {
    pub async fn list(
        pool: &SqlitePool,
        project: Uuid,
        offset: u32,
    ) -> Result<Vec<Self>, DesignNoteError> {
        if offset > 1_000_000 {
            return Err(DesignNoteError::Invalid);
        }
        let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM projects WHERE id=?)")
            .bind(project)
            .fetch_one(pool)
            .await?;
        if !exists {
            return Err(DesignNoteError::NotFound);
        }
        Ok(sqlx::query_as("SELECT * FROM design_notes WHERE project_id=? ORDER BY updated_at DESC,id LIMIT 100 OFFSET ?").bind(project).bind(offset).fetch_all(pool).await?)
    }
    pub async fn get(pool: &SqlitePool, project: Uuid, id: Uuid) -> Result<Self, DesignNoteError> {
        sqlx::query_as("SELECT * FROM design_notes WHERE project_id=? AND id=?")
            .bind(project)
            .bind(id)
            .fetch_optional(pool)
            .await?
            .ok_or(DesignNoteError::NotFound)
    }
    pub async fn detail(
        pool: &SqlitePool,
        project: Uuid,
        id: Uuid,
    ) -> Result<DesignNoteDetail, DesignNoteError> {
        let note = Self::get(pool, project, id).await?;
        let additions = sqlx::query_as(
            "SELECT * FROM design_note_additions WHERE note_id=? ORDER BY created_at,id",
        )
        .bind(id)
        .fetch_all(pool)
        .await?;
        Ok(DesignNoteDetail { note, additions })
    }
    pub async fn history(
        pool: &SqlitePool,
        project: Uuid,
        id: Uuid,
        offset: u32,
    ) -> Result<Vec<Self>, DesignNoteError> {
        Self::get(pool, project, id).await?;
        if offset > 1_000_000 {
            return Err(DesignNoteError::Invalid);
        }
        let rows: Vec<String> = sqlx::query_scalar("SELECT snapshot FROM design_note_revisions WHERE note_id=? ORDER BY revision LIMIT 100 OFFSET ?").bind(id).bind(offset).fetch_all(pool).await?;
        rows.iter().map(|s| Ok(serde_json::from_str(s)?)).collect()
    }
    pub async fn addition_history(
        pool: &SqlitePool,
        project: Uuid,
        id: Uuid,
        addition: Uuid,
        offset: u32,
    ) -> Result<Vec<DesignNoteAddition>, DesignNoteError> {
        Self::get(pool, project, id).await?;
        if offset > 1_000_000 {
            return Err(DesignNoteError::Invalid);
        }
        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM design_note_additions WHERE id=? AND note_id=?)",
        )
        .bind(addition)
        .bind(id)
        .fetch_one(pool)
        .await?;
        if !exists {
            return Err(DesignNoteError::NotFound);
        }
        let rows: Vec<String> = sqlx::query_scalar("SELECT snapshot FROM design_note_addition_revisions WHERE addition_id=? ORDER BY revision LIMIT 100 OFFSET ?").bind(addition).bind(offset).fetch_all(pool).await?;
        rows.iter().map(|s| Ok(serde_json::from_str(s)?)).collect()
    }
    pub async fn write(
        pool: &SqlitePool,
        project: Uuid,
        actor: &str,
        input: DesignNoteWrite,
    ) -> Result<DesignNoteResult, DesignNoteError> {
        let serialized = serde_json::to_string(&input.operation)?;
        let mut tx = pool.begin().await?;
        // First statement is a write: serialize competing SQLite writers before reading revisions.
        sqlx::query("INSERT INTO design_note_requests(project_id,actor,request_id,input) SELECT ?,?,?,? WHERE EXISTS(SELECT 1 FROM projects WHERE id=?) ON CONFLICT DO NOTHING")
            .bind(project).bind(actor).bind(input.request_id).bind(&serialized).bind(project).execute(&mut *tx).await?;
        let prior: Option<(String, Option<String>)> = sqlx::query_as("SELECT input,result FROM design_note_requests WHERE project_id=? AND actor=? AND request_id=?").bind(project).bind(actor).bind(input.request_id).fetch_optional(&mut *tx).await?;
        let (saved_input, result) = prior.ok_or(DesignNoteError::NotFound)?;
        if saved_input != serialized {
            return Err(DesignNoteError::Conflict);
        }
        if let Some(result) = result {
            return Ok(serde_json::from_str(&result)?);
        }
        let now = Utc::now().to_rfc3339();
        let result = match input.operation {
            DesignNoteAction::Create {
                title,
                body: content,
            } => {
                body(&title, &content, None)?;
                let note = Self {
                    id: Uuid::new_v4(),
                    project_id: project,
                    title,
                    body: content,
                    revision: 1,
                    editor: actor.into(),
                    change_reason: None,
                    created_at: now.clone(),
                    updated_at: now.clone(),
                };
                sqlx::query("INSERT INTO design_notes(id,project_id,title,body,revision,editor,created_at,updated_at) VALUES(?,?,?,?,?,?,?,?)")
                    .bind(note.id).bind(project).bind(&note.title).bind(&note.body).bind(1).bind(actor).bind(&now).bind(&now).execute(&mut *tx).await?;
                save_note(&mut tx, &note).await?;
                DesignNoteResult {
                    note,
                    addition: None,
                }
            }
            operation => {
                let id = match &operation {
                    DesignNoteAction::Update { note_id, .. }
                    | DesignNoteAction::Restore { note_id, .. }
                    | DesignNoteAction::Append { note_id, .. }
                    | DesignNoteAction::CorrectAddition { note_id, .. }
                    | DesignNoteAction::SetAdditionHidden { note_id, .. }
                    | DesignNoteAction::Integrate { note_id, .. } => *note_id,
                    DesignNoteAction::Create { .. } => unreachable!(),
                };
                let mut note: Self =
                    sqlx::query_as("SELECT * FROM design_notes WHERE project_id=? AND id=?")
                        .bind(project)
                        .bind(id)
                        .fetch_optional(&mut *tx)
                        .await?
                        .ok_or(DesignNoteError::NotFound)?;
                let mut addition_result = None;
                match operation {
                    DesignNoteAction::Update {
                        expected_revision,
                        title,
                        body,
                        change_reason,
                        ..
                    } => {
                        check_revision(note.revision, expected_revision)?;
                        note.title = title;
                        note.body = body;
                        note.change_reason = change_reason;
                    }
                    DesignNoteAction::Restore {
                        expected_revision,
                        revision,
                        change_reason,
                        ..
                    } => {
                        check_revision(note.revision, expected_revision)?;
                        let snapshot: String = sqlx::query_scalar("SELECT snapshot FROM design_note_revisions WHERE note_id=? AND revision=?").bind(id).bind(revision).fetch_optional(&mut *tx).await?.ok_or(DesignNoteError::NotFound)?;
                        let old: Self = serde_json::from_str(&snapshot)?;
                        note.title = old.title;
                        note.body = old.body;
                        note.change_reason =
                            change_reason.or(Some(format!("Restore revision {revision}")));
                    }
                    DesignNoteAction::Integrate {
                        expected_revision,
                        addition_id,
                        expected_addition_revision,
                        change_reason,
                        ..
                    } => {
                        check_revision(note.revision, expected_revision)?;
                        let a = get_addition(&mut tx, id, addition_id).await?;
                        check_revision(a.revision, expected_addition_revision)?;
                        if a.hidden {
                            return Err(DesignNoteError::Invalid);
                        }
                        note.body.push_str(&format!("\n\n## {}\n\n{}\n\n出典: {} / 発言者: {} / 区分: {} / 追記: {} (v{})\n", a.heading, a.content, a.source, a.speaker, a.kind, a.id, a.revision));
                        note.change_reason = change_reason.or(Some(format!(
                            "Integrate addition {} revision {}",
                            a.id, a.revision
                        )));
                    }
                    DesignNoteAction::Append { addition, .. } => {
                        validate_addition(&mut tx, id, &addition).await?;
                        let a = DesignNoteAddition {
                            id: Uuid::new_v4(),
                            note_id: id,
                            heading: addition.heading,
                            content: addition.content,
                            kind: addition.kind,
                            recorder: actor.into(),
                            source: addition.source,
                            speaker: addition.speaker,
                            referenced_body_revision: addition.referenced_body_revision,
                            short_quote: addition.short_quote,
                            hidden: false,
                            revision: 1,
                            editor: actor.into(),
                            created_at: now.clone(),
                            updated_at: now.clone(),
                        };
                        sqlx::query("INSERT INTO design_note_additions(id,note_id,heading,content,kind,recorder,source,speaker,referenced_body_revision,short_quote,hidden,revision,editor,created_at,updated_at) VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)")
                            .bind(a.id).bind(id).bind(&a.heading).bind(&a.content).bind(&a.kind).bind(actor).bind(&a.source).bind(&a.speaker).bind(a.referenced_body_revision).bind(&a.short_quote).bind(false).bind(1).bind(actor).bind(&now).bind(&now).execute(&mut *tx).await?;
                        save_addition(&mut tx, &a).await?;
                        addition_result = Some(a);
                    }
                    DesignNoteAction::CorrectAddition {
                        addition_id,
                        expected_revision,
                        addition,
                        ..
                    } => {
                        let mut a = get_addition(&mut tx, id, addition_id).await?;
                        check_revision(a.revision, expected_revision)?;
                        validate_addition(&mut tx, id, &addition).await?;
                        a.heading = addition.heading;
                        a.content = addition.content;
                        a.kind = addition.kind;
                        a.source = addition.source;
                        a.speaker = addition.speaker;
                        a.referenced_body_revision = addition.referenced_body_revision;
                        a.short_quote = addition.short_quote;
                        update_addition(&mut tx, &mut a, actor, &now).await?;
                        addition_result = Some(a);
                    }
                    DesignNoteAction::SetAdditionHidden {
                        addition_id,
                        expected_revision,
                        hidden,
                        ..
                    } => {
                        let mut a = get_addition(&mut tx, id, addition_id).await?;
                        check_revision(a.revision, expected_revision)?;
                        a.hidden = hidden;
                        update_addition(&mut tx, &mut a, actor, &now).await?;
                        addition_result = Some(a);
                    }
                    DesignNoteAction::Create { .. } => unreachable!(),
                }
                if addition_result.is_none() {
                    body(&note.title, &note.body, note.change_reason.as_deref())?;
                    note.revision += 1;
                    note.editor = actor.into();
                    note.updated_at = now.clone();
                    sqlx::query("UPDATE design_notes SET title=?,body=?,revision=?,editor=?,change_reason=?,updated_at=? WHERE id=?")
                        .bind(&note.title).bind(&note.body).bind(note.revision).bind(actor).bind(&note.change_reason).bind(&now).bind(id).execute(&mut *tx).await?;
                    save_note(&mut tx, &note).await?;
                }
                DesignNoteResult {
                    note,
                    addition: addition_result,
                }
            }
        };
        sqlx::query("UPDATE design_note_requests SET result=? WHERE project_id=? AND actor=? AND request_id=?").bind(serde_json::to_string(&result)?).bind(project).bind(actor).bind(input.request_id).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(result)
    }
}
fn check_revision(actual: i32, expected: i32) -> Result<(), DesignNoteError> {
    if actual != expected {
        return Err(DesignNoteError::Conflict);
    }
    Ok(())
}
async fn save_note(c: &mut SqliteConnection, note: &DesignNote) -> Result<(), DesignNoteError> {
    sqlx::query("INSERT INTO design_note_revisions VALUES(?,?,?)")
        .bind(note.id)
        .bind(note.revision)
        .bind(serde_json::to_string(note)?)
        .execute(c)
        .await?;
    Ok(())
}
async fn get_addition(
    c: &mut SqliteConnection,
    note: Uuid,
    id: Uuid,
) -> Result<DesignNoteAddition, DesignNoteError> {
    sqlx::query_as("SELECT * FROM design_note_additions WHERE note_id=? AND id=?")
        .bind(note)
        .bind(id)
        .fetch_optional(c)
        .await?
        .ok_or(DesignNoteError::NotFound)
}
async fn save_addition(
    c: &mut SqliteConnection,
    a: &DesignNoteAddition,
) -> Result<(), DesignNoteError> {
    sqlx::query("INSERT INTO design_note_addition_revisions VALUES(?,?,?)")
        .bind(a.id)
        .bind(a.revision)
        .bind(serde_json::to_string(a)?)
        .execute(c)
        .await?;
    Ok(())
}
async fn update_addition(
    c: &mut SqliteConnection,
    a: &mut DesignNoteAddition,
    actor: &str,
    now: &str,
) -> Result<(), DesignNoteError> {
    a.revision += 1;
    a.editor = actor.into();
    a.updated_at = now.into();
    sqlx::query("UPDATE design_note_additions SET heading=?,content=?,kind=?,source=?,speaker=?,referenced_body_revision=?,short_quote=?,hidden=?,revision=?,editor=?,updated_at=? WHERE id=?")
        .bind(&a.heading).bind(&a.content).bind(&a.kind).bind(&a.source).bind(&a.speaker).bind(a.referenced_body_revision).bind(&a.short_quote).bind(a.hidden).bind(a.revision).bind(actor).bind(now).bind(a.id).execute(&mut *c).await?;
    save_addition(c, a).await
}
async fn validate_addition(
    c: &mut SqliteConnection,
    note: Uuid,
    a: &AdditionContent,
) -> Result<(), DesignNoteError> {
    text(&a.heading, 300, true)?;
    text(&a.content, 100_000, true)?;
    text(&a.source, 2000, true)?;
    text(&a.speaker, 2000, true)?;
    text(a.short_quote.as_deref().unwrap_or(""), 2000, false)?;
    if !["decision", "proposal", "open_question"].contains(&a.kind.as_str()) {
        return Err(DesignNoteError::Invalid);
    }
    let snapshot: String = sqlx::query_scalar(
        "SELECT snapshot FROM design_note_revisions WHERE note_id=? AND revision=?",
    )
    .bind(note)
    .bind(a.referenced_body_revision)
    .fetch_optional(c)
    .await?
    .ok_or(DesignNoteError::NotFound)?;
    if let Some(quote) = &a.short_quote {
        let referenced: DesignNote = serde_json::from_str(&snapshot)?;
        if !referenced.body.contains(quote) {
            return Err(DesignNoteError::Invalid);
        }
    }
    Ok(())
}
#[cfg(test)]
#[path = "design_note_tests.rs"]
mod tests;
