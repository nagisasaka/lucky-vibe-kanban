use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

use crate::some_if_present;

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct Project {
    pub id: Uuid,
    pub organization_id: Uuid,
    pub name: String,
    pub key: String,
    pub color: String,
    pub sort_order: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize, TS)]
pub struct CreateProjectRequest {
    /// Optional client-generated ID. If not provided, server generates one.
    /// Using client-generated IDs enables stable optimistic updates.
    #[ts(optional)]
    pub id: Option<Uuid>,
    pub organization_id: Uuid,
    pub name: String,
    pub key: String,
    pub color: String,
}

#[derive(Debug, Clone, Deserialize, TS)]
pub struct UpdateProjectRequest {
    #[serde(default, deserialize_with = "some_if_present")]
    pub name: Option<String>,
    #[serde(default, deserialize_with = "some_if_present")]
    pub color: Option<String>,
    #[serde(default, deserialize_with = "some_if_present")]
    pub sort_order: Option<i32>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ListProjectsQuery {
    pub organization_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct ListProjectsResponse {
    pub projects: Vec<Project>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BulkUpdateProjectItem {
    pub id: Uuid,
    #[serde(flatten)]
    pub changes: UpdateProjectRequest,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BulkUpdateProjectsRequest {
    pub updates: Vec<BulkUpdateProjectItem>,
}

#[derive(Debug, Clone, Serialize)]
pub struct BulkUpdateProjectsResponse {
    pub data: Vec<Project>,
    pub txid: i64,
}

/// Keep this normalization aligned with the Project creation form.
pub fn normalize_project_key(value: &str) -> Result<String, &'static str> {
    let key = value.trim().to_ascii_uppercase();
    if !(2..=10).contains(&key.len())
        || !key.as_bytes()[0].is_ascii_uppercase()
        || !key
            .bytes()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
    {
        return Err("Project key must be 2–10 ASCII letters or digits, starting with a letter");
    }
    Ok(key)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn project_key_validation_and_normalization() {
        for (raw, expected) in [
            (" lvk ", "LVK"),
            ("ab2", "AB2"),
            ("ABCDEFGHIJ", "ABCDEFGHIJ"),
            ("A1", "A1"),
            ("\u{0085}ab\u{0085}", "AB"),
        ] {
            assert_eq!(normalize_project_key(raw), Ok(expected.into()));
        }
        for raw in [
            "",
            "A",
            "1A",
            "AB-C",
            "AB_C",
            "ＡＢ",
            "éa",
            "あい",
            "ABCDEFGHIJK",
            "\u{feff}AB",
        ] {
            assert!(normalize_project_key(raw).is_err(), "{raw}");
        }
    }
}
