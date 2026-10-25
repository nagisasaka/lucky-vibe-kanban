//! Board classification shared by Web, stdio and remote MCP. System is only a
//! reserved label, never an execution permission or an ownership credential.
use sqlx::{Row, SqliteConnection};
use uuid::Uuid;

#[derive(Debug, thiserror::Error)]
pub enum TagError {
    #[error("{0}")]
    Validation(&'static str),
    #[error(transparent)]
    Database(#[from] sqlx::Error),
}

pub fn is_system(name: &str) -> bool {
    name.trim().eq_ignore_ascii_case("system")
}

pub async fn add(
    conn: &mut SqliteConnection,
    issue: Uuid,
    tag: Uuid,
    id: Option<Uuid>,
) -> Result<Uuid, TagError> {
    let row = sqlx::query("SELECT t.name, t.project_id AS tag_project, i.project_id AS issue_project FROM local_tags t CROSS JOIN local_issues i WHERE t.id=? AND i.id=?")
        .bind(tag).bind(issue).fetch_optional(&mut *conn).await?
        .ok_or(TagError::Validation("Issue or tag not found"))?;
    if row.try_get::<Uuid, _>("tag_project")? != row.try_get::<Uuid, _>("issue_project")? {
        return Err(TagError::Validation("Tag is not in this Issue's Project"));
    }
    if is_system(&row.try_get::<String, _>("name")?) {
        return Err(TagError::Validation("System tags are managed by LVK"));
    }
    sqlx::query("INSERT INTO local_issue_tags(id,issue_id,tag_id) VALUES(?,?,?) ON CONFLICT(issue_id,tag_id) DO NOTHING")
        .bind(id.unwrap_or_else(Uuid::new_v4)).bind(issue).bind(tag).execute(&mut *conn).await?;
    Ok(
        sqlx::query_scalar("SELECT id FROM local_issue_tags WHERE issue_id=? AND tag_id=?")
            .bind(issue)
            .bind(tag)
            .fetch_one(&mut *conn)
            .await?,
    )
}

pub async fn remove(conn: &mut SqliteConnection, relation: Uuid) -> Result<(), TagError> {
    let name: Option<String> = sqlx::query_scalar(
        "SELECT t.name FROM local_issue_tags it JOIN local_tags t ON t.id=it.tag_id WHERE it.id=?",
    )
    .bind(relation)
    .fetch_optional(&mut *conn)
    .await?;
    if name.as_deref().is_some_and(is_system) {
        return Err(TagError::Validation("System tags are managed by LVK"));
    }
    sqlx::query("DELETE FROM local_issue_tags WHERE id=?")
        .bind(relation)
        .execute(conn)
        .await?;
    Ok(())
}

/// Called only from the existing auto-generation admission transaction.
pub async fn attach_system(
    conn: &mut SqliteConnection,
    project: Uuid,
    issue: Uuid,
) -> Result<(), sqlx::Error> {
    sqlx::query("INSERT INTO local_tags(id,project_id,name,color) SELECT ?,?,'system','0 0% 50%' WHERE NOT EXISTS(SELECT 1 FROM local_tags WHERE project_id=? AND name='system')")
        .bind(Uuid::new_v4()).bind(project).bind(project).execute(&mut *conn).await?;
    sqlx::query("INSERT INTO local_issue_tags(id,issue_id,tag_id) SELECT ?,i.id,t.id FROM local_issues i JOIN local_tags t ON t.project_id=i.project_id AND t.name='system' WHERE i.id=? AND i.project_id=? ORDER BY t.id LIMIT 1 ON CONFLICT(issue_id,tag_id) DO NOTHING")
        .bind(Uuid::new_v4()).bind(issue).bind(project).execute(conn).await?;
    Ok(())
}
