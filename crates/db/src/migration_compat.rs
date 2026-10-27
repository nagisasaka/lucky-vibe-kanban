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

const ORIGINAL_CRLF_CHECKSUM: [u8; 48] = [
    41, 45, 58, 142, 185, 97, 153, 87, 37, 85, 30, 130, 39, 242, 71, 52, 185, 94, 32, 9, 25, 232,
    178, 241, 251, 197, 132, 46, 151, 67, 254, 237, 112, 116, 157, 95, 148, 94, 29, 194, 143, 251,
    223, 85, 26, 44, 194, 71,
];

const COMPAT_CRLF_CHECKSUM: [u8; 48] = [
    94, 209, 180, 150, 19, 93, 185, 115, 219, 118, 49, 39, 201, 234, 252, 72, 105, 28, 85, 196,
    134, 46, 25, 105, 8, 22, 169, 170, 146, 187, 117, 204, 181, 84, 55, 199, 88, 230, 40, 222, 29,
    206, 41, 248, 116, 58, 51, 154,
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

fn with_sql(template: &Migration, sql: String) -> Migration {
    Migration::new(
        template.version,
        template.description.clone(),
        template.migration_type,
        Cow::Owned(sql),
        template.no_tx,
    )
}

fn select_project_keys(
    original: &Migration,
    compatible_sql: &str,
    applied: Option<&[u8]>,
) -> Result<Migration, Error> {
    // Only these exact LF/CRLF artifacts are supported. Do not normalize
    // arbitrary modified SQL into an accepted migration.
    if original.checksum.as_ref() != ORIGINAL_CHECKSUM
        && original.checksum.as_ref() != ORIGINAL_CRLF_CHECKSUM
    {
        return Err(mismatch());
    }
    let compatible = with_sql(original, compatible_sql.to_owned());
    if compatible.checksum.as_ref() != COMPAT_CHECKSUM
        && compatible.checksum.as_ref() != COMPAT_CRLF_CHECKSUM
    {
        return Err(mismatch());
    }
    // New receipts describe the actual checkout's compatible SQL. Existing
    // receipts instead select the exact historical bytes, across checkouts.
    let wanted = applied.unwrap_or(compatible.checksum.as_ref());
    for template in [original, &compatible] {
        let lf = template.sql.replace("\r\n", "\n");
        for sql in [lf.clone(), lf.replace('\n', "\r\n")] {
            let variant = with_sql(template, sql);
            if variant.checksum.as_ref() == wanted {
                return Ok(variant);
            }
        }
    }
    Err(mismatch())
}

pub(crate) async fn migrator(pool: &SqlitePool) -> Result<Migrator, Error> {
    let embedded = sqlx::migrate!("./migrations");
    let mut migrations: Vec<_> = embedded.iter().cloned().collect();
    let original = migrations
        .iter_mut()
        .find(|m| m.version == PROJECT_KEYS_VERSION)
        .ok_or_else(mismatch)?;
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
    *original = select_project_keys(
        original,
        include_str!("../migration_compat/project_keys_multi.sql"),
        applied.as_deref(),
    )?;
    Ok(Migrator::new(ResolvedMigrations(migrations)).await?)
}

#[cfg(test)]
mod tests {
    use sqlx::sqlite::SqlitePoolOptions;
    use uuid::Uuid;

    use super::*;

    #[tokio::test]
    async fn reservation_terminal_migration_supports_fresh_and_previous_main_upgrade() {
        for upgrade in [false, true] {
            let pool = SqlitePoolOptions::new()
                .max_connections(1)
                .connect("sqlite::memory:")
                .await
                .unwrap();
            if upgrade {
                let embedded = sqlx::migrate!("./migrations");
                Migrator::new(ResolvedMigrations(
                    embedded
                        .iter()
                        .filter(|m| m.version < 20261027010000)
                        .cloned()
                        .collect(),
                ))
                .await
                .unwrap()
                .run(&pool)
                .await
                .unwrap();
            }
            super::super::run_migrations(&pool).await.unwrap();
            super::super::run_migrations(&pool).await.unwrap();
            let triggers: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='trigger' AND name='release_unstarted_agent_reservation'")
                .fetch_one(&pool).await.unwrap();
            assert_eq!(triggers, 1);
            let indexes: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='index' AND tbl_name='agent_process_registry' AND name LIKE 'idx_agent_process_registry_%'")
                .fetch_one(&pool).await.unwrap();
            assert_eq!(indexes, 3);
            assert!(
                sqlx::query("PRAGMA foreign_key_check")
                    .fetch_all(&pool)
                    .await
                    .unwrap()
                    .is_empty()
            );
        }
    }

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

    async fn apply_target_sql(pool: &SqlitePool, sql: String) {
        let embedded = sqlx::migrate!("./migrations");
        let migrations = embedded
            .iter()
            .map(|m| {
                if m.version == PROJECT_KEYS_VERSION {
                    with_sql(m, sql.clone())
                } else {
                    m.clone()
                }
            })
            .collect();
        Migrator::new(ResolvedMigrations(migrations))
            .await
            .unwrap()
            .run(pool)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn original_receipt_and_key_survive_restart_without_checksum_changes() {
        for count in [0, 1] {
            for crlf in [false, true] {
                let pool = old_pool(count).await;
                let lf = include_str!("../migrations/20261025120000_project_keys.sql")
                    .replace("\r\n", "\n");
                apply_target_sql(&pool, if crlf { lf.replace('\n', "\r\n") } else { lf }).await;
                let before: Vec<(i64, Vec<u8>)> = sqlx::query_as(
                    "SELECT version,checksum FROM _sqlx_migrations ORDER BY version",
                )
                .fetch_all(&pool)
                .await
                .unwrap();
                super::super::run_migrations(&pool).await.unwrap();
                let after: Vec<(i64, Vec<u8>)> = sqlx::query_as(
                    "SELECT version,checksum FROM _sqlx_migrations ORDER BY version",
                )
                .fetch_all(&pool)
                .await
                .unwrap();
                assert_eq!(before, after);
                let checksum = &after
                    .iter()
                    .find(|(version, _)| *version == PROJECT_KEYS_VERSION)
                    .unwrap()
                    .1;
                assert_eq!(
                    checksum.as_slice(),
                    if crlf {
                        &ORIGINAL_CRLF_CHECKSUM
                    } else {
                        &ORIGINAL_CHECKSUM
                    }
                );
                let keys: Vec<String> = sqlx::query_scalar("SELECT key FROM projects")
                    .fetch_all(&pool)
                    .await
                    .unwrap();
                assert_eq!(keys, vec!["LOCAL".to_owned(); count as usize]);
            }
        }
    }

    #[tokio::test]
    async fn compatible_crlf_receipt_is_validated_without_rewriting_history() {
        let pool = old_pool(3).await;
        let crlf = include_str!("../migration_compat/project_keys_multi.sql")
            .replace("\r\n", "\n")
            .replace('\n', "\r\n");
        apply_target_sql(&pool, crlf).await;
        let before: Vec<(i64, Vec<u8>)> =
            sqlx::query_as("SELECT version,checksum FROM _sqlx_migrations ORDER BY version")
                .fetch_all(&pool)
                .await
                .unwrap();
        super::super::run_migrations(&pool).await.unwrap();
        super::super::run_migrations(&pool).await.unwrap();
        let after: Vec<(i64, Vec<u8>)> =
            sqlx::query_as("SELECT version,checksum FROM _sqlx_migrations ORDER BY version")
                .fetch_all(&pool)
                .await
                .unwrap();
        assert_eq!(before, after);
        assert_eq!(
            after
                .iter()
                .find(|(version, _)| *version == PROJECT_KEYS_VERSION)
                .unwrap()
                .1,
            COMPAT_CRLF_CHECKSUM
        );
    }

    #[test]
    fn known_line_endings_select_exact_historical_bytes_across_artifacts() {
        let embedded = sqlx::migrate!("./migrations");
        let template = embedded
            .iter()
            .find(|m| m.version == PROJECT_KEYS_VERSION)
            .unwrap();
        let old_lf = template.sql.replace("\r\n", "\n");
        let new_lf =
            include_str!("../migration_compat/project_keys_multi.sql").replace("\r\n", "\n");
        let originals = [old_lf.clone(), old_lf.replace('\n', "\r\n")];
        let compatibles = [new_lf.clone(), new_lf.replace('\n', "\r\n")];
        for old in &originals {
            for new in &compatibles {
                let original = with_sql(template, old.clone());
                assert_eq!(
                    select_project_keys(&original, new, None)
                        .unwrap()
                        .sql
                        .as_ref(),
                    new
                );
                for historical in originals.iter().chain(compatibles.iter()) {
                    let applied = with_sql(template, historical.clone());
                    let selected =
                        select_project_keys(&original, new, Some(applied.checksum.as_ref()))
                            .unwrap();
                    assert_eq!(selected.checksum, applied.checksum);
                    assert_eq!(selected.sql.as_ref(), historical.as_str());
                }
                assert!(select_project_keys(&original, new, Some(&[1, 2])).is_err());
            }
        }
    }

    #[test]
    fn modified_artifacts_are_rejected_instead_of_normalized_into_known_sql() {
        let embedded = sqlx::migrate!("./migrations");
        let original = embedded
            .iter()
            .find(|m| m.version == PROJECT_KEYS_VERSION)
            .unwrap();
        let new = include_str!("../migration_compat/project_keys_multi.sql");
        let altered = with_sql(original, format!("{}-- unrecognised edit", original.sql));
        assert!(select_project_keys(&altered, new, None).is_err());
        assert!(
            select_project_keys(original, &format!("{new}-- unrecognised edit"), None).is_err()
        );
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
