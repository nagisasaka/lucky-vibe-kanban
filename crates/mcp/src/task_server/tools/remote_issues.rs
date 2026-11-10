use std::collections::HashMap;

use api_types::{
    FilterIssuesRequest, Issue, IssueFilters, IssuePriority, IssueRelationshipType, IssueSortField,
    ListIssueRelationshipsResponse, ListIssueTagsResponse, ListIssuesResponse,
    ListPullRequestsResponse, ListTagsResponse, PullRequestStatus, SortDirection,
};
use rmcp::{
    ErrorData, handler::server::wrapper::Parameters, model::CallToolResult, schemars, tool,
    tool_router,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{McpServer, ToolError};

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
struct McpCreateIssueRequest {
    /// Stable caller ID enables same-principal durable replay protection. Omitted: no replay guarantee.
    request_id: Option<Uuid>,
    #[schemars(
        description = "The ID of the project to create the issue in. Optional if running inside a workspace linked to a remote project."
    )]
    project_id: Option<Uuid>,
    #[schemars(description = "The title of the issue")]
    title: String,
    #[schemars(description = "Optional description of the issue")]
    description: Option<String>,
    #[schemars(
        description = "Optional priority of the issue. Allowed values: 'urgent', 'high', 'medium', 'low'."
    )]
    priority: Option<String>,
    #[schemars(description = "Optional parent issue ID to create a subissue")]
    parent_issue_id: Option<Uuid>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct McpListIssuesRequest {
    #[schemars(
        description = "The ID of the project to list issues from. Optional if running inside a workspace linked to a remote project."
    )]
    project_id: Option<Uuid>,
    /// false (default): unarchived, true: archived, "all": both. Independent of hidden statuses.
    #[serde(default)]
    archived: api_types::IssueArchiveFilter,
    #[schemars(description = "Maximum number of issues to return (default: 50)")]
    limit: Option<i32>,
    #[schemars(description = "Number of results to skip before returning rows (default: 0)")]
    offset: Option<i32>,
    #[schemars(description = "Filter by status name (case-insensitive exact match) or UUID")]
    status: Option<String>,
    #[schemars(
        description = "Filter by priority. Allowed values: 'urgent', 'high', 'medium', 'low'."
    )]
    priority: Option<String>,
    #[schemars(description = "Filter by parent issue ID (subissues of this issue)")]
    parent_issue_id: Option<Uuid>,
    #[schemars(
        description = "Case-insensitive literal substring match against full title or description; surrounding whitespace is preserved"
    )]
    search: Option<String>,
    #[schemars(description = "Filter by issue simple ID (case-insensitive exact match)")]
    simple_id: Option<String>,
    #[schemars(description = "Filter to issues assigned to this user ID")]
    assignee_user_id: Option<Uuid>,
    #[schemars(description = "Filter to issues having this tag ID")]
    tag_id: Option<Uuid>,
    #[schemars(
        description = "Filter by exact tag name (case-insensitive); when tag_id is also supplied, that same tag ID must have this name"
    )]
    tag_name: Option<String>,
    #[schemars(
        description = "Field to sort by. Allowed values: 'sort_order', 'priority', 'created_at', 'updated_at', 'title'. Default: 'sort_order'."
    )]
    sort_field: Option<String>,
    #[schemars(description = "Sort direction. Allowed values: 'asc', 'desc'. Default: 'asc'.")]
    sort_direction: Option<String>,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
struct IssueSummary {
    #[schemars(description = "The unique identifier of the issue")]
    id: String,
    #[schemars(description = "The title of the issue")]
    title: String,
    #[schemars(description = "The human-readable issue simple ID")]
    simple_id: String,
    #[schemars(description = "Current status of the issue")]
    status: String,
    archived: bool,
    #[schemars(description = "Current priority of the issue")]
    priority: Option<String>,
    #[schemars(description = "Parent issue ID if this is a subissue")]
    parent_issue_id: Option<String>,
    #[schemars(description = "When the issue was created")]
    created_at: String,
    #[schemars(description = "When the issue was last updated")]
    updated_at: String,
    #[schemars(description = "Number of pull requests linked to this issue")]
    pull_request_count: usize,
    #[schemars(description = "URL of the most recent pull request, if any")]
    latest_pr_url: Option<String>,
    #[schemars(
        description = "Status of the most recent pull request: 'open', 'merged', or 'closed'"
    )]
    latest_pr_status: Option<PullRequestStatus>,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
struct PullRequestSummary {
    #[schemars(description = "PR number")]
    number: i32,
    #[schemars(description = "URL of the pull request")]
    url: String,
    #[schemars(description = "Status of the pull request: 'open', 'merged', or 'closed'")]
    status: PullRequestStatus,
    #[schemars(description = "When the PR was merged, if applicable")]
    merged_at: Option<String>,
    #[schemars(description = "Target branch for the PR")]
    target_branch_name: String,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
struct McpTagSummary {
    #[schemars(description = "The tag ID")]
    id: String,
    #[schemars(description = "The tag name")]
    name: String,
    #[schemars(description = "The tag color")]
    color: String,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
struct McpRelationshipSummary {
    #[schemars(description = "The relationship ID (use this to delete)")]
    id: String,
    #[schemars(description = "The related issue ID")]
    related_issue_id: String,
    #[schemars(description = "The related issue's simple ID (e.g. 'PROJ-42')")]
    related_simple_id: String,
    #[schemars(description = "Relationship type: blocking, related, or has_duplicate")]
    relationship_type: String,
    /// This Issue is the source or target of the stored relationship.
    direction: String,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
struct McpSubIssueSummary {
    #[schemars(description = "The sub-issue ID")]
    id: String,
    #[schemars(description = "Short human-readable identifier (e.g. 'PROJ-43')")]
    simple_id: String,
    #[schemars(description = "The sub-issue title")]
    title: String,
    #[schemars(description = "Current status of the sub-issue")]
    status: String,
    archived: bool,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
struct IssueDetails {
    #[schemars(description = "The unique identifier of the issue")]
    id: String,
    #[schemars(description = "The title of the issue")]
    title: String,
    #[schemars(description = "The human-readable issue simple ID")]
    simple_id: String,
    #[schemars(description = "Optional description of the issue")]
    description: Option<String>,
    #[schemars(description = "Current status of the issue")]
    status: String,
    #[schemars(description = "The status ID (UUID)")]
    status_id: String,
    archived: bool,
    #[schemars(description = "Current priority of the issue")]
    priority: Option<String>,
    #[schemars(description = "Parent issue ID if this is a subissue")]
    parent_issue_id: Option<String>,
    #[schemars(description = "Optional planned start date")]
    start_date: Option<String>,
    #[schemars(description = "Optional planned target date")]
    target_date: Option<String>,
    #[schemars(description = "Optional completion date")]
    completed_at: Option<String>,
    #[schemars(description = "When the issue was created")]
    created_at: String,
    #[schemars(description = "When the issue was last updated")]
    updated_at: String,
    #[schemars(description = "Pull requests linked to this issue")]
    pull_requests: Vec<PullRequestSummary>,
    #[schemars(description = "Tags attached to this issue")]
    tags: Vec<McpTagSummary>,
    #[schemars(description = "Relationships to other issues")]
    relationships: Vec<McpRelationshipSummary>,
    #[schemars(description = "Sub-issues under this issue")]
    sub_issues: Vec<McpSubIssueSummary>,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
struct McpListIssuesResponse {
    issues: Vec<IssueSummary>,
    total_count: usize,
    returned_count: usize,
    limit: usize,
    offset: usize,
    project_id: String,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
struct McpUpdateIssueRequest {
    /// Stable caller ID enables same-principal durable replay protection. Omitted: no replay guarantee.
    request_id: Option<Uuid>,
    /// Caller-observed Issue revision. Omitted: no protection since the caller read. Independent of request_id.
    expected_revision: Option<String>,
    #[schemars(description = "The ID of the issue to update")]
    issue_id: Uuid,
    #[schemars(description = "New title for the issue")]
    title: Option<String>,
    #[schemars(description = "New description for the issue")]
    description: Option<String>,
    #[schemars(description = "New status name for the issue (must match a project status name)")]
    status: Option<String>,
    #[schemars(
        description = "New priority for the issue. Allowed values: 'urgent', 'high', 'medium', 'low'."
    )]
    priority: Option<String>,
    #[schemars(
        description = "Set/replace the parent Issue. Omitted or null preserves it; clear_parent:true explicitly detaches."
    )]
    parent_issue_id: Option<Uuid>,
    clear_parent: Option<bool>,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
struct McpDeleteIssueRequest {
    /// Stable caller ID enables same-principal durable replay protection. Omitted: no replay guarantee.
    request_id: Option<Uuid>,
    #[schemars(description = "The ID of the issue to delete")]
    issue_id: Uuid,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct McpGetIssueRequest {
    #[schemars(description = "The ID of the issue to retrieve")]
    issue_id: Uuid,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
struct McpGetIssueResponse {
    issue: IssueDetails,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
struct McpListIssuePrioritiesResponse {
    priorities: Vec<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct PrepareIssueArchiveRequest {
    project_id: Uuid,
    issue_id: Uuid,
    /// true archives linked idle Workspaces and physically deletes their worktrees.
    archive_workspaces: bool,
}
#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct ConfirmIssueArchiveRequest {
    project_id: Uuid,
    issue_id: Uuid,
    confirmation_token: Uuid,
    /// Reuse this ID after response loss. Never blindly prepare another deletion.
    request_id: Uuid,
}
#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct UnarchiveIssueRequest {
    project_id: Uuid,
    issue_id: Uuid,
    request_id: Uuid,
}
#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct IssueArchiveOperationRequest {
    request_id: Uuid,
}

#[tool_router(router = remote_issues_tools_router, vis = "pub")]
impl McpServer {
    #[tool(
        description = "Prepare Issue archive without mutation. Show the returned message and exact Workspace targets to the human once. After confirmation call confirm_issue_archive. The checkbox value and target set are bound; false leaves Workspaces unchanged. Busy child refuses the whole operation."
    )]
    async fn prepare_issue_archive(
        &self,
        Parameters(args): Parameters<PrepareIssueArchiveRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        let url = self.url(&format!(
            "/api/board/issues/{}/archive/prepare",
            args.issue_id
        ));
        match self.send_json::<serde_json::Value>(self.client.post(url).json(&serde_json::json!({"project_id":args.project_id,"archive_workspaces":args.archive_workspaces}))).await {
            Ok(value)=>Self::success(&value),Err(error)=>Ok(Self::tool_error(error))
        }
    }
    #[tool(
        description = "After human confirmation, archive the exact prepared Issue and optional Workspaces without changing status. Reuse request_id after response loss; inspect get_issue_archive_operation. Never automatically retry an uncertain outcome or regenerate deleted worktrees."
    )]
    async fn confirm_issue_archive(
        &self,
        Parameters(args): Parameters<ConfirmIssueArchiveRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        let url = self.url(&format!(
            "/api/board/issues/{}/archive/confirm",
            args.issue_id
        ));
        match self.send_json::<serde_json::Value>(self.client.post(url).json(&serde_json::json!({"project_id":args.project_id,"confirmation_token":args.confirmation_token,"request_id":args.request_id}))).await {
            Ok(value)=>Self::success(&value),Err(error)=>Ok(Self::tool_error(error))
        }
    }
    #[tool(
        description = "Unarchive only the Issue. Preserve status; do not unarchive Workspaces or regenerate worktrees. Reuse request_id for receipt recovery."
    )]
    async fn unarchive_issue(
        &self,
        Parameters(args): Parameters<UnarchiveIssueRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        let url = self.url(&format!("/api/board/issues/{}/unarchive", args.issue_id));
        match self
            .send_json::<serde_json::Value>(self.client.post(url).json(
                &serde_json::json!({"project_id":args.project_id,"request_id":args.request_id}),
            ))
            .await
        {
            Ok(value) => Self::success(&value),
            Err(error) => Ok(Self::tool_error(error)),
        }
    }
    #[tool(
        description = "Read the original Issue archive/unarchive receipt after response loss. Unknown/uncertain outcomes must be inspected before any new request."
    )]
    async fn get_issue_archive_operation(
        &self,
        Parameters(args): Parameters<IssueArchiveOperationRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        let url = self.url(&format!(
            "/api/board/issue-archive-operations/{}",
            args.request_id
        ));
        match self
            .send_json::<serde_json::Value>(self.client.get(url))
            .await
        {
            Ok(value) => Self::success(&value),
            Err(error) => Ok(Self::tool_error(error)),
        }
    }
    #[tool(
        description = "Create an Issue. Optional request_id gives durable same-principal deduplication; omission gives no replay guarantee. Read get_operation after response loss. Create a new issue in a project. `project_id` is optional if running inside a workspace linked to a remote project."
    )]
    async fn create_issue(
        &self,
        Parameters(mut args): Parameters<McpCreateIssueRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        args.project_id = match self.resolve_project_id(args.project_id) {
            Ok(id) => Some(id),
            Err(e) => return Ok(Self::tool_error(e)),
        };
        let input = serde_json::to_value(&args).expect("Issue input");
        let mut value = match self
            .send_json::<serde_json::Value>(
                self.client
                    .post(self.url("/api/mcp-stdio/issues/create_issue"))
                    .json(&input),
            )
            .await
        {
            Ok(value) => value,
            Err(error) => return Ok(Self::tool_error(error)),
        };
        if value["status"] != "succeeded" {
            if matches!(value["status"].as_str(), Some("failed" | "uncertain")) {
                let mut result =
                    CallToolResult::error(vec![rmcp::model::Content::text(value.to_string())]);
                result.structured_content = Some(value);
                return Ok(result);
            }
            return Self::success(&value);
        }
        value["issue_id"] = value["result"]["issue_id"].clone();
        Self::success(&value)
    }

    #[tool(
        description = "List Issues in a project. archived omitted/false selects unarchived, true selects archived, and 'all' selects both. Filters combine with AND before offset paging. `project_id` is optional if running inside a workspace linked to a remote project."
    )]
    async fn list_issues(
        &self,
        Parameters(McpListIssuesRequest {
            project_id,
            archived,
            limit,
            offset,
            status,
            priority,
            parent_issue_id,
            search,
            simple_id,
            assignee_user_id,
            tag_id,
            tag_name,
            sort_field,
            sort_direction,
        }): Parameters<McpListIssuesRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        let project_id = match self.resolve_project_id(project_id) {
            Ok(id) => id,
            Err(e) => return Ok(McpServer::tool_error(e)),
        };

        let project_statuses = match self.fetch_project_statuses(project_id).await {
            Ok(statuses) => Some(statuses),
            Err(e) => {
                if status.is_some() {
                    return Ok(McpServer::tool_error(e));
                }
                None
            }
        };
        let status_names_by_id = project_statuses.as_ref().map(|statuses| {
            statuses
                .iter()
                .map(|status| (status.id, status.name.clone()))
                .collect::<HashMap<_, _>>()
        });

        let sort_field = match Self::parse_issue_sort_field(sort_field.as_deref()) {
            Ok(value) => Some(value),
            Err(e) => return Ok(McpServer::tool_error(e)),
        };
        let sort_direction = match Self::parse_sort_direction(sort_direction.as_deref()) {
            Ok(value) => Some(value),
            Err(e) => return Ok(McpServer::tool_error(e)),
        };

        let query = FilterIssuesRequest {
            project_id,
            filters: IssueFilters {
                archived,
                search,
                simple_id,
                status,
                priority,
                parent_issue_id,
                assignee_user_id,
                tag_id,
                tag_name,
            },
            sort_field,
            sort_direction,
            limit: Some(limit.unwrap_or(50).max(0)),
            offset: Some(offset.unwrap_or(0).max(0)),
        };
        let url = self.url("/api/board/issues/filter");
        let response: ListIssuesResponse =
            match self.send_json(self.client.post(&url).json(&query)).await {
                Ok(r) => r,
                Err(e) => return Ok(McpServer::tool_error(e)),
            };

        let mut summaries = Vec::with_capacity(response.issues.len());
        for issue in &response.issues {
            let pull_requests = self.fetch_pull_requests(issue.id).await;
            summaries.push(self.issue_to_summary(
                issue,
                status_names_by_id.as_ref(),
                &pull_requests,
            ));
        }

        McpServer::success(&McpListIssuesResponse {
            total_count: response.total_count,
            returned_count: summaries.len(),
            limit: response.limit,
            offset: response.offset,
            issues: summaries,
            project_id: project_id.to_string(),
        })
    }

    #[tool(
        description = "Get detailed information about a specific issue. You can use `list_issues` to find issue IDs. `issue_id` is required."
    )]
    async fn get_issue(
        &self,
        Parameters(McpGetIssueRequest { issue_id }): Parameters<McpGetIssueRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        let url = self.url(&format!("/api/board/issues/{}", issue_id));
        let issue: Issue = match self.send_json(self.client.get(&url)).await {
            Ok(i) => i,
            Err(e) => return Ok(McpServer::tool_error(e)),
        };

        let pull_requests = self.fetch_pull_requests(issue_id).await;
        let details = match self.issue_to_details(&issue, pull_requests).await {
            Ok(details) => details,
            Err(e) => return Ok(Self::tool_error(e)),
        };
        McpServer::success(&McpGetIssueResponse { issue: details })
    }

    #[tool(
        description = "Update an Issue without starting implementation. Optional request_id gives durable deduplication; optional expected_revision checks the caller-observed revision. Either guarantee works independently; omission gives no corresponding protection. priority:null preserves it; set medium explicitly. Omitted fields and null parent IDs preserve existing values. parent_issue_id sets/replaces the parent; clear_parent:true explicitly removes only that link. Read get_issue before/after. Parent cycles are rejected."
    )]
    async fn update_issue(
        &self,
        Parameters(args): Parameters<McpUpdateIssueRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        let input = serde_json::to_value(&args).expect("Issue input");
        let mut value = match self
            .send_json::<serde_json::Value>(
                self.client
                    .post(self.url("/api/mcp-stdio/issues/update_issue"))
                    .json(&input),
            )
            .await
        {
            Ok(value) => value,
            Err(error) => return Ok(Self::tool_error(error)),
        };
        if value["status"] != "succeeded" {
            if matches!(value["status"].as_str(), Some("failed" | "uncertain")) {
                let mut result =
                    CallToolResult::error(vec![rmcp::model::Content::text(value.to_string())]);
                result.structured_content = Some(value);
                return Ok(result);
            }
            return Self::success(&value);
        }
        let issue: Issue = match self
            .send_json(
                self.client
                    .get(self.url(&format!("/api/board/issues/{}", args.issue_id))),
            )
            .await
        {
            Ok(v) => v,
            Err(mut e) => {
                e.diagnostic.outcome = utils::mcp_error::Outcome::Unknown;
                return Ok(Self::tool_error(e));
            }
        };
        let prs = self.fetch_pull_requests(args.issue_id).await;
        match self.issue_to_details(&issue, prs).await {
            Ok(details) => value["issue"] = serde_json::to_value(details).expect("Issue details"),
            Err(mut e) => {
                e.diagnostic.outcome = utils::mcp_error::Outcome::Unknown;
                return Ok(Self::tool_error(e));
            }
        }
        Self::success(&value)
    }

    #[tool(description = "List allowed issue priority values.")]
    async fn list_issue_priorities(&self) -> Result<CallToolResult, ErrorData> {
        McpServer::success(&McpListIssuePrioritiesResponse {
            priorities: ["urgent", "high", "medium", "low"]
                .iter()
                .map(|s| s.to_string())
                .collect(),
        })
    }

    #[tool(
        description = "Permanently delete an Issue with existing Board constraints; no worktree deletion. Optional request_id gives durable same-principal receipt/replay protection; omission gives none. Use get_operation after uncertainty. `issue_id` is required."
    )]
    async fn delete_issue(
        &self,
        Parameters(args): Parameters<McpDeleteIssueRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        let input = serde_json::to_value(&args).expect("Issue input");
        let mut value = match self
            .send_json::<serde_json::Value>(
                self.client
                    .post(self.url("/api/mcp-stdio/issues/delete_issue"))
                    .json(&input),
            )
            .await
        {
            Ok(value) => value,
            Err(error) => return Ok(Self::tool_error(error)),
        };
        if value["status"] != "succeeded" {
            if matches!(value["status"].as_str(), Some("failed" | "uncertain")) {
                let mut result =
                    CallToolResult::error(vec![rmcp::model::Content::text(value.to_string())]);
                result.structured_content = Some(value);
                return Ok(result);
            }
            return Self::success(&value);
        }
        value["deleted_issue_id"] = value["result"]["deleted_issue_id"].clone();
        Self::success(&value)
    }
}

impl McpServer {
    fn parse_issue_sort_field(sort_field: Option<&str>) -> Result<IssueSortField, ToolError> {
        match sort_field
            .unwrap_or("sort_order")
            .trim()
            .to_ascii_lowercase()
            .as_str()
        {
            "sort_order" => Ok(IssueSortField::SortOrder),
            "priority" => Ok(IssueSortField::Priority),
            "created_at" => Ok(IssueSortField::CreatedAt),
            "updated_at" => Ok(IssueSortField::UpdatedAt),
            "title" => Ok(IssueSortField::Title),
            _ => Err(ToolError::message(
                "Unknown sort_field. Allowed values: sort_order, priority, created_at, updated_at, title",
            )),
        }
    }

    fn parse_sort_direction(sort_direction: Option<&str>) -> Result<SortDirection, ToolError> {
        match sort_direction
            .unwrap_or("asc")
            .trim()
            .to_ascii_lowercase()
            .as_str()
        {
            "asc" => Ok(SortDirection::Asc),
            "desc" => Ok(SortDirection::Desc),
            _ => Err(ToolError::message(
                "Unknown sort_direction. Allowed values: asc, desc",
            )),
        }
    }

    fn issue_to_summary(
        &self,
        issue: &Issue,
        status_names_by_id: Option<&HashMap<Uuid, String>>,
        pull_requests: &ListPullRequestsResponse,
    ) -> IssueSummary {
        let status = status_names_by_id
            .and_then(|status_map| status_map.get(&issue.status_id).cloned())
            .unwrap_or_else(|| issue.status_id.to_string());
        let latest_pr = pull_requests.pull_requests.first();
        IssueSummary {
            id: issue.id.to_string(),
            title: issue.title.clone(),
            simple_id: issue.simple_id.clone(),
            status,
            archived: issue.archived,
            priority: issue
                .priority
                .map(Self::issue_priority_label)
                .map(str::to_string),
            parent_issue_id: issue.parent_issue_id.map(|id| id.to_string()),
            created_at: issue.created_at.to_rfc3339(),
            updated_at: issue.updated_at.to_rfc3339(),
            pull_request_count: pull_requests.pull_requests.len(),
            latest_pr_url: latest_pr.map(|pr| pr.url.clone()),
            latest_pr_status: latest_pr.map(|pr| pr.status),
        }
    }

    async fn issue_to_details(
        &self,
        issue: &Issue,
        pull_requests: ListPullRequestsResponse,
    ) -> Result<IssueDetails, ToolError> {
        let status = self
            .resolve_status_name(issue.project_id, issue.status_id)
            .await;

        let tags = self
            .fetch_issue_tags_resolved(issue.project_id, issue.id)
            .await;

        let relationships = self
            .fetch_issue_relationships_resolved(issue.project_id, issue.id)
            .await?;

        let sub_issues = self.fetch_sub_issues(issue.project_id, issue.id).await;

        Ok(IssueDetails {
            id: issue.id.to_string(),
            title: issue.title.clone(),
            simple_id: issue.simple_id.clone(),
            description: issue.description.clone(),
            status,
            status_id: issue.status_id.to_string(),
            archived: issue.archived,
            priority: issue
                .priority
                .map(Self::issue_priority_label)
                .map(str::to_string),
            parent_issue_id: issue.parent_issue_id.map(|id| id.to_string()),
            start_date: issue.start_date.map(|date| date.to_rfc3339()),
            target_date: issue.target_date.map(|date| date.to_rfc3339()),
            completed_at: issue.completed_at.map(|date| date.to_rfc3339()),
            created_at: issue.created_at.to_rfc3339(),
            updated_at: issue.updated_at.to_rfc3339(),
            pull_requests: pull_requests
                .pull_requests
                .into_iter()
                .map(|pr| PullRequestSummary {
                    number: pr.number,
                    url: pr.url,
                    status: pr.status,
                    merged_at: pr.merged_at.map(|dt| dt.to_rfc3339()),
                    target_branch_name: pr.target_branch_name,
                })
                .collect(),
            tags,
            relationships,
            sub_issues,
        })
    }

    async fn fetch_pull_requests(&self, issue_id: Uuid) -> ListPullRequestsResponse {
        let url = self.url(&format!("/api/board/pull-requests?issue_id={}", issue_id));
        match self
            .send_json::<ListPullRequestsResponse>(self.client.get(&url))
            .await
        {
            Ok(response) => response,
            Err(_) => ListPullRequestsResponse {
                pull_requests: vec![],
            },
        }
    }

    /// Fetches tags for an issue, resolving tag_ids to names via project tags.
    async fn fetch_issue_tags_resolved(
        &self,
        project_id: Uuid,
        issue_id: Uuid,
    ) -> Vec<McpTagSummary> {
        let tags_url = self.url(&format!("/api/board/tags?project_id={}", project_id));
        let project_tags: ListTagsResponse = match self.send_json(self.client.get(&tags_url)).await
        {
            Ok(r) => r,
            Err(_) => return Vec::new(),
        };
        let tag_map: HashMap<Uuid, &api_types::Tag> =
            project_tags.tags.iter().map(|t| (t.id, t)).collect();

        let url = self.url(&format!("/api/board/issue-tags?issue_id={}", issue_id));
        let response: ListIssueTagsResponse = match self.send_json(self.client.get(&url)).await {
            Ok(r) => r,
            Err(_) => return Vec::new(),
        };

        response
            .issue_tags
            .iter()
            .filter_map(|it| {
                tag_map.get(&it.tag_id).map(|tag| McpTagSummary {
                    id: tag.id.to_string(),
                    name: tag.name.clone(),
                    color: tag.color.clone(),
                })
            })
            .collect()
    }

    /// Resolve both endpoints without hiding transport failures or partial lists.
    async fn fetch_issue_relationships_resolved(
        &self,
        _project_id: Uuid,
        issue_id: Uuid,
    ) -> Result<Vec<McpRelationshipSummary>, ToolError> {
        let rel_url = self.url(&format!(
            "/api/board/issue-relationships?issue_id={}",
            issue_id
        ));
        let response: ListIssueRelationshipsResponse =
            self.send_read_json(self.client.get(&rel_url)).await?;
        let mut result = Vec::new();
        for r in response.issue_relationships {
            let source = r.issue_id == issue_id;
            let other = if source {
                r.related_issue_id
            } else {
                r.issue_id
            };
            let url = self.url(&format!("/api/board/issues/{}", other));
            let other_issue: Issue = self.send_read_json(self.client.get(&url)).await?;
            result.push(McpRelationshipSummary {
                id: r.id.to_string(),
                related_issue_id: other.to_string(),
                related_simple_id: other_issue.simple_id,
                relationship_type: match r.relationship_type {
                    IssueRelationshipType::Blocking => "blocking",
                    IssueRelationshipType::Related => "related",
                    IssueRelationshipType::HasDuplicate => "has_duplicate",
                }
                .into(),
                direction: if source { "source" } else { "target" }.into(),
            });
        }
        Ok(result)
    }

    /// Fetches sub-issues for a given parent issue.
    async fn fetch_sub_issues(
        &self,
        project_id: Uuid,
        parent_issue_id: Uuid,
    ) -> Vec<McpSubIssueSummary> {
        let url = self.url(&format!("/api/board/issues?project_id={}", project_id));
        let response: api_types::ListIssuesResponse =
            match self.send_json(self.client.get(&url)).await {
                Ok(r) => r,
                Err(_) => return Vec::new(),
            };

        let status_names = self
            .fetch_project_statuses(project_id)
            .await
            .ok()
            .map(|statuses| {
                statuses
                    .into_iter()
                    .map(|s| (s.id, s.name))
                    .collect::<HashMap<_, _>>()
            });

        response
            .issues
            .iter()
            .filter(|i| i.parent_issue_id == Some(parent_issue_id))
            .map(|i| {
                let status = status_names
                    .as_ref()
                    .and_then(|m| m.get(&i.status_id).cloned())
                    .unwrap_or_else(|| i.status_id.to_string());
                McpSubIssueSummary {
                    id: i.id.to_string(),
                    simple_id: i.simple_id.clone(),
                    title: i.title.clone(),
                    archived: i.archived,
                    status,
                }
            })
            .collect()
    }

    fn issue_priority_label(priority: IssuePriority) -> &'static str {
        match priority {
            IssuePriority::Urgent => "urgent",
            IssuePriority::High => "high",
            IssuePriority::Medium => "medium",
            IssuePriority::Low => "low",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn stdio_relationship_reads_resolve_incoming_and_propagate_failure() {
        use serde_json::json;
        use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
        super::super::tests::install_rustls_provider();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let project = Uuid::new_v4();
        let source = Uuid::new_v4();
        let target = Uuid::new_v4();
        let relationship = Uuid::new_v4();
        let backend = tokio::spawn(async move {
            let issue = json!({"id":source,"project_id":project,"issue_number":1,"simple_id":"TEST-1","status_id":Uuid::new_v4(),"title":"blocker","description":null,"priority":null,"start_date":null,"target_date":null,"completed_at":null,"sort_order":0.0,"parent_issue_id":null,"parent_issue_sort_order":null,"extension_metadata":{},"creator_user_id":null,"created_at":"2026-11-06T00:00:00Z","updated_at":"2026-11-06T00:00:00Z"});
            for (path, status, data) in [
                (
                    format!("/api/board/issue-relationships?issue_id={target}"),
                    200,
                    json!({"issue_relationships":[{"id":relationship,"issue_id":source,"related_issue_id":target,"relationship_type":"blocking","created_at":"2026-11-06T00:00:00Z"}]}),
                ),
                (format!("/api/board/issues/{source}"), 200, issue),
                (
                    format!("/api/board/issue-relationships?issue_id={target}"),
                    503,
                    json!(null),
                ),
                (
                    format!("/api/board/issue-relationships?issue_id={target}"),
                    200,
                    json!({"issue_relationships":[]}),
                ),
            ] {
                let (socket, _) = listener.accept().await.unwrap();
                let mut socket = BufReader::new(socket);
                let mut first = String::new();
                socket.read_line(&mut first).await.unwrap();
                assert!(first.starts_with(&format!("GET {path} HTTP/")), "{first}");
                loop {
                    let mut header = String::new();
                    socket.read_line(&mut header).await.unwrap();
                    if header == "\r\n" {
                        break;
                    }
                }
                let body = json!({"success":true,"data":data}).to_string();
                socket.write_all(format!("HTTP/1.1 {status} Fixture\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).as_bytes()).await.unwrap();
            }
        });
        let server = McpServer::new_global(&format!("http://{address}"));
        let rows = server
            .fetch_issue_relationships_resolved(project, target)
            .await
            .unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].related_issue_id, source.to_string());
        assert_eq!(rows[0].related_simple_id, "TEST-1");
        assert_eq!(rows[0].direction, "target");
        assert_eq!(rows[0].id, relationship.to_string());
        assert!(
            server
                .fetch_issue_relationships_resolved(project, target)
                .await
                .is_err()
        );
        assert!(
            server
                .fetch_issue_relationships_resolved(project, target)
                .await
                .unwrap()
                .is_empty()
        );
        backend.await.unwrap();
    }

    #[tokio::test]
    async fn stdio_archive_filters_preserve_offset_count_contract() {
        use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
        super::super::tests::install_rustls_provider();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let project = Uuid::new_v4();
        let tag = Uuid::new_v4();
        let backend = tokio::spawn(async move {
            for archive in [
                serde_json::json!(false),
                serde_json::json!(true),
                serde_json::json!("all"),
            ] {
                // Existing stdio status-name projection stays separate from the shared filter.
                for path in [
                    format!("GET /api/board/project-statuses?project_id={project}"),
                    "POST /api/board/issues/filter".to_string(),
                ] {
                    let (socket, _) = listener.accept().await.unwrap();
                    let mut socket = BufReader::new(socket);
                    let mut first = String::new();
                    socket.read_line(&mut first).await.unwrap();
                    let mut length = 0;
                    loop {
                        let mut header = String::new();
                        socket.read_line(&mut header).await.unwrap();
                        if header == "\r\n" {
                            break;
                        }
                        if let Some(value) = header.to_lowercase().strip_prefix("content-length:") {
                            length = value.trim().parse::<usize>().unwrap();
                        }
                    }
                    let mut bytes = vec![0; length];
                    socket.read_exact(&mut bytes).await.unwrap();
                    assert_eq!(first, format!("{path} HTTP/1.1\r\n"));
                    let data = if path.starts_with("POST") {
                        let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
                        assert_eq!(body["project_id"], serde_json::json!(project));
                        assert_eq!(body["archived"], archive);
                        assert_eq!(body["search"], " Needle ");
                        assert_eq!(body["tag_id"], serde_json::json!(tag));
                        assert_eq!(body["tag_name"], "BuG");
                        assert_eq!(body["limit"], 1);
                        assert_eq!(body["offset"], 2);
                        assert_eq!(body["sort_field"], "title");
                        assert_eq!(body["sort_direction"], "desc");
                        serde_json::json!({"issues":[],"total_count":2,"limit":1,"offset":2})
                    } else {
                        serde_json::json!({"project_statuses":[]})
                    };
                    let body = serde_json::json!({"success":true,"data":data}).to_string();
                    socket.get_mut().write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).as_bytes()).await.unwrap();
                }
            }
        });
        let server = McpServer::new_global(&format!("http://{address}"));
        for archive in [
            None,
            Some(serde_json::json!(true)),
            Some(serde_json::json!("all")),
        ] {
            let mut input = serde_json::json!({"project_id":project,"search":" Needle ","tag_id":tag,"tag_name":"BuG","limit":1,"offset":2,"sort_field":"title","sort_direction":"desc"});
            if let Some(archive) = archive {
                input["archived"] = archive;
            }
            let result = server
                .list_issues(Parameters(serde_json::from_value(input).unwrap()))
                .await
                .unwrap();
            assert_ne!(result.is_error, Some(true), "{result:?}");
            let text = result.content[0].as_text().unwrap();
            let result: serde_json::Value = serde_json::from_str(&text.text).unwrap();
            assert_eq!(result["total_count"], 2);
            assert_eq!(result["returned_count"], 0);
            assert_eq!(result["offset"], 2);
            assert_eq!(result["limit"], 1);
        }
        backend.await.unwrap();
    }

    #[tokio::test]
    async fn stdio_issue_archive_adapter_preserves_confirmation_and_receipt_requests() {
        use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
        super::super::tests::install_rustls_provider();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let project = Uuid::new_v4();
        let issue = Uuid::new_v4();
        let token = Uuid::new_v4();
        let request = Uuid::new_v4();
        let backend = tokio::spawn(async move {
            for action in ["archive/prepare", "archive/confirm", "unarchive", "receipt"] {
                let (socket, _) = listener.accept().await.unwrap();
                let mut socket = BufReader::new(socket);
                let mut first = String::new();
                socket.read_line(&mut first).await.unwrap();
                let mut length = 0;
                loop {
                    let mut header = String::new();
                    socket.read_line(&mut header).await.unwrap();
                    if header == "\r\n" {
                        break;
                    }
                    if let Some(value) = header.to_lowercase().strip_prefix("content-length:") {
                        length = value.trim().parse::<usize>().unwrap();
                    }
                }
                let mut body = vec![0; length];
                socket.read_exact(&mut body).await.unwrap();
                if action == "receipt" {
                    assert_eq!(
                        first,
                        format!("GET /api/board/issue-archive-operations/{request} HTTP/1.1\r\n")
                    );
                } else {
                    assert_eq!(
                        first,
                        format!("POST /api/board/issues/{issue}/{action} HTTP/1.1\r\n")
                    );
                    let body: serde_json::Value = serde_json::from_slice(&body).unwrap();
                    assert_eq!(body["project_id"], serde_json::json!(project));
                    if action == "archive/prepare" {
                        assert_eq!(body["archive_workspaces"], false);
                        assert!(body.get("confirmation_token").is_none());
                    } else {
                        assert_eq!(body["request_id"], serde_json::json!(request));
                        if action == "archive/confirm" {
                            assert_eq!(body["confirmation_token"], serde_json::json!(token));
                        }
                    }
                }
                let body=serde_json::json!({"success":true,"data":{"request_id":request,"status":"succeeded"}}).to_string();
                socket.get_mut().write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).as_bytes()).await.unwrap();
            }
        });
        let server = McpServer::new_global(&format!("http://{address}"));
        assert_ne!(
            server
                .prepare_issue_archive(Parameters(PrepareIssueArchiveRequest {
                    project_id: project,
                    issue_id: issue,
                    archive_workspaces: false
                }))
                .await
                .unwrap()
                .is_error,
            Some(true)
        );
        assert_ne!(
            server
                .confirm_issue_archive(Parameters(ConfirmIssueArchiveRequest {
                    project_id: project,
                    issue_id: issue,
                    confirmation_token: token,
                    request_id: request
                }))
                .await
                .unwrap()
                .is_error,
            Some(true)
        );
        assert_ne!(
            server
                .unarchive_issue(Parameters(UnarchiveIssueRequest {
                    project_id: project,
                    issue_id: issue,
                    request_id: request
                }))
                .await
                .unwrap()
                .is_error,
            Some(true)
        );
        assert_ne!(
            server
                .get_issue_archive_operation(Parameters(IssueArchiveOperationRequest {
                    request_id: request
                }))
                .await
                .unwrap()
                .is_error,
            Some(true)
        );
        backend.await.unwrap();
    }
}
