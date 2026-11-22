//! One-time removal of files owned by the retired resource coordinator.
//!
//! Only the configured instance database is associated with runtime files.
//! Database copies and in-memory migrations must not touch instance files.
use std::path::{Component, Path, PathBuf};

use sqlx::{
    Error, SqlitePool,
    migrate::{MigrateError, Migrator},
};
use uuid::Uuid;

const RETIRE_VERSION: i64 = 20261122000000;

pub(crate) async fn cleanup(pool: &SqlitePool, migrator: &Migrator) -> Result<(), Error> {
    let file: String =
        sqlx::query_scalar("SELECT file FROM pragma_database_list WHERE name='main'")
            .fetch_one(pool)
            .await?;
    let database = Path::new(&file);
    if database
        .file_name()
        .is_none_or(|name| name != "db.v2.sqlite")
    {
        return Ok(());
    }
    let Some(root) = database.parent() else {
        return Ok(());
    };
    let root = tokio::fs::canonicalize(root).await?;
    let configured = utils::assets::asset_dir_path()?;
    let configured = match tokio::fs::canonicalize(configured).await {
        Ok(path) => path,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.into()),
    };
    if root != configured {
        return Ok(());
    }
    let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='resource_mediations')")
        .fetch_one(pool).await?;
    if !exists {
        return Ok(());
    }

    // Never delete files from a database whose migration history is inconsistent.
    // SQLx still performs its normal validation and applies the actual migration.
    let rows: Vec<(i64, Vec<u8>, bool)> =
        sqlx::query_as("SELECT version,checksum,success FROM _sqlx_migrations")
            .fetch_all(pool)
            .await?;
    for (version, checksum, success) in rows {
        if !success {
            return Err(MigrateError::Dirty(version).into());
        }
        let migration = migrator
            .iter()
            .find(|item| item.version == version)
            .ok_or(MigrateError::VersionMissing(version))?;
        if migration.checksum.as_ref() != checksum.as_slice() {
            return Err(MigrateError::VersionMismatch(version).into());
        }
        if version >= RETIRE_VERSION {
            return Err(Error::Protocol(
                "Retired resource tables remain after migration".into(),
            ));
        }
    }

    let mediations: Vec<Uuid> = sqlx::query_scalar("SELECT id FROM resource_mediations")
        .fetch_all(pool)
        .await?;
    let sessions: Vec<Uuid> = sqlx::query_scalar("SELECT id FROM sessions WHERE workspace_id IN (SELECT id FROM workspaces WHERE json_extract(execution_owner,'$.kind')='resource_mediation') UNION SELECT m.session_id FROM resource_mediations m WHERE m.session_id IS NOT NULL AND NOT EXISTS(SELECT 1 FROM sessions s WHERE s.id=m.session_id)")
        .fetch_all(pool).await?;
    let processes: Vec<(Uuid, Uuid)> = sqlx::query_as(
        "SELECT session_id,id FROM execution_processes WHERE run_reason='resourcecommand' UNION SELECT o.session_id,o.process_id FROM resource_operations o WHERE o.process_id IS NOT NULL AND NOT EXISTS(SELECT 1 FROM execution_processes p WHERE p.id=o.process_id) UNION SELECT o.session_id,o.id FROM resource_operations o WHERE o.process_id IS NULL AND json_extract(o.spec,'$.runner') IS NULL AND NOT EXISTS(SELECT 1 FROM execution_processes p WHERE p.id=o.id)",
    )
    .fetch_all(pool)
    .await?;
    for id in mediations {
        remove(
            &root,
            &PathBuf::from("resource-mediation").join(id.to_string()),
            true,
        )
        .await?;
    }
    for id in sessions {
        for path in [
            utils::execution_logs::process_logs_session_dir_in_root(&root, id),
            utils::native_audit::session_dir_in_root(&root, id),
        ] {
            let relative = path
                .strip_prefix(root.as_path())
                .map_err(|e| Error::Protocol(e.to_string()))?;
            remove(&root, relative, true).await?;
        }
    }
    for (session, process) in processes {
        let path = utils::execution_logs::process_log_file_path_in_root(&root, session, process);
        let relative = path
            .strip_prefix(root.as_path())
            .map_err(|e| Error::Protocol(e.to_string()))?;
        remove(&root, relative, false).await?;
    }
    Ok(())
}

async fn remove(root: &Path, relative: &Path, directory: bool) -> Result<(), Error> {
    let mut path = root.to_path_buf();
    for component in relative.components() {
        let Component::Normal(name) = component else {
            return Err(Error::Protocol("Invalid retired resource path".into()));
        };
        path.push(name);
        match tokio::fs::symlink_metadata(&path).await {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(Error::Protocol("Symlink in retired resource path".into()));
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(error.into()),
        }
    }
    let metadata = tokio::fs::symlink_metadata(&path).await?;
    if directory && metadata.is_dir() {
        tokio::fs::remove_dir_all(path).await?;
    } else if !directory && metadata.is_file() {
        tokio::fs::remove_file(path).await?;
    } else {
        return Err(Error::Protocol(
            "Unexpected retired resource file type".into(),
        ));
    }
    Ok(())
}

#[cfg(all(test, unix))]
mod tests {
    use std::os::unix::fs::symlink;

    use super::*;

    #[tokio::test]
    async fn database_copy_does_not_touch_adjacent_files() {
        let root = tempfile::tempdir().unwrap();
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                sqlx::sqlite::SqliteConnectOptions::new()
                    .filename(root.path().join("db.v2.sqlite"))
                    .create_if_missing(true),
            )
            .await
            .unwrap();
        sqlx::query("CREATE TABLE resource_mediations(id BLOB)")
            .execute(&pool)
            .await
            .unwrap();
        let marker = root.path().join("resource-mediation");
        std::fs::write(&marker, "user data").unwrap();
        let migrator = sqlx::migrate!("./migrations");
        cleanup(&pool, &migrator).await.unwrap();
        assert_eq!(std::fs::read_to_string(marker).unwrap(), "user data");
    }

    #[tokio::test]
    async fn cleanup_refuses_symlinks_without_touching_target() {
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        std::fs::write(outside.path().join("keep"), "user data").unwrap();
        symlink(outside.path(), root.path().join("resource-mediation")).unwrap();
        assert!(
            remove(root.path(), Path::new("resource-mediation/keep"), false)
                .await
                .is_err()
        );
        assert_eq!(
            std::fs::read_to_string(outside.path().join("keep")).unwrap(),
            "user data"
        );
    }
}
