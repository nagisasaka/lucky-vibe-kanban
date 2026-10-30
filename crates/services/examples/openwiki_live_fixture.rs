//! Opt-in real-model maintenance fixture. No DB or shared runtime is used.
//! Caller supplies disposable root, evidence directory and the test-only wire tee.
use std::{collections::HashSet, io::BufRead, path::PathBuf, time::Duration};

use anyhow::{Context, ensure};
use executors::{
    env::{ExecutionEnv, RepoContext},
    executors::{ExecutorExitResult, StandardCodingAgentExecutor, codex::Codex},
    runtime::{
        NativeAuditChannel, NativeAuditDirection, NativeAuditMetadata, NativeAuditReader,
        NativeAuditWriter,
    },
};
use serde_json::{Value, json};
use services::services::openwiki::{
    OpenWikiAdapter,
    completion::{PhaseCompletionProof, WriterPhase},
    publication_paths, setup,
};
use utils::repository_memory::RepositoryMemoryStore;
use uuid::Uuid;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    ensure!(
        args.len() == 3,
        "Expected disposable root, evidence directory, wire proxy path"
    );
    let root = PathBuf::from(&args[0]).canonicalize()?;
    let evidence = PathBuf::from(&args[1]).canonicalize()?;
    ensure!(
        root.starts_with(std::env::temp_dir().canonicalize()?)
            && evidence.starts_with(std::env::temp_dir().canonicalize()?),
        "Fixture paths must be temporary"
    );
    let replay = std::env::var_os("LVK_OPENWIKI_FIXTURE_REPLAY").is_some();
    let instructions = root.join("openwiki/INSTRUCTIONS.md");
    if !replay && !instructions.exists() {
        std::fs::write(
            &instructions,
            include_bytes!("../../../assets/openwiki-instructions.md"),
        )?;
    }
    let source = std::process::Command::new("git")
        .arg("-C")
        .arg(&root)
        .args(["rev-parse", "HEAD"])
        .output()?;
    ensure!(source.status.success(), "Fixture source missing");
    let source = String::from_utf8(source.stdout)?.trim().to_owned();
    std::fs::create_dir_all(evidence.join("memory"))?;
    let store = RepositoryMemoryStore::at_persistent(&evidence.join("memory"))?;
    let workspace = Uuid::new_v4();
    if !replay {
        setup::prepare(&store, workspace, &root, &source)?;
        OpenWikiAdapter::default()
            .prepare_codex(&root, true)
            .await?;
    }
    let codex: Codex = serde_json::from_value(
        json!({"sandbox":"danger-full-access","ask_for_approval":"never","model_reasoning_effort":"low", "base_command_override":format!("python3 {}",args[2])}),
    )?;
    let session = Uuid::new_v4();
    let mut thread = None;
    let mut proof = PhaseCompletionProof::new(WriterPhase::Generate);
    let mut seen = HashSet::new();
    let mut owned_run: Option<String> = None;
    let mut current_wiki = String::new();
    let mut workspaces = HashSet::new();
    let mut linked_wikis = HashSet::new();
    let mut linked_read = false;
    let mut linked_searches = HashSet::new();
    for number in 1..=2 {
        let wire = evidence.join(format!("wire-{number}.jsonl"));
        let mut env = ExecutionEnv::new(RepoContext::default(), false, String::new());
        env.insert("LVK_OPENWIKI_MAINTENANCE", "1");
        env.insert("LVK_FIXTURE_WIRE", wire.to_str().unwrap());
        let prompt = if number == 1 {
            format!(
                "This is an authorised disposable OpenWiki maintenance fixture. Use registered OpenWiki MCP, not a custom bridge. First call openwiki_list_workspaces(root={root:?}); choose its workspace and call openwiki_list_wikis. Search for UniqueLinkedToken and read the relevant linked section, clearly treating it only as reference. Then call openwiki_begin(root={root:?},mode=init,language=en). Stop the turn immediately after the successful begin, leaving this exact run active for a deliberate next-AgentRun resume. Do not write files, plan, finish, commit or update linked Wikis."
            )
        } else {
            format!(
                "Continue this authorised disposable maintenance fixture. Call openwiki_begin(root={root:?},mode=init,language=en) to resume the SAME active run. Use the returned runId. Submit a plan containing only /openwiki/quickstart.md, title Counter, purpose Explain add, seedPaths [index.js]. Request next page, inspect its claims, write that single Markdown page with type concept, title Counter, description Counter fixture, tags [fixture], a heading and the statement The add function returns the sum of a and b. Submit that claim with evidence repo://index.js#L1. Request next page then finish. Finally search local add function and read the matching section. Do not change source/config/INSTRUCTIONS or any linked Wiki. Do not commit. Only report success after finish complete with no sourceChanged=true."
            )
        };
        if !replay {
            let mut child = if let Some(id) = thread.as_deref() {
                codex
                    .spawn_follow_up(&root, &prompt, id, None, &env)
                    .await?
            } else {
                codex.spawn(&root, &prompt, &env).await?
            };
            let mut stdout = child.child.inner().stdout.take().context("stdout")?;
            let mut stderr = child.child.inner().stderr.take().context("stderr")?;
            let output =
                tokio::spawn(
                    async move { tokio::io::copy(&mut stdout, &mut tokio::io::sink()).await },
                );
            let error_path = evidence.join(format!("stderr-{number}.log"));
            let errors = tokio::spawn(async move {
                let mut file = tokio::fs::File::create(error_path).await?;
                tokio::io::copy(&mut stderr, &mut file).await
            });
            let result = tokio::time::timeout(
                Duration::from_secs(600),
                child.exit_signal.take().context("exit signal")?,
            )
            .await;
            if let Some(cancel) = child.cancel.take() {
                cancel.cancel();
            }
            child.control.take();
            child.child.kill().await.ok();
            child.child.wait().await?;
            drop(child);
            // The adapter can retain its log-pipe writer until its task unwinds.
            // All provider processes are settled; do not wait indefinitely on it.
            let mut output = output;
            let mut errors = errors;
            if tokio::time::timeout(Duration::from_secs(2), &mut output)
                .await
                .is_err()
            {
                output.abort();
            }
            if tokio::time::timeout(Duration::from_secs(2), &mut errors)
                .await
                .is_err()
            {
                errors.abort();
            }
            ensure!(
                matches!(result, Ok(Ok(ExecutorExitResult::Success))),
                "Real maintenance failed/timed out; evidence retained: {result:?}"
            );
        }
        let mut begins = 0;
        let mut writer = NativeAuditWriter::create_in(
            &evidence,
            NativeAuditMetadata {
                session_id: session,
                agent_run_id: Uuid::new_v4(),
                turn_id: Uuid::new_v4(),
                run_attempt_id: Uuid::new_v4(),
                run_attempt_number: 1,
                provider_id: "codex".into(),
                runtime_profile_id: "real-openwiki-fixture".into(),
                workspace_path: root.to_string_lossy().into(),
                runtime_version: None,
                protocol_version: None,
                adapter_version: "fixture".into(),
                mapper_version: "fixture".into(),
                created_at: chrono::Utc::now(),
            },
        )?;
        for line in std::io::BufReader::new(std::fs::File::open(wire)?).lines() {
            let line = line?;
            let value: Value = serde_json::from_str(&line)?;
            if let Some(id) = value.pointer("/result/thread/id").and_then(Value::as_str) {
                thread = Some(id.to_owned());
            }
            if value["method"] == "item/completed"
                && value["params"]["item"]["server"] == "openwiki"
            {
                let item = &value["params"]["item"];
                if item["status"] == "completed" && item["result"]["isError"] != true {
                    let tool = item["tool"].as_str().unwrap_or_default();
                    seen.insert(tool.to_owned());
                    let data = &item["result"]["structuredContent"];
                    match tool {
                        "openwiki_begin" => {
                            begins += 1;
                            ensure!(data["status"] == "active", "Expected active begin");
                            let id = data["runId"].as_str().context("Live begin runId")?;
                            if number == 1 {
                                ensure!(
                                    owned_run.is_none() && data["resumed"] == false,
                                    "First turn must own a fresh run"
                                );
                                owned_run = Some(id.to_owned());
                            } else {
                                ensure!(
                                    owned_run.as_deref() == Some(id) && data["resumed"] == true,
                                    "Second turn must resume the first run"
                                );
                            }
                        }
                        "openwiki_list_workspaces" => {
                            current_wiki = data["wiki"]["id"]
                                .as_str()
                                .context("Current wiki ID")?
                                .to_owned();
                            for w in data["workspaces"].as_array().context("Workspace list")? {
                                workspaces
                                    .insert(w["id"].as_str().context("Workspace ID")?.to_owned());
                            }
                        }
                        "openwiki_list_wikis" => {
                            let selected = item["arguments"]["workspace"]
                                .as_str()
                                .context("Selected workspace")?;
                            ensure!(
                                workspaces.contains(selected),
                                "Selected workspace must have been listed"
                            );
                            for w in data["wikis"].as_array().context("Wiki list")? {
                                let id = w["id"].as_str().context("Linked wiki ID")?;
                                if id != current_wiki {
                                    linked_wikis.insert(id.to_owned());
                                }
                            }
                        }
                        "openwiki_search" => {
                            for hit in data["results"].as_array().into_iter().flatten() {
                                if let Some(wiki) = hit["wiki"].as_str()
                                    && linked_wikis.contains(wiki)
                                    && hit["content"]
                                        .as_str()
                                        .is_some_and(|text| text.contains("UniqueLinkedToken"))
                                {
                                    linked_searches.insert(wiki.to_owned());
                                }
                            }
                        }
                        "openwiki_read" => {
                            if let Some(wiki) = item["arguments"]["wiki"].as_str()
                                && linked_wikis.contains(wiki)
                                && linked_searches.contains(wiki)
                                && data["wiki"] == wiki
                                && data["sections"].as_array().is_some_and(|sections| {
                                    sections.iter().any(|section| {
                                        section["content"]
                                            .as_str()
                                            .is_some_and(|text| text.contains("UniqueLinkedToken"))
                                    })
                                })
                            {
                                linked_read = true;
                            }
                        }
                        _ => {}
                    }
                }
            }
            writer.append_bytes(
                NativeAuditDirection::Output,
                NativeAuditChannel::Stdout,
                "application/json",
                Uuid::new_v4(),
                line.as_bytes(),
                None,
            )?;
        }
        ensure!(
            begins == 1,
            "Each fixture turn must have exactly one successful begin"
        );
        let manifest = writer.close()?;
        let audit = NativeAuditReader::read(
            evidence
                .join(manifest.manifest_relative_path)
                .parent()
                .unwrap(),
        )?;
        proof.replay_continuation(&audit, &root)?;
        if number == 1 {
            ensure!(
                proof.validate().is_err(),
                "First AgentRun must leave operation active"
            );
        }
    }
    proof.validate()?;
    ensure!(
        linked_read,
        "Missing successful linked Wiki read containing UniqueLinkedToken"
    );
    for tool in [
        "openwiki_search",
        "openwiki_read",
        "openwiki_list_workspaces",
        "openwiki_list_wikis",
        "openwiki_inspect_page_claims",
    ] {
        ensure!(seen.contains(tool), "Missing successful real call: {tool}");
    }
    if replay {
        println!(
            "PASS: replayed existing real wire; owned resume and linked search/read assertions. No new model execution or setup/publication check."
        );
        return Ok(());
    }
    {
        setup::validate_provenance(&store, workspace, &root, &source)
            .context("Live fixture provenance")?;
        setup::restore(&store, workspace, &root, &source)
            .context("Live fixture instruction restoration")?;
        let paths = publication_paths(&git::GitService::new(), &root, &source)
            .context("Live fixture publication paths")?;
        ensure!(
            paths.iter().all(|p| p.starts_with("openwiki/")),
            "Non-Wiki publication"
        );
    }
    println!(
        "PASS: real Codex maintenance, four retrieval tools, two AgentRuns with owned resume, Native Audit completion, restored setup and Wiki-only publication paths; no publication performed. Source={source}"
    );
    Ok(())
}
