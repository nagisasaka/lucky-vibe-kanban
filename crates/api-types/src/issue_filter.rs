//! Filter input shared by stdio and HTTP MCP; paging remains transport-specific.
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{IssueSortField, SortDirection};

#[derive(Debug, Default, Clone, PartialEq, Deserialize, Serialize, JsonSchema)]
pub struct IssueFilters {
    /// Case-insensitive substring of the full title or description. Whitespace is literal.
    pub search: Option<String>,
    /// Case-insensitive exact simple ID.
    pub simple_id: Option<String>,
    /// Status name (case-insensitive) or UUID.
    pub status: Option<String>,
    /// urgent, high, medium or low.
    pub priority: Option<String>,
    pub parent_issue_id: Option<Uuid>,
    pub assignee_user_id: Option<Uuid>,
    pub tag_id: Option<Uuid>,
    /// Case-insensitive exact tag name. When tag_id is supplied, that ID must have this name.
    pub tag_name: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct FilterIssuesRequest {
    pub project_id: Uuid,
    #[serde(flatten)]
    pub filters: IssueFilters,
    pub sort_field: Option<IssueSortField>,
    pub sort_direction: Option<SortDirection>,
    pub limit: Option<i32>,
    pub offset: Option<i32>,
}
