//! On-demand bounded queries. Reuses confined Git reference reads and content revisions.
use std::time::{Duration, Instant};

use serde::Serialize;

use super::*;

const MAX_OUTPUT_BYTES: usize = 64 * 1024;
const MAX_SCAN_BYTES: usize = 16 * 1024 * 1024;
const MAX_FILES: usize = 1000;
const MAX_DURATION: Duration = Duration::from_secs(2);

#[derive(Default, Deserialize, Serialize, JsonSchema)]
pub(super) struct PathFilter {
    /// Repository-relative directory, without leading/trailing slash. Matches descendants only.
    pub directory_prefix: Option<String>,
    /// Case-sensitive literal substring of the basename, not the full path.
    pub filename_contains: Option<String>,
}
impl PathFilter {
    pub(super) fn validate(&self) -> anyhow::Result<()> {
        ensure!(
            self.directory_prefix
                .as_ref()
                .is_none_or(|s| allowed_path(s)),
            "Invalid directory_prefix"
        );
        ensure!(
            self.filename_contains
                .as_ref()
                .is_none_or(|s| !s.is_empty() && s.len() <= 255 && !s.contains(['/', '\\', '\0'])),
            "Invalid filename_contains"
        );
        Ok(())
    }
    pub(super) fn is_active(&self) -> bool {
        self.directory_prefix.is_some() || self.filename_contains.is_some()
    }
    pub(super) fn matches(&self, path: &str) -> bool {
        self.directory_prefix.as_ref().is_none_or(|prefix| {
            path.strip_prefix(prefix)
                .is_some_and(|rest| rest.starts_with('/'))
        }) && self
            .filename_contains
            .as_ref()
            .is_none_or(|name| path.rsplit('/').next().unwrap_or(path).contains(name))
    }
}
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct FileListArgs {
    #[serde(flatten)]
    target: Target,
    #[serde(default)]
    offset: u32,
    #[serde(default = "super::super::default_limit")]
    #[schemars(range(min = 1, max = 50))]
    limit: u32,
    /// Required after the first page; repeat both filters on continuation.
    expected_revision: Option<String>,
    #[serde(flatten)]
    filter: PathFilter,
}
impl FileListArgs {
    // Do not flatten ListArgs: its deny_unknown_fields would reject the filters.
    pub(super) fn into_parts(self) -> (ListArgs, PathFilter) {
        (
            ListArgs {
                target: self.target,
                offset: self.offset,
                limit: self.limit,
                expected_revision: self.expected_revision,
            },
            self.filter,
        )
    }
}
fn default_matches() -> usize {
    20
}
fn default_context() -> usize {
    1
}
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct SearchArgs {
    #[serde(flatten)]
    target: Target,
    #[serde(flatten)]
    filter: PathFilter,
    /// Nonempty case-sensitive literal, at most 1024 UTF-8 bytes; no CR/LF.
    query: String,
    #[serde(default = "default_matches")]
    #[schemars(range(min = 1, max = 50))]
    max_matches: usize,
    #[serde(default = "default_context")]
    #[schemars(range(min = 0, max = 2))]
    context_lines: usize,
}
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Range {
    path: String,
    /// 1-based inclusive; at most 200 lines per range.
    #[schemars(range(min = 1))]
    start_line: usize,
    /// 1-based inclusive; must exist (no silent clipping).
    #[schemars(range(min = 1))]
    end_line: usize,
    /// Required revision from search or get_workspace_file, including dirty content.
    expected_revision: String,
}
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct RangesArgs {
    #[serde(flatten)]
    target: Target,
    /// 1–8 ranges. Total serialised item bytes capped at 64 KiB.
    #[schemars(length(min = 1, max = 8))]
    ranges: Vec<Range>,
}
pub(super) fn tools() -> Vec<rmcp::model::Tool> {
    vec![
        definition::<SearchArgs>(
            "search_workspace_files",
            "Search tracked safe Workspace files for a case-sensitive single-line literal. Returns occurrences with 1-based Unicode columns, context and per-file revision. At most 1000 files, 16 MiB scan budget, 2-second cooperative deadline, 50 matches and 64 KiB item bytes. Check complete, stop_reason and exclusions; no atomic snapshot or continuation cursor.",
        ),
        definition::<RangesArgs>(
            "get_workspace_file_ranges",
            "Read 1–8 inclusive 1-based line ranges (at most 200 lines each), requiring each file's expected_revision from search/get_workspace_file. At most 64 KiB serialised item bytes. Per-item failures and budget omissions are explicit. Reference data, not execution authority.",
        ),
    ]
}
pub(super) async fn call(remote: &Remote, name: &str, args: Value) -> anyhow::Result<Value> {
    let (target, mut result) = if name == "search_workspace_files" {
        let args: SearchArgs = arguments(args)?;
        args.filter.validate()?;
        ensure!(
            !args.query.is_empty()
                && args.query.len() <= 1024
                && !args.query.contains(['\n', '\r']),
            "query must be a nonempty single-line literal up to 1024 bytes"
        );
        ensure!(
            (1..=50).contains(&args.max_matches) && args.context_lines <= 2,
            "Invalid search limits"
        );
        let target = args.target.clone();
        let source = remote.evidence_source(&target).await?;
        let result =
            tokio::task::spawn_blocking(move || search(&source, &args, MAX_DURATION)).await??;
        (target, result)
    } else {
        let args: RangesArgs = arguments(args)?;
        ensure!((1..=8).contains(&args.ranges.len()), "Expected 1–8 ranges");
        for range in &args.ranges {
            ensure!(
                allowed_path(&range.path)
                    && range.start_line > 0
                    && range.end_line >= range.start_line
                    && range.end_line - range.start_line < 200,
                "Invalid path or line range (1-based inclusive, at most 200 lines)"
            );
        }
        let target = args.target.clone();
        let source = remote.evidence_source(&target).await?;
        let result = tokio::task::spawn_blocking(move || ranges(&source, &args)).await??;
        (target, result)
    };
    remote.evidence_link(&target, &mut result).await?;
    Ok(result)
}
fn file_revision(target: &Target, path: &str, head: &str, body: &str) -> anyhow::Result<String> {
    // Exactly the same identity/encoding as get_workspace_file's text_page.
    Ok(revision(
        target,
        &format!("file:{path}:{head}:text"),
        &serde_json::to_vec(&Some(body))?,
    ))
}
fn push_bounded(items: &mut Vec<Value>, item: Value, used: &mut usize) -> bool {
    let size = item.to_string().len() + 1;
    if *used + size > MAX_OUTPUT_BYTES {
        return false;
    }
    *used += size;
    items.push(item);
    true
}
fn verify_head(source: &Source, root: &std::path::Path, head: &str) -> anyhow::Result<()> {
    ensure!(
        heads(source, root)?.1 == head,
        "Checkout changed during read; retry"
    );
    Ok(())
}
fn search(source: &Source, args: &SearchArgs, duration: Duration) -> anyhow::Result<Value> {
    let started = Instant::now();
    let root = source.root()?;
    let (git, head) = heads(source, &root)?;
    let paths: Vec<_> = git
        .reference_files(&root)
        .map_err(unavailable)?
        .into_iter()
        .filter(|p| args.filter.matches(p))
        .collect();
    let mut items = Vec::new();
    let mut exclusions = Vec::new();
    let (mut used, mut scanned, mut scan_bytes) = (0, 0, 0);
    let mut stop = None;
    'files: for path in &paths {
        if started.elapsed() >= duration {
            stop = Some("time_limit");
            break;
        }
        if scanned >= MAX_FILES {
            stop = Some("file_limit");
            break;
        }
        // Reserve the maximum confined read before opening; even binary/failed
        // reads cannot cause the budget to be exceeded.
        if scan_bytes + git::reference::MAX_REFERENCE_BYTES + 1 > MAX_SCAN_BYTES {
            stop = Some("scan_byte_limit");
            break;
        }
        scanned += 1;
        let content = git.reference_file(&root, path);
        let body = match content {
            Ok(Content::Text(body)) => {
                scan_bytes += body.len();
                body
            }
            other => {
                scan_bytes += git::reference::MAX_REFERENCE_BYTES + 1;
                let reason = match &other {
                    Ok(content) => status(content),
                    Err(_) => "unsafe_or_unreadable",
                };
                if !push_bounded(
                    &mut exclusions,
                    json!({"path":path,"reason":reason}),
                    &mut used,
                ) {
                    stop = Some("output_byte_limit");
                    break;
                }
                continue;
            }
        };
        let revision = file_revision(&args.target, path, &head, &body)?;
        let lines: Vec<_> = body.lines().collect();
        for (index, line) in lines.iter().enumerate() {
            if started.elapsed() >= duration {
                stop = Some("time_limit");
                break 'files;
            }
            for (byte, matched) in line.match_indices(&args.query) {
                if items.len() >= args.max_matches {
                    stop = Some("match_limit");
                    break 'files;
                }
                let start = index.saturating_sub(args.context_lines);
                let end = (index + args.context_lines + 1).min(lines.len());
                let column = line[..byte].chars().count() + 1;
                let item = json!({"path":path,"line":index+1,"column":column,"end_column_exclusive":column+matched.chars().count(),"matched":matched,"context_start_line":start+1,"context":lines[start..end],"revision":revision});
                if !push_bounded(&mut items, item, &mut used) {
                    stop = Some("output_byte_limit");
                    break 'files;
                }
            }
        }
    }
    verify_head(source, &root, &head)?;
    Ok(
        json!({"items":items,"exclusions":exclusions,"complete":stop.is_none() && exclusions.is_empty(),"stop_reason":stop,"candidate_files":paths.len(),"attempted_files":scanned,"scan_bytes_charged":scan_bytes,"item_bytes":used,"source_head":head,"snapshot":"per_file_observation","private_paths_excluded":true,"evidence":"reference_only"}),
    )
}
fn ranges(source: &Source, args: &RangesArgs) -> anyhow::Result<Value> {
    let root = source.root()?;
    let (git, head) = heads(source, &root)?;
    let mut items = Vec::new();
    let mut used = 0;
    let mut complete = true;
    let mut reserved_metadata: usize = args
        .ranges
        .iter()
        .map(|r| json!(r.path).to_string().len() + 512)
        .sum();
    ensure!(
        reserved_metadata <= MAX_OUTPUT_BYTES,
        "Batch metadata exceeds output limit"
    );
    for range in &args.ranges {
        reserved_metadata -= json!(range.path).to_string().len() + 512;
        let mut item =
            json!({"path":range.path,"start_line":range.start_line,"end_line":range.end_line});
        match git.reference_file(&root, &range.path) {
            Ok(Content::Text(body)) => {
                let current = file_revision(&args.target, &range.path, &head, &body)?;
                item["revision"] = json!(current);
                if current != range.expected_revision {
                    item["status"] = json!("revision_mismatch");
                } else {
                    let lines: Vec<_> = body.lines().collect();
                    if range.end_line > lines.len() {
                        item["status"] = json!("range_out_of_bounds");
                    } else {
                        item["status"] = json!("text");
                        item["lines"] = json!(lines[range.start_line - 1..range.end_line]);
                    }
                }
            }
            Ok(content) => item["status"] = json!(status(&content)),
            Err(_) => item["status"] = json!("unsafe_or_unreadable"),
        }
        if item["status"] != "text" {
            complete = false;
        }
        // Reserve room for the bounded metadata of all remaining failures.
        if used + item.to_string().len() + 1 > MAX_OUTPUT_BYTES - reserved_metadata {
            item.as_object_mut().unwrap().remove("lines");
            item["status"] = json!("output_byte_limit");
            complete = false;
        }
        ensure!(
            push_bounded(&mut items, item, &mut used),
            "Batch metadata exceeds output limit"
        );
    }
    verify_head(source, &root, &head)?;
    Ok(
        json!({"items":items,"complete":complete,"item_bytes":used,"source_head":head,"snapshot":"per_file_observation","evidence":"reference_only"}),
    )
}

#[cfg(all(test, unix))]
mod tests {
    use std::fs;

    use super::{super::tests::fixture, *};

    #[tokio::test]
    async fn filters_precede_paging_and_bind_revision_without_changing_defaults() {
        let (_temp, remote, args) = fixture().await;
        let original = super::super::call(&remote, "list_workspace_files", args.clone())
            .await
            .unwrap();
        assert_eq!(original["items"].as_array().unwrap().len(), 20);
        let mut filtered = args.clone();
        filtered["directory_prefix"] = json!("src");
        filtered["filename_contains"] = json!("00");
        filtered["limit"] = json!(2);
        let first = super::super::call(&remote, "list_workspace_files", filtered.clone())
            .await
            .unwrap();
        assert_eq!(
            first["items"],
            json!([{"path":"src/000.txt"},{"path":"src/001.txt"}])
        );
        filtered["offset"] = json!(2);
        filtered["expected_revision"] = first["revision"].clone();
        let second = super::super::call(&remote, "list_workspace_files", filtered.clone())
            .await
            .unwrap();
        assert_eq!(second["items"][0]["path"], "src/002.txt");
        filtered["filename_contains"] = json!("0");
        assert!(
            super::super::call(&remote, "list_workspace_files", filtered)
                .await
                .is_err()
        );
        let filter = PathFilter {
            directory_prefix: Some("src".into()),
            filename_contains: None,
        };
        assert!(!filter.matches("src2/a"));
        assert!(!filter.matches("src"));
        assert!(filter.matches("src/nested/a"));
        for prefix in ["../src", "/src", "src/", "src\\a", ".git"] {
            let mut bad = args.clone();
            bad["directory_prefix"] = json!(prefix);
            assert!(
                super::super::call(&remote, "list_workspace_files", bad)
                    .await
                    .is_err()
            );
        }
    }

    #[tokio::test]
    async fn literal_unicode_context_batch_and_dirty_revision() {
        let (temp, remote, args) = fixture().await;
        fs::write(
            temp.path().join("src/000.txt"),
            "前\r\n猫.*猫.*\r\n後\n末\n",
        )
        .unwrap();
        let mut search_args = args.clone();
        search_args["directory_prefix"] = json!("src");
        search_args["query"] = json!("猫.*");
        let found = call(&remote, "search_workspace_files", search_args.clone())
            .await
            .unwrap();
        assert_eq!(found["complete"], true);
        assert_eq!(found["items"].as_array().unwrap().len(), 2);
        assert_eq!(found["items"][0]["line"], 2);
        assert_eq!(found["items"][1]["column"], 4);
        assert_eq!(found["items"][1]["end_column_exclusive"], 7);
        assert_eq!(
            found["items"][0]["context"],
            json!(["前", "猫.*猫.*", "後"])
        );
        let mut detail = args.clone();
        detail["path"] = json!("src/000.txt");
        let full = super::super::call(&remote, "get_workspace_file", detail)
            .await
            .unwrap();
        assert_eq!(full["revision"], found["items"][0]["revision"]);
        let range = json!({"path":"src/000.txt","start_line":2,"end_line":3,"expected_revision":found["items"][0]["revision"]});
        let mut batch = args.clone();
        batch["ranges"] = json!([range.clone(), range.clone()]);
        let read = call(&remote, "get_workspace_file_ranges", batch.clone())
            .await
            .unwrap();
        assert_eq!(read["complete"], true);
        assert_eq!(read["items"][0]["lines"], json!(["猫.*猫.*", "後"]));
        batch["ranges"][1]["path"] = json!("src/001.txt");
        let partial = call(&remote, "get_workspace_file_ranges", batch.clone())
            .await
            .unwrap();
        assert_eq!(partial["complete"], false);
        assert_eq!(partial["items"][0]["status"], "text");
        assert_eq!(partial["items"][1]["status"], "revision_mismatch");
        batch["ranges"][0]["end_line"] = json!(5);
        assert_eq!(
            call(&remote, "get_workspace_file_ranges", batch.clone())
                .await
                .unwrap()["items"][0]["status"],
            "range_out_of_bounds"
        );
        batch["ranges"][0]["start_line"] = json!(0);
        assert!(
            call(&remote, "get_workspace_file_ranges", batch.clone())
                .await
                .is_err()
        );
        for (start, end) in [(1, 201), (3, 2)] {
            batch["ranges"] = json!([range.clone()]);
            batch["ranges"][0]["start_line"] = json!(start);
            batch["ranges"][0]["end_line"] = json!(end);
            assert!(
                call(&remote, "get_workspace_file_ranges", batch.clone())
                    .await
                    .is_err()
            );
        }
        batch["ranges"] = json!([]);
        assert!(
            call(&remote, "get_workspace_file_ranges", batch.clone())
                .await
                .is_err()
        );
        batch["ranges"] = json!(vec![range.clone(); 9]);
        assert!(
            call(&remote, "get_workspace_file_ranges", batch.clone())
                .await
                .is_err()
        );
        batch["ranges"] = json!([range]);
        fs::write(temp.path().join("src/000.txt"), "changed without commit\n").unwrap();
        let stale = call(&remote, "get_workspace_file_ranges", batch)
            .await
            .unwrap();
        assert_eq!(stale["complete"], false);
        assert_eq!(stale["items"][0]["status"], "revision_mismatch");
        assert!(stale["items"][0].get("lines").is_none());
        search_args["query"] = json!("BEFORE");
        let absent = call(&remote, "search_workspace_files", search_args.clone())
            .await
            .unwrap();
        assert_eq!(absent["complete"], true);
        assert!(absent["items"].as_array().unwrap().is_empty());
        search_args["max_matches"] = json!(1);
        search_args["query"] = json!("before");
        assert_eq!(
            call(&remote, "search_workspace_files", search_args.clone())
                .await
                .unwrap()["stop_reason"],
            "match_limit"
        );
        for (field, value) in [
            ("max_matches", json!(0)),
            ("max_matches", json!(51)),
            ("context_lines", json!(3)),
        ] {
            let mut invalid = search_args.clone();
            invalid[field] = value;
            assert!(
                call(&remote, "search_workspace_files", invalid)
                    .await
                    .is_err()
            );
        }
        for query in ["", "a\nb"] {
            search_args["query"] = json!(query);
            assert!(
                call(&remote, "search_workspace_files", search_args.clone())
                    .await
                    .is_err()
            );
        }
    }

    #[tokio::test]
    async fn exclusions_deadline_output_and_scan_limits_are_not_no_matches() {
        let (temp, remote, args) = fixture().await;
        let mut raw = args.clone();
        raw["query"] = json!("no match");
        let result = call(&remote, "search_workspace_files", raw.clone())
            .await
            .unwrap();
        assert_eq!(result["complete"], false);
        assert!(result["items"].as_array().unwrap().is_empty());
        let excluded = result["exclusions"].as_array().unwrap();
        assert!(excluded.iter().any(|v| v["reason"] == "binary"));
        assert!(excluded.iter().any(|v| v["reason"] == "too_large"));
        let parsed: SearchArgs = arguments(raw.clone()).unwrap();
        let source = remote.evidence_source(&parsed.target).await.unwrap();
        let timed = search(&source, &parsed, Duration::ZERO).unwrap();
        assert_eq!(timed["stop_reason"], "time_limit");
        assert_eq!(timed["complete"], false);
        fs::write(temp.path().join("src/000.txt"), "猫".repeat(30000)).unwrap();
        raw["query"] = json!("猫");
        raw["directory_prefix"] = json!("src");
        let large = call(&remote, "search_workspace_files", raw.clone())
            .await
            .unwrap();
        assert_eq!(large["stop_reason"], "output_byte_limit");
        assert!(large.to_string().len() < MAX_OUTPUT_BYTES + 2048);
        let body = "x".repeat(500_000);
        for i in 0..40 {
            fs::write(temp.path().join(format!("src/{i:03}.txt")), &body).unwrap();
        }
        raw["query"] = json!("absent");
        let parsed: SearchArgs = arguments(raw).unwrap();
        let limited = search(&source, &parsed, Duration::from_secs(60)).unwrap();
        assert_eq!(limited["stop_reason"], "scan_byte_limit");
        assert!(limited["scan_bytes_charged"].as_u64().unwrap() <= MAX_SCAN_BYTES as u64);
        let rev = file_revision(
            &parsed.target,
            "src/000.txt",
            limited["source_head"].as_str().unwrap(),
            &body,
        )
        .unwrap();
        let mut batch = args;
        batch["ranges"] =
            json!([{"path":"src/000.txt","start_line":1,"end_line":1,"expected_revision":rev}]);
        let limited = call(&remote, "get_workspace_file_ranges", batch)
            .await
            .unwrap();
        assert_eq!(limited["items"][0]["status"], "output_byte_limit");
        assert_eq!(limited["complete"], false);
        let body = "b".repeat(20_000);
        fs::write(temp.path().join("src/000.txt"), &body).unwrap();
        let current = file_revision(
            &parsed.target,
            "src/000.txt",
            limited["source_head"].as_str().unwrap(),
            &body,
        )
        .unwrap();
        let mut batch = json!({"project_id":parsed.target.project_id,"workspace_id":parsed.target.workspace_id,"repository_id":parsed.target.repository_id});
        batch["ranges"] = json!(vec![
            json!({"path":"src/000.txt","start_line":1,"end_line":1,"expected_revision":current});
            4
        ]);
        let cumulative = call(&remote, "get_workspace_file_ranges", batch)
            .await
            .unwrap();
        assert_eq!(cumulative["items"][0]["status"], "text");
        assert_eq!(cumulative["items"][3]["status"], "output_byte_limit");
        assert!(cumulative["item_bytes"].as_u64().unwrap() <= MAX_OUTPUT_BYTES as u64);
    }

    #[tokio::test]
    async fn query_boundaries_reject_foreign_targets_and_symlink_content() {
        let (temp, remote, args) = fixture().await;
        let mut search_args = args.clone();
        search_args["query"] = json!("private");
        let outside = tempfile::tempdir().unwrap();
        fs::write(outside.path().join("data"), "private").unwrap();
        fs::remove_file(temp.path().join("src/000.txt")).unwrap();
        std::os::unix::fs::symlink(outside.path().join("data"), temp.path().join("src/000.txt"))
            .unwrap();
        let result = call(&remote, "search_workspace_files", search_args.clone())
            .await
            .unwrap();
        assert!(result["items"].as_array().unwrap().is_empty());
        assert!(
            result["exclusions"]
                .as_array()
                .unwrap()
                .iter()
                .any(|i| i["path"] == "src/000.txt" && i["reason"] == "unsafe_or_unreadable")
        );
        let mut batch = args.clone();
        batch["ranges"] =
            json!([{"path":"src/000.txt","start_line":1,"end_line":1,"expected_revision":"stale"}]);
        assert_eq!(
            call(&remote, "get_workspace_file_ranges", batch.clone())
                .await
                .unwrap()["items"][0]["status"],
            "unsafe_or_unreadable"
        );
        for field in ["project_id", "workspace_id", "repository_id"] {
            for (name, mut input) in [
                ("search_workspace_files", search_args.clone()),
                ("get_workspace_file_ranges", batch.clone()),
            ] {
                input[field] = json!(uuid::Uuid::new_v4());
                assert!(call(&remote, name, input).await.is_err());
            }
        }
        for path in [
            "../outside",
            "/etc/passwd",
            ".env",
            "src/../a",
            "secret.key",
        ] {
            batch["ranges"][0]["path"] = json!(path);
            assert!(
                call(&remote, "get_workspace_file_ranges", batch.clone())
                    .await
                    .is_err()
            );
        }
    }
    #[tokio::test]
    async fn file_limit_and_small_fixture_transfer_reduction() {
        let (temp, remote, args) = fixture().await;
        let mut full_bytes = 0;
        for i in 0..3 {
            let path = format!("src/{i:03}.txt");
            fs::write(
                temp.path().join(&path),
                format!(
                    "{}needle\n{}",
                    "padding line\n".repeat(300),
                    "tail line\n".repeat(300)
                ),
            )
            .unwrap();
            let mut input = args.clone();
            input["path"] = json!(path);
            let result = super::super::call(&remote, "get_workspace_file", input)
                .await
                .unwrap();
            assert_eq!(result["has_more"], false);
            full_bytes += result.to_string().len();
        }
        let mut listing = args.clone();
        listing["limit"] = json!(50);
        loop {
            let page = super::super::call(&remote, "list_workspace_files", listing.clone())
                .await
                .unwrap();
            full_bytes += page.to_string().len();
            if page["has_more"] == false {
                break;
            }
            listing["offset"] = page["next_offset"].clone();
            listing["expected_revision"] = page["revision"].clone();
        }
        let mut raw = args.clone();
        raw["directory_prefix"] = json!("src");
        raw["query"] = json!("needle");
        let found = call(&remote, "search_workspace_files", raw.clone())
            .await
            .unwrap();
        assert_eq!(found["complete"], true);
        let mut batch = args.clone();
        batch["ranges"] = json!(found["items"].as_array().unwrap().iter().map(|m| json!({"path":m["path"],"start_line":m["line"],"end_line":m["line"],"expected_revision":m["revision"]})).collect::<Vec<_>>());
        let excerpts = call(&remote, "get_workspace_file_ranges", batch)
            .await
            .unwrap();
        assert_eq!(excerpts["complete"], true);
        let focused_bytes = found.to_string().len() + excerpts.to_string().len();
        assert!(focused_bytes * 3 < full_bytes);
        eprintln!(
            "fixture serialised response bytes: listing + 3 full files={full_bytes}, search + ranges={focused_bytes}"
        );
        for i in 105..1001 {
            fs::write(temp.path().join(format!("src/{i:04}.txt")), "tiny\n").unwrap();
        }
        let output = std::process::Command::new("git")
            .arg("-C")
            .arg(temp.path())
            .args(["add", "src"])
            .output()
            .unwrap();
        assert!(output.status.success());
        raw["query"] = json!("absent");
        let parsed: SearchArgs = arguments(raw).unwrap();
        let source = remote.evidence_source(&parsed.target).await.unwrap();
        let result = search(&source, &parsed, Duration::from_secs(60)).unwrap();
        assert_eq!(result["stop_reason"], "file_limit");
        assert_eq!(result["attempted_files"], 1000);
    }

    #[test]
    fn advertised_schema_has_filters_and_new_read_tools() {
        let tools = super::super::tools();
        for (name, fields) in [
            (
                "list_workspace_files",
                vec![
                    "directory_prefix",
                    "filename_contains",
                    "offset",
                    "expected_revision",
                ],
            ),
            (
                "search_workspace_files",
                vec!["query", "max_matches", "context_lines", "directory_prefix"],
            ),
            ("get_workspace_file_ranges", vec!["ranges"]),
        ] {
            let tool = tools.iter().find(|t| t.name == name).unwrap();
            let schema = serde_json::to_value(&tool.input_schema).unwrap();
            for field in fields {
                assert!(schema["properties"].get(field).is_some(), "{name}: {field}");
            }
        }
    }
}
