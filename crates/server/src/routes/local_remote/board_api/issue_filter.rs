//! Read-only resolution and filtering shared by the two MCP transports.
use api_types::*;
use sqlx::SqlitePool;
use uuid::Uuid;

use crate::{error::ApiError, routes::local_remote};

pub(crate) async fn filter_project_issues(
    pool: &SqlitePool,
    project: Uuid,
    filters: &IssueFilters,
) -> Result<Vec<Issue>, ApiError> {
    let statuses = local_remote::list_project_statuses(pool, project).await?;
    let project_tags = local_remote::list_project_tags(pool, project).await?;
    let mut q: SearchIssuesRequest =
        serde_json::from_value(serde_json::json!({"project_id":project})).expect("empty search");
    q.search = filters.search.clone();
    q.simple_id = filters.simple_id.clone();
    q.parent_issue_id = filters.parent_issue_id;
    q.assignee_user_id = filters.assignee_user_id;
    q.tag_id = filters.tag_id;
    if let Some(priority) = &filters.priority {
        q.priority = Some(match priority.trim().to_ascii_lowercase().as_str() {
            "urgent" => IssuePriority::Urgent,
            "high" => IssuePriority::High,
            "medium" => IssuePriority::Medium,
            "low" => IssuePriority::Low,
            _ => {
                return Err(ApiError::BadRequest(
                    "Unknown priority. Allowed values: urgent, high, medium, low".into(),
                ));
            }
        });
    }
    if let Some(status) = &filters.status {
        match Uuid::parse_str(status) {
            Ok(id) => q.status_id = Some(id),
            Err(_) => {
                q.status_ids = Some(
                    statuses
                        .iter()
                        .filter(|s| s.name.eq_ignore_ascii_case(status))
                        .map(|s| s.id)
                        .collect(),
                )
            }
        }
    }
    if let Some(name) = &filters.tag_name {
        q.tag_ids = Some(
            project_tags
                .iter()
                .filter(|t| t.name.eq_ignore_ascii_case(name))
                .map(|t| t.id)
                .collect(),
        );
    }
    // tag_id and tag_name describe the same tag, not two independent Issue tags.
    // Preserve stdio's former resolve_tag_filters intersection contract.
    if let (Some(id), Some(ids)) = (q.tag_id, q.tag_ids.as_ref())
        && !ids.contains(&id)
    {
        return Ok(Vec::new());
    }
    let tags = local_remote::list_project_issue_tags(pool, project).await?;
    let assignees = local_remote::list_project_issue_assignees(pool, project).await?;
    let mut issues = local_remote::list_project_issues(pool, project).await?;
    issues.retain(|i| matches(i, &tags, &assignees, &q));
    Ok(issues)
}

pub(super) fn matches(
    i: &Issue,
    tags: &[IssueTag],
    assignees: &[IssueAssignee],
    q: &SearchIssuesRequest,
) -> bool {
    q.status_id.is_none_or(|id| i.status_id == id)
        && q.status_ids
            .as_ref()
            .is_none_or(|ids| ids.contains(&i.status_id))
        && q.priority.is_none_or(|p| i.priority == Some(p))
        && q.parent_issue_id
            .is_none_or(|id| i.parent_issue_id == Some(id))
        && db::models::text_search::matches(
            q.search.as_deref(),
            &[&i.title, i.description.as_deref().unwrap_or("")],
        )
        && q.simple_id
            .as_ref()
            .is_none_or(|s| i.simple_id.eq_ignore_ascii_case(s))
        && q.assignee_user_id.is_none_or(|id| {
            assignees
                .iter()
                .any(|a| a.issue_id == i.id && a.user_id == id)
        })
        && q.tag_id
            .is_none_or(|id| tags.iter().any(|t| t.issue_id == i.id && t.tag_id == id))
        && q.tag_ids.as_ref().is_none_or(|ids| {
            tags.iter()
                .any(|t| t.issue_id == i.id && ids.contains(&t.tag_id))
        })
}
