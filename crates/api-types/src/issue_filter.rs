//! Filter input shared by stdio and HTTP MCP; paging remains transport-specific.
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{IssueSortField, SortDirection};

/// Archive selection shared by MCP transports. Existing boolean inputs remain valid.
/// Omission/false selects unarchived Issues; true selects archived; "all" selects both.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize, Serialize, JsonSchema)]
#[serde(untagged)]
pub enum IssueArchiveFilter {
    State(bool),
    Scope(AllIssues),
}

#[derive(Debug, Clone, Copy, PartialEq, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AllIssues {
    All,
}

impl Default for IssueArchiveFilter {
    fn default() -> Self {
        Self::State(false)
    }
}

impl IssueArchiveFilter {
    pub fn value(self) -> Option<bool> {
        match self {
            Self::State(value) => Some(value),
            Self::Scope(AllIssues::All) => None,
        }
    }
}

#[derive(Debug, Default, Clone, PartialEq, Deserialize, Serialize, JsonSchema)]
pub struct IssueFilters {
    /// false (default): unarchived; true: archived; "all": both.
    #[serde(default)]
    pub archived: IssueArchiveFilter,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn archive_filter_defaults_and_explicit_selection() {
        for (input, expected) in [
            (serde_json::json!({}), Some(false)),
            (serde_json::json!({"archived":false}), Some(false)),
            (serde_json::json!({"archived":true}), Some(true)),
            (serde_json::json!({"archived":"all"}), None),
        ] {
            let filters: IssueFilters = serde_json::from_value(input).unwrap();
            assert_eq!(filters.archived.value(), expected);
            let roundtrip: IssueFilters =
                serde_json::from_value(serde_json::to_value(filters).unwrap()).unwrap();
            assert_eq!(roundtrip.archived.value(), expected);
        }
        for value in [
            serde_json::json!(null),
            serde_json::json!("active"),
            serde_json::json!(1),
        ] {
            assert!(
                serde_json::from_value::<IssueFilters>(serde_json::json!({"archived":value}))
                    .is_err()
            );
        }
    }
}
