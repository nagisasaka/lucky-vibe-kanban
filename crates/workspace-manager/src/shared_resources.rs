//! Fixed, repository-scoped local resources. No tool-specific cache policy.
use std::{
    fs, io,
    path::{Path, PathBuf},
    process::Command,
    sync::Mutex,
};

use git::GitService;
use uuid::Uuid;

static PROVISION_LOCK: Mutex<()> = Mutex::new(());
const KINDS: [&str; 3] = ["persistent", "cache", "tmp"];

pub fn ensure(repo_root: &Path, repo_name: &str, repo_id: Uuid) -> io::Result<()> {
    ensure_at(
        repo_root,
        &utils::path::shared_resources_dir(repo_name, repo_id),
    )
}

/// Provision source publication/Change Manifest storage for registered repositories.
/// Source completion is unconditional; paid Wiki maintenance starts explicitly.
pub async fn ensure_repository_memory(
    pool: &sqlx::SqlitePool,
    repo: &db::models::repo::Repo,
) -> io::Result<()> {
    if db::models::repository_state::RepositoryMemoryStore::existing_for_repository(
        pool, &repo.name, repo.id,
    )
    .await?
    .is_some()
    {
        return Ok(());
    }
    // Registration does not own the user's checkout or its shared links.
    // Workspaces provision their own links when materialised.
    {
        let _guard = PROVISION_LOCK
            .lock()
            .map_err(|_| io::Error::other("shared resource lock poisoned"))?;
        ensure_storage(&utils::path::shared_resources_dir(&repo.name, repo.id))?;
    }
    db::models::repository_state::RepositoryMemoryStore::for_repository(pool, &repo.name, repo.id)?
        .initialize_defaults(default_memory_target_branch(repo))
        .await
}

pub fn default_memory_target_branch(repo: &db::models::repo::Repo) -> Option<String> {
    let git = GitService::new();
    repo.default_target_branch.clone().or_else(|| {
        if git.get_branch_oid(&repo.path, "main").is_ok() {
            Some("main".into())
        } else {
            git.get_current_branch(&repo.path).ok()
        }
    })
}

fn real_directory(path: &Path) -> io::Result<()> {
    match fs::create_dir(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            let metadata = fs::symlink_metadata(path)?;
            if metadata.is_dir() && !metadata.file_type().is_symlink() {
                Ok(())
            } else {
                Err(io::Error::other(format!(
                    "Shared directory conflicts with an existing path: {}",
                    path.display()
                )))
            }
        }
        Err(error) => Err(error),
    }
}

fn ensure_storage(shared: &Path) -> io::Result<()> {
    let parent = shared
        .parent()
        .ok_or_else(|| io::Error::other("missing shared parent"))?;
    fs::create_dir_all(
        parent
            .parent()
            .ok_or_else(|| io::Error::other("missing shared base"))?,
    )?;
    real_directory(parent)?;
    real_directory(shared)?;
    for kind in KINDS {
        real_directory(&shared.join(kind))?;
    }
    Ok(())
}

fn validate_link(link: &Path, target: &Path) -> io::Result<bool> {
    match fs::symlink_metadata(link) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
        Ok(meta) if meta.file_type().is_symlink() && fs::canonicalize(link)? == target => Ok(true),
        Ok(_) => Err(io::Error::other(format!(
            "LVK shared link conflicts with an existing path; move it manually before retrying: {}",
            link.display()
        ))),
    }
}

fn ensure_at(repo_root: &Path, shared: &Path) -> io::Result<()> {
    let _guard = PROVISION_LOCK
        .lock()
        .map_err(|_| io::Error::other("shared resource lock poisoned"))?;
    let repo_root = fs::canonicalize(repo_root)?;
    let tracked = Command::new("git")
        .arg("-C")
        .arg(&repo_root)
        .args(["ls-files", "-z", "--", ".lvk/shared"])
        .output()?;
    if !tracked.status.success() || !tracked.stdout.is_empty() {
        return Err(io::Error::other(
            "Cannot provision LVK shared directories: .lvk/shared is tracked or Git status could not be checked",
        ));
    }
    ensure_storage(shared)?;
    let shared = fs::canonicalize(shared)?;
    let lvk = utils::lvk_directory::LvkDirectory::open(&repo_root)?;
    let mount = lvk.anchored_path().join("shared");
    real_directory(&mount)?;
    // Check all targets before creating links; never replace user paths.
    for kind in KINDS {
        validate_link(&mount.join(kind), &shared.join(kind))?;
    }

    lvk.ensure_runtime_ignores()?;
    for kind in KINDS {
        let link = mount.join(kind);
        let target = shared.join(kind);
        if validate_link(&link, &target)? {
            continue;
        }
        let relative = pathdiff::diff_paths(&target, repo_root.join(".lvk/shared"))
            .unwrap_or_else(|| target.clone());
        if let Err(error) = create_directory_link(&relative, &link) {
            // Another LVK process may have provisioned the same link meanwhile.
            if error.kind() != io::ErrorKind::AlreadyExists || !validate_link(&link, &target)? {
                return Err(error);
            }
        }
    }
    lvk.verify()
}

#[cfg(unix)]
fn create_directory_link(target: &Path, link: &Path) -> io::Result<()> {
    std::os::unix::fs::symlink(target, link)
}

#[cfg(windows)]
fn create_directory_link(target: &Path, link: &Path) -> io::Result<()> {
    std::os::windows::fs::symlink_dir(target, link).map_err(|error| io::Error::new(error.kind(), format!("Cannot create LVK directory link; enable Windows Developer Mode or run with symlink privileges: {error}")))
}

/// Only return already provisioned paths for this repository, not the shared
/// parent containing other repositories.
pub fn writable_roots(repo_name: &str, repo_id: Uuid) -> Vec<PathBuf> {
    let root = utils::path::shared_resources_dir(repo_name, repo_id);
    writable_roots_at(&root)
}

fn writable_roots_at(root: &Path) -> Vec<PathBuf> {
    KINDS
        .into_iter()
        .filter_map(|kind| fs::canonicalize(root.join(kind)).ok())
        .collect()
}

#[cfg(all(test, unix))]
mod tests {
    use tempfile::tempdir;

    use super::*;

    fn git(repo: &Path, args: &[&str]) -> String {
        let output = Command::new("git")
            .arg("-C")
            .arg(repo)
            .args(args)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap()
    }

    #[test]
    fn worktrees_share_data_exclude_links_and_keep_data_after_deletion() {
        let tmp = tempdir().unwrap();
        let repo = tmp.path().join("repo");
        fs::create_dir(&repo).unwrap();
        git(&repo, &["init"]);
        git(
            &repo,
            &[
                "-c",
                "user.name=Test",
                "-c",
                "user.email=test@example.com",
                "commit",
                "--allow-empty",
                "-m",
                "init",
            ],
        );
        let tree = tmp.path().join("worktree");
        git(
            &repo,
            &["worktree", "add", "-b", "test", tree.to_str().unwrap()],
        );
        let storage = utils::path::shared_resources_dir("repo", Uuid::nil());
        let shared = tmp.path().join("shared").join(storage.file_name().unwrap());
        ensure_at(&repo, &shared).unwrap();
        ensure_at(&tree, &shared).unwrap();
        ensure_at(&tree, &shared).unwrap();
        // A producer process may finish before another Workspace consumes its handoff.
        assert!(
            Command::new("sh")
                .args(["-c", "printf 'handoff' > .lvk/shared/tmp/result"])
                .current_dir(&repo)
                .status()
                .unwrap()
                .success()
        );
        assert_eq!(
            fs::read_to_string(tree.join(".lvk/shared/tmp/result")).unwrap(),
            "handoff"
        );
        assert_eq!(
            writable_roots_at(&shared),
            KINDS.map(|kind| shared.join(kind))
        );
        fs::write(repo.join(".lvk/shared/persistent/data"), "retained").unwrap();
        assert_eq!(
            fs::read_to_string(tree.join(".lvk/shared/persistent/data")).unwrap(),
            "retained"
        );
        assert_eq!(
            git(&tree, &["ls-files", "--others", "--exclude-standard"]),
            ".lvk/.gitignore\n"
        );
        assert!(
            !fs::read_link(tree.join(".lvk/shared/cache"))
                .unwrap()
                .is_absolute()
        );
        let ignore = fs::read_to_string(repo.join(".lvk/.gitignore")).unwrap();
        assert_eq!(ignore.matches("/shared/").count(), 1);
        assert!(
            !fs::read_to_string(repo.join(".git/info/exclude"))
                .unwrap()
                .contains(".lvk")
        );
        assert_eq!(
            fs::canonicalize(tree.join(".lvk/shared/cache")).unwrap(),
            shared.join("cache")
        );
        git(
            &repo,
            &["worktree", "remove", "--force", tree.to_str().unwrap()],
        );
        assert_eq!(
            fs::read_to_string(shared.join("persistent/data")).unwrap(),
            "retained"
        );
        assert_eq!(
            fs::read_to_string(shared.join("tmp/result")).unwrap(),
            "handoff"
        );
    }

    #[test]
    fn repositories_are_isolated_and_tracked_settings_survive() {
        let tmp = tempdir().unwrap();
        for name in ["first", "second"] {
            let repo = tmp.path().join(name);
            fs::create_dir(&repo).unwrap();
            git(&repo, &["init"]);
            fs::create_dir(repo.join(".lvk")).unwrap();
            fs::write(repo.join(".lvk/integration-validation.json"), "{}").unwrap();
            fs::write(repo.join(".lvk/.gitignore"), "# user rules\n/custom/\n").unwrap();
            git(&repo, &["add", ".lvk"]);
            let shared = tmp.path().join("shared").join(name);
            ensure_at(&repo, &shared).unwrap();
            fs::write(repo.join(".lvk/shared/persistent/data"), name).unwrap();
            fs::write(repo.join(".lvk/shared/tmp/result"), name).unwrap();
            assert!(writable_roots_at(&shared).contains(&shared.join("tmp")));
            assert_eq!(
                fs::read_to_string(repo.join(".lvk/integration-validation.json")).unwrap(),
                "{}"
            );
            assert!(
                fs::read_to_string(repo.join(".lvk/.gitignore"))
                    .unwrap()
                    .starts_with("# user rules\n/custom/\n")
            );
            assert_eq!(
                git(&repo, &["ls-files", ".lvk/integration-validation.json"]),
                ".lvk/integration-validation.json\n"
            );
        }
        for name in ["first", "second"] {
            assert_eq!(
                fs::read_to_string(tmp.path().join(name).join(".lvk/shared/persistent/data"))
                    .unwrap(),
                name
            );
            assert_eq!(
                fs::read_to_string(tmp.path().join(name).join(".lvk/shared/tmp/result")).unwrap(),
                name
            );
        }
    }

    #[test]
    fn tmp_conflicts_preserve_existing_storage_and_links() {
        let tmp = tempdir().unwrap();
        let repo = tmp.path().join("repo");
        fs::create_dir(&repo).unwrap();
        git(&repo, &["init"]);
        let shared = tmp.path().join("shared/repo-id");
        ensure_at(&repo, &shared).unwrap();
        fs::write(shared.join("persistent/data"), "retained").unwrap();
        fs::write(shared.join("cache/data"), "reusable").unwrap();
        // Provisioning a newly added kind must leave earlier persistent/cache data intact.
        fs::remove_file(repo.join(".lvk/shared/tmp")).unwrap();
        fs::remove_dir(shared.join("tmp")).unwrap();
        ensure_at(&repo, &shared).unwrap();
        assert!(shared.join("tmp").is_dir());
        fs::remove_file(repo.join(".lvk/shared/tmp")).unwrap();
        fs::write(repo.join(".lvk/shared/tmp"), "user file").unwrap();
        assert!(ensure_at(&repo, &shared).is_err());
        assert_eq!(
            fs::read_to_string(repo.join(".lvk/shared/tmp")).unwrap(),
            "user file"
        );
        fs::remove_file(repo.join(".lvk/shared/tmp")).unwrap();
        let other = tmp.path().join("other");
        fs::create_dir(&other).unwrap();
        create_directory_link(&other, &repo.join(".lvk/shared/tmp")).unwrap();
        assert!(ensure_at(&repo, &shared).is_err());
        assert_eq!(
            fs::canonicalize(repo.join(".lvk/shared/tmp")).unwrap(),
            other
        );
        for kind in ["persistent", "cache"] {
            assert_eq!(
                fs::canonicalize(repo.join(".lvk/shared").join(kind)).unwrap(),
                shared.join(kind)
            );
        }
        assert_eq!(
            fs::read_to_string(shared.join("persistent/data")).unwrap(),
            "retained"
        );
        assert_eq!(
            fs::read_to_string(shared.join("cache/data")).unwrap(),
            "reusable"
        );
        assert!(fs::read_dir(&other).unwrap().next().is_none());
        // Conflicting repository-external tmp storage is also never replaced.
        fs::remove_file(repo.join(".lvk/shared/tmp")).unwrap();
        fs::remove_dir(shared.join("tmp")).unwrap();
        create_directory_link(&other, &shared.join("tmp")).unwrap();
        assert!(ensure_at(&repo, &shared).is_err());
        assert!(
            fs::symlink_metadata(shared.join("tmp"))
                .unwrap()
                .file_type()
                .is_symlink()
        );
    }

    #[test]
    fn refuses_existing_files_and_links_to_other_repositories() {
        let tmp = tempdir().unwrap();
        let repo = tmp.path().join("repo");
        fs::create_dir(&repo).unwrap();
        git(&repo, &["init"]);
        fs::create_dir_all(repo.join(".lvk/shared")).unwrap();
        fs::write(repo.join(".lvk/shared/cache"), "user data").unwrap();
        let shared = tmp.path().join("shared").join("first");
        assert!(ensure_at(&repo, &shared).is_err());
        assert_eq!(
            fs::read_to_string(repo.join(".lvk/shared/cache")).unwrap(),
            "user data"
        );
        assert!(!repo.join(".lvk/shared/persistent").exists());
        fs::remove_file(repo.join(".lvk/shared/cache")).unwrap();
        ensure_at(&repo, &shared).unwrap();
        assert!(ensure_at(&repo, &tmp.path().join("shared/other")).is_err());
        assert_eq!(
            fs::canonicalize(repo.join(".lvk/shared/cache")).unwrap(),
            shared.join("cache")
        );
    }

    #[test]
    fn refuses_tracked_mount_and_symlinked_storage_without_overwriting() {
        let tmp = tempdir().unwrap();
        let repo = tmp.path().join("repo");
        fs::create_dir(&repo).unwrap();
        git(&repo, &["init"]);
        fs::create_dir(repo.join(".lvk")).unwrap();
        fs::write(repo.join(".lvk/shared"), "tracked config").unwrap();
        git(&repo, &["add", ".lvk/shared"]);
        let shared = tmp.path().join("shared/repo-id");
        assert!(ensure_at(&repo, &shared).is_err());
        assert!(!shared.exists());
        assert_eq!(
            fs::read_to_string(repo.join(".lvk/shared")).unwrap(),
            "tracked config"
        );
        git(&repo, &["rm", "--cached", ".lvk/shared"]);
        fs::remove_file(repo.join(".lvk/shared")).unwrap();
        fs::create_dir_all(&shared).unwrap();
        let external = tmp.path().join("external");
        fs::create_dir(&external).unwrap();
        create_directory_link(&external, &shared.join("persistent")).unwrap();
        assert!(ensure_at(&repo, &shared).is_err());
        assert!(!repo.join(".lvk/shared").exists());
        assert!(fs::read_dir(external).unwrap().next().is_none());
    }
}
