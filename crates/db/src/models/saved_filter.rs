//! Atomic Project-owned definitions and retry receipts shared by HTTP and MCP.
use api_types::*;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::{FromRow, SqlitePool};
use uuid::Uuid;

#[derive(Debug, thiserror::Error)]
pub enum SavedFilterError {
    #[error("Saved filter or Project not found")]
    NotFound,
    #[error("Saved filter revision or request_id conflict")]
    Conflict,
    #[error("Invalid saved filter: {0}")]
    Invalid(String),
    #[error("Commit outcome unknown; retry the identical request_id and input")]
    CommitUnknown(sqlx::Error),
    #[error(transparent)]
    Database(#[from] sqlx::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case")]
pub enum SavedFilterWrite {
    Create(CreateSavedFilterRequest),
    Update {
        id: Uuid,
        input: UpdateSavedFilterRequest,
    },
    Delete {
        id: Uuid,
        input: DeleteSavedFilterRequest,
    },
}
impl SavedFilterWrite {
    pub fn request_id(&self) -> Uuid {
        match self {
            Self::Create(a) => a.request_id,
            Self::Update { input, .. } => input.request_id,
            Self::Delete { input, .. } => input.request_id,
        }
    }
}
#[derive(FromRow)]
struct Row {
    id: Uuid,
    project_id: Uuid,
    definition: String,
    revision: i32,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}
impl Row {
    fn decode(self) -> Result<SavedFilter, SavedFilterError> {
        let d: SavedFilterDefinition = serde_json::from_str(&self.definition)?;
        Ok(SavedFilter {
            id: self.id,
            project_id: self.project_id,
            name: d.name,
            machine: d.machine,
            natural_language: d.natural_language,
            context: d.context,
            revision: self.revision,
            created_at: self.created_at,
            updated_at: self.updated_at,
        })
    }
}
pub async fn require_project(pool: &SqlitePool, project: Uuid) -> Result<(), SavedFilterError> {
    if sqlx::query_scalar::<_, i64>("SELECT 1 FROM projects WHERE id=?")
        .bind(project)
        .fetch_optional(pool)
        .await?
        .is_none()
    {
        return Err(SavedFilterError::NotFound);
    }
    Ok(())
}
pub async fn get(
    pool: &SqlitePool,
    project: Uuid,
    id: Uuid,
) -> Result<SavedFilter, SavedFilterError> {
    sqlx::query_as::<_, Row>("SELECT * FROM saved_filters WHERE project_id=? AND id=?")
        .bind(project)
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or(SavedFilterError::NotFound)?
        .decode()
}
pub async fn list(
    pool: &SqlitePool,
    project: Uuid,
    page: &SavedFilterPage,
) -> Result<ListSavedFiltersResponse, SavedFilterError> {
    require_project(pool, project).await?;
    let limit = page.limit.unwrap_or(100).clamp(1, 500) as usize;
    let offset = page.offset as usize;
    let mut tx = pool.begin().await?;
    let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM saved_filters WHERE project_id=?")
        .bind(project)
        .fetch_one(&mut *tx)
        .await?;
    let rows = sqlx::query_as::<_, Row>(
        "SELECT * FROM saved_filters WHERE project_id=? ORDER BY created_at,id LIMIT ? OFFSET ?",
    )
    .bind(project)
    .bind(limit as i64)
    .bind(offset as i64)
    .fetch_all(&mut *tx)
    .await?;
    tx.commit().await?;
    let total_count = total as usize;
    Ok(ListSavedFiltersResponse {
        saved_filters: rows
            .into_iter()
            .map(Row::decode)
            .collect::<Result<_, _>>()?,
        total_count,
        offset,
        limit,
        next_offset: (offset.saturating_add(limit) < total_count)
            .then_some(offset.saturating_add(limit)),
    })
}
pub fn validate(d: &SavedFilterDefinition) -> Result<(), SavedFilterError> {
    let invalid = |m: &str| SavedFilterError::Invalid(m.into());
    if d.name.trim().is_empty() || d.name.chars().count() > 200 {
        return Err(invalid("name must contain 1–200 characters"));
    }
    if d.natural_language
        .as_ref()
        .is_some_and(|s| s.chars().count() > 16000)
        || d.context
            .as_ref()
            .is_some_and(|s| s.chars().count() > 32000)
        || d.machine
            .search
            .as_ref()
            .is_some_and(|s| s.chars().count() > 16000)
    {
        return Err(invalid("condition text is too long"));
    }
    if d.machine
        .simple_id
        .as_ref()
        .is_some_and(|s| s.chars().count() > 200)
    {
        return Err(invalid("simple_id is too long"));
    }
    for period in [&d.machine.created_at, &d.machine.updated_at]
        .into_iter()
        .flatten()
    {
        match period {
            SavedFilterPeriod::Absolute {
                start: Some(start),
                end: Some(end),
            } if start >= end => return Err(invalid("absolute start must precede end")),
            SavedFilterPeriod::Absolute {
                start: None,
                end: None,
            } => return Err(invalid("absolute period requires a bound")),
            SavedFilterPeriod::Relative { seconds: 0 } => {
                return Err(invalid("relative seconds must be positive"));
            }
            _ => {}
        }
    }
    if [
        &d.machine.status_ids,
        &d.machine.assignee_ids,
        &d.machine.tag_ids,
    ]
    .iter()
    .any(|ids| ids.len() > 500)
    {
        return Err(invalid("at most 500 IDs per condition"));
    }
    Ok(())
}
pub async fn write(
    pool: &SqlitePool,
    project: Uuid,
    actor: &str,
    input: SavedFilterWrite,
) -> Result<Value, SavedFilterError> {
    let serialized = serde_json::to_string(&input)?;
    let request = input.request_id();
    let mut tx = pool.begin().await?;
    // First statement obtains the writer lock, before reading any revision.
    sqlx::query("INSERT INTO saved_filter_requests(project_id,actor,request_id,input) SELECT ?,?,?,? WHERE EXISTS(SELECT 1 FROM projects WHERE id=?) ON CONFLICT DO NOTHING").bind(project).bind(actor).bind(request).bind(&serialized).bind(project).execute(&mut *tx).await?;
    let (prior,result):(String,Option<String>)=sqlx::query_as("SELECT input,result FROM saved_filter_requests WHERE project_id=? AND actor=? AND request_id=?").bind(project).bind(actor).bind(request).fetch_optional(&mut *tx).await?.ok_or(SavedFilterError::NotFound)?;
    if prior != serialized {
        return Err(SavedFilterError::Conflict);
    }
    if let Some(result) = result {
        return Ok(serde_json::from_str(&result)?);
    }
    let definition = match &input {
        SavedFilterWrite::Create(a) => Some(&a.definition),
        SavedFilterWrite::Update { input, .. } => Some(&input.definition),
        _ => None,
    };
    if let Some(d) = definition {
        validate(d)?;
        for id in &d.machine.status_ids {
            if sqlx::query_scalar::<_, i64>(
                "SELECT 1 FROM local_project_statuses WHERE id=? AND project_id=?",
            )
            .bind(id)
            .bind(project)
            .fetch_optional(&mut *tx)
            .await?
            .is_none()
            {
                return Err(SavedFilterError::Invalid(
                    "status belongs to another Project or no longer exists".into(),
                ));
            }
        }
        for id in &d.machine.tag_ids {
            if sqlx::query_scalar::<_, i64>("SELECT 1 FROM local_tags WHERE id=? AND project_id=?")
                .bind(id)
                .bind(project)
                .fetch_optional(&mut *tx)
                .await?
                .is_none()
            {
                return Err(SavedFilterError::Invalid(
                    "tag belongs to another Project or no longer exists".into(),
                ));
            }
        }
        if let Some(r) = &d.machine.relation
            && sqlx::query_scalar::<_, i64>(
                "SELECT 1 FROM local_issues WHERE id=? AND project_id=?",
            )
            .bind(r.target_issue_id)
            .bind(project)
            .fetch_optional(&mut *tx)
            .await?
            .is_none()
        {
            return Err(SavedFilterError::Invalid(
                "relation target belongs to another Project or no longer exists".into(),
            ));
        }
    }
    let now = Utc::now();
    let result = match input {
        SavedFilterWrite::Create(a) => {
            let id = Uuid::new_v4();
            sqlx::query("INSERT INTO saved_filters(id,project_id,definition,revision,created_at,updated_at) VALUES(?,?,?,1,?,?)").bind(id).bind(project).bind(serde_json::to_string(&a.definition)?).bind(now).bind(now).execute(&mut *tx).await?;
            serde_json::to_value(
                Row {
                    id,
                    project_id: project,
                    definition: serde_json::to_string(&a.definition)?,
                    revision: 1,
                    created_at: now,
                    updated_at: now,
                }
                .decode()?,
            )?
        }
        SavedFilterWrite::Update { id, input: a } => {
            let old =
                sqlx::query_as::<_, Row>("SELECT * FROM saved_filters WHERE project_id=? AND id=?")
                    .bind(project)
                    .bind(id)
                    .fetch_optional(&mut *tx)
                    .await?
                    .ok_or(SavedFilterError::NotFound)?;
            if old.revision != a.expected_revision {
                return Err(SavedFilterError::Conflict);
            }
            let revision = old
                .revision
                .checked_add(1)
                .ok_or(SavedFilterError::Conflict)?;
            let definition = serde_json::to_string(&a.definition)?;
            sqlx::query("UPDATE saved_filters SET definition=?,revision=?,updated_at=? WHERE id=? AND project_id=?").bind(&definition).bind(revision).bind(now).bind(id).bind(project).execute(&mut *tx).await?;
            serde_json::to_value(
                Row {
                    definition,
                    revision,
                    updated_at: now,
                    ..old
                }
                .decode()?,
            )?
        }
        SavedFilterWrite::Delete { id, input: a } => {
            let old: i32 = sqlx::query_scalar(
                "SELECT revision FROM saved_filters WHERE id=? AND project_id=?",
            )
            .bind(id)
            .bind(project)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or(SavedFilterError::NotFound)?;
            if old != a.expected_revision {
                return Err(SavedFilterError::Conflict);
            }
            sqlx::query("DELETE FROM saved_filters WHERE id=? AND project_id=?")
                .bind(id)
                .bind(project)
                .execute(&mut *tx)
                .await?;
            serde_json::json!({"deleted":true,"saved_filter_id":id,"revision":old})
        }
    };
    sqlx::query(
        "UPDATE saved_filter_requests SET result=? WHERE project_id=? AND actor=? AND request_id=?",
    )
    .bind(serde_json::to_string(&result)?)
    .bind(project)
    .bind(actor)
    .bind(request)
    .execute(&mut *tx)
    .await?;
    tx.commit().await.map_err(|e| {
        if super::todo::commit_definitely_rejected(&e) {
            SavedFilterError::Database(e)
        } else {
            SavedFilterError::CommitUnknown(e)
        }
    })?;
    Ok(result)
}

#[cfg(test)]
mod tests;
