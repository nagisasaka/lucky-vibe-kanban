//! Project-owned conditions. Results are derived data, never a saved fixed selection.
use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

use crate::{Issue, IssueAssignee, IssuePriority, IssueRelationship, IssueTag, ProjectStatus, Tag};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SavedFilterArchive {
    #[default]
    Active,
    Archived,
    All,
}
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SavedFilterTagMode {
    #[default]
    Any,
    All,
}
/// Absolute bounds are inclusive start, exclusive end. Relative periods use
/// [now - seconds, now], so an Issue created exactly now is included.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum SavedFilterPeriod {
    Absolute {
        start: Option<DateTime<Utc>>,
        end: Option<DateTime<Utc>>,
    },
    Relative {
        seconds: u32,
    },
}
/// Results relative to target_issue_id: parent/children are one hop; descendants
/// traverse child links (excluding target). Related is symmetric, one hop.
/// Blocking returns sources blocking target; blocked_by returns targets blocked
/// by target. These two follow explicit blocking edges for one hop only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SavedFilterRelationKind {
    Parent,
    Children,
    Descendants,
    Related,
    Blocking,
    BlockedBy,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SavedFilterRelation {
    pub kind: SavedFilterRelationKind,
    pub target_issue_id: Uuid,
}
/// Fields combine with AND. Nonempty selections within a field combine with OR,
/// except tag_ids with tag_mode=all. Empty selections impose no restriction.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, TS, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct MachineFilter {
    /// Case-insensitive literal substring of title/description; whitespace is literal.
    pub search: Option<String>,
    pub status_ids: Vec<Uuid>,
    pub priorities: Vec<IssuePriority>,
    pub assignee_ids: Vec<Uuid>,
    pub tag_ids: Vec<Uuid>,
    pub tag_mode: SavedFilterTagMode,
    pub archived: SavedFilterArchive,
    pub created_at: Option<SavedFilterPeriod>,
    pub updated_at: Option<SavedFilterPeriod>,
    pub relation: Option<SavedFilterRelation>,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SavedFilterDefinition {
    pub name: String,
    #[serde(default)]
    pub machine: MachineFilter,
    pub natural_language: Option<String>,
    pub context: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct SavedFilter {
    pub id: Uuid,
    pub project_id: Uuid,
    pub name: String,
    pub machine: MachineFilter,
    pub natural_language: Option<String>,
    pub context: Option<String>,
    pub revision: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CreateSavedFilterRequest {
    pub request_id: Uuid,
    pub definition: SavedFilterDefinition,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct UpdateSavedFilterRequest {
    pub request_id: Uuid,
    pub expected_revision: i32,
    pub definition: SavedFilterDefinition,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DeleteSavedFilterRequest {
    pub request_id: Uuid,
    pub expected_revision: i32,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SavedFilterEvaluationState {
    Unevaluated,
    Evaluating,
    Complete,
    Failed,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct SavedFilterEvaluation {
    pub saved_filter_id: Uuid,
    pub project_id: Uuid,
    pub state: SavedFilterEvaluationState,
    pub issue_ids: Vec<Uuid>,
    pub total_count: usize,
    pub offset: usize,
    pub limit: usize,
    pub next_offset: Option<usize>,
    pub stale: bool,
    pub error: Option<String>,
    pub definition_revision: i32,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize, TS, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct SavedFilterPage {
    pub offset: u32,
    /// Default 100, maximum 500. Pagination is applied after complete evaluation.
    pub limit: Option<u32>,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct ListSavedFiltersResponse {
    pub saved_filters: Vec<SavedFilter>,
    pub total_count: usize,
    pub offset: usize,
    pub limit: usize,
    pub next_offset: Option<usize>,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct SavedFilterLink {
    pub saved_filter_id: Uuid,
    pub project_id: Uuid,
    /// Same-origin relative URL. MCP resolves it against the public base URL.
    pub url: String,
}
/// Full Project evidence, including archived/noncandidate Issues. Consumers must
/// include definition.context and all these fields in semantic invalidation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedFilterCorpus {
    pub issues: Vec<Issue>,
    pub statuses: Vec<ProjectStatus>,
    pub tags: Vec<Tag>,
    pub issue_tags: Vec<IssueTag>,
    pub assignees: Vec<IssueAssignee>,
    pub relationships: Vec<IssueRelationship>,
}
/// Explicit retry is separate from activate/poll, to avoid repeated provider failures.
#[derive(Debug, Clone, Default, Serialize, Deserialize, TS, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct EvaluateSavedFilterRequest {
    pub offset: u32,
    pub limit: Option<u32>,
    pub retry_failed: bool,
}
