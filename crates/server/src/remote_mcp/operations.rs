//! Durable admission receipts. The caller schedules a domain operation only
//! when `admit` reports a new reservation. Unknown outcomes are never replayed.
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use sqlx::{FromRow, SqlitePool};
use uuid::Uuid;

#[derive(Clone)]
pub(super) struct Operations {
    pub pool: SqlitePool,
    pub process_instance: Uuid,
}

#[derive(FromRow, Serialize)]
pub(super) struct Receipt {
    pub id: Uuid,
    pub request_id: Uuid,
    pub project_id: Uuid,
    pub operation: String,
    pub status: String,
    pub result: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    #[serde(skip)]
    process_instance: Uuid,
    #[serde(skip)]
    input_hash: String,
}

fn fingerprint(value: &Value) -> String {
    fn canonical(value: &Value) -> Value {
        match value {
            Value::Object(map) => {
                let sorted: std::collections::BTreeMap<_, _> =
                    map.iter().map(|(k, v)| (k.clone(), canonical(v))).collect();
                Value::Object(sorted.into_iter().collect())
            }
            Value::Array(values) => Value::Array(values.iter().map(canonical).collect()),
            other => other.clone(),
        }
    }
    format!("{:x}", Sha256::digest(canonical(value).to_string()))
}

impl Operations {
    pub async fn admit(
        &self,
        principal: &str,
        request_id: Uuid,
        project_id: Uuid,
        operation: &str,
        input: &Value,
        safe_targets: &Value,
    ) -> anyhow::Result<(Receipt, bool)> {
        let hash = fingerprint(input);
        let inserted = sqlx::query("INSERT INTO remote_mcp_operations(id,principal,request_id,project_id,operation,input_hash,targets,process_instance) VALUES(?,?,?,?,?,?,?,?) ON CONFLICT(principal,request_id) DO NOTHING")
            .bind(Uuid::new_v4()).bind(principal).bind(request_id).bind(project_id).bind(operation).bind(&hash).bind(safe_targets.to_string()).bind(self.process_instance).execute(&self.pool).await?.rows_affected() == 1;
        let receipt = self.by_request(principal, request_id).await?;
        anyhow::ensure!(
            receipt.project_id == project_id
                && receipt.operation == operation
                && receipt.input_hash == hash,
            "Operation key already used with different input; inspect the existing receipt"
        );
        Ok((receipt, inserted))
    }

    pub async fn by_request(&self, principal: &str, request_id: Uuid) -> anyhow::Result<Receipt> {
        let mut receipt: Receipt = sqlx::query_as(
            "SELECT * FROM remote_mcp_operations WHERE principal=? AND request_id=?",
        )
        .bind(principal)
        .bind(request_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Operation not found"))?;
        if receipt.process_instance != self.process_instance
            && matches!(receipt.status.as_str(), "accepted" | "running")
        {
            receipt.status = "uncertain".into();
        }
        Ok(receipt)
    }

    pub async fn start(&self, id: Uuid) -> anyhow::Result<bool> {
        Ok(sqlx::query("UPDATE remote_mcp_operations SET status='running',updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id=? AND process_instance=? AND status='accepted'")
            .bind(id).bind(self.process_instance).execute(&self.pool).await?.rows_affected() == 1)
    }

    pub async fn finish(&self, id: Uuid, status: &str, safe_result: &Value) -> anyhow::Result<()> {
        anyhow::ensure!(
            matches!(status, "succeeded" | "failed" | "uncertain"),
            "Invalid final receipt status"
        );
        let updated = sqlx::query("UPDATE remote_mcp_operations SET status=?,result=?,updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id=? AND process_instance=? AND status='running'")
            .bind(status).bind(safe_result.to_string()).bind(id).bind(self.process_instance).execute(&self.pool).await?.rows_affected();
        anyhow::ensure!(
            updated == 1,
            "Receipt is not owned by this execution or already settled"
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[tokio::test]
    async fn concurrent_retries_have_one_owner_and_restart_never_replays_uncertainty() {
        let dir = tempfile::tempdir().unwrap();
        let options = sqlx::sqlite::SqliteConnectOptions::new()
            .filename(dir.path().join("receipts.sqlite"))
            .create_if_missing(true);
        let pool = SqlitePool::connect_with(options.clone()).await.unwrap();
        sqlx::query("CREATE TABLE projects(id BLOB PRIMARY KEY)")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::raw_sql(include_str!(
            "../../../db/migrations/20261013000000_remote_mcp_operations.sql"
        ))
        .execute(&pool)
        .await
        .unwrap();
        let project = Uuid::new_v4();
        sqlx::query("INSERT INTO projects VALUES(?)")
            .bind(project)
            .execute(&pool)
            .await
            .unwrap();
        let store = Operations {
            pool: pool.clone(),
            process_instance: Uuid::new_v4(),
        };
        let key = Uuid::new_v4();
        let input = json!({"title":"agreed","nested":{"b":1,"a":2}});
        let targets = json!({"project_id":project});
        let (first, second) = tokio::join!(
            store.admit("owner", key, project, "create_issue", &input, &targets),
            store.admit("owner", key, project, "create_issue", &input, &targets)
        );
        let (first, own_first) = first.unwrap();
        let (second, own_second) = second.unwrap();
        assert_eq!(first.id, second.id);
        assert_ne!(own_first, own_second);
        assert!(
            store
                .admit(
                    "owner",
                    key,
                    project,
                    "create_issue",
                    &json!({"title":"changed"}),
                    &targets
                )
                .await
                .is_err()
        );
        assert!(store.by_request("another-owner", key).await.is_err());
        assert!(store.start(first.id).await.unwrap());
        assert!(!store.start(first.id).await.unwrap());
        // Simulate a lost response and a new process opening the same durable DB.
        drop(store);
        pool.close().await;
        let pool = SqlitePool::connect_with(options).await.unwrap();
        let restarted = Operations {
            pool,
            process_instance: Uuid::new_v4(),
        };
        let (old, acquired) = restarted
            .admit("owner", key, project, "create_issue", &input, &targets)
            .await
            .unwrap();
        assert!(!acquired);
        assert_eq!(old.status, "uncertain");
        assert!(!restarted.start(old.id).await.unwrap());
        let new_key = Uuid::new_v4();
        let (new, acquired) = restarted
            .admit("owner", new_key, project, "create_issue", &input, &targets)
            .await
            .unwrap();
        assert!(acquired);
        assert!(restarted.start(new.id).await.unwrap());
        restarted
            .finish(new.id, "succeeded", &json!({"issue_id":Uuid::new_v4()}))
            .await
            .unwrap();
        let (same, acquired) = restarted
            .admit("owner", new_key, project, "create_issue", &input, &targets)
            .await
            .unwrap();
        assert!(!acquired);
        assert_eq!(same.status, "succeeded");
        assert!(same.result.is_some());
        assert!(
            restarted
                .finish(new.id, "failed", &Value::Null)
                .await
                .is_err()
        );
    }
}
