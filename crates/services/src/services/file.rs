use std::{
    fs,
    path::{Path, PathBuf},
};

use db::models::file::{CreateFile, File};
use mime_guess::MimeGuess;
use sha2::{Digest, Sha256};
use sqlx::SqlitePool;
use uuid::Uuid;

#[derive(Debug, thiserror::Error)]
pub enum FileError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("File too large: {0} bytes (max: {1} bytes)")]
    TooLarge(u64, u64),

    #[error("Attachment placement could not be verified")]
    Placement,

    #[error("File not found")]
    NotFound,

    #[error("Failed to build response: {0}")]
    ResponseBuildError(String),
}

/// Sanitize filename for filesystem safety:
/// - Lowercase
/// - Spaces → underscores
/// - Remove special characters (keep alphanumeric and underscores)
/// - Truncate if too long
fn sanitize_filename(name: &str) -> String {
    let stem = Path::new(name)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("file");

    let clean: String = stem
        .to_lowercase()
        .chars()
        .map(|c| if c.is_whitespace() { '_' } else { c })
        .filter(|c| c.is_alphanumeric() || *c == '_')
        .collect();

    // Truncate to reasonable length to avoid filesystem limits
    let max_len = 50;
    if clean.len() > max_len {
        clean
            .chars()
            .scan(0, |bytes, c| {
                *bytes += c.len_utf8();
                (*bytes <= max_len).then_some(c)
            })
            .collect()
    } else if clean.is_empty() {
        "file".to_string()
    } else {
        clean
    }
}

#[derive(Clone)]
pub struct FileService {
    cache_dir: PathBuf,
    legacy_cache_dir: PathBuf,
    pool: SqlitePool,
    max_size_bytes: u64,
}

impl FileService {
    pub fn new(pool: SqlitePool) -> Result<Self, FileError> {
        let cache_dir = utils::cache_dir().join("attachments");
        let legacy_cache_dir = utils::cache_dir().join("images");
        fs::create_dir_all(&cache_dir)?;
        Ok(Self {
            cache_dir,
            legacy_cache_dir,
            pool,
            max_size_bytes: 20 * 1024 * 1024, // 20MB default
        })
    }

    pub async fn store_file(
        &self,
        data: &[u8],
        original_filename: &str,
    ) -> Result<File, FileError> {
        let file_size = data.len() as u64;

        if file_size > self.max_size_bytes {
            return Err(FileError::TooLarge(file_size, self.max_size_bytes));
        }

        let hash = format!("{:x}", Sha256::digest(data));

        if original_filename.is_empty()
            || original_filename.len() > 255
            || original_filename
                .chars()
                .any(|c| c.is_control() || matches!(c, '/' | '\\'))
        {
            return Err(FileError::Placement);
        }
        let extension = Path::new(original_filename)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("bin");

        let mime_type = MimeGuess::from_path(original_filename)
            .first_raw()
            .map(str::to_string)
            .or_else(|| {
                MimeGuess::from_ext(extension)
                    .first_raw()
                    .map(str::to_string)
            });

        let existing_file = sqlx::query_as::<_, File>(
            "SELECT * FROM attachments WHERE hash=? AND original_name=? AND mime_type IS ? LIMIT 1",
        )
        .bind(&hash)
        .bind(original_filename)
        .bind(&mime_type)
        .fetch_optional(&self.pool)
        .await?;

        if let Some(existing) = existing_file {
            verify_bytes(
                &existing,
                &read_regular(&self.get_absolute_path(&existing))?,
            )?;
            return Ok(existing);
        }

        let clean_name = sanitize_filename(original_filename);
        let extension: String = extension
            .chars()
            .filter(|c| c.is_ascii_alphanumeric())
            .take(16)
            .collect();
        let new_filename = format!("{}_{}.{}", Uuid::new_v4(), clean_name, extension);
        let cached_path = self.cache_dir.join(&new_filename);
        fs::write(&cached_path, data)?;

        let result = File::create(
            &self.pool,
            &CreateFile {
                file_path: new_filename,
                original_name: original_filename.to_string(),
                mime_type: mime_type.clone(),
                size_bytes: file_size as i64,
                hash,
            },
        )
        .await;
        match result {
            Ok(file) => Ok(file),
            Err(error) => {
                let _ = fs::remove_file(&cached_path);
                let existing = sqlx::query_as::<_, File>("SELECT * FROM attachments WHERE hash=? AND original_name=? AND mime_type IS ? LIMIT 1")
                    .bind(format!("{:x}", Sha256::digest(data))).bind(original_filename).bind(&mime_type).fetch_optional(&self.pool).await?;
                let existing = existing.ok_or(FileError::Database(error))?;
                verify_bytes(
                    &existing,
                    &read_regular(&self.get_absolute_path(&existing))?,
                )?;
                Ok(existing)
            }
        }
    }

    pub async fn delete_orphaned_files(&self) -> Result<(), FileError> {
        let orphaned_files = File::find_orphaned_files(&self.pool).await?;
        if orphaned_files.is_empty() {
            tracing::debug!("No orphaned files found during cleanup");
            return Ok(());
        }

        tracing::debug!("Found {} orphaned files to clean up", orphaned_files.len());
        let mut deleted_count = 0;
        let mut failed_count = 0;

        for file in orphaned_files {
            match self.delete_orphaned_file(file.id).await {
                Ok(false) => continue,
                Ok(true) => {
                    deleted_count += 1;
                    tracing::debug!("Deleted orphaned file: {}", file.id);
                }
                Err(e) => {
                    failed_count += 1;
                    tracing::error!("Failed to delete orphaned file {}: {}", file.id, e);
                }
            }
        }

        tracing::info!(
            "File cleanup completed: {} deleted, {} failed",
            deleted_count,
            failed_count
        );

        Ok(())
    }

    async fn delete_orphaned_file(&self, id: Uuid) -> Result<bool, FileError> {
        // A reference may have appeared since the cleanup snapshot. SQLite's
        // conditional delete is the authority, never the earlier SELECT.
        let path:Option<String>=sqlx::query_scalar("DELETE FROM attachments WHERE id=? AND NOT EXISTS (SELECT 1 FROM workspace_attachments WHERE attachment_id=attachments.id) AND NOT EXISTS (SELECT 1 FROM project_attachments WHERE attachment_id=attachments.id) RETURNING file_path").bind(id).fetch_optional(&self.pool).await?;
        let Some(path) = path else {
            return Ok(false);
        };
        for directory in [&self.cache_dir, &self.legacy_cache_dir] {
            match fs::remove_file(directory.join(&path)) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(e.into()),
            }
        }
        Ok(true)
    }

    pub fn get_absolute_path(&self, file: &File) -> PathBuf {
        self.resolve_cached_path(&file.file_path)
            .unwrap_or_else(|| self.cache_dir.join(&file.file_path))
    }

    pub async fn get_file(&self, id: Uuid) -> Result<Option<File>, FileError> {
        Ok(File::find_by_id(&self.pool, id).await?)
    }

    pub async fn delete_file(&self, id: Uuid) -> Result<(), FileError> {
        if let Some(file) = File::find_by_id(&self.pool, id).await? {
            let file_path = self.cache_dir.join(&file.file_path);
            if file_path.exists() {
                fs::remove_file(file_path)?;
            }

            let legacy_file_path = self.legacy_cache_dir.join(&file.file_path);
            if legacy_file_path.exists() {
                fs::remove_file(legacy_file_path)?;
            }

            File::delete(&self.pool, id).await?;
        }

        Ok(())
    }

    pub async fn copy_files_by_workspace_to_worktree(
        &self,
        worktree_path: &Path,
        workspace_id: Uuid,
        agent_working_dir: Option<&str>,
    ) -> Result<(), FileError> {
        let files = File::find_by_workspace_id(&self.pool, workspace_id).await?;
        let target_path = match agent_working_dir {
            Some(dir) if !dir.is_empty() => worktree_path.join(dir),
            _ => worktree_path.to_path_buf(),
        };
        let root = fs::canonicalize(worktree_path)?;
        let target = fs::canonicalize(&target_path)?;
        if !target.starts_with(root) {
            return Err(FileError::Placement);
        }
        self.copy_files(&target, files)
    }

    pub async fn copy_files_by_ids_to_worktree(
        &self,
        worktree_path: &Path,
        file_ids: &[Uuid],
    ) -> Result<(), FileError> {
        let mut files = Vec::new();
        for id in file_ids {
            files.push(
                File::find_by_id(&self.pool, *id)
                    .await?
                    .ok_or(FileError::NotFound)?,
            );
        }
        self.copy_files(worktree_path, files)
    }

    /// Copies are verified even when a destination already exists. Never overwrite
    /// agent edits or follow symlinks inside the attachment directory.
    fn copy_files(&self, worktree_path: &Path, files: Vec<File>) -> Result<(), FileError> {
        if files.is_empty() {
            return Ok(());
        }
        let root = fs::canonicalize(worktree_path)?;
        let dir = root.join(utils::path::VIBE_ATTACHMENTS_DIR);
        if fs::symlink_metadata(&dir).is_ok_and(|m| m.file_type().is_symlink()) {
            return Err(FileError::Placement);
        }
        fs::create_dir_all(&dir)?;
        if fs::canonicalize(&dir)? != dir {
            return Err(FileError::Placement);
        }
        // Hold the directory, so a concurrent Linux symlink replacement cannot
        // redirect writes between validation and publication.
        #[cfg(unix)]
        let directory = {
            use std::os::unix::fs::OpenOptionsExt;
            fs::OpenOptions::new()
                .read(true)
                .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW)
                .open(&dir)?
        };
        #[cfg(target_os = "linux")]
        let anchored_dir = {
            use std::os::fd::AsRawFd;
            PathBuf::from(format!("/proc/self/fd/{}", directory.as_raw_fd()))
        };
        #[cfg(not(target_os = "linux"))]
        let anchored_dir = dir.clone();
        let ignore = anchored_dir.join(".gitignore");
        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&ignore)
        {
            Ok(mut f) => {
                use std::io::Write;
                f.write_all(b"*\n")?;
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(e.into()),
        }
        for file in files {
            if Path::new(&file.file_path).components().count() != 1
                || file.file_path.contains(['/', '\\'])
            {
                return Err(FileError::Placement);
            }
            let dst = anchored_dir.join(&file.file_path);
            if let Ok(m) = fs::symlink_metadata(&dst) {
                if !m.is_file() || m.file_type().is_symlink() {
                    return Err(FileError::Placement);
                }
            } else {
                let src = self
                    .resolve_cached_path(&file.file_path)
                    .ok_or(FileError::NotFound)?;
                let data = read_regular(&src)?;
                verify_bytes(&file, &data)?;
                // Publish a complete file atomically without clobbering concurrent writers.
                let mut temp = tempfile::NamedTempFile::new_in(&anchored_dir)?;
                use std::io::Write;
                temp.write_all(&data)?;
                temp.as_file().sync_all()?;
                if let Err(e) = temp.persist_noclobber(&dst)
                    && e.error.kind() != std::io::ErrorKind::AlreadyExists
                {
                    return Err(e.error.into());
                }
            }
            if fs::symlink_metadata(&dst)?.file_type().is_symlink() {
                return Err(FileError::Placement);
            }
            verify_bytes(&file, &read_regular(&dst)?)?;
        }
        let current = fs::symlink_metadata(&dir)?;
        if current.file_type().is_symlink() {
            return Err(FileError::Placement);
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            let held = directory.metadata()?;
            if current.dev() != held.dev() || current.ino() != held.ino() {
                return Err(FileError::Placement);
            }
        }
        Ok(())
    }

    fn resolve_cached_path(&self, file_path: &str) -> Option<PathBuf> {
        let primary = self.cache_dir.join(file_path);
        if primary.exists() {
            return Some(primary);
        }

        let legacy = self.legacy_cache_dir.join(file_path);
        if legacy.exists() {
            tracing::info!(
                "Using legacy attachment cache path for {}: {}",
                file_path,
                legacy.display()
            );
            return Some(legacy);
        }

        None
    }
}

fn read_regular(path: &Path) -> Result<Vec<u8>, FileError> {
    use std::io::Read;
    let mut options = fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    let file = options.open(path)?;
    let metadata = file.metadata()?;
    const LIMIT: u64 = 20 * 1024 * 1024;
    if !metadata.is_file() || metadata.len() > LIMIT {
        return Err(FileError::Placement);
    }
    let mut bytes = Vec::new();
    file.take(LIMIT + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > LIMIT {
        return Err(FileError::Placement);
    }
    Ok(bytes)
}

fn verify_bytes(file: &File, bytes: &[u8]) -> Result<(), FileError> {
    if bytes.len() as i64 != file.size_bytes || format!("{:x}", Sha256::digest(bytes)) != file.hash
    {
        return Err(FileError::Placement);
    }
    Ok(())
}

#[cfg(test)]
mod attachment_tests {
    use super::*;
    async fn fixture() -> (FileService, tempfile::TempDir) {
        let root = tempfile::tempdir().unwrap();
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        sqlx::query("CREATE TABLE attachments(id BLOB PRIMARY KEY,file_path TEXT NOT NULL,original_name TEXT NOT NULL,mime_type TEXT,size_bytes INTEGER,hash TEXT NOT NULL,created_at TEXT NOT NULL DEFAULT '2026-10-29T00:00:00Z',updated_at TEXT NOT NULL DEFAULT '2026-10-29T00:00:00Z')").execute(&pool).await.unwrap();
        sqlx::query("CREATE UNIQUE INDEX identity ON attachments(hash,original_name,COALESCE(mime_type,''))").execute(&pool).await.unwrap();
        let cache = root.path().join("cache");
        fs::create_dir(&cache).unwrap();
        let service = FileService {
            pool,
            cache_dir: cache,
            legacy_cache_dir: root.path().join("legacy"),
            max_size_bytes: 20 * 1024 * 1024,
        };
        (service, root)
    }
    #[tokio::test]
    async fn attachment_bytes_names_retries_and_strict_placement() {
        let (svc, root) = fixture().await;
        let svg = b"<svg xmlns='http://www.w3.org/2000/svg'><text>hello</text></svg>";
        let name = format!("{}.svg", "素材".repeat(30));
        let first = svc.store_file(svg, &name).await.unwrap();
        let retry = svc.store_file(svg, &name).await.unwrap();
        assert_eq!(first.id, retry.id);
        let alias = svc.store_file(svg, "alias.txt").await.unwrap();
        assert_ne!(first.id, alias.id);
        assert_eq!(alias.original_name, "alias.txt");
        assert_eq!(alias.mime_type.as_deref(), Some("text/plain"));
        let different = svc.store_file(b"other bytes", &name).await.unwrap();
        assert_ne!(first.file_path, different.file_path);
        let png = include_bytes!("../../../../packages/public/favicon.png");
        let binary = svc.store_file(png, "image.png").await.unwrap();
        let work = root.path().join("work");
        fs::create_dir(&work).unwrap();
        let ids = [first.id, alias.id, binary.id, different.id];
        svc.copy_files_by_ids_to_worktree(&work, &ids)
            .await
            .unwrap();
        for file in [&first, &alias, &binary, &different] {
            let path = work
                .join(utils::path::VIBE_ATTACHMENTS_DIR)
                .join(&file.file_path);
            verify_bytes(file, &fs::read(path).unwrap()).unwrap();
        }
        svc.copy_files_by_ids_to_worktree(&work, &ids)
            .await
            .unwrap();
        assert!(
            svc.copy_files_by_ids_to_worktree(&work, &[Uuid::new_v4()])
                .await
                .is_err()
        );
        let dst = work
            .join(utils::path::VIBE_ATTACHMENTS_DIR)
            .join(&first.file_path);
        fs::write(&dst, b"changed").unwrap();
        assert!(
            svc.copy_files_by_ids_to_worktree(&work, &[first.id])
                .await
                .is_err()
        );
        fs::remove_file(&dst).unwrap();
        fs::remove_file(svc.get_absolute_path(&first)).unwrap();
        assert!(
            svc.copy_files_by_ids_to_worktree(&work, &[first.id])
                .await
                .is_err()
        );
        for name in [
            "../escape.svg",
            "bad\\name.png",
            "bad\nname",
            "",
            &"x".repeat(256),
        ] {
            assert!(svc.store_file(svg, name).await.is_err());
        }
        assert!(
            svc.store_file(&vec![0; 20 * 1024 * 1024 + 1], "big.bin")
                .await
                .is_err()
        );
    }
    #[tokio::test]
    async fn attachment_migration_preserves_existing_links_and_project_retention() {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::raw_sql("PRAGMA foreign_keys=ON; CREATE TABLE projects(id BLOB PRIMARY KEY); CREATE TABLE attachments(id BLOB PRIMARY KEY,file_path TEXT NOT NULL,original_name TEXT NOT NULL,mime_type TEXT,size_bytes INTEGER,hash TEXT NOT NULL UNIQUE,created_at TEXT NOT NULL,updated_at TEXT NOT NULL); CREATE TABLE workspace_attachments(workspace_id BLOB,attachment_id BLOB REFERENCES attachments(id));").execute(&pool).await.unwrap();
        let id = Uuid::new_v4();
        let project = Uuid::new_v4();
        sqlx::query("INSERT INTO projects VALUES(?)")
            .bind(project)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO attachments VALUES(?,'old.svg','old.svg','image/svg+xml',3,'abc','2026-10-29T00:00:00Z','2026-10-29T00:00:00Z')").bind(id).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO workspace_attachments VALUES(?,?)")
            .bind(Uuid::new_v4())
            .bind(id)
            .execute(&pool)
            .await
            .unwrap();
        let mut connection = pool.acquire().await.unwrap();
        sqlx::raw_sql("BEGIN")
            .execute(&mut *connection)
            .await
            .unwrap();
        sqlx::raw_sql(include_str!(
            "../../../db/migrations/20261029160000_project_attachments.sql"
        ))
        .execute(&mut *connection)
        .await
        .unwrap();
        sqlx::raw_sql("COMMIT")
            .execute(&mut *connection)
            .await
            .unwrap();
        drop(connection);
        assert!(
            sqlx::query("PRAGMA foreign_key_check")
                .fetch_all(&pool)
                .await
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            File::find_by_id(&pool, id)
                .await
                .unwrap()
                .unwrap()
                .original_name,
            "old.svg"
        );
        sqlx::query("DELETE FROM workspace_attachments")
            .execute(&pool)
            .await
            .unwrap();
        assert_eq!(File::find_orphaned_files(&pool).await.unwrap().len(), 1);
        sqlx::query("INSERT INTO project_attachments(project_id,attachment_id) VALUES(?,?)")
            .bind(project)
            .bind(id)
            .execute(&pool)
            .await
            .unwrap();
        assert!(File::find_orphaned_files(&pool).await.unwrap().is_empty());
        sqlx::query("DELETE FROM projects WHERE id=?")
            .bind(project)
            .execute(&pool)
            .await
            .unwrap();
        assert_eq!(File::find_orphaned_files(&pool).await.unwrap().len(), 1);
        sqlx::query("INSERT INTO attachments SELECT ?,'alias.txt','alias.txt','text/plain',size_bytes,hash,created_at,updated_at FROM attachments WHERE id=?").bind(Uuid::new_v4()).bind(id).execute(&pool).await.unwrap();
    }
    #[tokio::test]
    async fn attachment_legacy_reuse_and_cleanup_reference_race() {
        let (svc, root) = fixture().await;
        sqlx::raw_sql("CREATE TABLE workspace_attachments(workspace_id BLOB,attachment_id BLOB);CREATE TABLE project_attachments(project_id BLOB,attachment_id BLOB);").execute(&svc.pool).await.unwrap();
        let bytes = b"<svg/>";
        let file = svc.store_file(bytes, "legacy.svg").await.unwrap();
        fs::create_dir(&svc.legacy_cache_dir).unwrap();
        let legacy = svc.legacy_cache_dir.join(&file.file_path);
        fs::rename(svc.cache_dir.join(&file.file_path), &legacy).unwrap();
        let reused = svc.store_file(bytes, "legacy.svg").await.unwrap();
        assert_eq!(reused.id, file.id);
        assert_eq!(svc.get_absolute_path(&reused), legacy);
        let work = root.path().join("legacy-work");
        fs::create_dir(&work).unwrap();
        svc.copy_files_by_ids_to_worktree(&work, &[file.id])
            .await
            .unwrap();
        assert_eq!(
            fs::read(
                work.join(utils::path::VIBE_ATTACHMENTS_DIR)
                    .join(&file.file_path)
            )
            .unwrap(),
            bytes
        );
        let stale = File::find_orphaned_files(&svc.pool).await.unwrap();
        assert_eq!(stale.len(), 1);
        sqlx::query("INSERT INTO project_attachments VALUES(?,?)")
            .bind(Uuid::new_v4())
            .bind(file.id)
            .execute(&svc.pool)
            .await
            .unwrap();
        assert!(!svc.delete_orphaned_file(stale[0].id).await.unwrap());
        assert!(legacy.exists());
        assert!(
            File::find_by_id(&svc.pool, file.id)
                .await
                .unwrap()
                .is_some()
        );
        sqlx::query("DELETE FROM project_attachments")
            .execute(&svc.pool)
            .await
            .unwrap();
        assert!(svc.delete_orphaned_file(file.id).await.unwrap());
        assert!(!legacy.exists());
    }
    #[cfg(unix)]
    #[tokio::test]
    async fn attachment_symlinks_are_rejected() {
        use std::os::unix::fs::symlink;
        let (svc, root) = fixture().await;
        let file = svc.store_file(b"abc", "a.bin").await.unwrap();
        let outside = root.path().join("outside");
        fs::create_dir(&outside).unwrap();
        let work = root.path().join("work");
        fs::create_dir(&work).unwrap();
        let dir = work.join(utils::path::VIBE_ATTACHMENTS_DIR);
        symlink(&outside, &dir).unwrap();
        assert!(
            svc.copy_files_by_ids_to_worktree(&work, &[file.id])
                .await
                .is_err()
        );
        fs::remove_file(&dir).unwrap();
        fs::create_dir(&dir).unwrap();
        let secret = outside.join("secret");
        fs::write(&secret, b"abc").unwrap();
        symlink(&secret, dir.join(&file.file_path)).unwrap();
        assert!(
            svc.copy_files_by_ids_to_worktree(&work, &[file.id])
                .await
                .is_err()
        );
        assert_eq!(fs::read(secret).unwrap(), b"abc");
    }
}
