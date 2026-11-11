//! Opt-in contract test against the installed upstream package, without a provider,
//! MCP operation, network call or any non-fixture repository writes.
use std::process::Command;

use super::*;

fn git(root: &Path, args: &[&str]) -> String {
    let result = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    String::from_utf8(result.stdout).unwrap().trim().to_owned()
}

#[test]
#[ignore = "requires the real locally installed OpenWiki 0.9.0 and Node.js"]
fn installed_openwiki_090_setup_matches_strict_integration_validator() {
    let package_root = std::env::var_os("OPENWIKI_TEST_PACKAGE_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/usr/local/lib/node_modules/openwiki"));
    let package: serde_json::Value = serde_json::from_slice(
        &fs::read(package_root.join("package.json")).expect("installed OpenWiki package"),
    )
    .unwrap();
    assert_eq!(package["name"], "openwiki");
    assert_eq!(
        package["version"], "0.9.0",
        "validator is intentionally version pinned"
    );
    let script = r#"
import { pathToFileURL } from 'node:url';
const { ensureCodeModeRepoSetup } = await import(pathToFileURL(process.argv[1]));
await ensureCodeModeRepoSetup(process.argv[2], {createWorkflow: false});
"#;
    let block = "<!-- OPENWIKI:START -->\r\nold generated instructions\r\n<!-- OPENWIKI:END -->";
    let existing = format!("# User prefix\r\n\r\n{block}\r\n\r\nUser suffix.  \r\n");
    let canonical = format!("{}\n", include_str!("agents-0.9.0.txt"));
    let cases = [
        ("missing", None, None),
        (
            "crlf",
            Some("# User rules\r\nKeep trailing spaces.  \r\n\r\n"),
            Some("# Claude rules\r\n"),
        ),
        (
            "existing-block-crlf",
            Some(existing.as_str()),
            Some(existing.as_str()),
        ),
        (
            "claude-import",
            Some("# Rules\n"),
            Some(" \r\n@AGENTS.md\r\n"),
        ),
        ("noop", Some(canonical.as_str()), Some("@AGENTS.md\n")),
    ];
    for (case, agents, claude) in cases {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("repo");
        fs::create_dir_all(root.join("openwiki")).unwrap();
        fs::create_dir(temp.path().join("persistent")).unwrap();
        fs::write(root.join("code"), "fixture source\n").unwrap();
        fs::write(root.join("openwiki/INSTRUCTIONS.md"), "user owned\r\n").unwrap();
        for (path, original) in [("AGENTS.md", agents), ("CLAUDE.md", claude)] {
            if let Some(original) = original {
                fs::write(root.join(path), original).unwrap();
            }
        }
        git(&root, &["init", "-b", "main"]);
        git(&root, &["config", "user.name", "Fixture"]);
        git(&root, &["config", "user.email", "fixture@example.invalid"]);
        git(&root, &["add", "."]);
        git(&root, &["commit", "-m", "code fixture"]);
        let code = git(&root, &["rev-parse", "HEAD"]);
        let storage = git::GitService::new()
            .storage_identity(&root)
            .unwrap()
            .to_string_lossy()
            .into_owned();
        let store = RepositoryMemoryStore::at_persistent(&temp.path().join("persistent")).unwrap();
        let id = integration::Identity {
            integration_id: Uuid::new_v4(),
            repository_id: Uuid::new_v4(),
            session_id: Uuid::new_v4(),
            workspace_id: Uuid::new_v4(),
            root: &root,
            storage_identity: &storage,
            code_commit: &code,
        };
        integration::prepare(&store, &id).unwrap();
        let original_digest = integration::original_digest(
            &store
                .integration_wiki_setup(id.integration_id)
                .unwrap()
                .unwrap(),
        )
        .unwrap();
        let mut first = None;
        for _ in 0..2 {
            let result = Command::new("node")
                .args(["--input-type=module", "--eval", script])
                .arg(package_root.join("dist/ingestion/code-mode.js"))
                .arg(&root)
                .output()
                .unwrap();
            assert!(
                result.status.success(),
                "{case}: {}",
                String::from_utf8_lossy(&result.stderr)
            );
            integration::validate(&store, &id, &code).unwrap_or_else(|e| panic!("{case}: {e:#}"));
            let generated = INSTRUCTION_PATHS.map(|path| fs::read(root.join(path)).ok());
            if let Some(first) = &first {
                assert_eq!(&generated, first, "repeated setup: {case}");
            } else {
                first = Some(generated);
            }
        }
        if case == "noop" {
            assert!(git(&root, &["status", "--porcelain"]).is_empty());
        }
        fs::write(root.join("openwiki/page.md"), "fixture wiki\n").unwrap();
        git(&root, &["add", "openwiki/page.md"]);
        git(&root, &["commit", "-m", "wiki fixture"]);
        let final_commit = git(&root, &["rev-parse", "HEAD"]);
        integration::restore(&store, &id, &final_commit).unwrap();
        for (path, original) in [("AGENTS.md", agents), ("CLAUDE.md", claude)] {
            assert_eq!(
                fs::read(root.join(path)).ok(),
                original.map(|s| s.as_bytes().to_vec()),
                "{case}: {path}"
            );
        }
        assert!(git(&root, &["status", "--porcelain"]).is_empty(), "{case}");
        assert_eq!(
            fs::read(root.join("openwiki/INSTRUCTIONS.md")).unwrap(),
            b"user owned\r\n"
        );
        assert_eq!(
            fs::read(root.join("openwiki/page.md")).unwrap(),
            b"fixture wiki\n"
        );
        // A later Run may legitimately begin again after restoration, e.g. a
        // target-side guard held publication. It keeps the authenticated origin.
        integration::prepare(&store, &id).unwrap();
        let reopened = store
            .integration_wiki_setup(id.integration_id)
            .unwrap()
            .unwrap();
        assert_eq!(
            integration::original_digest(&reopened).unwrap(),
            original_digest
        );
        assert_eq!(reopened.previous_final_commits, vec![final_commit.clone()]);
        assert_eq!(reopened.checkpoint.phase, WikiSetupPhase::Prepared);
        assert!(reopened.final_commit.is_none());
        let result = Command::new("node")
            .args(["--input-type=module", "--eval", script])
            .arg(package_root.join("dist/ingestion/code-mode.js"))
            .arg(&root)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "reopened {case}: {}",
            String::from_utf8_lossy(&result.stderr)
        );
        integration::validate(&store, &id, &final_commit).unwrap();
        fs::write(root.join("openwiki/second.md"), "later fixture wiki\n").unwrap();
        git(&root, &["add", "openwiki/second.md"]);
        git(&root, &["commit", "-m", "second wiki fixture"]);
        let second_commit = git(&root, &["rev-parse", "HEAD"]);
        integration::restore(&store, &id, &second_commit).unwrap();
        integration::restore(&store, &id, &second_commit).unwrap();
        let restored = store
            .integration_wiki_setup(id.integration_id)
            .unwrap()
            .unwrap();
        assert_eq!(
            integration::original_digest(&restored).unwrap(),
            original_digest
        );
        assert_eq!(restored.previous_final_commits, vec![final_commit]);
        assert_eq!(
            restored.final_commit.as_deref(),
            Some(second_commit.as_str())
        );
        assert!(
            git(&root, &["status", "--porcelain"]).is_empty(),
            "reopened {case}"
        );
        for (path, original) in [("AGENTS.md", agents), ("CLAUDE.md", claude)] {
            assert_eq!(
                fs::read(root.join(path)).ok(),
                original.map(|s| s.as_bytes().to_vec()),
                "reopened {case}: {path}"
            );
        }
        assert_eq!(
            fs::read(root.join("openwiki/page.md")).unwrap(),
            b"fixture wiki\n"
        );
        assert_eq!(
            fs::read(root.join("openwiki/second.md")).unwrap(),
            b"later fixture wiki\n"
        );
        assert_eq!(
            fs::read(root.join("openwiki/INSTRUCTIONS.md")).unwrap(),
            b"user owned\r\n"
        );
    }
}

#[test]
#[ignore = "requires installed OpenWiki 0.9.0 and Node.js; provider-free admission fixture only"]
fn installed_openwiki_090_forced_update_accepts_clean_reconstruction_target() {
    let package = std::env::var_os("OPENWIKI_TEST_PACKAGE_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/usr/local/lib/node_modules/openwiki"));
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("repo");
    fs::create_dir_all(root.join("openwiki")).unwrap();
    git(&root, &["init", "-b", "main"]);
    git(&root, &["config", "user.name", "Fixture"]);
    git(&root, &["config", "user.email", "fixture@example.invalid"]);
    fs::write(root.join("source.rs"), "// Current source\n").unwrap();
    fs::write(root.join(".openwikiignore"), "private/\n").unwrap();
    fs::write(root.join("openwiki/INSTRUCTIONS.md"), "User instructions\n").unwrap();
    fs::write(
        root.join("openwiki/unmarked.md"),
        "Markerless generated documentation\n",
    )
    .unwrap();
    fs::write(root.join("openwiki/index.md"), "Old index\n").unwrap();
    fs::write(
        root.join("openwiki/old.md"),
        "---\ngenerated: { by: codex }\n---\nOld knowledge\n",
    )
    .unwrap();
    git(&root, &["add", "."]);
    git(&root, &["commit", "-m", "fixture source"]);
    let source = git(&root, &["rev-parse", "HEAD"]);
    crate::services::openwiki::management::remove_generated(&root, &source).unwrap();
    let script = r#"
import {pathToFileURL} from 'node:url';
import {readFile} from 'node:fs/promises';
const pkg = JSON.parse(await readFile(`${process.argv[1]}/package.json`, 'utf8'));
if (pkg.version !== '0.9.0') throw Error('Pinned OpenWiki version mismatch');
const {beginRepositoryRun} = await import(pathToFileURL(`${process.argv[1]}/dist/generation/repository-run.js`));
const result = await beginRepositoryRun({root:process.argv[2], mode:'update', force:true, language:'en', actor:{producerActor:'codex',metadataModel:'codex'}});
process.stdout.write(JSON.stringify({status:result.view.status}));
"#;
    // No provider, writer process or publication is started. The temporary
    // target and its admitted run state are discarded together after this test.
    let output = Command::new("node")
        .args(["--input-type=module", "-e", script])
        .arg(package)
        .arg(&root)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let result: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(result["status"], "active");
    assert_eq!(
        fs::read_to_string(root.join("openwiki/INSTRUCTIONS.md")).unwrap(),
        "User instructions\n"
    );
    assert!(!root.join("openwiki/unmarked.md").exists());
    assert!(!root.join("openwiki/old.md").exists());
    assert!(git(&root, &["show", &format!("{source}:openwiki/old.md")]).contains("Old knowledge"));
}
