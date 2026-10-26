//! Two immutable SQL variants of one known migration, selected by the actual
//! applied checksum. SQLx still validates and records the SQL it really runs.
use std::borrow::Cow;

use futures::future::BoxFuture;
use sqlx::{
    Error, SqlitePool,
    error::BoxDynError,
    migrate::{MigrateError, Migration, MigrationSource, Migrator},
};

pub(crate) const PROJECT_KEYS_VERSION: i64 = 20261025120000;
const ORIGINAL_CHECKSUM: [u8; 48] = [
    227, 232, 252, 177, 49, 149, 85, 197, 31, 145, 108, 11, 142, 37, 138, 114, 60, 27, 75, 122,
    222, 2, 34, 162, 137, 180, 40, 149, 96, 51, 30, 147, 218, 197, 195, 200, 72, 170, 146, 231,
    154, 249, 189, 77, 160, 94, 100, 50,
];
const COMPAT_CHECKSUM: [u8; 48] = [
    216, 19, 16, 232, 46, 56, 55, 206, 176, 59, 89, 203, 221, 208, 18, 223, 101, 62, 196, 100, 180,
    106, 62, 82, 147, 2, 23, 2, 47, 243, 206, 126, 221, 34, 11, 41, 236, 151, 196, 94, 193, 172,
    221, 133, 203, 227, 100, 141,
];

#[derive(Debug)]
struct ResolvedMigrations(Vec<Migration>);

impl MigrationSource<'static> for ResolvedMigrations {
    fn resolve(self) -> BoxFuture<'static, Result<Vec<Migration>, BoxDynError>> {
        Box::pin(async move { Ok(self.0) })
    }
}

fn mismatch() -> Error {
    MigrateError::VersionMismatch(PROJECT_KEYS_VERSION).into()
}

pub(crate) async fn migrator(pool: &SqlitePool) -> Result<Migrator, Error> {
    let embedded = sqlx::migrate!("./migrations");
    let mut migrations: Vec<_> = embedded.iter().cloned().collect();
    let original = migrations
        .iter_mut()
        .find(|m| m.version == PROJECT_KEYS_VERSION)
        .ok_or_else(mismatch)?;
    // Freeze both variants: accidental edits are not silently new variants.
    if original.checksum.as_ref() != ORIGINAL_CHECKSUM {
        return Err(mismatch());
    }
    let compatible = Migration::new(
        original.version,
        original.description.clone(),
        original.migration_type,
        Cow::Borrowed(include_str!("../migration_compat/project_keys_multi.sql")),
        original.no_tx,
    );
    if compatible.checksum.as_ref() != COMPAT_CHECKSUM {
        return Err(mismatch());
    }
    let history_exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='_sqlx_migrations')",
    )
    .fetch_one(pool)
    .await?;
    let applied: Option<Vec<u8>> = if history_exists {
        sqlx::query_scalar("SELECT checksum FROM _sqlx_migrations WHERE version=?")
            .bind(PROJECT_KEYS_VERSION)
            .fetch_optional(pool)
            .await?
    } else {
        None
    };
    match applied.as_deref() {
        Some(checksum) if checksum == ORIGINAL_CHECKSUM => {}
        None => *original = compatible,
        Some(checksum) if checksum == COMPAT_CHECKSUM => *original = compatible,
        // Fail here even on Windows release, before the legacy generic
        // checksum-rewrite fallback. Never change an applied history receipt.
        Some(_) => return Err(mismatch()),
    }
    Ok(Migrator::new(ResolvedMigrations(migrations)).await?)
}

#[cfg(test)]
mod tests {
    use sqlx::sqlite::SqlitePoolOptions;
    use uuid::Uuid;

    use super::*;

    async fn old_pool(count: u128) -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        let embedded = sqlx::migrate!("./migrations");
        Migrator::new(ResolvedMigrations(
            embedded
                .iter()
                .filter(|m| m.version < 20261025000000)
                .cloned()
                .collect(),
        ))
        .await
        .unwrap()
        .run(&pool)
        .await
        .unwrap();
        for id in (1..=count).rev() {
            sqlx::query("INSERT INTO projects(id,name) VALUES(?,'Legacy')")
                .bind(Uuid::from_u128(id))
                .execute(&pool)
                .await
                .unwrap();
        }
        // Match a legacy upgrade that committed classification before the
        // original Project key migration failed.
        Migrator::new(ResolvedMigrations(
            embedded
                .iter()
                .filter(|m| m.version < PROJECT_KEYS_VERSION)
                .cloned()
                .collect(),
        ))
        .await
        .unwrap()
        .run(&pool)
        .await
        .unwrap();
        pool
    }

    #[tokio::test]
    async fn fresh_database_uses_actual_compatible_checksum() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        super::super::run_migrations(&pool).await.unwrap();
        super::super::run_migrations(&pool).await.unwrap();
        assert_eq!(
            sqlx::query_scalar::<_, Vec<u8>>(
                "SELECT checksum FROM _sqlx_migrations WHERE version=?"
            )
            .bind(PROJECT_KEYS_VERSION)
            .fetch_one(&pool)
            .await
            .unwrap(),
            COMPAT_CHECKSUM
        );
    }

    #[tokio::test]
    async fn original_receipt_and_key_survive_restart_without_checksum_changes() {
        for count in [0, 1] {
            let pool = old_pool(count).await;
            sqlx::migrate!("./migrations").run(&pool).await.unwrap();
            let before: Vec<u8> =
                sqlx::query_scalar("SELECT checksum FROM _sqlx_migrations WHERE version=?")
                    .bind(PROJECT_KEYS_VERSION)
                    .fetch_one(&pool)
                    .await
                    .unwrap();
            super::super::run_migrations(&pool).await.unwrap();
            let after: Vec<u8> =
                sqlx::query_scalar("SELECT checksum FROM _sqlx_migrations WHERE version=?")
                    .bind(PROJECT_KEYS_VERSION)
                    .fetch_one(&pool)
                    .await
                    .unwrap();
            assert_eq!(before, ORIGINAL_CHECKSUM);
            assert_eq!(before, after);
            let keys: Vec<String> = sqlx::query_scalar("SELECT key FROM projects")
                .fetch_all(&pool)
                .await
                .unwrap();
            assert_eq!(keys, vec!["LOCAL".to_owned(); count as usize]);
        }
    }

    #[tokio::test]
    async fn failed_multi_project_upgrade_and_fresh_database_resume_idempotently() {
        for count in [0, 1, 2, 3] {
            let pool = old_pool(count).await;
            if count > 1 {
                assert!(sqlx::migrate!("./migrations").run(&pool).await.is_err());
                assert_eq!(
                    sqlx::query_scalar::<_, i64>(
                        "SELECT count(*) FROM pragma_table_info('projects') WHERE name='key'"
                    )
                    .fetch_one(&pool)
                    .await
                    .unwrap(),
                    0
                );
            }
            super::super::run_migrations(&pool).await.unwrap();
            super::super::run_migrations(&pool).await.unwrap();
            assert_eq!(
                sqlx::query_scalar::<_, i64>("SELECT count(*) FROM local_tags")
                    .fetch_one(&pool)
                    .await
                    .unwrap(),
                (count * 4) as i64
            );
            let keys: Vec<String> = sqlx::query_scalar("SELECT key FROM projects ORDER BY hex(id)")
                .fetch_all(&pool)
                .await
                .unwrap();
            let expected: Vec<_> = (1..=count)
                .map(|n| {
                    if n == 1 {
                        "LOCAL".into()
                    } else {
                        format!("LOCAL{n}")
                    }
                })
                .collect();
            assert_eq!(keys, expected);
            assert_eq!(
                sqlx::query_scalar::<_, Vec<u8>>(
                    "SELECT checksum FROM _sqlx_migrations WHERE version=?"
                )
                .bind(PROJECT_KEYS_VERSION)
                .fetch_one(&pool)
                .await
                .unwrap(),
                COMPAT_CHECKSUM
            );
        }
    }

    #[tokio::test]
    async fn unknown_project_key_checksum_is_rejected_without_history_update() {
        let pool = old_pool(1).await;
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        sqlx::query("UPDATE _sqlx_migrations SET checksum=x'0102' WHERE version=?")
            .bind(PROJECT_KEYS_VERSION)
            .execute(&pool)
            .await
            .unwrap();
        assert!(matches!(migrator(&pool).await, Err(Error::Migrate(_))));
        assert_eq!(
            sqlx::query_scalar::<_, Vec<u8>>(
                "SELECT checksum FROM _sqlx_migrations WHERE version=?"
            )
            .bind(PROJECT_KEYS_VERSION)
            .fetch_one(&pool)
            .await
            .unwrap(),
            vec![1, 2]
        );
    }
}
