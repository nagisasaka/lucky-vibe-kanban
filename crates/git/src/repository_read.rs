//! Typed, bounded local Git exploration. Callers resolve and authorise registered
//! repository/workspace roots. No checkout, subprocess, filters or object writes.
use std::{
    path::Path,
    time::{Duration, Instant},
};

use anyhow::{Result, ensure};
use git2::{Oid, Repository, Tree};
use hmac::{Hmac, Mac};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::reference::{Content, MAX_REFERENCE_BYTES, allowed_path, content, worktree_content};

const MAX_PATHS: usize = 100_000;
const OUTPUT: usize = 64 * 1024;
const SCAN: usize = 16 * 1024 * 1024;
const FILES: usize = 1000;
const MATCH_VISITS: usize = 100_000;
const DEADLINE: Duration = Duration::from_secs(2);

/// The public source is an allowlisted product resource, never a host path.
/// Keep the existing Project request shape intact for older clients.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum QueryRequest {
    Project(Request),
    Bundled(BundledRequest),
}

impl JsonSchema for QueryRequest {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "QueryRequest".into()
    }

    fn json_schema(generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({
            "type": "object",
            "anyOf": [generator.subschema_for::<Request>(), generator.subschema_for::<BundledRequest>()]
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum BundledSource {
    Lvk,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BundledRequest {
    pub source: BundledSource,
    /// Omit to read the commit shipped with the running binary. Historical
    /// investigation may select an explicit commit; Workspace mode is rejected.
    pub target: Option<Target>,
    pub operation: Operation,
    #[serde(default = "default_limit")]
    pub limit: usize,
    pub cursor: Option<String>,
    pub expected_revision: Option<String>,
}

impl From<Request> for QueryRequest {
    fn from(request: Request) -> Self {
        Self::Project(request)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub project_id: Uuid,
    pub repository_id: Uuid,
    pub target: Target,
    pub operation: Operation,
    #[serde(default = "default_limit")]
    pub limit: usize,
    /// Opaque continuation; repeat all original arguments, including branch names.
    pub cursor: Option<String>,
    /// Optional on first read. Required fixed-point checks can use the response revision.
    pub expected_revision: Option<String>,
}
fn default_limit() -> usize {
    20
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "mode", rename_all = "snake_case", deny_unknown_fields)]
pub enum Target {
    Commit { r#ref: String },
    Workspace { workspace_id: Uuid },
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Operation {
    List {
        #[serde(default)]
        filter: Filter,
    },
    /// Supply either legacy path/start_line/end_line, or 1–8 ranges (not both).
    Read {
        #[serde(default, skip_serializing_if = "String::is_empty")]
        path: String,
        #[serde(default)]
        start_line: usize,
        #[serde(default)]
        end_line: usize,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        #[schemars(length(max = 8))]
        ranges: Vec<ReadRange>,
    },
    Grep {
        query: String,
        syntax: Syntax,
        case_sensitive: bool,
        output: Output,
        #[serde(default)]
        context_lines: usize,
        #[serde(default)]
        filter: Filter,
    },
    /// Topological/time order, fixed head; base hides its reachable history.
    Log { base: Option<String> },
    /// Missing base means selected end's parent (root: empty tree). Merge end requires parent.
    /// Workspace target permits only worktree mode; end and parent must be absent.
    Diff {
        base: Option<String>,
        end: Option<String>,
        comparison: Comparison,
        parent: Option<usize>,
        path: Option<String>,
    },
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ReadRange {
    pub path: String,
    pub start_line: usize,
    pub end_line: usize,
    /// Content revision from a query_repository read or grep item, not the page revision.
    pub expected_revision: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Syntax {
    Literal,
    Regex,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Output {
    Content,
    Files,
    Count,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Comparison {
    Direct,
    MergeBase,
    Worktree,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Filter {
    pub directory_prefix: Option<String>,
    pub filename_contains: Option<String>,
    /// Repository-relative glob; * does not cross /, ** does. Case-sensitive.
    pub glob: Option<String>,
}
impl Filter {
    fn matcher(&self) -> Result<impl Fn(&str) -> bool + '_> {
        ensure!(
            self.directory_prefix
                .as_ref()
                .is_none_or(|p| allowed_path(p)),
            "Invalid directory_prefix"
        );
        ensure!(
            self.filename_contains
                .as_ref()
                .is_none_or(|p| !p.is_empty() && p.len() <= 255 && !p.contains(['/', '\\', '\0'])),
            "Invalid filename_contains"
        );
        let glob = self
            .glob
            .as_ref()
            .map(|p| {
                ensure!(
                    p.len() <= 1024
                        && !p.starts_with('/')
                        && !p.contains(['\\', '\0'])
                        && !p.split('/').any(|x| x == ".."),
                    "Invalid glob"
                );
                Ok(globset::GlobBuilder::new(p)
                    .literal_separator(true)
                    .build()?
                    .compile_matcher())
            })
            .transpose()?;
        Ok(move |p: &str| {
            self.directory_prefix
                .as_ref()
                .is_none_or(|d| p.strip_prefix(d).is_some_and(|s| s.starts_with('/')))
                && self
                    .filename_contains
                    .as_ref()
                    .is_none_or(|s| p.rsplit('/').next().unwrap_or(p).contains(s))
                && glob
                    .as_ref()
                    .is_none_or(|g: &globset::GlobMatcher| g.is_match(p))
        })
    }
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Cursor {
    signature: String,
    exclusions: usize,
    skip_empty: bool,
    partial_count: usize,
    version: u8,
    query: String,
    head: String,
    base: Option<String>,
    end: String,
    revision: String,
    file: usize,
    item: usize,
}
impl Cursor {
    fn signature(&self) -> Result<String> {
        static KEY: std::sync::OnceLock<[u8; 16]> = std::sync::OnceLock::new();
        let key = KEY.get_or_init(|| *Uuid::new_v4().as_bytes());
        let fields = serde_json::to_vec(&(
            self.version,
            self.exclusions,
            self.skip_empty,
            self.partial_count,
            &self.query,
            &self.head,
            &self.base,
            &self.end,
            &self.revision,
            self.file,
            self.item,
        ))?;
        let mut mac = Hmac::<Sha256>::new_from_slice(key).expect("HMAC key");
        mac.update(&fields);
        Ok(format!("{:x}", mac.finalize().into_bytes()))
    }

    fn encode(mut self) -> Result<String> {
        self.signature = self.signature()?;
        Ok(serde_json::to_string(&self)?)
    }
}
fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn resolve(repo: &Repository, value: &str) -> Result<Oid> {
    ensure!(
        !value.is_empty() && value.len() <= 1024 && !value.contains(['\0', '\n', '\r']),
        "Invalid ref"
    );
    // Do not expose revparse's expression/regex/reflog language as another
    // query engine. Only local named refs, HEAD and complete commit OIDs.
    if value == "HEAD" {
        return Ok(repo.head()?.peel_to_commit()?.id());
    }
    if value.len() == 40 && value.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Ok(repo.find_commit(Oid::from_str(value)?)?.id());
    }
    let names = if value.starts_with("refs/") {
        vec![value.to_owned()]
    } else {
        vec![
            format!("refs/heads/{value}"),
            format!("refs/remotes/{value}"),
            format!("refs/tags/{value}"),
        ]
    };
    for name in names {
        ensure!(
            git2::Reference::is_valid_name(&name),
            "Invalid local ref name"
        );
        match repo.find_reference(&name) {
            Ok(reference) => return Ok(reference.peel_to_commit()?.id()),
            Err(error) if error.code() == git2::ErrorCode::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }
    anyhow::bail!("Unknown local ref; supply a branch, tag, HEAD or full commit SHA")
}
fn tree_content(repo: &Repository, tree: Option<&Tree<'_>>, path: &str) -> Result<Content> {
    ensure!(allowed_path(path), "Private or invalid path");
    let Some(tree) = tree else {
        return Ok(Content::Missing);
    };
    match tree.get_path(Path::new(path)) {
        Ok(entry) if matches!(entry.filemode(), 0o100644 | 0o100755) => Ok(
            crate::snapshot::read_regular_blob(repo, entry.id(), MAX_REFERENCE_BYTES, content)?
                .unwrap_or(Content::TooLarge),
        ),
        Ok(_) => Ok(Content::Unsupported),
        Err(e) if e.code() == git2::ErrorCode::NotFound => Ok(Content::Missing),
        Err(e) => Err(e.into()),
    }
}
fn tree_paths(tree: &Tree<'_>) -> Result<Vec<String>> {
    let mut paths = Vec::new();
    let mut invalid = false;
    let mut visited = 0;
    tree.walk(git2::TreeWalkMode::PreOrder, |root, entry| {
        visited += 1;
        if visited > MAX_PATHS {
            invalid = true;
            return git2::TreeWalkResult::Abort;
        }
        let Some(name) = entry.name() else {
            invalid = true;
            return git2::TreeWalkResult::Abort;
        };
        let p = format!("{root}{name}");
        if !allowed_path(&p) {
            return git2::TreeWalkResult::Skip;
        }
        if matches!(entry.filemode(), 0o100644 | 0o100755) {
            paths.push(p);
        }
        git2::TreeWalkResult::Ok
    })?;
    ensure!(!invalid, "Unsupported or excessive repository paths");
    paths.sort();
    Ok(paths)
}
/// One Repository/index open per request. Shared with the legacy literal search.
pub struct WorkspaceReader {
    root: std::path::PathBuf,
    pub repo: Repository,
    index: git2::Index,
    paths: Vec<String>,
}
impl WorkspaceReader {
    pub fn open(root: &Path) -> Result<Self> {
        let repo = Repository::open(root)?;
        let index = repo.index()?;
        ensure!(index.len() <= MAX_PATHS, "Repository index exceeds limit");
        let mut paths = Vec::new();
        for entry in index.iter() {
            let p = std::str::from_utf8(&entry.path)?;
            if allowed_path(p) && matches!(entry.mode, 0o100644 | 0o100755) {
                paths.push(p.to_owned());
            }
        }
        paths.sort();
        paths.dedup();
        Ok(Self {
            root: root.to_owned(),
            repo,
            index,
            paths,
        })
    }
    pub fn tracked_paths(&self) -> &[String] {
        &self.paths
    }
    pub fn read_tracked(&self, path: &str) -> Result<Content> {
        ensure!(allowed_path(path), "Private or invalid path");
        let entry = self
            .index
            .get_path(Path::new(path), 0)
            .ok_or_else(|| anyhow::anyhow!("Untracked or conflicted path"))?;
        if !matches!(entry.mode, 0o100644 | 0o100755) {
            return Ok(Content::Unsupported);
        }
        Ok(worktree_content(&self.root, path)?)
    }
    fn all_paths(&self) -> Result<Vec<String>> {
        let mut paths = self.paths.clone();
        let mut options = git2::StatusOptions::new();
        options
            .include_untracked(true)
            .recurse_untracked_dirs(true)
            .include_ignored(false)
            .update_index(false);
        let statuses = self.repo.statuses(Some(&mut options))?;
        ensure!(
            statuses.len() <= MAX_PATHS,
            "Workspace status exceeds limit"
        );
        for entry in statuses.iter() {
            if let Some(p) = entry.path()
                && allowed_path(p)
            {
                paths.push(p.to_owned());
            }
        }
        paths.sort();
        paths.dedup();
        ensure!(paths.len() <= MAX_PATHS, "Workspace paths exceed limit");
        Ok(paths)
    }
    fn observed_revision(&self, paths: &[String]) -> Result<String> {
        // Metadata observation, never advertised as an atomic/content snapshot.
        // ctime catches same-size restored-mtime edits on Unix. Index bytes bind staging.
        let mut h = Sha256::new();
        h.update(self.repo.head()?.peel_to_commit()?.id().as_bytes());
        if let Some(p) = self.index.path() {
            h.update(std::fs::read(p)?);
        }
        for p in paths {
            h.update(p.as_bytes());
            h.update([0]);
            match std::fs::symlink_metadata(self.root.join(p)) {
                Ok(m) => {
                    h.update(format!(
                        "{}:{:?}:{:?}",
                        m.len(),
                        m.modified()?,
                        m.file_type()
                    ));
                    #[cfg(unix)]
                    {
                        use std::os::unix::fs::MetadataExt;
                        h.update(format!(
                            "{}:{}:{}:{}",
                            m.ino(),
                            m.ctime(),
                            m.ctime_nsec(),
                            m.mode()
                        ));
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => h.update(b"missing"),
                Err(e) => return Err(e.into()),
            }
        }
        Ok(format!("{:x}", h.finalize()))
    }
}
fn status(c: &Content) -> &'static str {
    match c {
        Content::Text(_) => "text",
        Content::Missing => "missing",
        Content::Binary => "binary",
        Content::TooLarge => "too_large",
        Content::Unsupported => "unsupported",
    }
}
fn advance_scalar(line: &str, byte: usize) -> usize {
    line[byte..]
        .chars()
        .next()
        .map_or(line.len() + 1, |c| byte + c.len_utf8())
}
fn charge(c: &Content) -> usize {
    match c {
        Content::Text(t) => t.len(),
        Content::Missing => 0,
        _ => MAX_REFERENCE_BYTES + 1,
    }
}
fn body(c: &Content) -> Option<&str> {
    match c {
        Content::Text(t) => Some(t),
        Content::Missing => Some(""),
        _ => None,
    }
}

/// root is selected from registered records, never from a tool argument.
pub fn read(root: &Path, request: &Request) -> Result<Value> {
    read_with_budget(root, request, DEADLINE)
}
fn read_with_budget(root: &Path, request: &Request, duration: Duration) -> Result<Value> {
    let started = Instant::now();
    ensure!((1..=50).contains(&request.limit), "limit must be 1..50");
    let mut identity = request.clone();
    identity.cursor = None;
    identity.expected_revision = None;
    let query = hash(&serde_json::to_vec(&identity)?);
    let prior: Option<Cursor> = request
        .cursor
        .as_ref()
        .map(|s| {
            ensure!(s.len() <= 8192, "Invalid cursor");
            let c: Cursor = serde_json::from_str(s)?;
            ensure!(
                c.version == 1
                    && c.query == query
                    && c.file <= MAX_PATHS
                    && c.item <= 10_000_000
                    && c.signature == c.signature()?,
                "Foreign cursor or query mismatch"
            );
            Ok(c)
        })
        .transpose()?;
    let workspace = matches!(request.target, Target::Workspace { .. });
    let reader = if workspace {
        Some(WorkspaceReader::open(root)?)
    } else {
        None
    };
    let standalone = if workspace {
        None
    } else {
        Some(Repository::open(root)?)
    };
    let repo = reader
        .as_ref()
        .map(|r| &r.repo)
        .or(standalone.as_ref())
        .unwrap();
    let head = if let Some(c) = &prior {
        Oid::from_str(&c.head)?
    } else {
        match &request.target {
            Target::Commit { r#ref } => resolve(repo, r#ref)?,
            Target::Workspace { .. } => repo.head()?.peel_to_commit()?.id(),
        }
    };
    let mut end = head;
    let mut base = None;
    if let Some(c) = &prior {
        end = Oid::from_str(&c.end)?;
        base = c.base.as_ref().map(|s| Oid::from_str(s)).transpose()?;
    } else {
        match &request.operation {
            Operation::Log { base: b } => {
                base = b.as_ref().map(|b| resolve(repo, b)).transpose()?;
            }
            Operation::Diff {
                base: b,
                end: e,
                comparison,
                parent,
                ..
            } => {
                ensure!(
                    workspace == matches!(comparison, Comparison::Worktree),
                    "Workspace diff requires worktree comparison; commit diff requires direct or merge_base"
                );
                ensure!(
                    !workspace || (e.is_none() && parent.is_none()),
                    "Workspace diff does not accept end/parent"
                );
                ensure!(
                    b.is_none() || parent.is_none(),
                    "Specify base or parent, not both"
                );
                if let Some(e) = e {
                    end = resolve(repo, e)?;
                }
                if let Some(b) = b {
                    base = Some(resolve(repo, b)?);
                } else if workspace {
                    base = Some(head);
                } else {
                    let c = repo.find_commit(end)?;
                    ensure!(
                        c.parent_count() <= 1 || parent.is_some(),
                        "Merge commit requires explicit 1-based parent"
                    );
                    if c.parent_count() > 0 {
                        let p = parent.unwrap_or(1);
                        ensure!(p > 0 && p <= c.parent_count(), "Invalid parent");
                        base = Some(c.parent_id(p - 1)?);
                    } else {
                        ensure!(parent.is_none(), "Root commit has no parent");
                    }
                }
                if matches!(comparison, Comparison::MergeBase) {
                    base = Some(repo.merge_base(
                        base.ok_or_else(|| anyhow::anyhow!("merge_base requires a base"))?,
                        end,
                    )?);
                }
            }
            _ => {}
        }
    }
    let tree = repo.find_commit(end)?.tree()?;
    let old_tree = base.map(|b| repo.find_commit(b)?.tree()).transpose()?;
    let all_paths = if let Some(r) = &reader {
        r.all_paths()?
    } else if matches!(
        request.operation,
        Operation::List { .. } | Operation::Grep { .. }
    ) {
        tree_paths(&tree)?
    } else {
        Vec::new()
    };
    let observed = if let Some(r) = &reader {
        r.observed_revision(&all_paths)?
    } else {
        head.to_string()
    };
    let revision = hash(&serde_json::to_vec(&(
        request.project_id,
        request.repository_id,
        match request.target {
            Target::Workspace { workspace_id } => Some(workspace_id),
            _ => None,
        },
        head.to_string(),
        &observed,
    ))?);
    ensure!(
        request
            .expected_revision
            .as_ref()
            .is_none_or(|r| *r == revision)
            && prior.as_ref().is_none_or(|c| c.revision == revision),
        "Revision mismatch; restart from first page"
    );
    let mut position = (
        prior.as_ref().map_or(0, |c| c.file),
        prior.as_ref().map_or(0, |c| c.item),
    );
    let mut skip_empty = prior.as_ref().is_some_and(|c| c.skip_empty);
    let mut partial_count = prior.as_ref().map_or(0, |c| c.partial_count);
    let mut items: Vec<Value> = Vec::new();
    let mut exclusions = Vec::new();
    let mut exclusions_total = prior.as_ref().map_or(0, |c| c.exclusions);
    let mut used = 0;
    let mut charged = 0;
    let mut scanned = 0;
    let mut matches_visited = 0;
    let mut stop = None;
    let get = |p: &str| -> Result<Content> {
        if let Some(r) = &reader {
            if r.paths.binary_search_by(|s| s.as_str().cmp(p)).is_ok() {
                r.read_tracked(p)
            } else {
                Ok(worktree_content(root, p)?)
            }
        } else {
            tree_content(repo, Some(&tree), p)
        }
    };
    let mut push = |item: Value, items: &mut Vec<Value>| -> bool {
        let bytes = item.to_string().len() + 1;
        if used + bytes > OUTPUT {
            return false;
        }
        used += bytes;
        items.push(item);
        true
    };
    let mut scan_started = Instant::now();
    match &request.operation {
        Operation::Read {
            path,
            start_line,
            end_line,
            ranges,
        } => {
            ensure!(
                request.cursor.is_none(),
                "Line read uses explicit ranges and expected_revision, not cursor"
            );
            let batch = !ranges.is_empty();
            ensure!(
                ranges.len() <= 8
                    && (!batch || (path.is_empty() && *start_line == 0 && *end_line == 0)),
                "Supply a single read or 1–8 ranges, not both"
            );
            let single = vec![ReadRange {
                path: path.clone(),
                start_line: *start_line,
                end_line: *end_line,
                expected_revision: None,
            }];
            let selected = if batch { ranges } else { &single };
            let mut reserved: usize = selected
                .iter()
                .map(|r| json!(r.path).to_string().len() + 512)
                .sum();
            ensure!(reserved <= OUTPUT, "Batch metadata exceeds output limit");
            for range in selected {
                reserved -= json!(range.path).to_string().len() + 512;
                let valid = allowed_path(&range.path)
                    && range.start_line > 0
                    && range.end_line >= range.start_line
                    && range.end_line - range.start_line < 200;
                if !batch {
                    ensure!(valid, "Invalid path/range (maximum 200 lines)");
                }
                let listed = valid
                    && if workspace {
                        all_paths.binary_search(&range.path).is_ok()
                    } else {
                        tree.get_path(Path::new(&range.path))
                            .is_ok_and(|e| matches!(e.filemode(), 0o100644 | 0o100755))
                    };
                if !batch {
                    ensure!(listed, "Path is not a listed safe file");
                }
                let mut item = json!({"path":range.path,"start_line":range.start_line,"end_line":range.end_line});
                if !listed {
                    item["status"] = json!("unsafe_or_unlisted");
                } else {
                    match get(&range.path) {
                        Ok(Content::Text(text)) => {
                            let current = hash(text.as_bytes());
                            item["revision"] = json!(current);
                            let lines: Vec<_> = text.lines().collect();
                            if range
                                .expected_revision
                                .as_ref()
                                .is_some_and(|v| v != &current)
                            {
                                item["status"] = json!("revision_mismatch");
                            } else if range.end_line > lines.len() {
                                if !batch {
                                    anyhow::bail!("Range out of bounds");
                                }
                                item["status"] = json!("range_out_of_bounds");
                            } else {
                                item["status"] = json!("text");
                                item["lines"] = json!(lines[range.start_line - 1..range.end_line]);
                            }
                            charged += text.len();
                        }
                        Ok(content) => {
                            charged += charge(&content);
                            item["status"] = json!(status(&content));
                        }
                        Err(error) => {
                            if !batch {
                                return Err(error);
                            }
                            item["status"] = json!("unsafe_or_unreadable");
                        }
                    }
                }
                scanned += 1;
                if items.iter().map(|v| v.to_string().len() + 1).sum::<usize>()
                    + item.to_string().len()
                    + 1
                    > OUTPUT - reserved
                {
                    if !batch {
                        anyhow::bail!("Range exceeds output byte budget; request fewer lines");
                    }
                    item.as_object_mut().unwrap().remove("lines");
                    item["status"] = json!("output_byte_limit");
                }
                if batch && item["status"] != "text" {
                    stop = Some("range_failure");
                }
                ensure!(
                    push(item, &mut items),
                    "Batch metadata exceeds output limit"
                );
            }
        }
        Operation::Log { .. } => {
            let mut walk = repo.revwalk()?;
            walk.set_sorting(git2::Sort::TOPOLOGICAL | git2::Sort::TIME)?;
            walk.push(head)?;
            if let Some(base) = base {
                walk.hide(base)?;
            }
            let mut walk = walk.skip(position.0).peekable();
            let _ = walk.peek(); // Complete bounded cursor reconstruction before timing new work.
            scan_started = Instant::now();
            while let Some(oid) = walk.peek() {
                ensure!(
                    position.0 < MAX_PATHS,
                    "History exceeds 100000 commits; narrow the range with base"
                );
                if items.len() >= request.limit {
                    stop = Some("item_limit");
                    break;
                }
                if scan_started.elapsed() >= duration {
                    stop = Some("time_limit");
                    break;
                }
                let oid = *oid
                    .as_ref()
                    .map_err(|e| anyhow::anyhow!(e.message().to_owned()))?;
                let c = repo.find_commit(oid)?;
                let item = json!({"sha":oid.to_string(),"parents":c.parent_ids().take(64).map(|p|p.to_string()).collect::<Vec<_>>(),"parent_count":c.parent_count(),"parents_truncated":c.parent_count()>64,"time":c.time().seconds(),"summary":c.summary().map(|s|s.chars().take(2000).collect::<String>()),"summary_truncated":c.summary().is_some_and(|s|s.chars().count()>2000)});
                if !push(item, &mut items) {
                    stop = Some("output_byte_limit");
                    break;
                }
                walk.next();
                position.0 += 1;
            }
        }
        op => {
            let mut paths = all_paths.clone();
            if let Operation::Diff { .. } = op {
                let mut options = git2::DiffOptions::new();
                options
                    .include_untracked(true)
                    .recurse_untracked_dirs(true)
                    .include_typechange(true);
                let diff = if workspace {
                    repo.diff_tree_to_workdir_with_index(old_tree.as_ref(), Some(&mut options))?
                } else {
                    repo.diff_tree_to_tree(old_tree.as_ref(), Some(&tree), Some(&mut options))?
                };
                ensure!(diff.deltas().len() <= MAX_PATHS, "Diff paths exceed limit");
                paths.clear();
                for delta in diff.deltas() {
                    for f in [delta.old_file(), delta.new_file()] {
                        if let Some(p) = f.path().and_then(|p| p.to_str())
                            && allowed_path(p)
                        {
                            paths.push(p.to_owned());
                        }
                    }
                }
                paths.sort();
                paths.dedup();
                if let Operation::Diff { path: Some(p), .. } = op {
                    ensure!(allowed_path(p), "Private or invalid path");
                    paths.retain(|s| s == p);
                }
            }
            let filter = match op {
                Operation::List { filter } | Operation::Grep { filter, .. } => {
                    Some(filter.matcher()?)
                }
                _ => None,
            };
            if let Some(filter) = filter {
                paths.retain(|p| filter(p));
            }
            ensure!(position.0 <= paths.len(), "Cursor outside paths");
            let regex = if let Operation::Grep {
                query,
                syntax,
                case_sensitive,
                context_lines,
                ..
            } = op
            {
                ensure!(
                    !query.is_empty()
                        && query.len() <= 1024
                        && !query.contains(['\n', '\r'])
                        && *context_lines <= 2,
                    "Invalid grep query/context"
                );
                let q = match syntax {
                    Syntax::Literal => regex::escape(query),
                    Syntax::Regex => query.clone(),
                };
                Some(
                    regex::RegexBuilder::new(&q)
                        .case_insensitive(!case_sensitive)
                        .size_limit(1024 * 1024)
                        .dfa_size_limit(1024 * 1024)
                        .build()?,
                )
            } else {
                None
            };
            scan_started = Instant::now();
            'files: while position.0 < paths.len() {
                if items.len() >= request.limit {
                    stop = Some("item_limit");
                    break;
                }
                if scan_started.elapsed() >= duration {
                    stop = Some("time_limit");
                    break;
                }
                if scanned >= FILES {
                    stop = Some("file_limit");
                    break;
                }
                let p = &paths[position.0];
                if matches!(op, Operation::List { .. })
                    || matches!(op, Operation::Diff { path: None, .. })
                {
                    if !push(json!({"path":p}), &mut items) {
                        stop = Some("output_byte_limit");
                        break;
                    }
                    position.0 += 1;
                    scanned += 1;
                    continue;
                }
                if charged
                    + (if matches!(op, Operation::Diff { path: Some(_), .. }) {
                        2
                    } else {
                        1
                    }) * (MAX_REFERENCE_BYTES + 1)
                    > SCAN
                {
                    stop = Some("scan_byte_limit");
                    break;
                }
                scanned += 1;
                let c = match get(p) {
                    Ok(c) => c,
                    Err(_) => {
                        charged += MAX_REFERENCE_BYTES + 1;
                        if !push(
                            json!({"path":p,"status":"unsafe_or_unreadable"}),
                            &mut exclusions,
                        ) {
                            stop = Some("output_byte_limit");
                            break;
                        }
                        exclusions_total += 1;
                        position.0 += 1;
                        position.1 = 0;
                        skip_empty = false;
                        partial_count = 0;
                        continue;
                    }
                };
                charged += charge(&c);
                if let Operation::Diff { path: Some(_), .. } = op {
                    let old = tree_content(repo, old_tree.as_ref(), p)?;
                    charged += charge(&old);
                    let patch = body(&old)
                        .zip(body(&c))
                        .map(|(a, b)| utils::diff::create_unified_diff(p, a, b));
                    let chars: Vec<char> = patch.as_deref().unwrap_or("").chars().collect();
                    ensure!(position.1 <= chars.len(), "Cursor outside diff");
                    let old_mode = old_tree
                        .as_ref()
                        .and_then(|t| t.get_path(Path::new(p)).ok())
                        .map(|e| e.filemode());
                    let new_mode = if workspace {
                        None
                    } else {
                        tree.get_path(Path::new(p)).ok().map(|e| e.filemode())
                    };
                    let mut end_char = (position.1 + 8000).min(chars.len());
                    loop {
                        let item = json!({"path":p,"old_status":status(&old),"new_status":status(&c),"old_git_mode":old_mode,"new_git_mode":new_mode,"text_equal":body(&old).zip(body(&c)).map(|(a,b)|a==b),"non_text_changes_require_review":true,"offset":position.1,"patch":patch.as_ref().map(|_|chars[position.1..end_char].iter().collect::<String>())});
                        if push(item, &mut items) {
                            break;
                        }
                        if end_char > position.1 + 1 {
                            end_char = position.1 + (end_char - position.1) / 2;
                        } else {
                            stop = Some("output_byte_limit");
                            break 'files;
                        }
                    }
                    if end_char < chars.len() {
                        position.1 = end_char;
                        stop = Some("item_limit");
                        break;
                    }
                    position.0 += 1;
                    position.1 = 0;
                    skip_empty = false;
                    partial_count = 0;
                    continue;
                }
                let Content::Text(t) = c else {
                    if !push(json!({"path":p,"status":status(&c)}), &mut exclusions) {
                        stop = Some("output_byte_limit");
                        break;
                    }
                    exclusions_total += 1;
                    position.0 += 1;
                    position.1 = 0;
                    skip_empty = false;
                    partial_count = 0;
                    continue;
                };
                let lines: Vec<_> = t.lines().collect();
                if let Operation::Grep {
                    output,
                    context_lines,
                    ..
                } = op
                {
                    let re = regex.as_ref().unwrap();
                    // Resume at the previous match's end, preserving find_iter's
                    // non-overlap and adjacent-empty suppression. No prefix regex rescan.
                    if !matches!(output, Output::Files) || partial_count == 0 {
                        let mut offset = 0;
                        'lines: for (n, line) in lines.iter().enumerate() {
                            let next_offset =
                                offset + t[offset..].find('\n').map_or(t.len() - offset, |i| i + 1);
                            if position.1 > offset + line.len() {
                                offset = next_offset;
                                continue;
                            }
                            if position.1 < offset {
                                position.1 = offset;
                                skip_empty = false;
                            }
                            let mut local = position.1 - offset;
                            ensure!(
                                line.is_char_boundary(local),
                                "Cursor outside Unicode boundary"
                            );
                            while local <= line.len() {
                                if scan_started.elapsed() >= duration && matches_visited > 0 {
                                    stop = Some("time_limit");
                                    break 'files;
                                }
                                if matches_visited >= MATCH_VISITS {
                                    stop = Some("match_scan_limit");
                                    break 'files;
                                }
                                let Some(m) = re.find_at(line, local) else {
                                    break;
                                };
                                matches_visited += 1;
                                if skip_empty && m.is_empty() && m.start() == local {
                                    local = advance_scalar(line, local);
                                    position.1 = offset + local;
                                    skip_empty = false;
                                    continue;
                                }
                                if matches!(output, Output::Content) {
                                    if items.len() >= request.limit {
                                        stop = Some("item_limit");
                                        break 'files;
                                    }
                                    let first = n.saturating_sub(*context_lines);
                                    let last = (n + context_lines + 1).min(lines.len());
                                    let context: Vec<_> = lines[first..last]
                                        .iter()
                                        .map(|l| l.chars().take(512).collect::<String>())
                                        .collect();
                                    let item = json!({"revision":hash(t.as_bytes()),"path":p,"line":n+1,"column":line[..m.start()].chars().count()+1,"end_column_exclusive":line[..m.end()].chars().count()+1,"matched":m.as_str().chars().take(1024).collect::<String>(),"matched_truncated":m.as_str().chars().count()>1024,"context_start_line":first+1,"context":context,"context_truncated":lines[first..last].iter().any(|l|l.chars().count()>512)});
                                    if !push(item, &mut items) {
                                        stop = Some("output_byte_limit");
                                        break 'files;
                                    }
                                } else {
                                    partial_count += 1;
                                }
                                if m.is_empty() {
                                    local = advance_scalar(line, m.end());
                                    skip_empty = false;
                                } else {
                                    local = m.end();
                                    skip_empty = true;
                                }
                                position.1 = offset + local;
                                if matches!(output, Output::Files) {
                                    break 'lines;
                                }
                            }
                            position.1 = next_offset.max(offset + line.len() + 1);
                            skip_empty = false;
                            offset = next_offset;
                        }
                    }
                    let row = match output {
                        Output::Files if partial_count > 0 => Some(json!({"path":p})),
                        Output::Count if partial_count > 0 => {
                            Some(json!({"path":p,"count":partial_count}))
                        }
                        _ => None,
                    };
                    if let Some(row) = row
                        && !push(row, &mut items)
                    {
                        stop = Some("output_byte_limit");
                        break;
                    }
                }

                position.0 += 1;
                position.1 = 0;
                skip_empty = false;
                partial_count = 0;
            }
        }
    }
    let scan_elapsed_ms = scan_started.elapsed().as_millis();
    if let Some(r) = &reader {
        let after = WorkspaceReader::open(root)?;
        let paths = after.all_paths()?;
        ensure!(
            paths == all_paths
                && after.observed_revision(&paths)? == observed
                && r.repo.head()?.peel_to_commit()?.id() == head,
            "Workspace changed during read; restart from first page"
        );
    }
    let page_revision = hash(&serde_json::to_vec(&(
        &revision,
        &query,
        head.to_string(),
        base.map(|b| b.to_string()),
        end.to_string(),
    ))?);
    let next = stop
        .filter(|_| !matches!(request.operation, Operation::Read { .. }))
        .map(|_| {
            Cursor {
                signature: String::new(),
                exclusions: exclusions_total,
                skip_empty,
                partial_count,
                version: 1,
                query,
                head: head.to_string(),
                base: base.map(|b| b.to_string()),
                end: end.to_string(),
                revision: revision.clone(),
                file: position.0,
                item: position.1,
            }
            .encode()
        })
        .transpose()?;
    let live_end = workspace && !matches!(request.operation, Operation::Log { .. });
    Ok(
        json!({"items":items,"exclusions":exclusions,"complete":stop.is_none(),"search_complete":stop.is_none() && exclusions_total==0,"exclusions_total":exclusions_total,"stop_reason":stop,"next_cursor":next,"revision":revision,"page_revision":page_revision,"source_head":head.to_string(),"end_commit":if live_end {None}else{Some(end.to_string())},"end_kind":if live_end {"workspace_observation"}else{"commit"},"base_commit":base.map(|b|b.to_string()),"target":request.target,"operation":request.operation,"snapshot":if workspace{"live_metadata_observation_not_atomic"}else{"immutable_commit"},"private_paths_excluded":true,"budgets":{"item_limit":request.limit,"output_bytes":OUTPUT,"scan_bytes":SCAN,"files":FILES,"milliseconds":duration.as_millis(),"maximum_source_bytes":MAX_REFERENCE_BYTES,"history_position_limit":MAX_PATHS,"match_visits":MATCH_VISITS},"matches_visited":matches_visited,"scan_bytes_charged":charged,"attempted_files":scanned,"item_bytes":used,"scan_elapsed_ms":scan_elapsed_ms,"elapsed_ms":started.elapsed().as_millis()}),
    )
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    struct Fixture {
        temp: tempfile::TempDir,
        repo: Repository,
        root: Oid,
    }
    impl Fixture {
        fn new() -> Self {
            let temp = tempfile::tempdir().unwrap();
            let repo = Repository::init(temp.path()).unwrap();
            let root = commit(
                &repo,
                "refs/heads/main",
                &[],
                &[
                    ("src/入口.tsx", "import Board from './Board';\n<Board />\n"),
                    ("src/Board.tsx", "export default function Board() {}\n"),
                ],
            );
            repo.set_head("refs/heads/main").unwrap();
            repo.checkout_head(Some(git2::build::CheckoutBuilder::new().force()))
                .unwrap();
            Self { temp, repo, root }
        }
        fn read(&self, r: &Request) -> Value {
            read(self.temp.path(), r).unwrap()
        }
        fn request(&self, operation: Operation) -> Request {
            Request {
                project_id: Uuid::from_u128(1),
                repository_id: Uuid::from_u128(2),
                target: Target::Commit {
                    r#ref: "main".into(),
                },
                operation,
                limit: 1,
                cursor: None,
                expected_revision: None,
            }
        }
    }
    fn commit(repo: &Repository, reference: &str, parents: &[Oid], files: &[(&str, &str)]) -> Oid {
        let mut index = git2::Index::new().unwrap();
        for (p, t) in files {
            let entry = git2::IndexEntry {
                ctime: git2::IndexTime::new(0, 0),
                mtime: git2::IndexTime::new(0, 0),
                dev: 0,
                ino: 0,
                mode: 0o100644,
                uid: 0,
                gid: 0,
                file_size: t.len() as u32,
                id: repo.blob(t.as_bytes()).unwrap(),
                flags: 0,
                flags_extended: 0,
                path: p.as_bytes().to_vec(),
            };
            index.add(&entry).unwrap();
        }
        let tree = repo.find_tree(index.write_tree_to(repo).unwrap()).unwrap();
        let parents: Vec<_> = parents
            .iter()
            .map(|o| repo.find_commit(*o).unwrap())
            .collect();
        let refs: Vec<_> = parents.iter().collect();
        let sig = git2::Signature::new(
            "Fixture",
            "fixture@example.invalid",
            &git2::Time::new(12345, 0),
        )
        .unwrap();
        repo.commit(Some(reference), &sig, &sig, "fixture", &tree, &refs)
            .unwrap()
    }
    fn diff(
        base: Option<String>,
        end: Option<String>,
        comparison: Comparison,
        parent: Option<usize>,
        path: Option<&str>,
    ) -> Operation {
        Operation::Diff {
            base,
            end,
            comparison,
            parent,
            path: path.map(str::to_owned),
        }
    }
    fn grep(query: &str, syntax: Syntax, output: Output) -> Operation {
        Operation::Grep {
            query: query.into(),
            syntax,
            case_sensitive: true,
            output,
            context_lines: 1,
            filter: Filter::default(),
        }
    }
    fn pages(f: &Fixture, r: &mut Request) -> Vec<Value> {
        let mut all = Vec::new();
        for _ in 0..500 {
            let v = f.read(r);
            all.extend(v["items"].as_array().unwrap().clone());
            if v["complete"] == true {
                assert!(v["next_cursor"].is_null());
                return all;
            }
            assert!(v["stop_reason"].is_string());
            r.cursor = Some(v["next_cursor"].as_str().unwrap().into());
        }
        panic!("Search did not finish")
    }
    #[test]
    fn first_read_full_sha_reproduces_without_worktree_and_ignores_dirty() {
        let f = Fixture::new();
        fs::write(f.temp.path().join("src/入口.tsx"), "dirty\n").unwrap();
        let mut r = f.request(Operation::Read {
            ranges: vec![],
            path: "src/入口.tsx".into(),
            start_line: 1,
            end_line: 2,
        });
        let v = f.read(&r);
        assert_eq!(v["source_head"], f.root.to_string());
        assert!(
            v["items"][0]["lines"][0]
                .as_str()
                .unwrap()
                .contains("import Board")
        );
        r.target = Target::Commit {
            r#ref: v["source_head"].as_str().unwrap().into(),
        };
        fs::remove_dir_all(f.temp.path().join("src")).unwrap();
        assert_eq!(f.read(&r)["items"], v["items"]);
    }
    #[test]
    fn branch_movement_does_not_mix_pages_and_cursor_is_bound_and_authenticated() {
        let f = Fixture::new();
        let mut r = f.request(Operation::List {
            filter: Filter::default(),
        });
        let first = f.read(&r);
        r.cursor = Some(first["next_cursor"].as_str().unwrap().into());
        commit(
            &f.repo,
            "refs/heads/main",
            &[f.root],
            &[("other.txt", "new")],
        );
        let second = f.read(&r);
        assert_eq!(first["source_head"], second["source_head"]);
        assert_eq!(second["items"][0]["path"], "src/入口.tsx");
        let mut bad = r.clone();
        bad.repository_id = Uuid::from_u128(3);
        assert!(read(f.temp.path(), &bad).is_err());
        bad = r.clone();
        bad.operation = Operation::List {
            filter: Filter {
                glob: Some("*.txt".into()),
                ..Default::default()
            },
        };
        assert!(read(f.temp.path(), &bad).is_err());
        let mut c: Value = serde_json::from_str(r.cursor.as_ref().unwrap()).unwrap();
        c["end"] = json!(resolve(&f.repo, "main").unwrap().to_string());
        bad = r;
        bad.cursor = Some(c.to_string());
        assert!(read(f.temp.path(), &bad).is_err());
    }
    #[test]
    fn grep_unicode_empty_regex_files_count_and_context_continue_exactly() {
        let f = Fixture::new();
        let sha = commit(
            &f.repo,
            "refs/heads/main",
            &[f.root],
            &[("日本語.txt", "あaあ\r\nあ\n")],
        );
        let mut r = f.request(grep("あ", Syntax::Literal, Output::Content));
        let matches = pages(&f, &mut r);
        assert_eq!(matches.len(), 3);
        assert_eq!(matches[1]["column"], 3);
        assert_eq!(matches[2]["line"], 2);
        let mut r = f.request(grep("", Syntax::Regex, Output::Content));
        assert!(read(f.temp.path(), &r).is_err());
        r.operation = grep("(?:)", Syntax::Regex, Output::Content);
        let empty = pages(&f, &mut r);
        assert_eq!(empty.len(), 6); // 4 + 2 Unicode boundaries
        let mut r = f.request(grep("あ", Syntax::Regex, Output::Count));
        assert_eq!(f.read(&r)["items"][0]["count"], 3);
        r.operation = grep("A", Syntax::Literal, Output::Files);
        assert!(f.read(&r)["items"].as_array().unwrap().is_empty());
        if let Operation::Grep { case_sensitive, .. } = &mut r.operation {
            *case_sensitive = false;
        }
        assert_eq!(f.read(&r)["items"][0]["path"], "日本語.txt");
        assert_eq!(f.read(&r)["source_head"], sha.to_string());
        r.operation = grep("[", Syntax::Regex, Output::Content);
        assert!(read(f.temp.path(), &r).is_err());
    }
    #[test]
    fn scan_limits_resume_without_rescanning_prefix_and_exclusions_are_explicit() {
        let f = Fixture::new();
        let huge = "x".repeat(MAX_REFERENCE_BYTES + 1);
        let names: Vec<_> = (0..70).map(|i| format!("{i:03}.txt")).collect();
        let sizable = "x".repeat(300 * 1024);
        let mut files: Vec<_> = names
            .iter()
            .map(|n| (n.as_str(), sizable.as_str()))
            .collect();
        files.push(("zz.txt", "needle"));
        files.push(("huge.txt", &huge));
        files.push(("binary.txt", "\0"));
        commit(&f.repo, "refs/heads/main", &[f.root], &files);
        let mut r = f.request(grep("needle", Syntax::Literal, Output::Content));
        let first = f.read(&r);
        assert_eq!(first["stop_reason"], "scan_byte_limit");
        assert_eq!(first["complete"], false);
        assert!(first["next_cursor"].is_string());
        let hits = pages(&f, &mut r);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0]["path"], "zz.txt");
        let zero = read_with_budget(
            f.temp.path(),
            &f.request(Operation::List {
                filter: Filter::default(),
            }),
            Duration::ZERO,
        )
        .unwrap();
        assert_eq!(zero["stop_reason"], "time_limit");
        assert!(zero["next_cursor"].is_string());
    }
    #[test]
    fn workspace_untracked_staging_and_content_changes_invalidate_continuation() {
        let f = Fixture::new();
        fs::write(f.temp.path().join("untracked.txt"), "untracked\n").unwrap();
        let mut r = f.request(Operation::List {
            filter: Filter::default(),
        });
        r.target = Target::Workspace {
            workspace_id: Uuid::from_u128(3),
        };
        let before = f.read(&r);
        r.cursor = Some(before["next_cursor"].as_str().unwrap().into());
        fs::write(f.temp.path().join("src/Board.tsx"), "changed\n").unwrap();
        assert!(read(f.temp.path(), &r).is_err());
        r.cursor = None;
        let items = pages(&f, &mut r);
        assert!(items.iter().any(|i| i["path"] == "untracked.txt"));
        r.cursor = None;
        r.operation = Operation::Read {
            ranges: vec![],
            path: "untracked.txt".into(),
            start_line: 1,
            end_line: 1,
        };
        let v = f.read(&r);
        assert_eq!(v["items"][0]["lines"][0], "untracked");
        r.expected_revision = Some(v["revision"].as_str().unwrap().into());
        let mut index = f.repo.index().unwrap();
        index.add_path(Path::new("untracked.txt")).unwrap();
        index.write().unwrap();
        assert!(read(f.temp.path(), &r).is_err());
    }
    #[test]
    fn direct_non_ancestor_merge_base_root_merge_parent_and_old_interval() {
        let f = Fixture::new();
        let a = commit(&f.repo, "refs/heads/a", &[f.root], &[("old.txt", "a\n")]);
        let b = commit(&f.repo, "refs/heads/b", &[f.root], &[("new.txt", "b\n")]);
        let merge = commit(
            &f.repo,
            "refs/heads/merged",
            &[a, b],
            &[("merged.txt", "merge\n")],
        );
        fs::write(f.temp.path().join("dirty.txt"), "dirty\n").unwrap();
        let mut r = f.request(diff(
            Some(a.to_string()),
            Some(b.to_string()),
            Comparison::Direct,
            None,
            None,
        ));
        r.limit = 50;
        let v = f.read(&r);
        assert_eq!(v["base_commit"], a.to_string());
        assert_eq!(v["end_commit"], b.to_string());
        assert!(!v.to_string().contains("dirty.txt"));
        assert_eq!(v["items"].as_array().unwrap().len(), 2);
        r.operation = diff(
            Some(a.to_string()),
            Some(b.to_string()),
            Comparison::MergeBase,
            None,
            None,
        );
        assert_eq!(f.read(&r)["base_commit"], f.root.to_string());
        r.operation = diff(
            None,
            Some(merge.to_string()),
            Comparison::Direct,
            None,
            None,
        );
        assert!(read(f.temp.path(), &r).is_err());
        r.operation = diff(
            None,
            Some(merge.to_string()),
            Comparison::Direct,
            Some(2),
            None,
        );
        assert_eq!(f.read(&r)["base_commit"], b.to_string());
        r.operation = diff(
            None,
            Some(f.root.to_string()),
            Comparison::Direct,
            None,
            None,
        );
        assert!(f.read(&r)["base_commit"].is_null());
        r.operation = diff(
            None,
            Some(a.to_string()),
            Comparison::Direct,
            None,
            Some("old.txt"),
        );
        let patch = f.read(&r);
        assert_eq!(patch["base_commit"], f.root.to_string());
        assert!(patch["items"][0]["patch"].as_str().unwrap().contains("+a"));
        r.operation = diff(
            Some(a.to_string()),
            Some(b.to_string()),
            Comparison::Direct,
            None,
            Some("old.txt"),
        );
        assert_eq!(f.read(&r)["items"][0]["new_status"], "missing");
        r.target = Target::Workspace {
            workspace_id: Uuid::from_u128(3),
        };
        assert!(read(f.temp.path(), &r).is_err());
        r.operation = diff(
            Some(f.root.to_string()),
            None,
            Comparison::Worktree,
            None,
            None,
        );
        assert!(
            f.read(&r)["items"]
                .as_array()
                .unwrap()
                .iter()
                .any(|i| i["path"] == "dirty.txt")
        );
    }
    #[test]
    fn unrelated_roots_direct_diff_and_explicit_missing_merge_base() {
        let f = Fixture::new();
        let other = commit(
            &f.repo,
            "refs/heads/unrelated",
            &[],
            &[("other", "other\n")],
        );
        let mut r = f.request(diff(
            Some(f.root.to_string()),
            Some(other.to_string()),
            Comparison::Direct,
            None,
            None,
        ));
        assert!(!f.read(&r)["items"].as_array().unwrap().is_empty());
        r.operation = diff(
            Some(f.root.to_string()),
            Some(other.to_string()),
            Comparison::MergeBase,
            None,
            None,
        );
        assert!(read(f.temp.path(), &r).is_err());
        r.operation = diff(
            None,
            Some(f.root.to_string()),
            Comparison::Direct,
            Some(1),
            None,
        );
        assert!(read(f.temp.path(), &r).is_err());
    }
    #[test]
    fn regex_continuations_match_find_iter_including_adjacent_empty_suppression() {
        let f = Fixture::new();
        let text = "aaあaa\r\nあ\n\n";
        commit(&f.repo, "refs/heads/main", &[f.root], &[("text", text)]);
        for pattern in [
            "a*", "(?:)", "^|$", "a+z|.", "\\b", "aa|(?:)", "a?|あ", "a.*|$",
        ] {
            let re = regex::Regex::new(pattern).unwrap();
            let expected: Vec<_> = text
                .lines()
                .enumerate()
                .flat_map(|(n, l)| {
                    re.find_iter(l).map(move |m| {
                        json!([
                            n + 1,
                            l[..m.start()].chars().count() + 1,
                            l[..m.end()].chars().count() + 1,
                            m.as_str()
                        ])
                    })
                })
                .collect();
            let mut r = f.request(grep(pattern, Syntax::Regex, Output::Content));
            let actual: Vec<_> = pages(&f, &mut r)
                .into_iter()
                .map(|i| {
                    json!([
                        i["line"],
                        i["column"],
                        i["end_column_exclusive"],
                        i["matched"]
                    ])
                })
                .collect();
            assert_eq!(actual, expected, "{pattern}");
            let r = f.request(grep(pattern, Syntax::Regex, Output::Count));
            assert_eq!(f.read(&r)["items"][0]["count"], expected.len(), "{pattern}");
        }
    }
    #[test]
    fn count_continues_inside_file_with_partial_accumulator() {
        let f = Fixture::new();
        let text = "x".repeat(MATCH_VISITS + 50_000);
        commit(&f.repo, "refs/heads/main", &[f.root], &[("text", &text)]);
        let mut r = f.request(grep("x", Syntax::Literal, Output::Count));
        let first = f.read(&r);
        assert_eq!(first["stop_reason"], "match_scan_limit");
        assert_eq!(first["items"], json!([]));
        r.cursor = Some(first["next_cursor"].as_str().unwrap().into());
        let last = f.read(&r);
        assert_eq!(last["complete"], true);
        assert_eq!(last["items"][0]["count"], text.len());
    }
    #[test]
    fn ref_selectors_do_not_expose_revparse_expressions_or_regex() {
        let f = Fixture::new();
        for invalid in [
            "HEAD~1",
            "main^{/fixture}",
            "main:src/Board.tsx",
            "main@{0}",
            "main..main",
            "https://example.invalid/repo",
        ] {
            let mut r = f.request(Operation::Log { base: None });
            r.target = Target::Commit {
                r#ref: invalid.into(),
            };
            assert!(read(f.temp.path(), &r).is_err(), "{invalid}");
        }
        f.repo
            .tag_lightweight("v1", f.repo.find_commit(f.root).unwrap().as_object(), false)
            .unwrap();
        assert_eq!(resolve(&f.repo, "v1").unwrap(), f.root);
        assert_eq!(resolve(&f.repo, "refs/heads/main").unwrap(), f.root);
    }
    #[test]
    fn log_fixed_head_and_base_with_equal_timestamps() {
        let f = Fixture::new();
        let a = commit(&f.repo, "refs/heads/main", &[f.root], &[("a", "a")]);
        let b = commit(&f.repo, "refs/heads/main", &[a], &[("b", "b")]);
        let mut r = f.request(Operation::Log { base: None });
        let first = f.read(&r);
        assert_eq!(first["items"][0]["sha"], b.to_string());
        r.cursor = Some(first["next_cursor"].as_str().unwrap().into());
        commit(&f.repo, "refs/heads/main", &[b], &[("c", "c")]);
        let rest = pages(&f, &mut r);
        assert_eq!(rest.len(), 2);
        assert_eq!(rest[0]["sha"], a.to_string());
        assert_eq!(rest[1]["sha"], f.root.to_string());
        let mut r = f.request(Operation::Log {
            base: Some(a.to_string()),
        });
        r.target = Target::Commit {
            r#ref: b.to_string(),
        };
        let all = pages(&f, &mut r);
        assert_eq!(all.len(), 1);
    }
    #[test]
    fn private_binary_huge_symlink_gitlink_and_invalid_ranges() {
        let f = Fixture::new();
        let huge = "x".repeat(MAX_REFERENCE_BYTES + 1);
        let sha = commit(
            &f.repo,
            "refs/heads/main",
            &[f.root],
            &[
                (".env", "secret"),
                ("secrets/token", "secret"),
                ("key.pem", "secret"),
                ("binary", "\0"),
                ("large", &huge),
                ("safe", "ok\n"),
            ],
        );
        let tree = f.repo.find_commit(sha).unwrap().tree().unwrap();
        let mut builder = f.repo.treebuilder(Some(&tree)).unwrap();
        builder
            .insert("link", f.repo.blob(b"safe").unwrap(), 0o120000)
            .unwrap();
        builder.insert("module", f.root, 0o160000).unwrap();
        let oid = builder.write().unwrap();
        let sig = git2::Signature::now("fixture", "fixture@example.invalid").unwrap();
        f.repo
            .commit(
                Some("refs/heads/types"),
                &sig,
                &sig,
                "types",
                &f.repo.find_tree(oid).unwrap(),
                &[],
            )
            .unwrap();
        let mut r = f.request(Operation::List {
            filter: Filter::default(),
        });
        r.target = Target::Commit {
            r#ref: "types".into(),
        };
        r.limit = 50;
        let paths = f.read(&r)["items"].clone();
        assert_eq!(paths.as_array().unwrap().len(), 3);
        for path in [
            ".env",
            "../safe",
            "secrets/token",
            "link",
            "module",
            "/etc/passwd",
            "key.pem",
        ] {
            r.operation = Operation::Read {
                ranges: vec![],
                path: path.into(),
                start_line: 1,
                end_line: 1,
            };
            assert!(read(f.temp.path(), &r).is_err());
        }
        for (path, status) in [("binary", "binary"), ("large", "too_large")] {
            r.operation = Operation::Read {
                ranges: vec![],
                path: path.into(),
                start_line: 1,
                end_line: 1,
            };
            assert_eq!(f.read(&r)["items"][0]["status"], status);
        }
        for (start, end) in [(0, 1), (2, 1), (1, 201), (1, 2)] {
            r.operation = Operation::Read {
                ranges: vec![],
                path: "safe".into(),
                start_line: start,
                end_line: end,
            };
            assert!(read(f.temp.path(), &r).is_err());
        }
        r.operation = Operation::List {
            filter: Filter {
                glob: Some("[".into()),
                ..Default::default()
            },
        };
        assert!(read(f.temp.path(), &r).is_err());
        r.operation = Operation::List {
            filter: Filter {
                glob: Some("s*".into()),
                ..Default::default()
            },
        };
        assert_eq!(f.read(&r)["items"][0]["path"], "safe");
    }
    #[test]
    fn kanban_style_import_and_usage_evidence_in_two_first_reads() {
        let f = Fixture::new();
        let mut r = f.request(grep("Board", Syntax::Literal, Output::Content));
        r.limit = 20;
        let v = f.read(&r);
        assert_eq!(v["complete"], true);
        let items = v["items"].as_array().unwrap();
        assert!(
            items
                .iter()
                .any(|i| i["line"] == 1 && i["path"] == "src/入口.tsx")
        );
        assert!(items.iter().any(|i| i["line"] == 2));
        r.operation = Operation::Read {
            ranges: vec![],
            path: "src/Board.tsx".into(),
            start_line: 1,
            end_line: 1,
        };
        assert!(
            f.read(&r)["items"][0]["lines"][0]
                .as_str()
                .unwrap()
                .contains("function Board")
        );
    }
    #[test]
    fn batch_reads_preserve_legacy_and_bound_ranges_revisions_failures_and_output() {
        let f = Fixture::new();
        let legacy: Operation = serde_json::from_value(
            json!({"kind":"read","path":"src/入口.tsx","start_line":1,"end_line":1}),
        )
        .unwrap();
        let old = f.read(&f.request(legacy));
        let revision = old["items"][0]["revision"].clone();
        let mut ranges = vec![
            json!({"path":"src/入口.tsx","start_line":1,"end_line":1,"expected_revision":revision});
            8
        ];
        let mut request =
            f.request(serde_json::from_value(json!({"kind":"read","ranges":ranges})).unwrap());
        let full = f.read(&request);
        assert_eq!(full["items"].as_array().unwrap().len(), 8);
        assert_eq!(full["complete"], true);
        assert_eq!(full["items"][0]["lines"], old["items"][0]["lines"]);
        ranges[0]["expected_revision"] = json!("stale");
        ranges[1]["path"] = json!(".git/config");
        ranges[2]["path"] = json!("missing");
        ranges[3]["end_line"] = json!(200);
        request.operation = serde_json::from_value(json!({"kind":"read","ranges":ranges})).unwrap();
        let partial = f.read(&request);
        assert_eq!(partial["items"][0]["status"], "revision_mismatch");
        assert_eq!(partial["items"][1]["status"], "unsafe_or_unlisted");
        assert_eq!(partial["items"][2]["status"], "unsafe_or_unlisted");
        assert_eq!(partial["items"][3]["status"], "range_out_of_bounds");
        assert_eq!(partial["complete"], false);
        assert!(partial["next_cursor"].is_null());
        ranges.push(ranges[7].clone());
        request.operation = serde_json::from_value(json!({"kind":"read","ranges":ranges})).unwrap();
        assert!(read(f.temp.path(), &request).is_err());
        request.operation=serde_json::from_value(json!({"kind":"read","path":"src/入口.tsx","start_line":1,"end_line":1,"ranges":[{"path":"src/入口.tsx","start_line":1,"end_line":1}]})).unwrap();
        assert!(read(f.temp.path(), &request).is_err());
        fs::write(f.temp.path().join("src/入口.tsx"), "x".repeat(40000)).unwrap();
        request.target = Target::Workspace {
            workspace_id: Uuid::from_u128(3),
        };
        request.operation=serde_json::from_value(json!({"kind":"read","ranges":[{"path":"src/入口.tsx","start_line":1,"end_line":1},{"path":"src/入口.tsx","start_line":1,"end_line":1}]})).unwrap();
        let bounded = f.read(&request);
        assert_eq!(bounded["items"][1]["status"], "output_byte_limit");
        assert!(bounded["item_bytes"].as_u64().unwrap() <= 65536);
        assert_eq!(bounded["complete"], false);
        request.target = Target::Commit {
            r#ref: "main".into(),
        };
        assert_eq!(f.read(&request)["complete"], true);
    }
}

#[cfg(test)]
mod more_tests {
    use std::fs;

    use super::*;
    #[test]
    fn escaped_output_budget_and_in_file_continuation_lose_no_hits() {
        let temp = tempfile::tempdir().unwrap();
        let repo = Repository::init(temp.path()).unwrap();
        let line = format!("needle{}", "猫\t\"".repeat(150));
        let text = (0..100).map(|_| format!("{line}\n")).collect::<String>();
        let mut builder = repo.treebuilder(None).unwrap();
        builder
            .insert("large.txt", repo.blob(text.as_bytes()).unwrap(), 0o100644)
            .unwrap();
        let tree = repo.find_tree(builder.write().unwrap()).unwrap();
        let sig = git2::Signature::now("fixture", "fixture@example.invalid").unwrap();
        let sha = repo
            .commit(Some("refs/heads/main"), &sig, &sig, "large", &tree, &[])
            .unwrap();
        let mut r:Request=serde_json::from_value(json!({"project_id":Uuid::from_u128(1),"repository_id":Uuid::from_u128(2),"target":{"mode":"commit","ref":"main"},"operation":{"kind":"grep","query":"needle","syntax":"literal","case_sensitive":true,"output":"content","context_lines":2},"limit":50})).unwrap();
        let mut lines = Vec::new();
        let mut saw_output_limit = false;
        loop {
            let v = read(temp.path(), &r).unwrap();
            assert!(v["item_bytes"].as_u64().unwrap() <= OUTPUT as u64);
            let bytes: usize = v["items"]
                .as_array()
                .unwrap()
                .iter()
                .chain(v["exclusions"].as_array().unwrap())
                .map(|i| i.to_string().len() + 1)
                .sum();
            assert_eq!(v["item_bytes"], bytes);
            lines.extend(
                v["items"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|i| i["line"].as_u64().unwrap()),
            );
            saw_output_limit |= v["stop_reason"] == "output_byte_limit";
            if v["complete"] == true {
                break;
            }
            r.cursor = Some(v["next_cursor"].as_str().unwrap().into());
        }
        assert!(saw_output_limit);
        assert_eq!(lines, (1..=100).collect::<Vec<_>>());
        r.cursor = None;
        r.expected_revision = Some(
            read(temp.path(), &r).unwrap()["revision"]
                .as_str()
                .unwrap()
                .into(),
        );
        r.target = Target::Commit {
            r#ref: sha.to_string(),
        };
        r.operation = Operation::Read {
            ranges: vec![],
            path: "large.txt".into(),
            start_line: 1,
            end_line: 1,
        };
        assert_eq!(read(temp.path(), &r).unwrap()["items"][0]["lines"][0], line);
    }
    #[cfg(unix)]
    #[test]
    fn live_parent_symlink_never_reads_external_content() {
        let temp = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let repo = Repository::init(temp.path()).unwrap();
        fs::create_dir(temp.path().join("src")).unwrap();
        fs::write(temp.path().join("src/file"), "safe\n").unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(Path::new("src/file")).unwrap();
        index.write().unwrap();
        let tree = repo.find_tree(index.write_tree().unwrap()).unwrap();
        let sig = git2::Signature::now("fixture", "fixture@example.invalid").unwrap();
        repo.commit(Some("HEAD"), &sig, &sig, "safe", &tree, &[])
            .unwrap();
        fs::write(outside.path().join("file"), "outside-private\n").unwrap();
        fs::rename(temp.path().join("src"), temp.path().join("saved")).unwrap();
        std::os::unix::fs::symlink(outside.path(), temp.path().join("src")).unwrap();
        let r:Request=serde_json::from_value(json!({"project_id":Uuid::from_u128(1),"repository_id":Uuid::from_u128(2),"target":{"mode":"workspace","workspace_id":Uuid::from_u128(3)},"operation":{"kind":"read","path":"src/file","start_line":1,"end_line":1}})).unwrap();
        assert!(read(temp.path(), &r).is_err());
    }
}

#[cfg(test)]
mod measurements {
    use super::*;
    /// Explicit contributor measurement. Reads this checkout's committed HEAD only.
    /// No transport/network latency is measured by this engine-level harness.
    #[test]
    #[ignore]
    fn measure_repository_exploration() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let repo = Repository::open(&root).unwrap();
        let sha = resolve(&repo, "HEAD").unwrap().to_string();
        let grep = |q: &str, dir: &str, file: Option<&str>, syntax: Syntax| Operation::Grep {
            query: q.into(),
            syntax,
            case_sensitive: true,
            output: Output::Content,
            context_lines: 1,
            filter: Filter {
                directory_prefix: Some(dir.into()),
                filename_contains: file.map(str::to_owned),
                glob: None,
            },
        };
        let range = |path: &str, start, end| Operation::Read {
            ranges: vec![],
            path: path.into(),
            start_line: start,
            end_line: end,
        };
        let operations = vec![
            grep(
                "LocalProjectKanban",
                "packages/local-web/src/routes",
                None,
                Syntax::Literal,
            ),
            range(
                "packages/local-web/src/routes/_app.projects.$projectId.tsx",
                1,
                8,
            ),
            range(
                "packages/web-core/src/pages/kanban/LocalProjectKanban.tsx",
                1,
                5,
            ),
            grep(
                "KanbanContainer",
                "packages/web-core/src/pages/kanban",
                Some("ProjectKanban.tsx"),
                Syntax::Literal,
            ),
            range(
                "packages/web-core/src/pages/kanban/ProjectKanban.tsx",
                1,
                15,
            ),
            range(
                "packages/web-core/src/pages/kanban/ProjectKanban.tsx",
                109,
                115,
            ),
            grep(
                "KanbanBoard|KanbanGrid",
                "packages/web-core/src/features/kanban/ui",
                Some("KanbanContainer.tsx"),
                Syntax::Regex,
            ),
            range(
                "packages/web-core/src/features/kanban/ui/KanbanContainer.tsx",
                50,
                65,
            ),
            range(
                "packages/web-core/src/features/kanban/ui/KanbanContainer.tsx",
                1094,
                1145,
            ),
        ];
        let started = Instant::now();
        let mut calls = 0;
        let mut bytes = 0;
        let mut evidence = Vec::new();
        for operation in operations {
            let mut request = Request {
                project_id: Uuid::from_u128(1),
                repository_id: Uuid::from_u128(2),
                target: Target::Commit { r#ref: sha.clone() },
                operation,
                limit: 20,
                cursor: None,
                expected_revision: None,
            };
            loop {
                let v = read(&root, &request).unwrap();
                calls += 1;
                bytes += v.to_string().len();
                assert_eq!(v["source_head"], sha);
                assert_eq!(v["exclusions_total"], 0);
                evidence.push(json!({"kind":v["operation"]["kind"],"items":v["items"].as_array().unwrap().len(),"bytes":v.to_string().len(),"engine_ms":v["elapsed_ms"],"complete":v["complete"]}));
                if v["complete"] == true {
                    break;
                }
                request.cursor = Some(v["next_cursor"].as_str().unwrap().into());
            }
        }
        println!(
            "{}",
            json!({"scenario":"Kanban route/import/usage trace","sha":sha,"execution":"in_process_shared_engine","logical_tool_calls":calls,"wire_round_trips":null,"response_bytes":bytes,"wall_ms":started.elapsed().as_millis(),"completed":true,"pages":evidence})
        );
    }
}
