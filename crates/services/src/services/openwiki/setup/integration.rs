//! Auto-merge journal. Preparation is an explicit operation after code evidence;
//! finalisation is host-only, after all writers and operation proof have settled.
use sha2::{Digest, Sha256};
use utils::repository_memory::IntegrationWikiSetup;

use super::*;

pub struct Identity<'a> {
    pub integration_id: Uuid,
    pub repository_id: Uuid,
    pub session_id: Uuid,
    pub workspace_id: Uuid,
    pub storage_identity: &'a str,
    pub root: &'a Path,
    pub code_commit: &'a str,
}

/// Bind immutable original identity/bytes to the host's durable request receipt.
/// Phase progress is independently validated and never replaces this origin.
pub fn original_digest(journal: &IntegrationWikiSetup) -> anyhow::Result<String> {
    let mut original = journal.clone();
    original.final_commit = None;
    original.previous_final_commits.clear();
    original.checkpoint.phase = WikiSetupPhase::Prepared;
    for entry in &mut original.checkpoint.files {
        entry.restoring_from = None;
    }
    Ok(format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&original)?)
    ))
}

/// No materialisation is needed here: auto-merge rejects instruction aliases.
/// Save original bytes durably before acknowledging permission to call begin.
pub fn prepare(store: &RepositoryMemoryStore, id: &Identity<'_>) -> anyhow::Result<()> {
    let _lock = store.try_workspace_completion_lock(id.workspace_id)?;
    if store.integration_wiki_setup(id.integration_id)?.is_some() {
        let head = super::super::git_text(id.root, &["rev-parse", "HEAD"])?;
        let mut journal = validate(store, id, &head)?;
        ensure!(
            journal.checkpoint.phase != WikiSetupPhase::Restoring
                && (journal.final_commit.is_none()
                    || journal.checkpoint.phase == WikiSetupPhase::Restored),
            "Host restoration is unfinished; republish the same final SHA without begin and let the host settle restoration first"
        );
        if journal.checkpoint.phase == WikiSetupPhase::Restored {
            // A new explicit writer phase, never a new original snapshot. The
            // caller has checked the original host receipt and other writers.
            git::GitService::new().require_clean_source(id.root, &head)?;
            journal.previous_final_commits.push(
                journal
                    .final_commit
                    .take()
                    .context("Missing restored Wiki SHA")?,
            );
            journal.checkpoint.phase = WikiSetupPhase::Prepared;
            for entry in &mut journal.checkpoint.files {
                entry.restoring_from = None;
            }
            store.save_integration_wiki_setup(&journal)?;
        }
        return Ok(());
    }
    git::GitService::new().require_clean_source(id.root, id.code_commit)?;
    preflight(id.root)?;
    let files = INSTRUCTION_PATHS
        .iter()
        .map(|path| {
            let entry = capture_instruction(id.root, path)?;
            ensure!(
                !matches!(entry.original, WikiSetupFile::Symlink { .. }),
                "Auto-merge instruction aliases are not supported"
            );
            Ok(WikiSetupEntry {
                prepared: entry.original.clone(),
                ..entry
            })
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    let journal = IntegrationWikiSetup {
        version: 1,
        integration_id: id.integration_id,
        repository_id: id.repository_id,
        session_id: id.session_id,
        storage_identity: id.storage_identity.into(),
        prepared_at: chrono::Utc::now(),
        final_commit: None,
        previous_final_commits: Vec::new(),
        checkpoint: WikiSetupCheckpoint {
            version: 1,
            workspace_id: id.workspace_id,
            repository_path: id.root.into(),
            source_commit: id.code_commit.into(),
            phase: WikiSetupPhase::Prepared,
            files,
        },
    };
    validate_journal(&journal, id)?;
    store.save_integration_wiki_setup(&journal)?;
    Ok(())
}

/// Read-only admission check. Dirty bytes are permitted only when they are the
/// exact pinned begin transformation, never arbitrary edits inside the markers.
pub fn validate(
    store: &RepositoryMemoryStore,
    id: &Identity<'_>,
    final_commit: &str,
) -> anyhow::Result<IntegrationWikiSetup> {
    let journal = store
        .integration_wiki_setup(id.integration_id)?
        .context("Missing original auto-merge OpenWiki setup journal; candidate preserved")?;
    validate_journal(&journal, id)?;
    ensure!(
        journal
            .final_commit
            .as_deref()
            .is_none_or(|sha| sha == final_commit),
        "Setup finalisation SHA changed"
    );
    ensure!(
        super::super::git_text(id.root, &["rev-parse", "HEAD"])? == final_commit,
        "Final Wiki HEAD changed"
    );
    ensure!(
        git::GitService::new().is_ancestor(id.root, id.code_commit, final_commit)?,
        "Wiki candidate lost code ancestry"
    );
    let mut previous = id.code_commit;
    for phase in &journal.previous_final_commits {
        ensure!(
            git::GitService::new().is_ancestor(id.root, previous, phase)?
                && git::GitService::new().is_ancestor(id.root, phase, final_commit)?,
            "Wiki writer phase lost a previous finalized candidate"
        );
        previous = phase;
    }
    // Inspect every commit, so temporary source edits later reverted cannot be
    // hidden by a net Wiki-only diff. Merge parents must all descend from code.
    let commits = raw_git(
        id.root,
        &[
            "rev-list",
            "--parents",
            &format!("{}..{final_commit}", id.code_commit),
        ],
    )?;
    for line in commits.lines() {
        let mut words = line.split_whitespace();
        let commit = words.next().context("Missing Wiki commit")?;
        for parent in words {
            ensure!(
                git::GitService::new().is_ancestor(id.root, id.code_commit, parent)?,
                "Wiki history imported an unrelated parent"
            );
            let paths = raw_git(
                id.root,
                &["diff", "--no-renames", "--name-only", "-z", parent, commit],
            )?;
            ensure!(
                paths
                    .split('\0')
                    .filter(|p| !p.is_empty())
                    .all(allowed_wiki_path),
                "Wiki history changed source or user INSTRUCTIONS"
            );
        }
    }
    ensure!(
        raw_git(
            id.root,
            &["diff", "--cached", "--name-only", "-z", final_commit]
        )?
        .is_empty(),
        "OpenWiki index differs from final HEAD"
    );
    for name in [
        "MERGE_HEAD",
        "CHERRY_PICK_HEAD",
        "REVERT_HEAD",
        "rebase-merge",
        "rebase-apply",
        "sequencer",
    ] {
        let path = super::super::git_text(id.root, &["rev-parse", "--git-path", name])?;
        ensure!(
            !id.root.join(path).exists(),
            "Unfinished Git operation during Wiki finalisation"
        );
    }
    let status = raw_git(
        id.root,
        &["status", "--porcelain=v1", "-z", "--untracked-files=all"],
    )?;
    for entry in status.split('\0').filter(|p| !p.is_empty()) {
        ensure!(
            entry.len() >= 3
                && matches!(&entry[..3], " M " | "?? ")
                && INSTRUCTION_PATHS.contains(&&entry[3..]),
            "Unexpected dirty OpenWiki candidate; preserved for inspection"
        );
    }
    preflight(id.root)?;
    for entry in &journal.checkpoint.files {
        let current = read_entry(&id.root.join(&entry.path))?;
        validate_exact_generated(entry, &current)?;
        if let Some(before) = &entry.restoring_from {
            validate_exact_generated(entry, before)?;
        }
        match journal.checkpoint.phase {
            WikiSetupPhase::Prepared => ensure!(
                entry.restoring_from.is_none(),
                "Unexpected restoration bytes"
            ),
            WikiSetupPhase::Restoring => ensure!(
                current == entry.original || Some(&current) == entry.restoring_from.as_ref(),
                "Instructions edited during restoration"
            ),
            WikiSetupPhase::Restored => ensure!(
                current == entry.original,
                "Instructions edited after restoration"
            ),
            WikiSetupPhase::Preparing => bail!("Invalid auto-merge setup phase"),
        }
    }
    Ok(journal)
}

/// Caller MUST hold the integration publication fence and confirm writer exit.
/// Pin final SHA before the first replacement. Retry retains both SHA identities.
pub fn restore(
    store: &RepositoryMemoryStore,
    id: &Identity<'_>,
    final_commit: &str,
) -> anyhow::Result<()> {
    let _lock = store.try_workspace_completion_lock(id.workspace_id)?;
    let mut journal = validate(store, id, final_commit)?;
    if journal.final_commit.is_none() {
        journal.final_commit = Some(final_commit.into());
        store.save_integration_wiki_setup(&journal)?;
    }
    let mut checkpoint = journal.checkpoint.clone();
    restore_checkpoint(
        id.root,
        &mut checkpoint,
        validate_exact_generated,
        |checkpoint| {
            journal.checkpoint = checkpoint.clone();
            store
                .save_integration_wiki_setup(&journal)
                .map_err(Into::into)
        },
    )?;
    git::GitService::new().require_clean_source(id.root, final_commit)?;
    Ok(())
}

fn validate_journal(journal: &IntegrationWikiSetup, id: &Identity<'_>) -> anyhow::Result<()> {
    ensure!(
        journal.version == 1
            && journal.integration_id == id.integration_id
            && journal.repository_id == id.repository_id
            && journal.session_id == id.session_id
            && journal.storage_identity == id.storage_identity,
        "Auto-merge setup owner/repository identity mismatch"
    );
    ensure!(
        git::GitService::new()
            .storage_identity(id.root)?
            .to_string_lossy()
            == id.storage_identity,
        "Auto-merge Git storage identity changed"
    );
    validate_identity(
        &journal.checkpoint,
        id.workspace_id,
        id.root,
        id.code_commit,
    )?;
    if matches!(
        journal.checkpoint.phase,
        WikiSetupPhase::Restoring | WikiSetupPhase::Restored
    ) {
        ensure!(
            journal.final_commit.is_some()
                && journal
                    .checkpoint
                    .files
                    .iter()
                    .all(|entry| entry.restoring_from.is_some()),
            "Incomplete restoration journal"
        );
    }
    for entry in &journal.checkpoint.files {
        ensure!(
            entry.prepared == entry.original,
            "Auto-merge setup snapshot was altered"
        );
        // The immutable code tree authenticates the original; JSON edits cannot
        // manufacture bytes to overwrite user instructions during restoration.
        let tree = raw_git(id.root, &["ls-tree", id.code_commit, "--", &entry.path])?;
        match &entry.original {
            WikiSetupFile::Missing => {
                ensure!(tree.is_empty(), "Original instruction missing from journal")
            }
            WikiSetupFile::Regular { content, mode } => {
                // Git authenticates the executable bit; exact current modes are also
                // checked below. Special permission bits require review.
                ensure!(
                    *mode & !0o777 == 0,
                    "Special instruction permissions require review"
                );
                ensure!(
                    tree.starts_with(if mode & 0o111 == 0 {
                        "100644 blob "
                    } else {
                        "100755 blob "
                    }),
                    "Original instruction type/mode mismatch"
                );
                ensure!(
                    raw_git(
                        id.root,
                        &["show", &format!("{}:{}", id.code_commit, entry.path)]
                    )? == *content,
                    "Original instruction bytes do not match validated code"
                );
            }
            WikiSetupFile::Symlink { .. } => bail!("Auto-merge instruction alias in journal"),
        }
    }
    Ok(())
}

fn allowed_wiki_path(path: &str) -> bool {
    crate::services::repository_memory::canonical_wiki_path(path)
        && path != "openwiki/INSTRUCTIONS.md"
}

fn raw_git(root: &Path, args: &[&str]) -> anyhow::Result<String> {
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()?;
    ensure!(
        output.status.success(),
        "Cannot verify auto-merge setup Git evidence"
    );
    Ok(String::from_utf8(output.stdout)?)
}

fn validate_exact_generated(entry: &WikiSetupEntry, current: &WikiSetupFile) -> anyhow::Result<()> {
    if current == &entry.original {
        return Ok(());
    }
    let (original, expected_mode) = match &entry.original {
        WikiSetupFile::Missing => ("", None),
        WikiSetupFile::Regular { content, mode } => (content.as_str(), Some(*mode)),
        WikiSetupFile::Symlink { .. } => bail!("Unexpected instruction alias"),
    };
    ensure!(
        !(entry.path == "CLAUDE.md"
            && (matches!(entry.original, WikiSetupFile::Missing)
                || original.trim() == "@AGENTS.md")),
        "OpenWiki changed a file begin should leave untouched"
    );
    let snippet = if entry.path == "AGENTS.md" {
        include_str!("agents-0.9.0.txt")
    } else {
        include_str!("claude-0.9.0.txt")
    };
    super::super::without_managed_block(original)?;
    let expected = if let Some(start) = original.find("<!-- OPENWIKI:START -->") {
        let end = original
            .find("<!-- OPENWIKI:END -->")
            .context("Missing end marker")?
            + "<!-- OPENWIKI:END -->".len();
        format!("{}{snippet}{}", &original[..start], &original[end..])
    } else {
        format!(
            "{}{separator}{snippet}\n",
            original.trim_end(),
            separator = if original.trim().is_empty() {
                ""
            } else {
                "\n\n"
            }
        )
    };
    let WikiSetupFile::Regular { content, mode } = current else {
        bail!("Generated instruction changed type; preserved");
    };
    ensure!(
        *content == expected
            && expected_mode.map_or(*mode & !0o666 == 0, |expected| *mode == expected),
        "Unknown generated change or later edit in {}; preserved",
        entry.path
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fixture {
        _temp: tempfile::TempDir,
        root: PathBuf,
        store: RepositoryMemoryStore,
        owner: Uuid,
        repo: Uuid,
        workspace: Uuid,
        session: Uuid,
        code: String,
        storage: String,
    }
    impl Fixture {
        fn new() -> Self {
            let temp = tempfile::tempdir().unwrap();
            let root = temp.path().join("repo");
            fs::create_dir_all(root.join("openwiki")).unwrap();
            fs::create_dir(temp.path().join("persistent")).unwrap();
            let store =
                RepositoryMemoryStore::at_persistent(&temp.path().join("persistent")).unwrap();
            raw_git(&root, &["init", "-b", "main"]).unwrap();
            raw_git(&root, &["config", "user.name", "Fixture"]).unwrap();
            raw_git(&root, &["config", "user.email", "fixture@example.invalid"]).unwrap();
            fs::write(
                root.join("AGENTS.md"),
                "# User rules\r\nPreserve bytes.  \r\n\r\n",
            )
            .unwrap();
            fs::write(root.join("CLAUDE.md"), "# Other rules\n").unwrap();
            fs::write(root.join("code"), "source\n").unwrap();
            fs::write(root.join("openwiki/INSTRUCTIONS.md"), "user instructions\n").unwrap();
            raw_git(&root, &["add", "."]).unwrap();
            raw_git(&root, &["commit", "-m", "code"]).unwrap();
            let code = super::super::super::git_text(&root, &["rev-parse", "HEAD"]).unwrap();
            let storage = git::GitService::new()
                .storage_identity(&root)
                .unwrap()
                .to_string_lossy()
                .into_owned();
            Self {
                storage,
                _temp: temp,
                root,
                store,
                owner: Uuid::new_v4(),
                repo: Uuid::new_v4(),
                workspace: Uuid::new_v4(),
                session: Uuid::new_v4(),
                code,
            }
        }
        fn id(&self) -> Identity<'_> {
            Identity {
                integration_id: self.owner,
                repository_id: self.repo,
                session_id: self.session,
                workspace_id: self.workspace,
                storage_identity: &self.storage,
                root: &self.root,
                code_commit: &self.code,
            }
        }
        fn prepare(&self) {
            prepare(&self.store, &self.id()).unwrap();
        }
        fn generate(&self) {
            let journal = self
                .store
                .integration_wiki_setup(self.owner)
                .unwrap()
                .unwrap();
            for entry in journal.checkpoint.files {
                let WikiSetupFile::Regular { content, .. } = entry.original else {
                    panic!()
                };
                let snippet = if entry.path == "AGENTS.md" {
                    include_str!("agents-0.9.0.txt")
                } else {
                    include_str!("claude-0.9.0.txt")
                };
                fs::write(
                    self.root.join(entry.path),
                    format!("{}\n\n{snippet}\n", content.trim_end()),
                )
                .unwrap();
            }
        }
        fn wiki_commit(&self) -> String {
            fs::write(self.root.join("openwiki/index.md"), "Wiki content\n").unwrap();
            raw_git(&self.root, &["add", "openwiki"]).unwrap();
            raw_git(&self.root, &["commit", "-m", "Wiki only"]).unwrap();
            super::super::super::git_text(&self.root, &["rev-parse", "HEAD"]).unwrap()
        }
    }

    #[test]
    fn integration_wiki_history_restores_exact_originals_and_retries() {
        let f = Fixture::new();
        f.prepare();
        f.prepare(); // repeated begin / run restart retains originals
        f.generate();
        let wiki = f.wiki_commit();
        let original = f.store.integration_wiki_setup(f.owner).unwrap().unwrap();
        f.prepare(); // continuation after Wiki commit never recaptures originals
        assert_eq!(
            original,
            f.store.integration_wiki_setup(f.owner).unwrap().unwrap()
        );
        validate(&f.store, &f.id(), &wiki).unwrap();
        restore(&f.store, &f.id(), &wiki).unwrap();
        restore(&f.store, &f.id(), &wiki).unwrap();
        git::GitService::new()
            .require_clean_source(&f.root, &wiki)
            .unwrap();
        let restored = f.store.integration_wiki_setup(f.owner).unwrap().unwrap();
        assert_eq!(
            original_digest(&original).unwrap(),
            original_digest(&restored).unwrap()
        );
        let mut changed_origin = restored;
        changed_origin.prepared_at -= chrono::Duration::seconds(1);
        assert_ne!(
            original_digest(&original).unwrap(),
            original_digest(&changed_origin).unwrap()
        );
        assert_eq!(
            fs::read_to_string(f.root.join("AGENTS.md")).unwrap(),
            "# User rules\r\nPreserve bytes.  \r\n\r\n"
        );
        assert_eq!(
            fs::read_to_string(f.root.join("openwiki/index.md")).unwrap(),
            "Wiki content\n"
        );
        assert_eq!(
            fs::read_to_string(f.root.join("openwiki/INSTRUCTIONS.md")).unwrap(),
            "user instructions\n"
        );
        assert_eq!(
            f.store
                .integration_wiki_setup(f.owner)
                .unwrap()
                .unwrap()
                .final_commit
                .as_deref(),
            Some(wiki.as_str())
        );
    }

    #[test]
    fn integration_refuses_dirty_late_snapshot_and_corrupt_journal() {
        let f = Fixture::new();
        fs::write(f.root.join("AGENTS.md"), "late generated changes").unwrap();
        assert!(prepare(&f.store, &f.id()).is_err());
        assert!(f.store.integration_wiki_setup(f.owner).unwrap().is_none());
        raw_git(&f.root, &["checkout", "--", "AGENTS.md"]).unwrap();
        f.prepare();
        f.generate();
        let before = fs::read(f.root.join("AGENTS.md")).unwrap();
        let path = f
            ._temp
            .path()
            .join("persistent/knowledge/integration-wiki-setups")
            .join(format!("{}.json", f.owner));
        fs::write(path, b"{corrupt journal").unwrap();
        assert!(restore(&f.store, &f.id(), &f.code).is_err());
        assert_eq!(fs::read(f.root.join("AGENTS.md")).unwrap(), before);
    }

    #[test]
    fn integration_existing_block_suffix_and_missing_files_are_preserved() {
        for absent in [false, true] {
            let mut f = Fixture::new();
            let original = "# Prefix\r\n<!-- OPENWIKI:START -->\nold generated block\n<!-- OPENWIKI:END -->\r\nUser appendix  \r\n";
            if absent {
                fs::remove_file(f.root.join("AGENTS.md")).unwrap();
                fs::remove_file(f.root.join("CLAUDE.md")).unwrap();
            } else {
                fs::write(f.root.join("AGENTS.md"), original).unwrap();
                fs::write(f.root.join("CLAUDE.md"), "@AGENTS.md\n").unwrap();
            }
            raw_git(&f.root, &["add", "."]).unwrap();
            raw_git(&f.root, &["commit", "-m", "instruction fixture"]).unwrap();
            f.code = super::super::super::git_text(&f.root, &["rev-parse", "HEAD"]).unwrap();
            f.prepare();
            let snippet = include_str!("agents-0.9.0.txt");
            let generated = if absent {
                format!("{snippet}\n")
            } else {
                original.replace(
                    "<!-- OPENWIKI:START -->\nold generated block\n<!-- OPENWIKI:END -->",
                    snippet,
                )
            };
            fs::write(f.root.join("AGENTS.md"), generated).unwrap();
            restore(&f.store, &f.id(), &f.code).unwrap();
            if absent {
                assert!(!f.root.join("AGENTS.md").exists());
                assert!(!f.root.join("CLAUDE.md").exists());
            } else {
                assert_eq!(
                    fs::read_to_string(f.root.join("AGENTS.md")).unwrap(),
                    original
                );
                assert_eq!(
                    fs::read_to_string(f.root.join("CLAUDE.md")).unwrap(),
                    "@AGENTS.md\n"
                );
            }
            git::GitService::new()
                .require_clean_source(&f.root, &f.code)
                .unwrap();
        }
    }

    #[test]
    fn integration_journal_fits_bounded_escaped_original_and_restoration_copies() {
        let mut f = Fixture::new();
        let original = "\"".repeat(20_000);
        for path in INSTRUCTION_PATHS {
            fs::write(f.root.join(path), &original).unwrap();
        }
        raw_git(&f.root, &["add", "."]).unwrap();
        raw_git(&f.root, &["commit", "-m", "large instructions"]).unwrap();
        f.code = super::super::super::git_text(&f.root, &["rev-parse", "HEAD"]).unwrap();
        f.prepare();
        f.generate();
        restore(&f.store, &f.id(), &f.code).unwrap();
        for path in INSTRUCTION_PATHS {
            assert_eq!(fs::read_to_string(f.root.join(path)).unwrap(), original);
        }
    }

    #[test]
    fn integration_begin_noop_retains_code_sha() {
        let f = Fixture::new();
        f.prepare();
        f.generate();
        restore(&f.store, &f.id(), &f.code).unwrap();
        git::GitService::new()
            .require_clean_source(&f.root, &f.code)
            .unwrap();
    }

    #[test]
    fn integration_missing_identity_and_tampered_original_fail_closed() {
        let f = Fixture::new();
        let mut redirected = f.id();
        redirected.storage_identity = "another-git-storage";
        assert!(prepare(&f.store, &redirected).is_err());
        assert!(f.store.integration_wiki_setup(f.owner).unwrap().is_none());
        assert!(restore(&f.store, &f.id(), &f.code).is_err());
        f.prepare();
        f.generate();
        for kind in [
            "owner",
            "repo",
            "session",
            "workspace",
            "storage",
            "source",
            "root",
            "original",
            "prepared",
        ] {
            let original = f.store.integration_wiki_setup(f.owner).unwrap().unwrap();
            let mut changed = original.clone();
            match kind {
                "owner" => changed.integration_id = Uuid::new_v4(),
                "repo" => changed.repository_id = Uuid::new_v4(),
                "session" => changed.session_id = Uuid::new_v4(),
                "workspace" => changed.checkpoint.workspace_id = Uuid::new_v4(),
                "storage" => changed.storage_identity = "other".into(),
                "source" => changed.checkpoint.source_commit = "0".repeat(40),
                "root" => changed.checkpoint.repository_path = f._temp.path().into(),
                "original" => {
                    changed.checkpoint.files[0].original = WikiSetupFile::Missing;
                    changed.checkpoint.files[0].prepared = WikiSetupFile::Missing;
                }
                _ => changed.checkpoint.files[0].prepared = WikiSetupFile::Missing,
            }
            // Validate identity without writing an alternate owner's journal.
            assert!(validate_journal(&changed, &f.id()).is_err(), "{kind}");
            assert_eq!(
                f.store.integration_wiki_setup(f.owner).unwrap().unwrap(),
                original
            );
        }
        assert!(
            fs::read_to_string(f.root.join("AGENTS.md"))
                .unwrap()
                .contains("OPENWIKI:START")
        );
    }

    #[test]
    fn integration_rejects_edits_markers_index_and_non_wiki_commits() {
        for kind in [
            "outside",
            "inside",
            "whitespace",
            "markers",
            "index",
            "head",
            "source",
            "instructions",
            "workflow",
            "untracked",
        ] {
            let f = Fixture::new();
            f.prepare();
            f.generate();
            let wiki = f.wiki_commit();
            let agents = fs::read_to_string(f.root.join("AGENTS.md")).unwrap();
            match kind {
                "outside" => {
                    fs::write(f.root.join("AGENTS.md"), format!("{agents}User edit\n")).unwrap()
                }
                "inside" => fs::write(
                    f.root.join("AGENTS.md"),
                    agents.replace("## OpenWiki", "## Manual edit"),
                )
                .unwrap(),
                "whitespace" => fs::write(f.root.join("AGENTS.md"), format!("{agents}\n")).unwrap(),
                "markers" => fs::write(
                    f.root.join("AGENTS.md"),
                    format!("{agents}<!-- OPENWIKI:START -->"),
                )
                .unwrap(),
                "index" => {
                    raw_git(&f.root, &["add", "AGENTS.md"]).unwrap();
                }
                "head" => {
                    raw_git(&f.root, &["checkout", "--detach", &f.code]).unwrap();
                }
                "source" | "instructions" => {
                    fs::write(
                        f.root.join(if kind == "source" {
                            "code"
                        } else {
                            "openwiki/INSTRUCTIONS.md"
                        }),
                        "later edit",
                    )
                    .unwrap();
                    raw_git(
                        &f.root,
                        &[
                            "add",
                            if kind == "source" {
                                "code"
                            } else {
                                "openwiki/INSTRUCTIONS.md"
                            },
                        ],
                    )
                    .unwrap();
                    raw_git(&f.root, &["commit", "-m", "unexpected source"]).unwrap();
                }
                "workflow" => {
                    fs::create_dir_all(f.root.join(".github/workflows")).unwrap();
                    fs::write(
                        f.root.join(".github/workflows/openwiki-update.yml"),
                        "unknown workflow",
                    )
                    .unwrap();
                }
                _ => fs::write(f.root.join("unexpected"), "keep").unwrap(),
            }
            let head = super::super::super::git_text(&f.root, &["rev-parse", "HEAD"]).unwrap();
            let bytes = fs::read(f.root.join("AGENTS.md")).unwrap();
            assert!(
                restore(
                    &f.store,
                    &f.id(),
                    if kind == "head" { &wiki } else { &head }
                )
                .is_err(),
                "{kind}"
            );
            assert_eq!(fs::read(f.root.join("AGENTS.md")).unwrap(), bytes);
            assert_eq!(
                f.store
                    .integration_wiki_setup(f.owner)
                    .unwrap()
                    .unwrap()
                    .checkpoint
                    .phase,
                WikiSetupPhase::Prepared
            );
        }
    }

    #[test]
    fn integration_reopens_restored_writer_phase_without_recapturing_originals() {
        let f = Fixture::new();
        f.prepare();
        f.generate();
        let first = f.wiki_commit();
        restore(&f.store, &f.id(), &first).unwrap();
        let original = f.store.integration_wiki_setup(f.owner).unwrap().unwrap();
        f.prepare();
        let reopened = f.store.integration_wiki_setup(f.owner).unwrap().unwrap();
        assert_eq!(reopened.previous_final_commits, vec![first.clone()]);
        assert!(reopened.final_commit.is_none());
        assert_eq!(reopened.checkpoint.phase, WikiSetupPhase::Prepared);
        assert_eq!(
            original_digest(&original).unwrap(),
            original_digest(&reopened).unwrap()
        );
        assert!(
            reopened
                .checkpoint
                .files
                .iter()
                .all(|entry| entry.restoring_from.is_none())
        );
        f.prepare(); // crash/retry of the reopening does not duplicate history
        assert_eq!(
            reopened,
            f.store.integration_wiki_setup(f.owner).unwrap().unwrap()
        );
        f.generate();
        fs::write(f.root.join("openwiki/reopened.md"), "continued Wiki").unwrap();
        let second = f.wiki_commit();
        assert_ne!(first, second);
        restore(&f.store, &f.id(), &second).unwrap();
        let finished = f.store.integration_wiki_setup(f.owner).unwrap().unwrap();
        assert_eq!(finished.previous_final_commits, vec![first]);
        assert_eq!(finished.final_commit.as_deref(), Some(second.as_str()));
        assert_eq!(
            original_digest(&original).unwrap(),
            original_digest(&finished).unwrap()
        );
        git::GitService::new()
            .require_clean_source(&f.root, &second)
            .unwrap();
        assert!(f.root.join("openwiki/reopened.md").is_file());
    }

    #[test]
    fn integration_prepare_refuses_crash_after_final_sha_pin_before_restoring() {
        let f = Fixture::new();
        f.prepare();
        f.generate();
        let first = f.wiki_commit();
        let mut pinned = validate(&f.store, &f.id(), &first).unwrap();
        let origin = original_digest(&pinned).unwrap();
        // Exact first durable save in restore(), before restore_checkpoint saves
        // Restoring or records replacement bytes. No instruction was replaced.
        pinned.final_commit = Some(first.clone());
        f.store.save_integration_wiki_setup(&pinned).unwrap();
        assert_eq!(pinned.checkpoint.phase, WikiSetupPhase::Prepared);
        let generated = fs::read(f.root.join("AGENTS.md")).unwrap();
        for _ in 0..2 {
            let error = prepare(&f.store, &f.id()).unwrap_err();
            assert!(error.to_string().contains("republish the same final SHA"));
            assert_eq!(
                pinned,
                f.store.integration_wiki_setup(f.owner).unwrap().unwrap()
            );
            assert_eq!(generated, fs::read(f.root.join("AGENTS.md")).unwrap());
        }
        // The original final SHA can finish restoration; only then does an
        // explicit prepare admit begin and a new Wiki commit, retaining origin.
        restore(&f.store, &f.id(), &first).unwrap();
        git::GitService::new()
            .require_clean_source(&f.root, &first)
            .unwrap();
        f.prepare();
        let reopened = f.store.integration_wiki_setup(f.owner).unwrap().unwrap();
        assert_eq!(origin, original_digest(&reopened).unwrap());
        assert_eq!(reopened.previous_final_commits, vec![first]);
        assert!(reopened.final_commit.is_none());
        f.generate();
        fs::write(f.root.join("openwiki/after-pin-crash.md"), "continued Wiki").unwrap();
        let second = f.wiki_commit();
        restore(&f.store, &f.id(), &second).unwrap();
        git::GitService::new()
            .require_clean_source(&f.root, &second)
            .unwrap();
        assert_eq!(
            origin,
            original_digest(&f.store.integration_wiki_setup(f.owner).unwrap().unwrap()).unwrap()
        );
        assert!(f.root.join("openwiki/after-pin-crash.md").is_file());
    }

    #[test]
    fn integration_partial_restoration_pins_final_sha_and_preserves_later_edits() {
        for edit in [false, true] {
            let f = Fixture::new();
            f.prepare();
            f.generate();
            let wiki = f.wiki_commit();
            let mut journal = validate(&f.store, &f.id(), &wiki).unwrap();
            journal.final_commit = Some(wiki.clone());
            journal.checkpoint.phase = WikiSetupPhase::Restoring;
            for entry in &mut journal.checkpoint.files {
                entry.restoring_from = Some(read_entry(&f.root.join(&entry.path)).unwrap());
            }
            f.store.save_integration_wiki_setup(&journal).unwrap();
            assert!(prepare(&f.store, &f.id()).is_err()); // no false begin permission mid-restore
            assert_eq!(
                journal,
                f.store.integration_wiki_setup(f.owner).unwrap().unwrap()
            );
            let entry = &journal.checkpoint.files[0];
            replace_if_expected(
                &f.root.join(&entry.path),
                entry.restoring_from.as_ref().unwrap(),
                &entry.original,
            )
            .unwrap();
            if edit {
                fs::write(f.root.join("AGENTS.md"), "later edit").unwrap();
            }
            let result = restore(&f.store, &f.id(), &wiki);
            if edit {
                assert!(result.is_err());
                assert_eq!(
                    fs::read_to_string(f.root.join("AGENTS.md")).unwrap(),
                    "later edit"
                );
            } else {
                result.unwrap();
                git::GitService::new()
                    .require_clean_source(&f.root, &wiki)
                    .unwrap();
                fs::write(f.root.join("openwiki/later.md"), "later").unwrap();
                raw_git(&f.root, &["add", "openwiki"]).unwrap();
                raw_git(&f.root, &["commit", "-m", "later Wiki"]).unwrap();
                let later = super::super::super::git_text(&f.root, &["rev-parse", "HEAD"]).unwrap();
                assert!(restore(&f.store, &f.id(), &later).is_err());
            }
        }
    }

    #[cfg(unix)]
    #[test]
    fn integration_rejects_instruction_symlink_and_hardlink_before_and_after_begin() {
        for kind in ["symlink", "hardlink"] {
            for after in [false, true] {
                let f = Fixture::new();
                if after {
                    f.prepare();
                    f.generate();
                }
                let outside = f._temp.path().join("outside");
                fs::write(&outside, "outside bytes").unwrap();
                fs::remove_file(f.root.join("CLAUDE.md")).unwrap();
                if kind == "symlink" {
                    std::os::unix::fs::symlink(&outside, f.root.join("CLAUDE.md")).unwrap();
                } else {
                    fs::hard_link(&outside, f.root.join("CLAUDE.md")).unwrap();
                }
                assert!(
                    if after {
                        restore(&f.store, &f.id(), &f.code)
                    } else {
                        prepare(&f.store, &f.id())
                    }
                    .is_err()
                );
                assert_eq!(fs::read_to_string(outside).unwrap(), "outside bytes");
            }
        }
    }
}
