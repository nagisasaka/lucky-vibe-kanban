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
