//! Durable admission receipts. The caller schedules a domain operation only
//! when `admit` reports a new reservation. Unknown outcomes are never replayed.
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use sqlx::{FromRow, SqlitePool};
use uuid::Uuid;

// If domain completion cannot be persisted, keep that uncertainty visible in
// this process as well. Restart recovery already treats all older pending rows
// as uncertain. Entries contain identities only and live until process exit;
// they never grant ownership or authorise re-execution.
static SETTLEMENT_UNCERTAIN: std::sync::LazyLock<
    std::sync::Mutex<std::collections::HashSet<(Uuid, Uuid)>>,
> = std::sync::LazyLock::new(Default::default);

#[derive(Clone)]
pub(crate) struct Operations {
    pub pool: SqlitePool,
    pub process_instance: Uuid,
}

#[derive(FromRow, Serialize)]
pub(crate) struct Receipt {
    pub id: Uuid,
    pub request_id: Uuid,
    pub project_id: Uuid,
    pub operation: String,
    pub status: String,
    pub result: Option<String>,
    #[serde(skip)]
    pub targets: String,
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

fn project_archive_diagnostics(value: &mut Value) {
    match value {
        Value::Object(object) => {
            for (key, value) in object {
                if key == "deletion_error" && !value.is_null() {
                    *value = serde_json::json!(
                        "Worktree cleanup failed; inspect the existing target and receipt before recovery"
                    );
                } else {
                    project_archive_diagnostics(value);
                }
            }
        }
        Value::Array(values) => {
            for value in values {
                project_archive_diagnostics(value);
            }
        }
        _ => {}
    }
}

impl Operations {
    /// The owned task starts before the first database await. Dropping the MCP
    /// request (even during admission) cannot cancel an admitted operation.
    #[allow(
        clippy::too_many_arguments,
        reason = "Keep receipt identity, idempotency input and safe audit targets explicit at this boundary"
    )]
    pub async fn submit<F, Fut>(
        &self,
        principal: String,
        request_id: Uuid,
        project_id: Uuid,
        operation: String,
        input: Value,
        safe_targets: Value,
        execute: F,
    ) -> anyhow::Result<Receipt>
    where
        F: FnOnce(Uuid) -> Fut + Send + 'static,
        Fut: std::future::Future<Output = (&'static str, Value)> + Send + 'static,
    {
        self.submit_timed(
            principal,
            request_id,
            project_id,
            operation,
            input,
            safe_targets,
            execute,
            false,
        )
        .await
    }

    /// Wait for domain admission (never for coding/Integration completion).
    /// The owned task still survives cancellation of this waiting caller.
    #[allow(
        clippy::too_many_arguments,
        reason = "Keep receipt identity, idempotency input and safe audit targets explicit at this boundary"
    )]
    pub async fn submit_and_wait<F, Fut>(
        &self,
        principal: String,
        request_id: Uuid,
        project_id: Uuid,
        operation: String,
        input: Value,
        safe_targets: Value,
        execute: F,
    ) -> anyhow::Result<Receipt>
    where
        F: FnOnce(Uuid) -> Fut + Send + 'static,
        Fut: std::future::Future<Output = (&'static str, Value)> + Send + 'static,
    {
        self.submit_timed(
            principal,
            request_id,
            project_id,
            operation,
            input,
            safe_targets,
            execute,
            true,
        )
        .await
    }

    #[allow(
        clippy::too_many_arguments,
        reason = "Keep receipt identity, idempotency input and safe audit targets explicit at this boundary"
    )]
    async fn submit_timed<F, Fut>(
        &self,
        principal: String,
        request_id: Uuid,
        project_id: Uuid,
        operation: String,
        input: Value,
        safe_targets: Value,
        execute: F,
        wait_for_domain: bool,
    ) -> anyhow::Result<Receipt>
    where
        F: FnOnce(Uuid) -> Fut + Send + 'static,
        Fut: std::future::Future<Output = (&'static str, Value)> + Send + 'static,
    {
        let store = self.clone();
        let (sender, receiver) = tokio::sync::oneshot::channel();
        // Owned admission deliberately survives the HTTP request. Preserve the
        // verified actor across both spawned tasks for domain write attribution.
        let access_actor = server_access::ACTOR.try_with(Clone::clone).ok();
        tokio::spawn(async move {
            let admitted = store
                .admit(
                    &principal,
                    request_id,
                    project_id,
                    &operation,
                    &input,
                    &safe_targets,
                )
                .await;
            let (mut receipt, owned) = match admitted {
                Ok(value) => value,
                Err(error) => {
                    let _ = sender.send(Err(error));
                    return;
                }
            };
            if !owned {
                let _ = sender.send(Ok(receipt));
                return;
            }
            let id = receipt.id;
            match store.start(id).await {
                Ok(true) => {}
                _ => {
                    let _ = sender.send(Err(super::errors::unknown()));
                    return;
                }
            }
            receipt.status = "running".into();
            let sender = if wait_for_domain {
                Some(sender)
            } else {
                let _ = sender.send(Ok(receipt));
                None
            };
            // A panic is an unknown domain outcome, never permission to replay.
            let result = tokio::spawn(async move {
                if let Some(actor) = access_actor {
                    server_access::ACTOR.scope(actor, execute(id)).await
                } else {
                    execute(id).await
                }
            })
            .await;
            let (status, result) = result.unwrap_or_else(|_| ("uncertain", serde_json::json!({"message":"Operation outcome unavailable; inspect its targets before any new request"})));
            if store.finish(id, status, &result).await.is_err() {
                SETTLEMENT_UNCERTAIN
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .insert((store.process_instance, id));
                tracing::error!(operation_id=%id, "MCP operation receipt could not be settled");
                if let Some(sender) = sender {
                    let _ = sender.send(Err(super::errors::unknown()));
                }
                return;
            }
            if let Some(sender) = sender {
                let _ = sender.send(store.by_request(&principal, request_id).await);
            }
        });
        receiver.await.map_err(|_| super::errors::unknown())?
    }

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
        if receipt.project_id != project_id
            || receipt.operation != operation
            || receipt.input_hash != hash
        {
            let mut diagnostic = utils::mcp_error::Diagnostic::new(
                utils::mcp_error::Category::Conflict,
                utils::mcp_error::Outcome::Unknown,
            );
            diagnostic.message =
                "Operation key already used with different input; inspect the existing receipt";
            return Err(diagnostic.into());
        }
        Ok((receipt, inserted))
    }

    /// Check an owned replay before resolving mutable domain targets. Uses the
    /// same canonical fingerprint as admission; never creates or re-executes work.
    pub async fn replay(
        &self,
        principal: &str,
        request_id: Uuid,
        operation: &str,
        input: &Value,
    ) -> anyhow::Result<Option<Receipt>> {
        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM remote_mcp_operations WHERE principal=? AND request_id=?)",
        )
        .bind(principal)
        .bind(request_id)
        .fetch_one(&self.pool)
        .await?;
        if !exists {
            return Ok(None);
        }
        let receipt = self.by_request(principal, request_id).await?;
        if receipt.operation != operation || receipt.input_hash != fingerprint(input) {
            return Err(utils::mcp_error::Diagnostic::new(
                utils::mcp_error::Category::Conflict,
                utils::mcp_error::Outcome::Unknown,
            )
            .into());
        }
        Ok(Some(receipt))
    }

    pub async fn by_request(&self, principal: &str, request_id: Uuid) -> anyhow::Result<Receipt> {
        let mut receipt: Receipt = sqlx::query_as(
            "SELECT * FROM remote_mcp_operations WHERE principal=? AND request_id=?",
        )
        .bind(principal)
        .bind(request_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| {
            super::errors::rejection_with_message(
                utils::mcp_error::Category::NotFound,
                "Operation not found",
            )
        })?;
        if (receipt.process_instance != self.process_instance
            || SETTLEMENT_UNCERTAIN
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .contains(&(self.process_instance, receipt.id)))
            && matches!(receipt.status.as_str(), "accepted" | "running")
        {
            receipt.status = "uncertain".into();
        }
        // Archive receipts predate the shared diagnostic contract and may contain
        // raw filesystem/SQL errors. Project at read time without rewriting the
        // stored evidence, including local stdio receipt responses.
        if matches!(
            receipt.operation.as_str(),
            "confirm_issue_archive" | "unarchive_issue" | "confirm_workspace_archive"
        ) && let Some(raw) = receipt.result.as_deref()
            && let Ok(mut value) = serde_json::from_str::<Value>(raw)
            && value.is_object()
        {
            project_archive_diagnostics(&mut value);
            if matches!(receipt.status.as_str(), "failed" | "uncertain") {
                let mut diagnostic =
                    super::errors::receipt_diagnostic(&receipt.status, Some(&value));
                diagnostic.correlation_id = receipt.id;
                if value.get("message").is_some() {
                    value["message"] = serde_json::json!(diagnostic.message);
                }
                if value.get("error").is_some() {
                    value["error"] = diagnostic.value();
                }
            }
            receipt.result = Some(value.to_string());
        }
        if matches!(receipt.status.as_str(), "failed" | "uncertain")
            && let Some(raw) = receipt.result.as_deref()
            && let Ok(mut value) = serde_json::from_str::<Value>(raw)
        {
            utils::mcp_error::redact_failure_context(&mut value);
            receipt.result = Some(value.to_string());
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
        if status != "succeeded" {
            tracing::warn!(operation_id=%id, status, "MCP operation settled without success");
        }
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

    async fn isolated_store() -> (Operations, Uuid) {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
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
        (
            Operations {
                pool,
                process_instance: Uuid::new_v4(),
            },
            project,
        )
    }

    #[tokio::test]
    async fn replay_checks_owner_operation_and_content_before_domain_resolution() {
        let (store, project) = isolated_store().await;
        let key = Uuid::new_v4();
        let input = json!({"session_id":Uuid::new_v4(),"prompt":"continue"});
        assert!(
            store
                .replay("owner", key, "continue_execution", &input)
                .await
                .unwrap()
                .is_none()
        );
        let first = store
            .submit_and_wait(
                "owner".into(),
                key,
                project,
                "continue_execution".into(),
                input.clone(),
                json!({}),
                |_| async { ("succeeded", json!({"execution_accepted":true})) },
            )
            .await
            .unwrap();
        // There need not be a surviving Session/Workspace row to recover its receipt.
        let replay = store
            .replay("owner", key, "continue_execution", &input)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(first.id, replay.id);
        assert!(
            store
                .replay("another-owner", key, "continue_execution", &input)
                .await
                .unwrap()
                .is_none()
        );
        assert!(
            store
                .replay("owner", key, "start_session", &input)
                .await
                .is_err()
        );
        assert!(
            store
                .replay(
                    "owner",
                    key,
                    "continue_execution",
                    &json!({"prompt":"different"})
                )
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn settlement_failure_does_not_repeat_a_committed_side_effect() {
        use std::sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        };
        let (store, project) = isolated_store().await;
        sqlx::query("CREATE TRIGGER reject_settlement BEFORE UPDATE ON remote_mcp_operations WHEN NEW.status IN ('succeeded','failed','uncertain') BEGIN SELECT RAISE(ABORT, 'SELECT secret FROM /private/host/user-input-token'); END")
            .execute(&store.pool).await.unwrap();
        let count = Arc::new(AtomicUsize::new(0));
        let write_count = count.clone();
        let key = Uuid::new_v4();
        let error = store
            .submit_and_wait(
                "owner".into(),
                key,
                project,
                "fixture".into(),
                json!({}),
                json!({}),
                move |_| async move {
                    write_count.fetch_add(1, Ordering::SeqCst);
                    ("succeeded", json!({"done":true}))
                },
            )
            .await
            .err()
            .expect("receipt settlement must fail");
        let public = super::super::errors::message(&error);
        for secret in ["SELECT", "secret", "/private/host", "user-input-token"] {
            assert!(!public.contains(secret));
        }
        assert_eq!(count.load(Ordering::SeqCst), 1);
        let retry = store
            .submit_and_wait(
                "owner".into(),
                key,
                project,
                "fixture".into(),
                json!({}),
                json!({}),
                |_| async { panic!("must not repeat") },
            )
            .await
            .unwrap();
        assert_eq!(retry.status, "uncertain");
        assert_eq!(count.load(Ordering::SeqCst), 1);
        // A fresh process cannot recover a final outcome from the failed write.
        let restarted = Operations {
            pool: store.pool.clone(),
            process_instance: Uuid::new_v4(),
        };
        assert_eq!(
            restarted.by_request("owner", key).await.unwrap().status,
            "uncertain"
        );
        store.pool.close().await;
    }

    #[tokio::test]
    async fn historical_archive_receipt_diagnostics_are_projected_without_rewriting_evidence() {
        let (store, project) = isolated_store().await;
        let request = Uuid::new_v4();
        let saved = json!({"message":"SQL SELECT /private/SECRET","workspaces":[{"workspace_id":Uuid::new_v4(),"archived":true,"deletion_error":"/private/SECRET"}]});
        let (receipt, _) = store
            .admit(
                "owner",
                request,
                project,
                "confirm_issue_archive",
                &json!({}),
                &json!({}),
            )
            .await
            .unwrap();
        store.start(receipt.id).await.unwrap();
        store.finish(receipt.id, "uncertain", &saved).await.unwrap();
        let public = store.by_request("owner", request).await.unwrap();
        let projected: Value = serde_json::from_str(public.result.as_deref().unwrap()).unwrap();
        assert!(projected["workspaces"][0]["archived"] == true);
        assert!(!projected.to_string().contains("SECRET"));
        assert!(!projected.to_string().contains("/private"));
        let raw: String = sqlx::query_scalar("SELECT result FROM remote_mcp_operations WHERE id=?")
            .bind(receipt.id)
            .fetch_one(&store.pool)
            .await
            .unwrap();
        assert_eq!(serde_json::from_str::<Value>(&raw).unwrap(), saved);
        let request = Uuid::new_v4();
        let (receipt, _) = store
            .admit(
                "owner",
                request,
                project,
                "confirm_workspace_archive",
                &json!({}),
                &json!({}),
            )
            .await
            .unwrap();
        store.start(receipt.id).await.unwrap();
        let diagnostic = utils::mcp_error::Diagnostic::new(
            utils::mcp_error::Category::Conflict,
            utils::mcp_error::Outcome::Failed,
        );
        store
            .finish(
                receipt.id,
                "failed",
                &json!({"code":"archive_refused","error":diagnostic}),
            )
            .await
            .unwrap();
        let public = store.by_request("owner", request).await.unwrap();
        let value: Value = serde_json::from_str(public.result.as_deref().unwrap()).unwrap();
        assert_eq!(value["error"]["category"], "conflict");
    }

    #[tokio::test]
    async fn panicked_domain_is_unknown_and_same_request_never_reexecutes() {
        let (store, project) = isolated_store().await;
        let key = Uuid::new_v4();
        let receipt = store
            .submit_and_wait(
                "owner".into(),
                key,
                project,
                "fixture".into(),
                json!({}),
                json!({}),
                |_| async { panic!("SELECT secret /private/host user-input-token") },
            )
            .await
            .unwrap();
        assert_eq!(receipt.status, "uncertain");
        let result = receipt.result.as_deref().unwrap();
        for secret in ["SELECT", "secret", "/private/host", "user-input-token"] {
            assert!(!result.contains(secret));
        }
        let retry = store
            .submit_and_wait(
                "owner".into(),
                key,
                project,
                "fixture".into(),
                json!({}),
                json!({}),
                |_| async { panic!("must not repeat") },
            )
            .await
            .unwrap();
        assert_eq!(receipt.id, retry.id);
        assert_eq!(retry.status, "uncertain");
        store.pool.close().await;
    }

    #[tokio::test]
    async fn receipt_states_distinguish_admission_execution_and_terminal_results() {
        let (store, project) = isolated_store().await;
        for final_status in ["succeeded", "failed"] {
            let key = Uuid::new_v4();
            let (accepted, owned) = store
                .admit("owner", key, project, "fixture", &json!({}), &json!({}))
                .await
                .unwrap();
            assert!(owned);
            assert_eq!(accepted.status, "accepted");
            assert!(accepted.result.is_none());
            assert!(store.start(accepted.id).await.unwrap());
            let running = store.by_request("owner", key).await.unwrap();
            assert_eq!(running.status, "running");
            assert!(running.result.is_none());
            store
                .finish(accepted.id, final_status, &json!({"done":true}))
                .await
                .unwrap();
            let finished = store.by_request("owner", key).await.unwrap();
            assert_eq!(finished.status, final_status);
            assert!(finished.result.is_some());
        }
        store.pool.close().await;
    }

    #[tokio::test]
    async fn disconnected_request_during_admission_still_finishes_once() {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
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
        let held = pool.acquire().await.unwrap();
        let (done, received) = tokio::sync::oneshot::channel();
        let mut request = Box::pin(store.submit(
            "owner".into(),
            key,
            project,
            "test".into(),
            json!({}),
            json!({}),
            move |_| async move {
                let _ = done.send(());
                ("succeeded", json!({"done":true}))
            },
        ));
        std::future::poll_fn(|cx| {
            assert!(std::future::Future::poll(request.as_mut(), cx).is_pending());
            std::task::Poll::Ready(())
        })
        .await;
        // The caller disappears while the spawned admission is waiting for DB.
        drop(request);
        drop(held);
        tokio::time::timeout(std::time::Duration::from_secs(5), received)
            .await
            .unwrap()
            .unwrap();
        tokio::time::timeout(std::time::Duration::from_secs(5), async {
            loop {
                let receipt = store.by_request("owner", key).await.unwrap();
                if receipt.status == "succeeded" {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        let receipt = store
            .submit(
                "owner".into(),
                key,
                project,
                "test".into(),
                json!({}),
                json!({}),
                |_| async { panic!("duplicate execution") },
            )
            .await
            .unwrap();
        assert_eq!(receipt.status, "succeeded");
    }

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
        let conflict = store
            .admit(
                "owner",
                key,
                project,
                "create_issue",
                &json!({"title":"different"}),
                &targets,
            )
            .await
            .err()
            .unwrap();
        let diagnostic = super::super::errors::diagnostic(&conflict, true);
        assert_eq!(diagnostic.category, utils::mcp_error::Category::Conflict);
        assert_eq!(diagnostic.outcome, utils::mcp_error::Outcome::Unknown);
        assert_eq!(diagnostic.recovery, "inspect_receipt_and_state");
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
