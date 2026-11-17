//! Project-owned portable data with optional, display-only completed history.
//!
//! This is a deliberately partial transfer core, not a whole-instance backup.
//! Callers must authorize export/import and present `coverage()` before export.
//! Native/provider histories, execution state and attachment bytes are excluded.
use std::{
    collections::{BTreeMap, HashSet},
    path::Path,
};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::{Connection, FromRow, SqliteConnection, SqlitePool, sqlite::SqliteConnectOptions};
use uuid::Uuid;

fn is_false(value: &bool) -> bool {
    !value
}

const FORMAT: &str = "lvk-project-issues";
const VERSION: i64 = 2;
mod history;
pub use history::{HistoryManifest, HistoryMessage, ImportedHistory, list_imported_history};
const MAX_BYTES: u64 = 32 * 1024 * 1024;
const TABLE_SQL: &str =
    "CREATE TABLE package (id INTEGER PRIMARY KEY CHECK(id=1), payload TEXT NOT NULL)";

#[derive(Debug, thiserror::Error)]
pub enum TransferError {
    #[error("{0}")]
    Invalid(String),
    #[error(transparent)]
    Database(#[from] sqlx::Error),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}
type Result<T> = std::result::Result<T, TransferError>;
fn invalid(message: &str) -> TransferError {
    TransferError::Invalid(message.into())
}

/// Must be shown by UI/API callers. Free text is intentionally preserved;
/// it can contain sensitive text and is not claimed to be sanitized.
pub fn coverage() -> &'static str {
    "Includes Project/Issue bodies, versions, comments, internal relationships and uniquely Project-owned DB-confirmed semantic Manifests. Completed canonical user messages and assistant final answers require explicit opt-in. Free text can contain sensitive information; it is not fully sanitized. Excludes provider raw events, thinking, tool I/O, auth, structured secrets, environment, native sessions/memory, legacy logs, execution/queue/locks, Design Notes, Todos, Workflow definitions, assignees/followers/reactions, all nonallowlisted Project data, Git and attachment bytes. Attachment summaries indicate missing files. Imported history is read-only, with no native resume. Legacy logs and unsupported/ambiguous records are omitted."
}

macro_rules! record {
    ($name:ident { $($field:ident : $ty:ty),* $(,)? }) => {
        #[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
        #[serde(deny_unknown_fields)]
        struct $name { $( $field: $ty, )* }
    }
}
record!(Project {
    id: Uuid,
    name: String,
    issue_number_high_water: i64,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>
});
record!(Metadata {
    color: String,
    sort_order: i64
});
record!(Status {
    id: Uuid,
    name: String,
    color: String,
    sort_order: i64,
    hidden: bool
});
record!(Issue {
    id: Uuid, issue_number: i64, status_id: Uuid, title: String,
    description: Option<String>, priority: Option<String>, start_date: Option<DateTime<Utc>>,
    target_date: Option<DateTime<Utc>>, completed_at: Option<DateTime<Utc>>, sort_order: f64,
    parent_issue_id: Option<Uuid>, parent_issue_sort_order: Option<f64>,
    created_at: DateTime<Utc>, updated_at: DateTime<Utc>, archived: bool, current_version_id: Uuid, current_version_revision: i64,
});
record!(Version { id: Uuid, issue_id: Uuid, name: String, description: Option<String>, revision: i64 });
record!(Comment { id: Uuid, issue_id: Uuid, parent_id: Option<Uuid>, message: String, created_at: DateTime<Utc>, updated_at: DateTime<Utc> });
record!(Relationship {
    id: Uuid,
    issue_id: Uuid,
    related_issue_id: Uuid,
    relationship_type: String,
    created_at: DateTime<Utc>
});
record!(Tag {
    id: Uuid,
    name: String,
    color: String
});
record!(IssueTag {
    id: Uuid,
    issue_id: Uuid,
    tag_id: Uuid
});
record!(Repository {
    id: Uuid,
    display_name: String
});
record!(MissingAttachment { original_name: String, size_bytes: Option<i64> });

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Snapshot {
    project: Project,
    metadata: Option<Metadata>,
    statuses: Vec<Status>,
    issues: Vec<Issue>,
    versions: Vec<Version>,
    comments: Vec<Comment>,
    relationships: Vec<Relationship>,
    tags: Vec<Tag>,
    issue_tags: Vec<IssueTag>,
    repositories: Vec<Repository>,
    missing_attachments: Vec<MissingAttachment>,
    external_reference_count: i64,
    #[serde(default, skip_serializing_if = "is_false")]
    history_included: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    history: Vec<ImportedHistory>,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Package {
    format: String,
    version: i64,
    export_id: Uuid,
    data: Snapshot,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TransferReport {
    pub project_id: Uuid,
    pub export_id: Uuid,
    pub coverage: String,
    pub missing_attachments: Vec<AttachmentSummary>,
    pub external_reference_count: i64,
    pub history_included: bool,
    pub history_count: usize,
    pub manifest_count: usize,
    pub history_omissions: String,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct AttachmentSummary {
    pub name: String,
    pub size_bytes: Option<i64>,
}

/// Reads only allowlisted columns in one SQLite snapshot. No Repo paths, scripts,
/// arbitrary JSON metadata, actor IDs or execution state enter the package.
async fn snapshot(pool: &SqlitePool, project_id: Uuid, include_history: bool) -> Result<Snapshot> {
    let mut tx = pool.begin().await?;
    let project = sqlx::query_as::<_, Project>(
        "SELECT id,name,issue_number_high_water,created_at,updated_at FROM projects WHERE id=?",
    )
    .bind(project_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| invalid("Project not found"))?;
    macro_rules! rows {
        ($ty:ty, $sql:expr) => {
            sqlx::query_as::<_, $ty>($sql)
                .bind(project_id)
                .fetch_all(&mut *tx)
                .await?
        };
    }
    let metadata = sqlx::query_as::<_, Metadata>(
        "SELECT color,sort_order FROM local_project_metadata WHERE project_id=?",
    )
    .bind(project_id)
    .fetch_optional(&mut *tx)
    .await?;
    let statuses = rows!(
        Status,
        "SELECT id,name,color,sort_order,hidden FROM local_project_statuses WHERE project_id=? ORDER BY id"
    );
    let mut issues = rows!(
        Issue,
        "SELECT id,issue_number,status_id,title,description,priority,start_date,target_date,completed_at,sort_order,parent_issue_id,parent_issue_sort_order,created_at,updated_at,archived,current_version_id,current_version_revision FROM local_issues WHERE project_id=? ORDER BY id"
    );
    let versions = rows!(
        Version,
        "SELECT v.id,v.issue_id,v.name,v.description,v.revision FROM issue_versions v JOIN local_issues i ON i.id=v.issue_id WHERE i.project_id=? ORDER BY v.id"
    );
    let mut comments = rows!(
        Comment,
        "SELECT c.id,c.issue_id,c.parent_id,c.message,c.created_at,c.updated_at FROM local_issue_comments c JOIN local_issues i ON i.id=c.issue_id WHERE i.project_id=? ORDER BY c.id"
    );
    let mut relationships = rows!(
        Relationship,
        "SELECT r.id,r.issue_id,r.related_issue_id,r.relationship_type,r.created_at FROM local_issue_relationships r JOIN local_issues i ON i.id=r.issue_id WHERE i.project_id=? ORDER BY r.id"
    );
    let tags = rows!(
        Tag,
        "SELECT id,name,color FROM local_tags WHERE project_id=? ORDER BY id"
    );
    let mut issue_tags = rows!(
        IssueTag,
        "SELECT t.id,t.issue_id,t.tag_id FROM local_issue_tags t JOIN local_issues i ON i.id=t.issue_id WHERE i.project_id=? ORDER BY t.id"
    );
    let mut repositories = rows!(
        Repository,
        "SELECT r.id,r.display_name FROM repos r JOIN project_repos p ON p.repo_id=r.id WHERE p.project_id=? ORDER BY r.id"
    );
    // Summaries only; never copy a file_path, URL or content hash which might
    // accidentally resolve against pre-existing destination attachment storage.
    let mut missing_attachments = rows!(
        MissingAttachment,
        "SELECT a.original_name,a.size_bytes FROM attachments a WHERE a.id IN (SELECT attachment_id FROM project_attachments WHERE project_id=?) ORDER BY a.id"
    );
    let ids: HashSet<_> = issues.iter().map(|i| i.id).collect();
    let tag_ids: HashSet<_> = tags.iter().map(|t| t.id).collect();
    let comment_owners: BTreeMap<_, _> = comments.iter().map(|c| (c.id, c.issue_id)).collect();
    let mut external_reference_count = 0;
    for issue in &mut issues {
        if issue.parent_issue_id.is_some_and(|id| !ids.contains(&id)) {
            issue.parent_issue_id = None;
            issue.parent_issue_sort_order = None;
            external_reference_count += 1;
        }
    }
    for comment in &mut comments {
        if comment
            .parent_id
            .is_some_and(|id| comment_owners.get(&id) != Some(&comment.issue_id))
        {
            comment.parent_id = None;
            external_reference_count += 1;
        }
    }
    relationships.retain(|r| {
        let keep = ids.contains(&r.related_issue_id);
        if !keep {
            external_reference_count += 1;
        }
        keep
    });
    issue_tags.retain(|r| {
        let keep = tag_ids.contains(&r.tag_id);
        if !keep {
            external_reference_count += 1;
        }
        keep
    });
    let history =
        history::snapshot(&mut tx, project_id, include_history, &mut repositories).await?;
    // Selected completed Workspace attachments have no portable bytes or paths.
    let mut seen_workspaces = HashSet::new();
    for h in &history {
        if seen_workspaces.insert(h.workspace_id) {
            let mut summaries = sqlx::query_as::<_, MissingAttachment>("SELECT a.original_name,a.size_bytes FROM attachments a JOIN workspace_attachments wa ON wa.attachment_id=a.id WHERE wa.workspace_id=? ORDER BY a.id").bind(h.workspace_id).fetch_all(&mut *tx).await?;
            missing_attachments.append(&mut summaries);
        }
    }
    // Imported missing-file summaries survive subsequent exports. They remain
    // data, never attachment identities that could resolve at the destination.
    let reports: Vec<String> = sqlx::query_scalar("SELECT json_extract(report,'$.missing_attachments') FROM project_transfer_receipts WHERE project_id=?").bind(project_id).fetch_all(&mut *tx).await?;
    for raw in reports {
        let summaries: Vec<AttachmentSummary> = serde_json::from_str(&raw)?;
        missing_attachments.extend(summaries.into_iter().map(|a| MissingAttachment {
            original_name: a.name,
            size_bytes: a.size_bytes,
        }));
    }

    tx.commit().await?;
    Ok(Snapshot {
        project,
        metadata,
        statuses,
        issues,
        versions,
        comments,
        relationships,
        tags,
        issue_tags,
        repositories,
        missing_attachments,
        external_reference_count,
        history_included: include_history,
        history,
    })
}

/// Atomically publishes a standalone SQLite file; an existing path is never
/// replaced. The source pool is read-only in this operation.
pub async fn export_project(pool: &SqlitePool, project: Uuid, output: &Path) -> Result<Uuid> {
    export_project_with_options(pool, project, output, false).await
}

pub async fn export_project_with_options(
    pool: &SqlitePool,
    project: Uuid,
    output: &Path,
    include_history: bool,
) -> Result<Uuid> {
    let mut data = snapshot(pool, project, include_history).await?;
    canonicalize(&mut data);
    validate(&data)?;
    let package = Package {
        format: FORMAT.into(),
        version: VERSION,
        export_id: Uuid::new_v4(),
        data,
    };
    let payload = serde_json::to_string(&package)?;
    if payload.len() as u64 > MAX_BYTES / 2 {
        return Err(invalid("Project package exceeds size limit"));
    }
    let parent = output
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let temp = tempfile::NamedTempFile::new_in(parent)?;
    let mut db =
        SqliteConnection::connect_with(&SqliteConnectOptions::new().filename(temp.path())).await?;
    let result: Result<()> = async {
        sqlx::query(TABLE_SQL).execute(&mut db).await?;
        sqlx::query("INSERT INTO package VALUES(1,?)")
            .bind(payload)
            .execute(&mut db)
            .await?;
        Ok(())
    }
    .await;
    db.close().await?;
    result?;
    temp.as_file().sync_all()?;
    temp.persist_noclobber(output).map_err(|e| e.error)?;
    Ok(package.export_id)
}

async fn read_package(path: &Path) -> Result<Package> {
    let metadata = std::fs::symlink_metadata(path)?;
    if !metadata.is_file() || metadata.len() > MAX_BYTES {
        return Err(invalid("Expected a bounded regular SQLite package"));
    }
    let mut db = SqliteConnection::connect_with(
        &SqliteConnectOptions::new()
            .filename(path)
            .read_only(true)
            // A package is one standalone file, never an adjacent WAL/journal.
            .immutable(true)
            .pragma("trusted_schema", "OFF")
            .pragma("query_only", "ON"),
    )
    .await?;
    let result: Result<Package> = async {
        // Pin schema and payload to the same read-only SQLite snapshot.
        let mut tx = db.begin().await?;
        // Do not interpret views, triggers, virtual tables or supplied schema SQL.
        let schema: Vec<(String, String, String)> = sqlx::query_as(
            "SELECT type,name,sql FROM sqlite_schema WHERE name NOT LIKE 'sqlite_%' ORDER BY name",
        )
        .fetch_all(&mut *tx)
        .await?;
        if schema != vec![("table".into(), "package".into(), TABLE_SQL.into())] {
            return Err(invalid("Unsupported package schema"));
        }
        let rows: Vec<(i64, String)> = sqlx::query_as("SELECT id,payload FROM package LIMIT 2")
            .fetch_all(&mut *tx)
            .await?;
        if rows.len() != 1 || rows[0].0 != 1 || rows[0].1.len() as u64 > MAX_BYTES / 2 {
            return Err(invalid("Invalid package envelope"));
        }
        let mut package: Package = serde_json::from_str(&rows[0].1)?;
        if package.format != FORMAT
            || !(1..=VERSION).contains(&package.version)
            || package.export_id.is_nil()
        {
            return Err(invalid("Unsupported Project transfer format/version"));
        }
        if package.version == 1
            && (package.data.history_included || !package.data.history.is_empty())
        {
            return Err(invalid("History requires package version 2"));
        }
        canonicalize(&mut package.data);
        validate(&package.data)?;
        tx.commit().await?;
        Ok(package)
    }
    .await;
    db.close().await?;
    result
}

// Export identity and incidental JSON/row ordering are excluded from the content
// fingerprint. A reordered copy with a new export UUID is still a retry.
fn canonicalize(data: &mut Snapshot) {
    data.history.sort_by_key(|h| h.run_id);
    for h in &mut data.history {
        h.manifests.sort_by_key(|m| (m.repo_id, m.id));
    }
    data.statuses.sort_by_key(|r| r.id);
    data.issues.sort_by_key(|r| r.id);
    data.versions.sort_by_key(|r| r.id);
    data.comments.sort_by_key(|r| r.id);
    data.relationships.sort_by_key(|r| r.id);
    data.tags.sort_by_key(|r| r.id);
    data.issue_tags.sort_by_key(|r| r.id);
    data.repositories.sort_by_key(|r| r.id);
    data.missing_attachments
        .sort_by(|a, b| (&a.original_name, a.size_bytes).cmp(&(&b.original_name, b.size_bytes)));
}

fn validate(data: &Snapshot) -> Result<()> {
    let mut all = HashSet::new();
    macro_rules! ids {
        ($rows:expr) => {{
            let mut set = HashSet::new();
            for row in $rows {
                if row.id.is_nil() || !all.insert(row.id) {
                    return Err(invalid("Duplicate or nil record identity"));
                }
                set.insert(row.id);
            }
            set
        }};
    }
    all.insert(data.project.id);
    let statuses = ids!(&data.statuses);
    let issues = ids!(&data.issues);
    let versions = ids!(&data.versions);
    let comments = ids!(&data.comments);
    let tags = ids!(&data.tags);
    ids!(&data.relationships);
    ids!(&data.issue_tags);
    ids!(&data.repositories);
    if data.project.id.is_nil()
        || data.project.issue_number_high_water < 0
        || data.external_reference_count < 0
    {
        return Err(invalid("Invalid Project metadata"));
    }
    let version_by_id: BTreeMap<_, _> = data.versions.iter().map(|v| (v.id, v)).collect();
    let comment_by_id: BTreeMap<_, _> = data.comments.iter().map(|c| (c.id, c)).collect();
    let mut numbers = HashSet::new();
    for i in &data.issues {
        if i.issue_number <= 0
            || i.issue_number > i64::from(i32::MAX)
            || i.issue_number > data.project.issue_number_high_water
            || !numbers.insert(i.issue_number)
            || !statuses.contains(&i.status_id)
            || !versions.contains(&i.current_version_id)
            || !version_by_id
                .get(&i.current_version_id)
                .is_some_and(|v| v.issue_id == i.id && v.description == i.description)
            || i.current_version_revision < 0
            || i.parent_issue_id
                .is_some_and(|p| !issues.contains(&p) || p == i.id)
            || !i.sort_order.is_finite()
        {
            return Err(invalid("Invalid Issue reference closure"));
        }
    }
    if data
        .versions
        .iter()
        .any(|v| !issues.contains(&v.issue_id) || v.revision < 0)
        || data.comments.iter().any(|c| {
            !issues.contains(&c.issue_id)
                || c.parent_id.is_some_and(|p| {
                    !comments.contains(&p)
                        || p == c.id
                        || comment_by_id
                            .get(&p)
                            .is_none_or(|parent| parent.issue_id != c.issue_id)
                })
        })
        || data.relationships.iter().any(|r| {
            r.issue_id == r.related_issue_id
                || !issues.contains(&r.issue_id)
                || !issues.contains(&r.related_issue_id)
        })
        || data
            .issue_tags
            .iter()
            .any(|t| !issues.contains(&t.issue_id) || !tags.contains(&t.tag_id))
    {
        return Err(invalid("Invalid Project reference closure"));
    }
    history::validate(
        &data.history,
        data.history_included,
        &issues,
        &all,
        &data.repositories,
    )?;
    validate_parents(
        data.issues
            .iter()
            .map(|i| (i.id, i.parent_issue_id))
            .collect(),
    )?;
    validate_parents(data.comments.iter().map(|c| (c.id, c.parent_id)).collect())?;
    Ok(())
}

fn validate_parents(parents: BTreeMap<Uuid, Option<Uuid>>) -> Result<()> {
    let mut finished = HashSet::new();
    for start in parents.keys() {
        let mut path = HashSet::new();
        let mut cursor = Some(*start);
        while let Some(id) = cursor {
            if finished.contains(&id) {
                break;
            }
            if !path.insert(id) {
                return Err(invalid("Cyclic parent references"));
            }
            cursor = parents.get(&id).copied().flatten();
        }
        finished.extend(path);
    }
    Ok(())
}

/// Adds a new Project only. Caller supplies destination organization and a
/// unique key, plus an exact mapping to already registered destination Repos.
/// Caller must NOT register/clone Repos as a side effect of this transaction.
pub async fn import_project(
    pool: &SqlitePool,
    input: &Path,
    key: &str,
    organization: Uuid,
    repositories: &BTreeMap<Uuid, Uuid>,
) -> Result<TransferReport> {
    if !(2..=10).contains(&key.len())
        || !key.as_bytes()[0].is_ascii_uppercase()
        || !key
            .bytes()
            .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit())
    {
        return Err(invalid(
            "Specify a unique Project key (2–10 uppercase letters/digits, starting with a letter)",
        ));
    }
    if organization.is_nil() {
        return Err(invalid("Destination organization is required"));
    }
    let package = read_package(input).await?;
    let data = &package.data;
    let expected: HashSet<_> = data.repositories.iter().map(|r| r.id).collect();
    if expected != repositories.keys().copied().collect()
        || repositories.values().copied().collect::<HashSet<_>>().len() != repositories.len()
    {
        return Err(invalid(
            "Provide an exact one-to-one mapping for all package Repos",
        ));
    }
    let hash = format!("{:x}", Sha256::digest(serde_json::to_vec(data)?));
    let mut tx = pool.begin_with("BEGIN IMMEDIATE").await?;
    let duplicate: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM project_transfer_receipts WHERE export_id=? OR content_hash=?",
    )
    .bind(package.export_id)
    .bind(&hash)
    .fetch_one(&mut *tx)
    .await?;
    if duplicate != 0 {
        return Err(invalid(
            "This export identity or snapshot was already imported",
        ));
    }
    let occupied: i64 = sqlx::query_scalar("SELECT count(*) FROM projects WHERE key=?")
        .bind(key)
        .fetch_one(&mut *tx)
        .await?;
    if occupied != 0 {
        return Err(invalid(
            "Project key already exists; choose a different key",
        ));
    }
    for repo in repositories.values() {
        let exists: i64 = sqlx::query_scalar("SELECT count(*) FROM repos WHERE id=?")
            .bind(repo)
            .fetch_one(&mut *tx)
            .await?;
        if exists != 1 {
            return Err(invalid("Mapped destination Repo is not registered"));
        }
    }
    // Self-referential parents and current-version pointers are checked at commit.
    sqlx::query("PRAGMA defer_foreign_keys=ON")
        .execute(&mut *tx)
        .await?;
    let project_id = Uuid::new_v4();
    let mut remap = BTreeMap::new();
    for id in std::iter::once(data.project.id)
        .chain(data.statuses.iter().map(|r| r.id))
        .chain(data.issues.iter().map(|r| r.id))
        .chain(data.versions.iter().map(|r| r.id))
        .chain(data.comments.iter().map(|r| r.id))
        .chain(data.tags.iter().map(|r| r.id))
        .chain(data.relationships.iter().map(|r| r.id))
        .chain(data.issue_tags.iter().map(|r| r.id))
    {
        remap.insert(id, Uuid::new_v4());
    }
    remap.insert(data.project.id, project_id);
    let id = |source: Uuid| remap[&source];
    let optional = |source: Option<Uuid>| source.map(id);
    sqlx::query("INSERT INTO projects(id,name,key,issue_number_high_water,created_at,updated_at) VALUES(?,?,?,?,?,?)").bind(project_id).bind(&data.project.name).bind(key).bind(data.project.issue_number_high_water).bind(data.project.created_at).bind(data.project.updated_at).execute(&mut *tx).await?;
    sqlx::query("INSERT INTO local_project_metadata(project_id,organization_id,color,sort_order) VALUES(?,?,?,?)").bind(project_id).bind(organization).bind(data.metadata.as_ref().map(|m|m.color.as_str()).unwrap_or("210 80% 52%")).bind(data.metadata.as_ref().map(|m|m.sort_order).unwrap_or(0)).execute(&mut *tx).await?;
    for r in &data.statuses {
        sqlx::query("INSERT INTO local_project_statuses(id,project_id,name,color,sort_order,hidden) VALUES(?,?,?,?,?,?)").bind(id(r.id)).bind(project_id).bind(&r.name).bind(&r.color).bind(r.sort_order).bind(r.hidden).execute(&mut *tx).await?;
    }
    for r in &data.issues {
        sqlx::query("INSERT INTO local_issues(id,project_id,issue_number,simple_id,status_id,title,description,priority,start_date,target_date,completed_at,sort_order,parent_issue_id,parent_issue_sort_order,created_at,updated_at,archived) VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)")
            .bind(id(r.id)).bind(project_id).bind(r.issue_number).bind(format!("{key}-{}",r.issue_number)).bind(id(r.status_id)).bind(&r.title).bind(&r.description).bind(&r.priority).bind(r.start_date).bind(r.target_date).bind(r.completed_at).bind(r.sort_order).bind(optional(r.parent_issue_id)).bind(r.parent_issue_sort_order).bind(r.created_at).bind(r.updated_at).bind(r.archived).execute(&mut *tx).await?;
    }
    for r in &data.versions {
        sqlx::query(
            "INSERT INTO issue_versions(id,issue_id,name,description,revision) VALUES(?,?,?,?,?)",
        )
        .bind(id(r.id))
        .bind(id(r.issue_id))
        .bind(&r.name)
        .bind(&r.description)
        .bind(r.revision)
        .execute(&mut *tx)
        .await?;
    }
    for r in &data.issues {
        // The ordinary insert trigger created an Initial version. Point at the
        // imported authoritative version before removing only that generated row.
        let generated: Uuid =
            sqlx::query_scalar("SELECT current_version_id FROM local_issues WHERE id=?")
                .bind(id(r.id))
                .fetch_one(&mut *tx)
                .await?;
        sqlx::query(
            "UPDATE local_issues SET current_version_id=?,current_version_revision=? WHERE id=?",
        )
        .bind(id(r.current_version_id))
        .bind(r.current_version_revision)
        .bind(id(r.id))
        .execute(&mut *tx)
        .await?;
        sqlx::query("DELETE FROM issue_versions WHERE id=?")
            .bind(generated)
            .execute(&mut *tx)
            .await?;
    }
    for r in &data.comments {
        sqlx::query("INSERT INTO local_issue_comments(id,issue_id,parent_id,message,created_at,updated_at) VALUES(?,?,?,?,?,?)").bind(id(r.id)).bind(id(r.issue_id)).bind(optional(r.parent_id)).bind(&r.message).bind(r.created_at).bind(r.updated_at).execute(&mut *tx).await?;
    }
    for r in &data.relationships {
        sqlx::query("INSERT INTO local_issue_relationships(id,issue_id,related_issue_id,relationship_type,created_at) VALUES(?,?,?,?,?)").bind(id(r.id)).bind(id(r.issue_id)).bind(id(r.related_issue_id)).bind(&r.relationship_type).bind(r.created_at).execute(&mut *tx).await?;
    }
    for r in &data.tags {
        sqlx::query("INSERT INTO local_tags(id,project_id,name,color) VALUES(?,?,?,?)")
            .bind(id(r.id))
            .bind(project_id)
            .bind(&r.name)
            .bind(&r.color)
            .execute(&mut *tx)
            .await?;
    }
    for r in &data.issue_tags {
        sqlx::query("INSERT INTO local_issue_tags(id,issue_id,tag_id) VALUES(?,?,?)")
            .bind(id(r.id))
            .bind(id(r.issue_id))
            .bind(id(r.tag_id))
            .execute(&mut *tx)
            .await?;
    }
    for repo in repositories.values() {
        sqlx::query("INSERT INTO project_repos(id,project_id,repo_id) VALUES(?,?,?)")
            .bind(Uuid::new_v4())
            .bind(project_id)
            .bind(repo)
            .execute(&mut *tx)
            .await?;
    }
    history::insert(&mut tx, project_id, &data.history, &remap, repositories).await?;
    let report = TransferReport {
        project_id,
        export_id: package.export_id,
        coverage: coverage().into(),
        external_reference_count: data.external_reference_count,
        history_included: data.history_included,
        history_count: data.history.len(),
        manifest_count: data.history.iter().map(|h| h.manifests.len()).sum(),
        history_omissions: history::OMISSIONS.into(),
        missing_attachments: data
            .missing_attachments
            .iter()
            .map(|a| AttachmentSummary {
                name: a.original_name.clone(),
                size_bytes: a.size_bytes,
            })
            .collect(),
    };
    sqlx::query("INSERT INTO project_transfer_receipts(export_id,content_hash,project_id,report) VALUES(?,?,?,?)").bind(package.export_id).bind(hash).bind(project_id).bind(serde_json::to_string(&report)?).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(report)
}

/// A bounded, validated preview for destination Repo selection. Contains no
/// conversation bodies or filesystem locations. Inspection never mutates a DB.
/// Callers must retain a private, stable input file throughout inspection/import.
#[derive(Debug, Serialize)]
pub struct PackagePreview {
    pub export_id: Uuid,
    pub project_name: String,
    pub issue_count: usize,
    pub version_count: usize,
    pub comment_count: usize,
    pub repositories: Vec<RepositorySelection>,
    pub coverage: String,
    pub external_reference_count: i64,
    pub history_included: bool,
    pub history_count: usize,
    pub manifest_count: usize,
    pub history_omissions: String,
    pub missing_attachments: Vec<AttachmentSummary>,
}
#[derive(Debug, Serialize)]
pub struct RepositorySelection {
    pub source_id: Uuid,
    pub display_name: String,
}

pub async fn inspect_package(path: &Path) -> Result<PackagePreview> {
    let package = read_package(path).await?;
    let d = package.data;
    Ok(PackagePreview {
        export_id: package.export_id,
        project_name: d.project.name,
        issue_count: d.issues.len(),
        version_count: d.versions.len(),
        comment_count: d.comments.len(),
        repositories: d
            .repositories
            .into_iter()
            .map(|r| RepositorySelection {
                source_id: r.id,
                display_name: r.display_name,
            })
            .collect(),
        coverage: coverage().into(),
        external_reference_count: d.external_reference_count,
        history_included: d.history_included,
        history_count: d.history.len(),
        manifest_count: d.history.iter().map(|h| h.manifests.len()).sum(),
        history_omissions: history::OMISSIONS.into(),
        missing_attachments: d
            .missing_attachments
            .into_iter()
            .map(|a| AttachmentSummary {
                name: a.original_name,
                size_bytes: a.size_bytes,
            })
            .collect(),
    })
}
