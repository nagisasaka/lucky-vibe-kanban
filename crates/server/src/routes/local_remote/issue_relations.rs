//! Board/stdio/remote share validation under SQLite's writer reservation. No
//! migration or cleanup of historical relationships is needed for additive writes.
use super::*;

pub(super) async fn validate_parent(
    conn: &mut sqlx::SqliteConnection,
    child: Uuid,
    project: Uuid,
    parent: Option<Uuid>,
) -> Result<(), ApiError> {
    let mut next = parent;
    let mut seen = std::collections::HashSet::new();
    while let Some(id) = next {
        if id == child || !seen.insert(id) {
            return Err(ApiError::BadRequest(
                "Parent relationship would contain a cycle".into(),
            ));
        }
        let row = sqlx::query("SELECT project_id,parent_issue_id FROM local_issues WHERE id=?")
            .bind(id)
            .fetch_optional(&mut *conn)
            .await?
            .ok_or_else(|| ApiError::BadRequest("Parent Issue not found".into()))?;
        if row.try_get::<Uuid, _>("project_id")? != project {
            return Err(ApiError::BadRequest(
                "Parent must be in the same Project".into(),
            ));
        }
        next = row.try_get("parent_issue_id")?;
    }
    Ok(())
}

// Remote supplies the observed source Issue revision and Project scope. UI and
// stdio retain their existing unversioned Board contract.
pub(crate) struct Guard {
    pub project: Uuid,
    pub issue: Uuid,
    pub revision: DateTime<Utc>,
}
async fn guard(conn: &mut sqlx::SqliteConnection, guard: &Guard) -> Result<(), ApiError> {
    let row = sqlx::query("SELECT project_id,julianday(updated_at)=julianday(?) AS current FROM local_issues WHERE id=?")
        .bind(guard.revision).bind(guard.issue).fetch_one(&mut *conn).await?;
    if row.try_get::<Uuid, _>("project_id")? != guard.project {
        return Err(ApiError::BadRequest("Issue not found".into()));
    }
    if !row.try_get::<bool, _>("current")? {
        return Err(ApiError::Conflict(
            "Issue changed while the update was being submitted".into(),
        ));
    }
    Ok(())
}
async fn endpoints(conn: &mut sqlx::SqliteConnection, a: Uuid, b: Uuid) -> Result<Uuid, ApiError> {
    if a == b {
        return Err(ApiError::BadRequest(
            "An Issue cannot relate to itself".into(),
        ));
    }
    let a = get_local_issue_on(&mut *conn, a).await?;
    let b = get_local_issue_on(&mut *conn, b).await?;
    if a.project_id != b.project_id {
        return Err(ApiError::BadRequest(
            "Related Issues must be in the same Project".into(),
        ));
    }
    Ok(a.project_id)
}

pub(crate) async fn create(
    pool: &SqlitePool,
    request: &CreateIssueRelationshipRequest,
    expected: Option<Guard>,
) -> Result<IssueRelationship, IssueMutationError> {
    let mut tx = pool.begin_with("BEGIN IMMEDIATE").await?;
    if let Some(expected) = &expected {
        guard(&mut tx, expected).await?;
        if expected.issue != request.issue_id {
            return Err(ApiError::BadRequest("Relationship source mismatch".into()).into());
        }
    }
    let project = endpoints(&mut tx, request.issue_id, request.related_issue_id).await?;
    if expected.as_ref().is_some_and(|g| g.project != project) {
        return Err(ApiError::BadRequest("Issue not found".into()).into());
    }
    let kind = relationship_type_to_str(request.relationship_type);
    // Related is undirected; blocking remains directional and may form cycles.
    // The reserved writer serializes all common entrypoints, including reversed
    // related requests. Existing duplicate rows are left intact and returned as-is.
    let row = sqlx::query("SELECT * FROM local_issue_relationships WHERE relationship_type=? AND ((issue_id=? AND related_issue_id=?) OR (?='related' AND issue_id=? AND related_issue_id=?)) ORDER BY id LIMIT 1")
        .bind(kind).bind(request.issue_id).bind(request.related_issue_id)
        .bind(kind).bind(request.related_issue_id).bind(request.issue_id)
        .fetch_optional(&mut *tx).await?;
    let data = if let Some(row) = row {
        issue_relationship_from_row(&row)?
    } else {
        let id = request.id.unwrap_or_else(Uuid::new_v4);
        // An unrelated primary-key collision is an error, never a retry success.
        sqlx::query("INSERT INTO local_issue_relationships(id,issue_id,related_issue_id,relationship_type) VALUES(?,?,?,?)")
            .bind(id).bind(request.issue_id).bind(request.related_issue_id).bind(kind)
            .execute(&mut *tx).await?;
        let row = sqlx::query("SELECT * FROM local_issue_relationships WHERE id=?")
            .bind(id)
            .fetch_one(&mut *tx)
            .await?;
        issue_relationship_from_row(&row)?
    };
    tx.commit().await.map_err(issue_commit_failure)?;
    Ok(data)
}

pub(crate) async fn delete(
    pool: &SqlitePool,
    id: Uuid,
    expected: Option<Guard>,
) -> Result<(), IssueMutationError> {
    let mut tx = pool.begin_with("BEGIN IMMEDIATE").await?;
    if let Some(expected) = &expected {
        guard(&mut tx, expected).await?;
    }
    let row = sqlx::query("SELECT * FROM local_issue_relationships WHERE id=?")
        .bind(id)
        .fetch_optional(&mut *tx)
        .await?;
    if let Some(row) = row {
        let relation = issue_relationship_from_row(&row)?;
        let project = endpoints(&mut tx, relation.issue_id, relation.related_issue_id).await?;
        if expected.as_ref().is_some_and(|g| {
            g.project != project
                || (g.issue != relation.issue_id && g.issue != relation.related_issue_id)
        }) {
            return Err(ApiError::BadRequest("Relationship not in this Issue".into()).into());
        }
        sqlx::query("DELETE FROM local_issue_relationships WHERE id=?")
            .bind(id)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await.map_err(issue_commit_failure)?;
    Ok(())
}
