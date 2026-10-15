//! Board collection metadata, independent of the Cloud server and its private dependencies.
use api_types::*;
use ts_rs::TS;

pub struct Shape {
    name: &'static str,
    table: &'static str,
    params: &'static [&'static str],
    type_name: String,
}
impl Shape {
    pub fn name(&self) -> &str {
        self.name
    }
    pub fn table(&self) -> &str {
        self.table
    }
    pub fn params(&self) -> &[&str] {
        self.params
    }
    pub fn ts_type_name(&self) -> &str {
        &self.type_name
    }
}
pub struct ShapeRoute {
    pub shape: Shape,
    pub fallback_url: &'static str,
}
fn shape<T: TS>(
    name: &'static str,
    table: &'static str,
    params: &'static [&'static str],
    fallback_url: &'static str,
) -> ShapeRoute {
    ShapeRoute {
        shape: Shape {
            name,
            table,
            params,
            type_name: T::name(),
        },
        fallback_url,
    }
}
pub fn all_shape_routes() -> Vec<ShapeRoute> {
    vec![
        shape::<Project>(
            "PROJECTS_SHAPE",
            "projects",
            &["organization_id"],
            "/fallback/projects",
        ),
        shape::<Notification>(
            "NOTIFICATIONS_SHAPE",
            "notifications",
            &["user_id"],
            "/fallback/notifications",
        ),
        shape::<OrganizationMember>(
            "ORGANIZATION_MEMBERS_SHAPE",
            "organization_member_metadata",
            &["organization_id"],
            "/fallback/organization_members",
        ),
        shape::<User>(
            "USERS_SHAPE",
            "users",
            &["organization_id"],
            "/fallback/users",
        ),
        shape::<Tag>(
            "PROJECT_TAGS_SHAPE",
            "tags",
            &["project_id"],
            "/fallback/tags",
        ),
        shape::<ProjectStatus>(
            "PROJECT_PROJECT_STATUSES_SHAPE",
            "project_statuses",
            &["project_id"],
            "/fallback/project_statuses",
        ),
        shape::<Issue>(
            "PROJECT_ISSUES_SHAPE",
            "issues",
            &["project_id"],
            "/fallback/issues",
        ),
        shape::<Workspace>(
            "USER_WORKSPACES_SHAPE",
            "workspaces",
            &["owner_user_id"],
            "/fallback/user_workspaces",
        ),
        shape::<Workspace>(
            "PROJECT_WORKSPACES_SHAPE",
            "workspaces",
            &["project_id"],
            "/fallback/project_workspaces",
        ),
        shape::<IssueAssignee>(
            "PROJECT_ISSUE_ASSIGNEES_SHAPE",
            "issue_assignees",
            &["project_id"],
            "/fallback/issue_assignees",
        ),
        shape::<IssueFollower>(
            "PROJECT_ISSUE_FOLLOWERS_SHAPE",
            "issue_followers",
            &["project_id"],
            "/fallback/issue_followers",
        ),
        shape::<IssueTag>(
            "PROJECT_ISSUE_TAGS_SHAPE",
            "issue_tags",
            &["project_id"],
            "/fallback/issue_tags",
        ),
        shape::<IssueRelationship>(
            "PROJECT_ISSUE_RELATIONSHIPS_SHAPE",
            "issue_relationships",
            &["project_id"],
            "/fallback/issue_relationships",
        ),
        shape::<PullRequest>(
            "PROJECT_PULL_REQUESTS_SHAPE",
            "pull_requests",
            &["project_id"],
            "/fallback/pull_requests",
        ),
        shape::<PullRequestIssue>(
            "PROJECT_PULL_REQUEST_ISSUES_SHAPE",
            "pull_request_issues",
            &["project_id"],
            "/fallback/pull_request_issues",
        ),
        shape::<IssueComment>(
            "ISSUE_COMMENTS_SHAPE",
            "issue_comments",
            &["issue_id"],
            "/fallback/issue_comments",
        ),
        shape::<IssueCommentReaction>(
            "ISSUE_REACTIONS_SHAPE",
            "issue_comment_reactions",
            &["issue_id"],
            "/fallback/issue_comment_reactions",
        ),
    ]
}
pub struct MutationDefinition {
    pub table: &'static str,
    pub row_type: String,
    pub create_type: Option<String>,
    pub update_type: Option<String>,
}
pub fn all_mutation_definitions() -> Vec<MutationDefinition> {
    vec![
        MutationDefinition {
            table: "projects",
            row_type: Project::name(),
            create_type: Some(CreateProjectRequest::name()),
            update_type: Some(UpdateProjectRequest::name()),
        },
        MutationDefinition {
            table: "notifications",
            row_type: Notification::name(),
            create_type: None,
            update_type: Some(UpdateNotificationRequest::name()),
        },
        MutationDefinition {
            table: "tags",
            row_type: Tag::name(),
            create_type: Some(CreateTagRequest::name()),
            update_type: Some(UpdateTagRequest::name()),
        },
        MutationDefinition {
            table: "project_statuses",
            row_type: ProjectStatus::name(),
            create_type: Some(CreateProjectStatusRequest::name()),
            update_type: Some(UpdateProjectStatusRequest::name()),
        },
        MutationDefinition {
            table: "issues",
            row_type: Issue::name(),
            create_type: Some(CreateIssueRequest::name()),
            update_type: Some(UpdateIssueRequest::name()),
        },
        MutationDefinition {
            table: "issue_assignees",
            row_type: IssueAssignee::name(),
            create_type: Some(CreateIssueAssigneeRequest::name()),
            update_type: None,
        },
        MutationDefinition {
            table: "issue_followers",
            row_type: IssueFollower::name(),
            create_type: Some(CreateIssueFollowerRequest::name()),
            update_type: None,
        },
        MutationDefinition {
            table: "issue_tags",
            row_type: IssueTag::name(),
            create_type: Some(CreateIssueTagRequest::name()),
            update_type: None,
        },
        MutationDefinition {
            table: "issue_relationships",
            row_type: IssueRelationship::name(),
            create_type: Some(CreateIssueRelationshipRequest::name()),
            update_type: None,
        },
        MutationDefinition {
            table: "issue_comments",
            row_type: IssueComment::name(),
            create_type: Some(CreateIssueCommentRequest::name()),
            update_type: Some(UpdateIssueCommentRequest::name()),
        },
        MutationDefinition {
            table: "issue_comment_reactions",
            row_type: IssueCommentReaction::name(),
            create_type: Some(CreateIssueCommentReactionRequest::name()),
            update_type: Some(UpdateIssueCommentReactionRequest::name()),
        },
    ]
}
