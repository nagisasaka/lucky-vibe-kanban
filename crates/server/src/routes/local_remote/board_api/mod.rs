use axum::Router;

use crate::DeploymentImpl;

mod issue_assignees;
mod issue_relationships;
mod issue_tags;
mod issues;
mod project_statuses;
mod projects;
pub mod pull_requests;
mod tags;
mod workspaces;

pub fn router() -> Router<DeploymentImpl> {
    Router::new()
        .route("/organizations", axum::routing::get(organizations))
        .route("/organizations/{id}/members", axum::routing::get(members))
        .merge(issue_assignees::router())
        .merge(issue_relationships::router())
        .merge(issue_tags::router())
        .merge(issues::router())
        .merge(projects::router())
        .merge(project_statuses::router())
        .merge(pull_requests::router())
        .merge(tags::router())
        .merge(workspaces::router())
}

use api_types::*;
use axum::{
    Json,
    extract::{Path, State},
};
use deployment::Deployment;
use uuid::Uuid;

use crate::error::ApiError;
struct Store<'a> {
    deployment: &'a DeploymentImpl,
}
impl<'a> Store<'a> {
    fn new(deployment: &'a DeploymentImpl) -> Self {
        Self { deployment }
    }
    fn pool(&self) -> &sqlx::SqlitePool {
        &self.deployment.db().pool
    }
    async fn list_remote_projects(
        &self,
        organization_id: Uuid,
    ) -> Result<ListProjectsResponse, ApiError> {
        Ok(ListProjectsResponse {
            projects: if organization_id == super::local_org_id() {
                super::list_local_projects(self.pool()).await?
            } else {
                vec![]
            },
        })
    }
    async fn get_remote_project(&self, id: Uuid) -> Result<Project, ApiError> {
        super::get_local_project(self.pool(), id).await
    }
    async fn list_project_statuses(
        &self,
        id: Uuid,
    ) -> Result<ListProjectStatusesResponse, ApiError> {
        Ok(ListProjectStatusesResponse {
            project_statuses: super::ensure_default_statuses(self.pool(), id).await?,
        })
    }
    async fn get_issue(&self, id: Uuid) -> Result<Issue, ApiError> {
        super::get_local_issue(self.pool(), id).await
    }
    async fn list_issues(&self, project_id: Uuid) -> Result<ListIssuesResponse, ApiError> {
        let issues = super::list_project_issues(self.pool(), project_id).await?;
        let total_count = issues.len();
        Ok(ListIssuesResponse {
            issues,
            total_count,
            limit: total_count,
            offset: 0,
        })
    }
    async fn get_tag(&self, id: Uuid) -> Result<Tag, ApiError> {
        super::get_local_tag(self.pool(), id).await
    }
    async fn list_tags(&self, project_id: Uuid) -> Result<ListTagsResponse, ApiError> {
        Ok(ListTagsResponse {
            tags: super::list_project_tags(self.pool(), project_id).await?,
        })
    }
    async fn get_workspace_by_local_id(&self, id: Uuid) -> Result<Workspace, ApiError> {
        super::list_user_workspaces(self.pool())
            .await?
            .into_iter()
            .find(|w| w.local_workspace_id == Some(id))
            .ok_or_else(|| ApiError::BadRequest("Workspace has no linked issue".into()))
    }
    async fn list_pull_requests(
        &self,
        issue_id: Uuid,
    ) -> Result<ListPullRequestsResponse, ApiError> {
        self.get_issue(issue_id).await?;
        // The local Board currently has no issue/PR join records.
        Ok(ListPullRequestsResponse {
            pull_requests: vec![],
        })
    }
    async fn list_issue_tags(&self, issue_id: Uuid) -> Result<ListIssueTagsResponse, ApiError> {
        let issue = self.get_issue(issue_id).await?;
        let rows = super::list_project_issue_tags(self.pool(), issue.project_id).await?;
        Ok(ListIssueTagsResponse {
            issue_tags: rows
                .into_iter()
                .filter(|row| row.issue_id == issue_id)
                .collect(),
        })
    }
    async fn get_issue_tag(&self, id: Uuid) -> Result<IssueTag, ApiError> {
        let row = sqlx::query("SELECT * FROM local_issue_tags WHERE id = ?")
            .bind(id)
            .fetch_one(self.pool())
            .await?;
        Ok(super::issue_tag_from_row(&row)?)
    }
    async fn list_issue_assignees(
        &self,
        issue_id: Uuid,
    ) -> Result<ListIssueAssigneesResponse, ApiError> {
        let issue = self.get_issue(issue_id).await?;
        let rows = super::list_project_issue_assignees(self.pool(), issue.project_id).await?;
        Ok(ListIssueAssigneesResponse {
            issue_assignees: rows
                .into_iter()
                .filter(|row| row.issue_id == issue_id)
                .collect(),
        })
    }
    async fn get_issue_assignee(&self, id: Uuid) -> Result<IssueAssignee, ApiError> {
        let row = sqlx::query("SELECT * FROM local_issue_assignees WHERE id = ?")
            .bind(id)
            .fetch_one(self.pool())
            .await?;
        Ok(super::issue_assignee_from_row(&row)?)
    }
    async fn list_issue_relationships(
        &self,
        issue_id: Uuid,
    ) -> Result<ListIssueRelationshipsResponse, ApiError> {
        let issue = self.get_issue(issue_id).await?;
        let rows = super::list_project_issue_relationships(self.pool(), issue.project_id).await?;
        Ok(ListIssueRelationshipsResponse {
            issue_relationships: rows
                .into_iter()
                .filter(|row| row.issue_id == issue_id || row.related_issue_id == issue_id)
                .collect(),
        })
    }
    async fn create_issue(
        &self,
        request: &CreateIssueRequest,
    ) -> Result<MutationResponse<Issue>, ApiError> {
        Ok(
            super::create_issue(State(self.deployment.clone()), Json(request.clone()))
                .await?
                .0,
        )
    }
    async fn delete_issue(&self, id: Uuid) -> Result<(), ApiError> {
        let _ = super::delete_issue(State(self.deployment.clone()), Path(id)).await?;
        Ok(())
    }
    async fn create_issue_tag(
        &self,
        request: &CreateIssueTagRequest,
    ) -> Result<MutationResponse<IssueTag>, ApiError> {
        Ok(
            super::create_issue_tag(State(self.deployment.clone()), Json(request.clone()))
                .await?
                .0,
        )
    }
    async fn delete_issue_tag(&self, id: Uuid) -> Result<(), ApiError> {
        let _ = super::delete_issue_tag(State(self.deployment.clone()), Path(id)).await?;
        Ok(())
    }
    async fn create_issue_assignee(
        &self,
        request: &CreateIssueAssigneeRequest,
    ) -> Result<MutationResponse<IssueAssignee>, ApiError> {
        Ok(
            super::create_issue_assignee(State(self.deployment.clone()), Json(request.clone()))
                .await?
                .0,
        )
    }
    async fn delete_issue_assignee(&self, id: Uuid) -> Result<(), ApiError> {
        let _ = super::delete_issue_assignee(State(self.deployment.clone()), Path(id)).await?;
        Ok(())
    }
    async fn create_issue_relationship(
        &self,
        request: &CreateIssueRelationshipRequest,
    ) -> Result<MutationResponse<IssueRelationship>, ApiError> {
        Ok(
            super::create_issue_relationship(State(self.deployment.clone()), Json(request.clone()))
                .await?
                .0,
        )
    }
    async fn delete_issue_relationship(&self, id: Uuid) -> Result<(), ApiError> {
        let _ = super::delete_issue_relationship(State(self.deployment.clone()), Path(id)).await?;
        Ok(())
    }
    async fn update_issue(
        &self,
        id: Uuid,
        request: &UpdateIssueRequest,
    ) -> Result<MutationResponse<Issue>, ApiError> {
        Ok(super::update_issue(
            State(self.deployment.clone()),
            Path(id),
            Json(request.clone()),
        )
        .await?
        .0)
    }
    async fn search_issues(
        &self,
        request: &SearchIssuesRequest,
    ) -> Result<ListIssuesResponse, ApiError> {
        let issues = super::list_project_issues(self.pool(), request.project_id).await?;
        let tags = super::list_project_issue_tags(self.pool(), request.project_id).await?;
        let assignees =
            super::list_project_issue_assignees(self.pool(), request.project_id).await?;
        let statuses = super::ensure_default_statuses(self.pool(), request.project_id).await?;
        let status_order = statuses.into_iter().map(|s| (s.id, s.sort_order)).collect();
        Ok(search(issues, &tags, &assignees, &status_order, request))
    }
}
fn search(
    mut issues: Vec<Issue>,
    tags: &[IssueTag],
    assignees: &[IssueAssignee],
    status_order: &std::collections::HashMap<Uuid, i32>,
    q: &SearchIssuesRequest,
) -> ListIssuesResponse {
    issues.retain(|i| {
        q.status_id.is_none_or(|id| i.status_id == id)
            && q.status_ids
                .as_ref()
                .is_none_or(|ids| ids.contains(&i.status_id))
            && q.priority.is_none_or(|p| i.priority == Some(p))
            && q.parent_issue_id
                .is_none_or(|id| i.parent_issue_id == Some(id))
            && q.search.as_ref().is_none_or(|s| {
                i.title.to_lowercase().contains(&s.to_lowercase())
                    || i.description
                        .as_deref()
                        .unwrap_or("")
                        .to_lowercase()
                        .contains(&s.to_lowercase())
            })
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
    });
    let rank = |p| match p {
        Some(IssuePriority::Urgent) => 0,
        Some(IssuePriority::High) => 1,
        Some(IssuePriority::Medium) => 2,
        Some(IssuePriority::Low) => 3,
        None => 4,
    };
    issues.sort_by(|a, b| {
        let order = match q.sort_field.unwrap_or(IssueSortField::SortOrder) {
            IssueSortField::SortOrder => status_order
                .get(&a.status_id)
                .unwrap_or(&i32::MAX)
                .cmp(status_order.get(&b.status_id).unwrap_or(&i32::MAX))
                .then(a.sort_order.total_cmp(&b.sort_order)),
            IssueSortField::Priority => rank(a.priority).cmp(&rank(b.priority)),
            IssueSortField::CreatedAt => a.created_at.cmp(&b.created_at),
            IssueSortField::UpdatedAt => a.updated_at.cmp(&b.updated_at),
            IssueSortField::Title => a.title.cmp(&b.title),
        };
        let order = if q.sort_direction == Some(SortDirection::Desc) {
            order.reverse()
        } else {
            order
        };
        order.then(a.issue_number.cmp(&b.issue_number))
    });
    let total_count = issues.len();
    let offset = q.offset.unwrap_or(0).max(0) as usize;
    let limit = q
        .limit
        .map(|n| n.max(0) as usize)
        .unwrap_or_else(|| total_count.saturating_sub(offset));
    ListIssuesResponse {
        issues: issues.into_iter().skip(offset).take(limit).collect(),
        total_count,
        limit,
        offset,
    }
}
async fn organizations() -> Json<utils::response::ApiResponse<ListOrganizationsResponse>> {
    Json(utils::response::ApiResponse::success(
        super::list_organizations().await.0,
    ))
}
async fn members(
    deployment: State<DeploymentImpl>,
    id: Path<Uuid>,
) -> Result<Json<utils::response::ApiResponse<ListMembersResponse>>, ApiError> {
    Ok(Json(utils::response::ApiResponse::success(
        super::list_members(deployment, id).await?.0,
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn issue(number: u128, title: &str, order: f64) -> Issue {
        serde_json::from_value(serde_json::json!({
            "id": Uuid::from_u128(number), "project_id": Uuid::from_u128(10),
            "status_id": Uuid::from_u128(20), "issue_number": number as i32,
            "simple_id": format!("LOCAL-{number}"), "title": title,
            "description": "Searchable description", "priority": "high", "sort_order": order,
            "extension_metadata": {}, "created_at": "2026-01-01T00:00:00Z", "updated_at": "2026-01-01T00:00:00Z"
        })).unwrap()
    }
    fn query() -> SearchIssuesRequest {
        serde_json::from_value(serde_json::json!({"project_id": Uuid::from_u128(10)})).unwrap()
    }
    #[test]
    fn search_filters_before_pagination_and_preserves_total_count() {
        let mut q = query();
        q.search = Some("SEARCHABLE".into());
        q.sort_field = Some(IssueSortField::Title);
        q.limit = Some(1);
        q.offset = Some(1);
        let result = search(
            vec![
                issue(1, "Zulu", 0.0),
                issue(2, "Alpha", 1.0),
                issue(3, "Bravo", 2.0),
            ],
            &[],
            &[],
            &Default::default(),
            &q,
        );
        assert_eq!(result.total_count, 3);
        assert_eq!(result.issues[0].title, "Bravo");
        assert_eq!(result.limit, 1);
        assert_eq!(result.offset, 1);
        q.simple_id = Some("local-1".into());
        q.offset = Some(0);
        assert_eq!(
            search(
                vec![issue(1, "Zulu", 0.0), issue(2, "Alpha", 1.0)],
                &[],
                &[],
                &Default::default(),
                &q
            )
            .total_count,
            1
        );
    }
    #[test]
    fn search_orders_by_column_then_card_and_honours_zero_limit() {
        let mut first_column = issue(1, "First", 99.0);
        first_column.status_id = Uuid::from_u128(21);
        let second_column = issue(2, "Second", 0.0);
        let order = [(first_column.status_id, 0), (second_column.status_id, 1)]
            .into_iter()
            .collect();
        let mut q = query();
        let rows = vec![second_column, first_column];
        let result = search(rows.clone(), &[], &[], &order, &q);
        assert_eq!(result.issues[0].title, "First");
        assert_eq!(result.limit, 2);
        q.limit = Some(0);
        let result = search(rows, &[], &[], &order, &q);
        assert!(result.issues.is_empty());
        assert_eq!(result.total_count, 2);
    }

    #[test]
    fn search_combines_tag_assignee_and_status_filters() {
        let mut q = query();
        q.tag_id = Some(Uuid::from_u128(30));
        q.assignee_user_id = Some(Uuid::from_u128(1));
        let tags = vec![IssueTag {
            id: Uuid::new_v4(),
            issue_id: Uuid::from_u128(2),
            tag_id: Uuid::from_u128(30),
        }];
        let assignees = vec![IssueAssignee {
            id: Uuid::new_v4(),
            issue_id: Uuid::from_u128(2),
            user_id: Uuid::from_u128(1),
            assigned_at: chrono::Utc::now(),
        }];
        let rows = vec![issue(1, "A", 0.0), issue(2, "B", 1.0)];
        assert_eq!(
            search(rows.clone(), &tags, &assignees, &Default::default(), &q).issues[0].id,
            Uuid::from_u128(2)
        );
        q.status_id = Some(Uuid::from_u128(999));
        assert_eq!(
            search(rows, &tags, &assignees, &Default::default(), &q).total_count,
            0
        );
    }
}
