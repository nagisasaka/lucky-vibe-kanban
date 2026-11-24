//! All semantic evidence is read from the same SQLite snapshot.
use api_types::{SavedFilterAssigneeDisplayName, SavedFilterCorpus};
use sqlx::{Row, SqlitePool};
use uuid::Uuid;

use crate::error::ApiError;

pub(crate) async fn load(pool: &SqlitePool, project: Uuid) -> Result<SavedFilterCorpus, ApiError> {
    let mut tx = pool.begin().await?;
    if sqlx::query_scalar::<_, i64>("SELECT 1 FROM projects WHERE id=?")
        .bind(project)
        .fetch_optional(&mut *tx)
        .await?
        .is_none()
    {
        return Err(db::models::saved_filter::SavedFilterError::NotFound.into());
    }
    macro_rules! read {
        ($query:expr,$decode:expr) => {{
            let rows = sqlx::query($query)
                .bind(project)
                .fetch_all(&mut *tx)
                .await?;
            rows.iter()
                .map($decode)
                .collect::<Result<Vec<_>, sqlx::Error>>()?
        }};
    }
    let corpus = SavedFilterCorpus {
        issues: read!(
            "SELECT * FROM local_issues WHERE project_id=? ORDER BY issue_number,id",
            super::issue_from_row
        ),
        statuses: read!(
            "SELECT * FROM local_project_statuses WHERE project_id=? ORDER BY id",
            super::status_from_row
        ),
        tags: read!(
            "SELECT * FROM local_tags WHERE project_id=? ORDER BY id",
            super::tag_from_row
        ),
        issue_tags: read!(
            "SELECT t.* FROM local_issue_tags t JOIN local_issues i ON i.id=t.issue_id WHERE i.project_id=? ORDER BY t.id",
            super::issue_tag_from_row
        ),
        assignees: read!(
            "SELECT a.* FROM local_issue_assignees a JOIN local_issues i ON i.id=a.issue_id WHERE i.project_id=? ORDER BY a.id",
            super::issue_assignee_from_row
        ),
        // Match visible_members' active-member display names, scoped by actual
        // Project assignments rather than the current viewer/issuer. Member IDs
        // are TEXT UUIDs; assignment IDs are BLOB UUIDs. Never select profile data.
        assignee_display_names: read!(
            "SELECT DISTINCT a.user_id,m.display_name FROM local_issue_assignees a JOIN local_issues i ON i.id=a.issue_id JOIN access_members m ON lower(replace(m.id,'-',''))=lower(hex(a.user_id)) WHERE i.project_id=? AND m.state='active' ORDER BY a.user_id",
            |row| Ok(SavedFilterAssigneeDisplayName {
                user_id: row.try_get("user_id")?,
                display_name: row.try_get("display_name")?
            })
        ),
        relationships: read!(
            "SELECT r.* FROM local_issue_relationships r JOIN local_issues i ON i.id=r.issue_id JOIN local_issues j ON j.id=r.related_issue_id AND j.project_id=i.project_id WHERE i.project_id=? ORDER BY r.id",
            super::issue_relationship_from_row
        ),
    };
    tx.commit().await?;
    Ok(corpus)
}
