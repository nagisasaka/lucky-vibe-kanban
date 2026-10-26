use sqlx::{Sqlite, Transaction};
use uuid::Uuid;

/// Allocate within the caller's BEGIN IMMEDIATE transaction. The caller must
/// insert the Issue before committing, so all creation paths serialize writers.
pub async fn next_issue_number(
    tx: &mut Transaction<'_, Sqlite>,
    project_id: Uuid,
) -> Result<(i32, String), sqlx::Error> {
    let key: String = sqlx::query_scalar("SELECT key FROM projects WHERE id=?")
        .bind(project_id)
        .fetch_one(&mut **tx)
        .await?;
    let number: i32 = sqlx::query_scalar(
        "SELECT COALESCE(MAX(issue_number),0)+1 FROM local_issues WHERE project_id=?",
    )
    .bind(project_id)
    .fetch_one(&mut **tx)
    .await?;
    Ok((number, format!("{key}-{number}")))
}

#[cfg(test)]
mod tests {
    use sqlx::sqlite::SqlitePoolOptions;

    use super::*;

    #[tokio::test]
    async fn project_key_migration_preserves_existing_issue_values_and_continues_local_numbers() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        let mut old = sqlx::migrate!("./migrations");
        old.migrations = std::borrow::Cow::Owned(
            old.migrations
                .into_owned()
                .into_iter()
                .filter(|m| m.version < 20261025120000)
                .collect(),
        );
        old.run(&pool).await.unwrap();
        let project = Uuid::new_v4();
        let status = Uuid::new_v4();
        let issue = Uuid::new_v4();
        sqlx::query("INSERT INTO projects(id,name) VALUES(?,'Old')")
            .bind(project)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO local_project_statuses(id,project_id,name,color,sort_order) VALUES(?,?,'Ready','blue',0)").bind(status).bind(project).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO local_issues(id,project_id,issue_number,simple_id,status_id,title,sort_order) VALUES(?,?,177,'LOCAL-177',?,'Old',0)").bind(issue).bind(project).bind(status).execute(&pool).await.unwrap();
        let before: (String, i32, String) =
            sqlx::query_as("SELECT hex(id),issue_number,simple_id FROM local_issues")
                .fetch_one(&pool)
                .await
                .unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        let after: (String, i32, String) =
            sqlx::query_as("SELECT hex(id),issue_number,simple_id FROM local_issues")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(before, after);
        assert_eq!(
            sqlx::query_scalar::<_, String>("SELECT key FROM projects")
                .fetch_one(&pool)
                .await
                .unwrap(),
            "LOCAL"
        );
        let mut tx = pool.begin_with("BEGIN IMMEDIATE").await.unwrap();
        assert_eq!(
            next_issue_number(&mut tx, project).await.unwrap(),
            (178, "LOCAL-178".into())
        );
        tx.rollback().await.unwrap();
    }
    #[tokio::test]
    async fn classification_migration_applies_after_existing_project_key_migration() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        let mut old = sqlx::migrate!("./migrations");
        old.migrations = std::borrow::Cow::Owned(
            old.migrations
                .into_owned()
                .into_iter()
                .filter(|m| m.version != 20261025000000)
                .collect(),
        );
        old.run(&pool).await.unwrap();
        let project = Uuid::new_v4();
        let status = Uuid::new_v4();
        let issue = Uuid::new_v4();
        sqlx::query("INSERT INTO projects(id,name,key) VALUES(?,'Already keyed','LVK')")
            .bind(project)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO local_project_statuses(id,project_id,name,color,sort_order) VALUES(?,?,'Ready','blue',0)").bind(status).bind(project).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO local_issues(id,project_id,issue_number,simple_id,status_id,title,sort_order) VALUES(?,?,177,'LVK-177',?,'Old',0)").bind(issue).bind(project).bind(status).execute(&pool).await.unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        let row: (String, i32, String, String) = sqlx::query_as("SELECT p.key,i.issue_number,i.simple_id,i.priority FROM local_issues i JOIN projects p ON p.id=i.project_id WHERE i.id=?").bind(issue).fetch_one(&pool).await.unwrap();
        assert_eq!(row, ("LVK".into(), 177, "LVK-177".into(), "medium".into()));
        let tags: Vec<String> =
            sqlx::query_scalar("SELECT name FROM local_tags WHERE project_id=? ORDER BY name")
                .bind(project)
                .fetch_all(&pool)
                .await
                .unwrap();
        assert_eq!(tags, vec!["bug", "feature", "survey", "system"]);
        let mut tx = pool.begin_with("BEGIN IMMEDIATE").await.unwrap();
        assert_eq!(
            next_issue_number(&mut tx, project).await.unwrap(),
            (178, "LVK-178".into())
        );
        tx.rollback().await.unwrap();
    }
}
