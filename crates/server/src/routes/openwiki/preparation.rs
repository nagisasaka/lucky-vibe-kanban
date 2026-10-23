//! In-flight host preparation has no AgentRun yet (e.g. waiting for the shared
//! installer). Track only the actual future lifetime, never the usage label.
use std::{
    collections::HashSet,
    sync::{LazyLock, Mutex},
};

use db::models::workspace::Workspace;
use sqlx::SqlitePool;
use uuid::Uuid;

use crate::error::ApiError;

static ACTIVE: LazyLock<Mutex<HashSet<Uuid>>> = LazyLock::new(Mutex::default);

pub(crate) struct Guard(Uuid);

impl Drop for Guard {
    fn drop(&mut self) {
        ACTIVE.lock().unwrap().remove(&self.0);
    }
}

pub(crate) fn is_active(id: Uuid) -> bool {
    ACTIVE.lock().unwrap().contains(&id)
}

pub(crate) async fn begin(pool: &SqlitePool, id: Uuid) -> Result<Guard, ApiError> {
    // Admission and archive share this barrier. If archive won before we could
    // reserve, do not regenerate or start using its deleted worktree.
    let _lifecycle = services::services::workspace_lifecycle::lock(id).await;
    let workspace = Workspace::find_by_id(pool, id)
        .await?
        .ok_or_else(|| ApiError::BadRequest("Workspace missing".into()))?;
    if workspace.archived || !ACTIVE.lock().unwrap().insert(id) {
        return Err(ApiError::Conflict(
            "Workspace cannot start maintenance preparation".into(),
        ));
    }
    Ok(Guard(id))
}
