use std::{fs, io, path::Path, sync::Arc, time::Duration};

use deployment::{Deployment, DeploymentError};
use services::services::{
    agent_runtime::AgentRunCommandService, container::ContainerService,
    orchestration::OrchestrationService,
};
use tokio_util::sync::CancellationToken;
use utils::assets::asset_dir;

use crate::{
    DeploymentImpl,
    routes::{
        self,
        scheduled_tasks::spawn_scheduled_task_loop,
        sessions::setup_gate::{reconcile_setup_launch_gates, spawn_setup_launch_gate_watcher},
    },
    workflow_runtime::runner::{
        DeploymentAgentRunReconciliationBoundary, recover_stale_workflow_runs_with_boundary,
        spawn_workflow_completion_watcher,
    },
};

/// A running server instance. Callers can read the port, then call `serve()`
/// to run the server until the shutdown token is cancelled.
pub struct ServerHandle {
    pub port: u16,
    pub deployment: DeploymentImpl,
    shutdown_token: CancellationToken,
    main_listener: tokio::net::TcpListener,
}

impl ServerHandle {
    /// The base URL the main server is listening on.
    ///
    /// Uses `localhost` rather than `127.0.0.1` so that macOS ATS
    /// (App Transport Security) exception domains apply correctly in
    /// the Tauri desktop app — IP address literals aren't reliably
    /// matched by ATS, which causes WebSocket connections to fail.
    pub fn url(&self) -> String {
        format!("http://localhost:{}", self.port)
    }

    /// Run the main server until the shutdown token is cancelled.
    pub async fn serve(self) -> anyhow::Result<()> {
        // Publish the bound addresses for local client connections.
        self.deployment
            .client_info()
            .set_server_addr(self.main_listener.local_addr()?)
            .expect("client server address already set");
        let access =
            server_access::Access::from_env(self.deployment.db().pool.clone(), asset_dir())?;
        let app_router = routes::router(self.deployment.clone(), access.clone());
        let main_shutdown = self.shutdown_token.clone();

        let main_server = axum::serve(self.main_listener, app_router)
            .with_graceful_shutdown(async move { main_shutdown.cancelled().await });

        let mut main_handle = tokio::spawn(async move {
            if let Err(e) = main_server.await {
                tracing::error!("Main server error: {}", e);
            }
        });

        let result = tokio::select! {
            _ = self.shutdown_token.cancelled() => Ok(()),
            result = &mut main_handle => result,
        };
        self.shutdown_token.cancel();
        finish_http_shutdown(&mut main_handle).await;

        perform_cleanup_actions(&self.deployment).await;
        result?;
        Ok(())
    }

    /// Return a clone of the shutdown token. Cancel it to stop `serve()`.
    pub fn shutdown_token(&self) -> CancellationToken {
        self.shutdown_token.clone()
    }
}

/// Call after cancellation: long-lived SSE/WebSocket connections must not keep
/// execution process cleanup waiting indefinitely. This bounds HTTP draining,
/// not the separate process cleanup. Runtime teardown closes remaining connections.
pub async fn finish_http_shutdown(handle: &mut tokio::task::JoinHandle<()>) {
    if !handle.is_finished()
        && tokio::time::timeout(Duration::from_secs(5), &mut *handle)
            .await
            .is_err()
    {
        tracing::warn!(
            "HTTP shutdown grace period elapsed; continuing with execution process cleanup"
        );
        handle.abort();
        let _ = handle.await;
    }
}

/// Initialize the deployment, bind listeners on `localhost` with OS-assigned
/// ports, and return a handle that is ready to serve.
///
/// Uses `localhost` rather than `127.0.0.1` so the bind address matches
/// the hostname the frontend connects to. On modern macOS, `localhost`
/// resolves to `::1` (IPv6) first — binding to `127.0.0.1` (IPv4) while
/// the browser connects via `::1` causes "connection refused".
pub async fn start() -> anyhow::Result<ServerHandle> {
    start_with_bind("localhost:0", CancellationToken::new()).await
}

/// Like [`start`], but lets the caller specify the main bind address.
pub async fn start_with_bind(
    main_addr: &str,
    shutdown_token: CancellationToken,
) -> anyhow::Result<ServerHandle> {
    let deployment = initialize_deployment(shutdown_token.clone()).await?;

    let listener = tokio::net::TcpListener::bind(main_addr).await?;
    let port = listener.local_addr()?.port();

    warn_retired_preview_settings();
    tracing::info!("Server on :{port}");

    Ok(ServerHandle {
        port,
        deployment,
        shutdown_token,
        main_listener: listener,
    })
}

/// Retired settings are inert; old installations must still start safely.
pub fn warn_retired_preview_settings() {
    for key in [
        "PREVIEW_PROXY_PORT",
        "VK_PREVIEW_DOMAIN",
        "LVK_PREVIEW_DOMAIN",
        "VK_PREVIEW_DISABLE_NAV_SCRIPT",
    ] {
        if std::env::var_os(key).is_some_and(|value| !value.is_empty()) {
            tracing::warn!(
                "{} is retired and ignored; use an external browser or SSH tunnel. LVK_PREVIEW_ROUTES remains supported",
                key
            );
        }
    }
}

/// Initialize the deployment: create asset directory, run migrations, backfill data,
/// and pre-warm caches. Shared between the standalone server and the Tauri app.
pub async fn initialize_deployment(
    shutdown: CancellationToken,
) -> Result<DeploymentImpl, DeploymentError> {
    // Create asset directory if it doesn't exist
    if !asset_dir().exists() {
        std::fs::create_dir_all(asset_dir()).map_err(|e| {
            DeploymentError::Other(anyhow::anyhow!("Failed to create asset directory: {}", e))
        })?;
    }

    // Copy old database to new location for safe downgrades
    let old_db = asset_dir().join("db.sqlite");
    let new_db = asset_dir().join("db.v2.sqlite");
    if !new_db.exists() && old_db.exists() {
        tracing::info!(
            "Copying database to new location: {:?} -> {:?}",
            old_db,
            new_db
        );
        std::fs::copy(&old_db, &new_db).expect("Failed to copy database file");
        tracing::info!("Database copy complete");
    }

    let deployment = DeploymentImpl::new(shutdown).await?;
    db::models::resource_coordination::ResourceMediation::fence_interrupted_preparations(
        &deployment.db().pool,
    )
    .await
    .map_err(DeploymentError::Other)?;
    migrate_legacy_attachment_directories().await?;
    deployment.update_sentry_scope().await?;
    crate::workflow_runtime::bootstrap::fence_interrupted_runs(&deployment).await?;

    // Runtime recovery is ordered from the real process owner outward.
    // Deployment construction has already re-attached durable provider hosts,
    // so replay direct control commands next and only then advance
    // orchestration. The old execution-process orphan cleanup is intentionally
    // excluded: it cannot distinguish a live managed AgentRun from a stale
    // legacy row and would violate restart reconciliation semantics.
    let command_reconciliation =
        AgentRunCommandService::new(&deployment.db().pool, deployment.agent_run_port())
            .reconcile_startup()
            .await
            .map_err(|err| DeploymentError::Other(anyhow::anyhow!(err.to_string())))?;
    tracing::info!(
        recovered_inflight = command_reconciliation.recovered_inflight,
        delivered = command_reconciliation.delivered,
        failed = command_reconciliation.failed,
        "Reconciled durable direct AgentRun commands"
    );
    spawn_setup_launch_gate_watcher(deployment.clone());
    let setup_gate_count = reconcile_setup_launch_gates(&deployment)
        .await
        .map_err(|err| DeploymentError::Other(anyhow::anyhow!(err.to_string())))?;
    tracing::info!(setup_gate_count, "Reconciled durable setup launch gates");
    let orchestration = OrchestrationService::new(
        deployment.db().pool.clone(),
        Arc::new(deployment.agent_run_port().clone()),
    );
    let reconciliation = orchestration
        .reconcile_startup()
        .await
        .map_err(|err| DeploymentError::Other(anyhow::anyhow!(err.to_string())))?;
    tracing::info!(
        recovered_outbox_deliveries = reconciliation.recovered_outbox_deliveries,
        recovered_inbox_claims = reconciliation.recovered_inbox_claims,
        recovered_leases = reconciliation.recovered_leases,
        reconciled_runs = reconciliation.reconciled_runs,
        unreachable_agent_runs = reconciliation.unreachable_agent_runs,
        delivery_failures = reconciliation.delivery_failures,
        "Reconciled durable AgentRun orchestration state"
    );
    let workflow_boundary = DeploymentAgentRunReconciliationBoundary::new(deployment.clone());
    recover_stale_workflow_runs_with_boundary(&deployment.db().pool, &workflow_boundary)
        .await
        .map_err(|err| DeploymentError::Other(anyhow::anyhow!(err.to_string())))?;
    crate::routes::openwiki::spawn_recovery_monitor(deployment.clone());
    crate::routes::resource_coordination::spawn_monitor(deployment.clone());
    // Start the event-driven workflow completion watcher. This eliminates the
    // need for HTTP polling to advance workflow runs when agent executions
    // complete — instead each execution's exit-monitor publishes an event that
    // this task picks up and uses to trigger `reconcile_workflow_run_with_arena`
    // immediately.
    spawn_workflow_completion_watcher(deployment.clone());
    spawn_scheduled_task_loop(deployment.clone());
    deployment
        .container()
        .backfill_repo_names()
        .await
        .map_err(DeploymentError::from)?;
    deployment
        .track_if_analytics_allowed("session_start", serde_json::json!({}))
        .await;

    // Preload global executor options cache for all executors with DEFAULT presets
    tokio::spawn(async move {
        executors::executors::utils::preload_global_executor_options_cache().await;
    });

    Ok(deployment)
}

/// Gracefully shut down running execution processes.
pub async fn perform_cleanup_actions(deployment: &DeploymentImpl) {
    deployment
        .container()
        .kill_all_running_processes()
        .await
        .expect("Failed to cleanly kill running execution processes");
}

const LEGACY_ATTACHMENT_MIGRATION_MARKER: &str = ".attachment-directories-migrated-v1";

#[derive(Default)]
struct DirectoryMigrationStats {
    moved_files: u64,
    removed_duplicates: u64,
    created_directories: u64,
    failures: u64,
}

impl DirectoryMigrationStats {
    fn merge(&mut self, other: DirectoryMigrationStats) {
        self.moved_files += other.moved_files;
        self.removed_duplicates += other.removed_duplicates;
        self.created_directories += other.created_directories;
        self.failures += other.failures;
    }
}

async fn migrate_legacy_attachment_directories() -> Result<(), DeploymentError> {
    let marker_path = asset_dir().join(LEGACY_ATTACHMENT_MIGRATION_MARKER);
    if marker_path.exists() {
        return Ok(());
    }

    let mut stats = DirectoryMigrationStats::default();

    let cache_root = utils::cache_dir();
    stats.merge(migrate_legacy_directory(
        &cache_root.join("images"),
        &cache_root.join("attachments"),
    ));

    // Repository attachment paths are not migrated. Old provider history and
    // checkout files remain untouched when switching the canonical LVK namespace.

    if stats.failures == 0 {
        fs::write(&marker_path, b"ok")?;
        tracing::info!(
            "Legacy attachment directory migration completed: moved {}, removed duplicates {}, created directories {}",
            stats.moved_files,
            stats.removed_duplicates,
            stats.created_directories
        );
    } else {
        tracing::warn!(
            "Legacy attachment directory migration completed with {} failures; will retry on next startup",
            stats.failures
        );
    }

    Ok(())
}

fn migrate_legacy_directory(src_dir: &Path, dst_dir: &Path) -> DirectoryMigrationStats {
    let mut stats = DirectoryMigrationStats::default();

    if !src_dir.exists() {
        return stats;
    }

    if let Err(error) = fs::create_dir_all(dst_dir) {
        tracing::warn!(
            "Failed to create attachment directory {}: {}",
            dst_dir.display(),
            error
        );
        stats.failures += 1;
        return stats;
    }
    stats.created_directories += 1;

    if let Err(error) = migrate_directory_contents(src_dir, dst_dir, &mut stats) {
        tracing::warn!(
            "Failed to migrate legacy attachment directory {} -> {}: {}",
            src_dir.display(),
            dst_dir.display(),
            error
        );
        stats.failures += 1;
    }

    if let Err(error) = remove_empty_dir_tree(src_dir) {
        tracing::warn!(
            "Failed to clean up legacy attachment directory {}: {}",
            src_dir.display(),
            error
        );
        stats.failures += 1;
    }

    stats
}

fn migrate_directory_contents(
    src_dir: &Path,
    dst_dir: &Path,
    stats: &mut DirectoryMigrationStats,
) -> io::Result<()> {
    for entry in fs::read_dir(src_dir)? {
        let entry = entry?;
        let src_path = entry.path();
        let file_name = entry.file_name();

        let dst_path = dst_dir.join(&file_name);
        let file_type = entry.file_type()?;

        if file_type.is_dir() {
            fs::create_dir_all(&dst_path)?;
            migrate_directory_contents(&src_path, &dst_path, stats)?;
            remove_empty_dir_tree(&src_path)?;
            continue;
        }

        if dst_path.exists() {
            fs::remove_file(&src_path)?;
            stats.removed_duplicates += 1;
            continue;
        }

        move_path(&src_path, &dst_path)?;
        stats.moved_files += 1;
    }

    Ok(())
}

fn move_path(src_path: &Path, dst_path: &Path) -> io::Result<()> {
    match fs::rename(src_path, dst_path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::CrossesDevices => {
            fs::copy(src_path, dst_path)?;
            fs::remove_file(src_path)
        }
        Err(error) => Err(error),
    }
}

fn remove_empty_dir_tree(path: &Path) -> io::Result<()> {
    if !path.exists() {
        return Ok(());
    }

    if fs::read_dir(path)?.next().is_none() {
        fs::remove_dir(path)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;

    #[test]
    fn migrates_legacy_cache_directory_contents() {
        let temp_dir = TempDir::new().unwrap();
        let src = temp_dir.path().join("images");
        let dst = temp_dir.path().join("attachments");
        fs::create_dir_all(&src).unwrap();
        fs::write(src.join("asset.png"), b"hello").unwrap();

        let stats = migrate_legacy_directory(&src, &dst);

        assert_eq!(stats.moved_files, 1);
        assert!(dst.join("asset.png").exists());
        assert!(!src.exists());
    }

    #[test]
    fn removes_legacy_duplicates_when_destination_exists() {
        let temp_dir = TempDir::new().unwrap();
        let src = temp_dir.path().join("images");
        let dst = temp_dir.path().join("attachments");
        fs::create_dir_all(&src).unwrap();
        fs::create_dir_all(&dst).unwrap();
        fs::write(src.join("asset.png"), b"old").unwrap();
        fs::write(dst.join("asset.png"), b"new").unwrap();

        let stats = migrate_legacy_directory(&src, &dst);

        assert_eq!(stats.removed_duplicates, 1);
        assert_eq!(fs::read(dst.join("asset.png")).unwrap(), b"new");
        assert!(!src.exists());
    }
}
