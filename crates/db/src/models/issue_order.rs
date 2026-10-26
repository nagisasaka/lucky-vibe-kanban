//! Targeted board ordering; never renumber unrelated cards.
use sqlx::SqliteConnection;
use uuid::Uuid;

pub async fn promote(conn: &mut SqliteConnection, issue_id: Uuid) -> Result<(), sqlx::Error> {
    // The existing global UI-preferences scratch is shared by browser sessions.
    // Read it in the event transaction, including while the board is unmounted.
    let preferences: Option<sqlx::types::Json<serde_json::Value>> = sqlx::query_scalar(
        "SELECT payload FROM scratch WHERE id = ? AND scratch_type = 'UI_PREFERENCES'",
    )
    .bind(Uuid::from_u128(1))
    .fetch_optional(&mut *conn)
    .await?;
    let project: Option<Uuid> =
        sqlx::query_scalar("SELECT project_id FROM local_issues WHERE id=?")
            .bind(issue_id)
            .fetch_optional(&mut *conn)
            .await?;
    let Some(project) = project else {
        return Ok(());
    };
    let Some(descending) =
        manual_direction(preferences.as_ref().map(|p| &p.0), &project.to_string())
    else {
        return Ok(());
    };
    sqlx::query(
        r#"UPDATE local_issues SET sort_order = (
            SELECT CASE WHEN ? THEN COALESCE(MAX(other.sort_order), -1) + 1 ELSE COALESCE(MIN(other.sort_order), 1) - 1 END FROM local_issues other
            WHERE other.project_id = local_issues.project_id
              AND other.status_id = local_issues.status_id
        )
        WHERE id = ?"#,
    )
    .bind(descending)
    .bind(issue_id)
    .execute(conn)
    .await?;
    Ok(())
}

// Match resolveKanbanProjectState/getKanbanDefaultView; no separate preference.
fn manual_direction(payload: Option<&serde_json::Value>, project: &str) -> Option<bool> {
    let data = payload.and_then(|p| p.get("data"));
    let view = data
        .and_then(|d| d.get("kanban_project_view_selections"))
        .and_then(|p| p.get(project))
        .and_then(|p| p.get("activeViewId"))
        .and_then(|v| v.as_str())
        .filter(|v| *v == "team" || *v == "personal")
        .unwrap_or("team");
    let filters = data
        .and_then(|d| d.get("kanban_project_view_preferences"))
        .and_then(|p| p.get(project))
        .and_then(|p| p.get(view))
        .and_then(|v| v.get("filters"));
    let field = filters
        .and_then(|f| f.get("sortField"))
        .and_then(|v| v.as_str())
        .unwrap_or(if view == "personal" {
            "priority"
        } else {
            "sort_order"
        });
    (field == "sort_order").then(|| {
        filters
            .and_then(|f| f.get("sortDirection"))
            .and_then(|v| v.as_str())
            == Some("desc")
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn explicit_sorts_suspend_activity_and_manual_direction_matches_view() {
        let project = "project";
        assert_eq!(manual_direction(None, project), Some(false));
        for field in ["title", "priority", "created_at", "updated_at"] {
            let payload = serde_json::json!({"data":{"kanban_project_view_preferences":{project:{"team":{"filters":{"sortField":field}}}}}});
            assert_eq!(manual_direction(Some(&payload), project), None);
        }
        let personal = serde_json::json!({"data":{"kanban_project_view_selections":{project:{"activeViewId":"personal"}}}});
        assert_eq!(manual_direction(Some(&personal), project), None);
        let descending = serde_json::json!({"data":{"kanban_project_view_preferences":{project:{"team":{"filters":{"sortField":"sort_order","sortDirection":"desc"}}}}}});
        assert_eq!(manual_direction(Some(&descending), project), Some(true));
    }

    #[tokio::test]
    async fn issue_activity_promotes_only_target_and_rolls_back() {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::query("CREATE TABLE local_issues(id BLOB PRIMARY KEY, project_id BLOB, status_id BLOB, sort_order REAL, updated_at TEXT)").execute(&pool).await.unwrap();
        sqlx::query("CREATE TABLE scratch(id BLOB, scratch_type TEXT, payload TEXT)")
            .execute(&pool)
            .await
            .unwrap();
        let project = Uuid::new_v4();
        let status = Uuid::new_v4();
        let ids = [Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4()];
        for (index, id) in ids.iter().enumerate() {
            sqlx::query("INSERT INTO local_issues VALUES(?,?,?,?,datetime('now'))")
                .bind(id)
                .bind(project)
                .bind(status)
                .bind(index as f64)
                .execute(&pool)
                .await
                .unwrap();
        }
        // A persisted explicit sort suppresses writes, even without a board.
        let payload = serde_json::json!({"data":{"kanban_project_view_preferences":{project.to_string():{"team":{"filters":{"sortField":"title"}}}}}});
        sqlx::query("INSERT INTO scratch VALUES(?,'UI_PREFERENCES',?)")
            .bind(Uuid::from_u128(1))
            .bind(payload.to_string())
            .execute(&pool)
            .await
            .unwrap();
        let mut tx = pool.begin().await.unwrap();
        promote(&mut tx, ids[2]).await.unwrap();
        tx.commit().await.unwrap();
        let unchanged: f64 = sqlx::query_scalar("SELECT sort_order FROM local_issues WHERE id=?")
            .bind(ids[2])
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(unchanged, 2.0);
        sqlx::query("DELETE FROM scratch")
            .execute(&pool)
            .await
            .unwrap();
        let mut tx = pool.begin().await.unwrap();
        promote(&mut tx, ids[2]).await.unwrap();
        tx.commit().await.unwrap();
        let ordered: Vec<Uuid> =
            sqlx::query_scalar("SELECT id FROM local_issues ORDER BY sort_order")
                .fetch_all(&pool)
                .await
                .unwrap();
        assert_eq!(ordered, vec![ids[2], ids[0], ids[1]]);
        let mut tx = pool.begin().await.unwrap();
        promote(&mut tx, ids[1]).await.unwrap();
        tx.rollback().await.unwrap();
        let ordered: Vec<Uuid> =
            sqlx::query_scalar("SELECT id FROM local_issues ORDER BY sort_order")
                .fetch_all(&pool)
                .await
                .unwrap();
        assert_eq!(ordered, vec![ids[2], ids[0], ids[1]]);
    }
}
