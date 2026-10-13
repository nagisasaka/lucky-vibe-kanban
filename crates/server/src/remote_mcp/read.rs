//! Bounded read projections over the existing local Board records. Reads do not
//! initialise metadata, reconcile jobs, or invoke a model.
use anyhow::{bail, ensure};
use rmcp::model::Tool;
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::Row;
use uuid::Uuid;

use super::Remote;

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ProjectArgs {
    project_id: Uuid,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct IssueListArgs {
    project_id: Uuid,
    #[serde(default = "super::default_limit")]
    #[schemars(range(min = 1, max = 50))]
    limit: u32,
    cursor: Option<Uuid>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct IssueArgs {
    project_id: Uuid,
    issue_id: Uuid,
    #[serde(default)]
    offset: u32,
    #[serde(default = "text_limit")]
    #[schemars(range(min = 1, max = 16000))]
    limit: u32,
    /// Required after the first page, to prevent combining changed requirements.
    expected_revision: Option<String>,
}
pub(super) fn text_limit() -> u32 {
    8000
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct WorkspaceListArgs {
    project_id: Uuid,
    issue_id: Uuid,
    #[serde(default = "super::default_limit")]
    #[schemars(range(min = 1, max = 50))]
    limit: u32,
    cursor: Option<Uuid>,
}

pub(super) fn definition<T: JsonSchema>(name: &str, description: &str) -> Tool {
    serde_json::from_value(json!({"name":name,"description":description,"inputSchema":schemars::schema_for!(T),"annotations":{"readOnlyHint":true,"destructiveHint":false,"openWorldHint":false},"_meta":{"securitySchemes":[{"type":"oauth2","scopes":["lvk:read"]}]}})).expect("read tool schema")
}

pub(super) fn tools() -> Vec<Tool> {
    vec![
        definition::<ProjectArgs>(
            "get_project",
            "Read current Project totals and status counts. Counts are facts, not evidence of test, merge or Wiki success. Use list_issues for individual work.",
        ),
        definition::<IssueListArgs>(
            "list_issues",
            "List up to 50 Issue summaries in one Project. Follow next_cursor; descriptions are available separately through get_issue.",
        ),
        definition::<IssueArgs>(
            "get_issue",
            "Read current agreed requirements in character pages (up to 16000). Use returned revision for subsequent pages and before edits/start. An Issue may have multiple Workspaces.",
        ),
        definition::<WorkspaceListArgs>(
            "list_issue_workspaces",
            "List every attempt for one Issue in pages up to 50. Choose an explicit Workspace; do not assume all attempts should be continued or merged.",
        ),
    ]
}

pub(super) fn arguments<T: serde::de::DeserializeOwned>(args: Value) -> anyhow::Result<T> {
    serde_json::from_value(args)
        .map_err(|_| anyhow::anyhow!("Invalid tool parameters; check the tool schema"))
}

pub(super) async fn call(remote: &Remote, name: &str, args: Value) -> anyhow::Result<Value> {
    match name {
        "get_project" => remote.project(arguments(args)?).await,
        "list_issues" => remote.issues(arguments(args)?).await,
        "get_issue" => remote.issue(arguments(args)?).await,
        "list_issue_workspaces" => remote.issue_workspaces(arguments(args)?).await,
        _ => bail!("Unknown read tool"),
    }
}

pub(super) fn page(items: Vec<Value>, limit: u32) -> Value {
    let more = items.len() > limit as usize;
    let items: Vec<_> = items.into_iter().take(limit as usize).collect();
    let next = if more {
        items.last().map(|item| item["id"].clone())
    } else {
        None
    };
    json!({"items":items,"has_more":more,"next_cursor":next,"observed_at":chrono::Utc::now(),"consistency":"live; restart listing after concurrent changes"})
}

impl Remote {
    pub(super) async fn require_project(&self, project: Uuid) -> anyhow::Result<()> {
        ensure!(
            self.config.project_ids.contains(&project),
            "Project is not available to this connection"
        );
        let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM projects WHERE id=?)")
            .bind(project)
            .fetch_one(&self.pool)
            .await?;
        ensure!(exists, "Project no longer exists");
        Ok(())
    }

    pub(super) async fn require_issue(&self, project: Uuid, issue: Uuid) -> anyhow::Result<()> {
        self.require_project(project).await?;
        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM local_issues WHERE id=? AND project_id=?)",
        )
        .bind(issue)
        .bind(project)
        .fetch_one(&self.pool)
        .await?;
        ensure!(exists, "Issue does not exist in this Project");
        Ok(())
    }

    pub(super) async fn require_workspace(
        &self,
        project: Uuid,
        workspace: Uuid,
    ) -> anyhow::Result<Uuid> {
        self.require_project(project).await?;
        let issue = sqlx::query_scalar("SELECT l.issue_id FROM local_workspace_links l JOIN local_issues i ON i.id=l.issue_id JOIN workspaces w ON w.id=l.workspace_id WHERE l.workspace_id=? AND l.project_id=? AND i.project_id=?")
            .bind(workspace).bind(project).bind(project).fetch_optional(&self.pool).await?;
        issue.ok_or_else(|| anyhow::anyhow!("Workspace does not exist in this Project"))
    }

    async fn project(&self, args: ProjectArgs) -> anyhow::Result<Value> {
        self.require_project(args.project_id).await?;
        let project = sqlx::query("SELECT substr(name,1,500) AS name,length(name)>500 AS name_truncated,updated_at FROM projects WHERE id=?").bind(args.project_id).fetch_one(&self.pool).await?;
        let counts = sqlx::query("SELECT s.id,substr(s.name,1,100) AS name,s.hidden,count(i.id) AS issue_count FROM local_project_statuses s LEFT JOIN local_issues i ON i.status_id=s.id AND i.project_id=s.project_id WHERE s.project_id=? GROUP BY s.id ORDER BY s.sort_order,s.id LIMIT 51")
            .bind(args.project_id).fetch_all(&self.pool).await?;
        let statuses = counts.iter().take(50).map(|row| -> anyhow::Result<Value> {
            Ok(json!({"id":row.try_get::<Uuid,_>("id")?,"name":row.try_get::<String,_>("name")?,"hidden":row.try_get::<bool,_>("hidden")?,"issue_count":row.try_get::<i64,_>("issue_count")?}))
        }).collect::<anyhow::Result<Vec<_>>>()?;
        let total: i64 = sqlx::query_scalar("SELECT count(*) FROM local_issues WHERE project_id=?")
            .bind(args.project_id)
            .fetch_one(&self.pool)
            .await?;
        Ok(
            json!({"id":args.project_id,"name":project.try_get::<String,_>("name")?,"name_truncated":project.try_get::<bool,_>("name_truncated")?,"revision":project.try_get::<String,_>("updated_at")?,"issue_count":total,"statuses":statuses,"statuses_incomplete":counts.len()>50,"url":self.config.project_url(args.project_id),"observed_at":chrono::Utc::now()}),
        )
    }

    async fn issues(&self, args: IssueListArgs) -> anyhow::Result<Value> {
        self.require_project(args.project_id).await?;
        ensure!(
            (1..=50).contains(&args.limit),
            "limit must be between 1 and 50"
        );
        if let Some(cursor) = args.cursor {
            self.require_issue(args.project_id, cursor)
                .await
                .map_err(|_| anyhow::anyhow!("Stale or foreign cursor; restart Issue listing"))?;
        }
        let rows = sqlx::query("SELECT i.id,i.simple_id,substr(i.title,1,500) AS title,length(i.title)>500 AS title_truncated,i.status_id,substr(s.name,1,100) AS status_name,i.updated_at,(SELECT count(*) FROM local_workspace_links l JOIN workspaces w ON w.id=l.workspace_id WHERE l.issue_id=i.id AND l.project_id=i.project_id) AS workspace_count FROM local_issues i LEFT JOIN local_project_statuses s ON s.id=i.status_id AND s.project_id=i.project_id WHERE i.project_id=? AND (? IS NULL OR i.id>?) ORDER BY i.id LIMIT ?")
            .bind(args.project_id).bind(args.cursor).bind(args.cursor).bind(args.limit+1).fetch_all(&self.pool).await?;
        let items = rows.into_iter().map(|row| -> anyhow::Result<Value> {
            let id: Uuid = row.try_get("id")?;
            Ok(json!({"id":id,"simple_id":row.try_get::<String,_>("simple_id")?,"title":row.try_get::<String,_>("title")?,"title_truncated":row.try_get::<bool,_>("title_truncated")?,"status_id":row.try_get::<Uuid,_>("status_id")?,"status_name":row.try_get::<Option<String>,_>("status_name")?,"revision":row.try_get::<String,_>("updated_at")?,"workspace_count":row.try_get::<i64,_>("workspace_count")?,"url":self.config.issue_url(args.project_id,id)}))
        }).collect::<anyhow::Result<Vec<_>>>()?;
        Ok(page(items, args.limit))
    }

    async fn issue(&self, args: IssueArgs) -> anyhow::Result<Value> {
        self.require_issue(args.project_id, args.issue_id).await?;
        ensure!(
            (1..=16000).contains(&args.limit),
            "limit must be between 1 and 16000 characters"
        );
        ensure!(
            args.offset == 0 || args.expected_revision.is_some(),
            "Supply expected_revision when reading another requirements page"
        );
        let row = sqlx::query("SELECT substr(title,1,500) AS title,length(title)>500 AS title_truncated,simple_id,status_id,updated_at,substr(coalesce(description,''),?,?) AS content,length(coalesce(description,'')) AS total FROM local_issues WHERE id=? AND project_id=?")
            .bind(i64::from(args.offset)+1).bind(args.limit).bind(args.issue_id).bind(args.project_id).fetch_one(&self.pool).await?;
        let revision: String = row.try_get("updated_at")?;
        ensure!(
            args.expected_revision
                .as_ref()
                .is_none_or(|expected| expected == &revision),
            "Issue changed; restart reading requirements before acting"
        );
        let total: i64 = row.try_get("total")?;
        ensure!(
            i64::from(args.offset) <= total,
            "Page offset is outside the requirements; restart reading"
        );
        let next = i64::from(args.offset) + i64::from(args.limit);
        Ok(
            json!({"id":args.issue_id,"project_id":args.project_id,"simple_id":row.try_get::<String,_>("simple_id")?,"title":row.try_get::<String,_>("title")?,"title_truncated":row.try_get::<bool,_>("title_truncated")?,"status_id":row.try_get::<Uuid,_>("status_id")?,"revision":revision,"description_page":row.try_get::<String,_>("content")?,"offset":args.offset,"total_characters":total,"next_offset":if next<total {Some(next)} else {None},"has_more":next<total,"url":self.config.issue_url(args.project_id,args.issue_id)}),
        )
    }

    async fn issue_workspaces(&self, args: WorkspaceListArgs) -> anyhow::Result<Value> {
        self.require_issue(args.project_id, args.issue_id).await?;
        ensure!(
            (1..=50).contains(&args.limit),
            "limit must be between 1 and 50"
        );
        if let Some(cursor) = args.cursor {
            ensure!(
                self.require_workspace(args.project_id, cursor).await? == args.issue_id,
                "Cursor belongs to another Issue; restart listing"
            );
        }
        let rows = sqlx::query("SELECT w.id,substr(w.name,1,500) AS name,w.archived,w.pinned,w.updated_at FROM workspaces w JOIN local_workspace_links l ON l.workspace_id=w.id WHERE l.project_id=? AND l.issue_id=? AND (? IS NULL OR w.id>?) ORDER BY w.id LIMIT ?")
            .bind(args.project_id).bind(args.issue_id).bind(args.cursor).bind(args.cursor).bind(args.limit+1).fetch_all(&self.pool).await?;
        let items = rows.into_iter().map(|row| -> anyhow::Result<Value> {
            let id: Uuid = row.try_get("id")?;
            Ok(json!({"id":id,"issue_id":args.issue_id,"name":row.try_get::<Option<String>,_>("name")?,"archived":row.try_get::<bool,_>("archived")?,"pinned":row.try_get::<bool,_>("pinned")?,"updated_at":row.try_get::<String,_>("updated_at")?,"url":self.config.workspace_url(args.project_id,args.issue_id,id)}))
        }).collect::<anyhow::Result<Vec<_>>>()?;
        Ok(page(items, args.limit))
    }
}
