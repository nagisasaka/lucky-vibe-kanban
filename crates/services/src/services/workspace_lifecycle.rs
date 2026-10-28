//! Serialize worktree lifecycle changes with runtime admission, per Workspace.
//! Lock order: integration MUTATIONS, lifecycle, then database transactions.
use std::{
    collections::HashMap,
    sync::{Arc, LazyLock, Mutex, Weak},
};

use tokio::sync::{Mutex as AsyncMutex, OwnedMutexGuard};
use uuid::Uuid;

static LOCKS: LazyLock<Mutex<HashMap<Uuid, Weak<AsyncMutex<()>>>>> = LazyLock::new(Mutex::default);

pub struct Guard {
    pub workspace_id: Uuid,
    _guard: OwnedMutexGuard<()>,
}

pub async fn lock(workspace_id: Uuid) -> Guard {
    let mutex = {
        let mut locks = LOCKS.lock().unwrap();
        locks.retain(|_, lock| lock.strong_count() > 0);
        let mutex = locks
            .get(&workspace_id)
            .and_then(Weak::upgrade)
            .unwrap_or_else(|| Arc::new(AsyncMutex::new(())));
        locks.insert(workspace_id, Arc::downgrade(&mutex));
        mutex
    };
    Guard {
        workspace_id,
        _guard: mutex.lock_owned().await,
    }
}

/// Shared persisted writer predicate, evaluated under the caller's admission transaction.
pub async fn has_active_writers(
    conn: &mut sqlx::SqliteConnection,
    id: Uuid,
) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM agent_runs WHERE workspace_id=?1 AND status NOT IN ('succeeded','failed','cancelled','crashed','audit_failed'))
        OR EXISTS(SELECT 1 FROM agent_process_registry p JOIN agent_run_attempts a ON a.id=p.run_attempt_id JOIN agent_runs r ON r.id=a.agent_run_id WHERE r.workspace_id=?1 AND p.registry_status IN ('reserved','spawned','running','unreachable'))
        OR EXISTS(SELECT 1 FROM execution_processes p JOIN sessions s ON s.id=p.session_id WHERE s.workspace_id=?1 AND p.status='running')
        OR EXISTS(SELECT 1 FROM integration_reservations WHERE resource_kind='workspace' AND resource_key=?2)
        OR EXISTS(SELECT 1 FROM resource_operations WHERE workspace_id=?1 AND status IN ('queued','blocked','launching','running','recovery_required'))
        OR EXISTS(SELECT 1 FROM resource_mediations WHERE workspace_id=?1 AND status IN ('pending','preparing','running'))
        OR EXISTS(SELECT 1 FROM workflow_runs w WHERE w.status NOT IN ('succeeded','failed','canceled') AND (
            w.workspace_id=?1
            OR EXISTS(SELECT 1 FROM workflow_attempts a WHERE a.id=w.attempt_id AND a.workspace_id=?1)
            OR EXISTS(SELECT 1 FROM node_executions n JOIN agent_runs r ON r.id=n.agent_run_id WHERE n.run_id=w.id AND r.workspace_id=?1)))
        OR EXISTS(SELECT 1 FROM integration_runs WHERE workspace_id=?1 AND status NOT IN ('succeeded','failed','cancelled'))"
    ).bind(id).bind(db::models::integration::resource_key(id)).fetch_one(conn).await
}
