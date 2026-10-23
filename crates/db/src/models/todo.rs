//! Project-wide notes shared by the UI and both MCP transports.
use chrono::Utc;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqlitePool};
use ts_rs::TS;
use uuid::Uuid;

/// Recorded by the trusted server entrypoint, never supplied in a mutation DTO.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type, TS)]
#[serde(rename_all = "snake_case")]
#[sqlx(rename_all = "snake_case")]
pub enum TodoCreationSource {
    Ui,
    Mcp,
    #[default]
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, TS)]
pub struct Todo {
    pub id: Uuid,
    pub project_id: Uuid,
    pub body: String,
    pub category: String,
    pub completed: bool,
    #[serde(default)]
    pub creation_source: TodoCreationSource,
    pub issue_id: Option<Uuid>,
    pub revision: i32,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct CreateTodo {
    /// Stable client-generated ID. Reuse this ID when retrying the same addition.
    pub id: Uuid,
    pub body: String,
    #[serde(default)]
    pub category: String,
    pub issue_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct UpdateTodo {
    pub expected_revision: i32,
    pub body: Option<String>,
    /// Empty string means uncategorized. Omitted fields are preserved.
    pub category: Option<String>,
    pub completed: Option<bool>,
    pub issue_id: Option<Uuid>,
    #[serde(default)]
    pub clear_issue: bool,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TodoQuery {
    #[serde(default)]
    pub include_completed: bool,
    #[serde(default = "default_limit")]
    pub limit: u32,
    #[serde(default)]
    pub offset: u32,
}
fn default_limit() -> u32 {
    100
}

#[derive(Debug, Serialize, Deserialize, TS)]
pub struct TodoPage {
    pub items: Vec<Todo>,
    pub next_offset: Option<u32>,
}

#[derive(Debug, thiserror::Error)]
pub enum TodoError {
    #[error("TODO text must contain 1–16000 characters; category must be at most 100 characters")]
    InvalidText,
    #[error("TODO or Project not found")]
    NotFound,
    #[error("Related card must belong to this Project")]
    InvalidIssue,
    #[error("TODO changed; refresh it before retrying")]
    Conflict,
    #[error("Invalid TODO request")]
    InvalidRequest,
    #[error(transparent)]
    Database(#[from] sqlx::Error),
}

fn validate(body: &str, category: &str) -> Result<(), TodoError> {
    if body.trim().is_empty() || body.chars().count() > 16000 || category.chars().count() > 100 {
        return Err(TodoError::InvalidText);
    }
    Ok(())
}

impl Todo {
    async fn require_project(pool: &SqlitePool, project: Uuid) -> Result<(), TodoError> {
        let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM projects WHERE id=?)")
            .bind(project)
            .fetch_one(pool)
            .await?;
        if !exists {
            return Err(TodoError::NotFound);
        }
        Ok(())
    }
    async fn require_issue(
        pool: &SqlitePool,
        project: Uuid,
        issue: Option<Uuid>,
    ) -> Result<(), TodoError> {
        if let Some(issue) = issue {
            let exists: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM local_issues WHERE id=? AND project_id=?)",
            )
            .bind(issue)
            .bind(project)
            .fetch_one(pool)
            .await?;
            if !exists {
                return Err(TodoError::InvalidIssue);
            }
        }
        Ok(())
    }
    pub async fn get(pool: &SqlitePool, project: Uuid, id: Uuid) -> Result<Self, TodoError> {
        sqlx::query_as("SELECT * FROM project_todos WHERE project_id=? AND id=?")
            .bind(project)
            .bind(id)
            .fetch_optional(pool)
            .await?
            .ok_or(TodoError::NotFound)
    }
    pub async fn list(
        pool: &SqlitePool,
        project: Uuid,
        query: TodoQuery,
    ) -> Result<TodoPage, TodoError> {
        Self::require_project(pool, project).await?;
        if query.limit == 0 || query.limit > 200 || query.offset > 1_000_000 {
            return Err(TodoError::InvalidRequest);
        }
        let mut items: Vec<Self> = sqlx::query_as("SELECT * FROM project_todos WHERE project_id=? AND (? OR completed=0) ORDER BY created_at DESC,id LIMIT ? OFFSET ?")
            .bind(project).bind(query.include_completed).bind(query.limit + 1).bind(query.offset).fetch_all(pool).await?;
        let next_offset =
            (items.len() > query.limit as usize).then_some(query.offset + query.limit);
        items.truncate(query.limit as usize);
        Ok(TodoPage { items, next_offset })
    }
    pub async fn create(
        pool: &SqlitePool,
        project: Uuid,
        input: CreateTodo,
    ) -> Result<Self, TodoError> {
        Self::create_from(pool, project, input, TodoCreationSource::Unknown).await
    }

    pub async fn create_from(
        pool: &SqlitePool,
        project: Uuid,
        mut input: CreateTodo,
        source: TodoCreationSource,
    ) -> Result<Self, TodoError> {
        input.category = input.category.trim().to_owned();
        validate(&input.body, &input.category)?;
        Self::require_project(pool, project).await?;
        Self::require_issue(pool, project, input.issue_id).await?;
        sqlx::query("INSERT INTO project_todos(id,project_id,body,category,issue_id,creation_source) VALUES(?,?,?,?,?,?) ON CONFLICT(id) DO NOTHING")
            .bind(input.id).bind(project).bind(&input.body).bind(&input.category).bind(input.issue_id).bind(source).execute(pool).await?;
        let todo = Self::get(pool, project, input.id).await?;
        if todo.body != input.body
            || todo.category != input.category
            || todo.issue_id != input.issue_id
        {
            return Err(TodoError::Conflict);
        }
        Ok(todo)
    }
    pub async fn update(
        pool: &SqlitePool,
        project: Uuid,
        id: Uuid,
        input: UpdateTodo,
    ) -> Result<Self, TodoError> {
        if input.clear_issue && input.issue_id.is_some() {
            return Err(TodoError::InvalidRequest);
        }
        let old = Self::get(pool, project, id).await?;
        if input.expected_revision != old.revision {
            return Err(TodoError::Conflict);
        }
        let body = input.body.unwrap_or(old.body);
        let category = input.category.unwrap_or(old.category).trim().to_owned();
        validate(&body, &category)?;
        let issue = if input.clear_issue {
            None
        } else {
            input.issue_id.or(old.issue_id)
        };
        Self::require_issue(pool, project, issue).await?;
        let updated = sqlx::query_as("UPDATE project_todos SET body=?,category=?,completed=?,issue_id=?,revision=revision+1,updated_at=? WHERE id=? AND project_id=? AND revision=? RETURNING *")
            .bind(body).bind(category).bind(input.completed.unwrap_or(old.completed)).bind(issue)
            .bind(Utc::now().to_rfc3339()).bind(id).bind(project).bind(input.expected_revision)
            .fetch_optional(pool).await?;
        updated.ok_or(TodoError::Conflict)
    }
    pub async fn delete(
        pool: &SqlitePool,
        project: Uuid,
        id: Uuid,
        revision: i32,
    ) -> Result<(), TodoError> {
        let result =
            sqlx::query("DELETE FROM project_todos WHERE project_id=? AND id=? AND revision=?")
                .bind(project)
                .bind(id)
                .bind(revision)
                .execute(pool)
                .await?;
        if result.rows_affected() == 0 {
            Self::get(pool, project, id).await?;
            return Err(TodoError::Conflict);
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "todo_tests.rs"]
mod tests;
