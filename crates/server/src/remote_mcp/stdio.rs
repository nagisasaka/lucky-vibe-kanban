//! Internal stdio authentication/input adapter. Shares mutations and durable
//! Operations with HTTP MCP; the common internal principal is NOT Run isolation.
use axum::{
    Extension, Json, Router,
    extract::{Path, State},
    routing::post,
};
use deployment::Deployment;
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::SqlitePool;
use utils::response::ApiResponse;
use uuid::Uuid;

use super::{
    issue_mutations::{Mutation, receipt_value},
    operations::Operations,
};
use crate::{DeploymentImpl, error::ApiError};

#[derive(Deserialize)]
struct IssueArgs {
    project_id: Option<Uuid>,
    issue_id: Option<Uuid>,
    request_id: Option<Uuid>,
    expected_revision: Option<String>,
    title: Option<String>,
    description: Option<String>,
    status: Option<String>,
    priority: Option<String>,
    parent_issue_id: Option<Uuid>,
    clear_parent: Option<bool>,
}
pub(super) fn router() -> Router<DeploymentImpl> {
    Router::new()
        .route("/mcp-stdio/workflow", post(workflow))
        .route("/mcp-stdio/issues/{operation}", post(issue))
        .route("/mcp-stdio/operation", post(operation))
        .route("/mcp-stdio/start-workspace", post(start_workspace))
        .route("/mcp-stdio/create-session", post(create_session))
        .route("/mcp-stdio/continue", post(continue_session))
}
fn owner(actor: &server_access::Principal, write: bool) -> Result<String, ApiError> {
    if actor.channel != "internal"
        || !actor.scopes.contains("lvk:read")
        || (write && !actor.scopes.contains("lvk:write"))
    {
        return Err(ApiError::Forbidden(
            "Authenticated internal MCP actor required".into(),
        ));
    }
    Ok(json!([actor.issuer, actor.subject]).to_string())
}
fn store(deployment: &DeploymentImpl) -> Operations {
    Operations {
        pool: deployment.db().pool.clone(),
        process_instance: *super::PROCESS_INSTANCE.get_or_init(Uuid::new_v4),
    }
}
#[derive(Deserialize)]
struct ReadOperation {
    project_id: Option<Uuid>,
    request_id: Uuid,
}
async fn operation(
    State(d): State<DeploymentImpl>,
    Extension(actor): Extension<server_access::Principal>,
    Json(args): Json<ReadOperation>,
) -> Result<Json<ApiResponse<Value>>, ApiError> {
    let receipt = store(&d)
        .by_request(&owner(&actor, false)?, args.request_id)
        .await
        .map_err(|_| ApiError::BadRequest("Operation unavailable to this principal".into()))?;
    if args.project_id.is_some_and(|id| id != receipt.project_id) {
        return Err(ApiError::BadRequest("Operation not in Project".into()));
    }
    Ok(Json(ApiResponse::success(receipt_value(receipt, |_| {
        String::new()
    }))))
}
async fn prepare(
    pool: &SqlitePool,
    name: &str,
    project: Uuid,
    a: IssueArgs,
) -> Result<Mutation, ApiError> {
    if a.parent_issue_id.is_some() && a.clear_parent == Some(true) {
        return Err(ApiError::BadRequest(
            "Specify a parent or clear_parent, not both".into(),
        ));
    }
    let priority = a
        .priority
        .map(|p| {
            serde_json::from_value::<api_types::IssuePriority>(json!(p.trim().to_ascii_lowercase()))
        })
        .transpose()
        .map_err(|_| ApiError::BadRequest("Invalid priority".into()))?;
    // Preserve the legacy prompt-tag expansion, but only after receipt admission.
    let description = if let Some(text) = a.description {
        let regex = regex::Regex::new(r"@([^\s@]+)").expect("static regex");
        let tags = db::models::tag::Tag::find_all(pool)
            .await
            .unwrap_or_default();
        Some(
            regex
                .replace_all(&text, |caps: &regex::Captures| {
                    tags.iter()
                        .find(|t| t.tag_name == caps[1])
                        .map(|t| t.content.clone())
                        .unwrap_or_else(|| caps[0].to_owned())
                })
                .into_owned(),
        )
    } else {
        None
    };
    if name == "create_issue" {
        let status: Uuid = sqlx::query_scalar("SELECT id FROM local_project_statuses WHERE project_id=? AND hidden=0 ORDER BY sort_order LIMIT 1").bind(project).fetch_one(pool).await?;
        let request = api_types::CreateIssueRequest {
            id: None,
            project_id: project,
            status_id: status,
            title: a
                .title
                .ok_or_else(|| ApiError::BadRequest("Title required".into()))?,
            description,
            priority,
            start_date: None,
            target_date: None,
            completed_at: None,
            sort_order: 0.0,
            parent_issue_id: a.parent_issue_id,
            parent_issue_sort_order: None,
            extension_metadata: json!({}),
        };
        return Ok(Mutation::Create {
            request,
            tag_ids: vec![],
        });
    }
    let issue_id = a
        .issue_id
        .ok_or_else(|| ApiError::BadRequest("Issue required".into()))?;
    if name == "delete_issue" {
        return Ok(Mutation::Delete {
            issue_id,
            project_id: project,
            missing_is_success: true,
        });
    }
    let belongs: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM local_issues WHERE id=? AND project_id=?)")
            .bind(issue_id)
            .bind(project)
            .fetch_one(pool)
            .await?;
    if !belongs {
        return Err(ApiError::BadRequest("Issue not found".into()));
    }
    let mut changes = json!({});
    if let Some(title) = a.title {
        changes["title"] = json!(title);
    }
    if let Some(description) = description {
        changes["description"] = json!(description);
    }
    if let Some(priority) = priority {
        changes["priority"] = json!(priority);
    }
    if let Some(parent) = a.parent_issue_id {
        changes["parent_issue_id"] = json!(parent);
    }
    if a.clear_parent == Some(true) {
        changes["parent_issue_id"] = Value::Null;
    }
    if let Some(status) = a.status {
        let id: Option<Uuid> = sqlx::query_scalar("SELECT id FROM local_project_statuses WHERE project_id=? AND (lower(name)=lower(?) OR id=?) LIMIT 1").bind(project).bind(&status).bind(status.parse::<Uuid>().ok()).fetch_optional(pool).await?;
        changes["status_id"] =
            json!(id.ok_or_else(|| ApiError::BadRequest("Status not found".into()))?);
    }
    Ok(Mutation::Update {
        issue_id,
        changes: serde_json::from_value(changes)
            .map_err(|_| ApiError::BadRequest("Invalid changes".into()))?,
        expected_revision: a.expected_revision,
    })
}
async fn issue(
    State(d): State<DeploymentImpl>,
    Extension(actor): Extension<server_access::Principal>,
    Path(name): Path<String>,
    Json(input): Json<Value>,
) -> Result<Json<ApiResponse<Value>>, ApiError> {
    let principal = owner(&actor, true)?;
    if !matches!(
        name.as_str(),
        "create_issue" | "update_issue" | "delete_issue"
    ) {
        return Err(ApiError::BadRequest("Unknown mutation".into()));
    }
    issue_write(store(&d), principal, name, input)
        .await
        .map(|value| Json(ApiResponse::success(value)))
}
async fn issue_write(
    store: Operations,
    principal: String,
    name: String,
    input: Value,
) -> Result<Value, ApiError> {
    let a: IssueArgs = serde_json::from_value(input.clone())
        .map_err(|_| ApiError::BadRequest("Invalid Issue parameters".into()))?;
    if let Some(revision) = &a.expected_revision {
        super::issue_mutations::parse_revision(revision)
            .map_err(|_| ApiError::BadRequest("Invalid revision".into()))?;
    }
    if name == "delete_issue"
        && a.request_id.is_none()
        && a.project_id.is_none()
        && let Some(id) = a.issue_id
    {
        let exists: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM local_issues WHERE id=?)")
                .bind(id)
                .fetch_one(&store.pool)
                .await?;
        if !exists {
            return Ok(
                json!({"status":"succeeded","result":{"deleted_issue_id":id},"guarantees":{"request_deduplication":false,"observed_revision_check":false}}),
            );
        }
    }
    // Resolve a replay's Project from its owned receipt, even after deletion.
    let saved_project: Option<Uuid> = if let Some(id) = a.request_id {
        sqlx::query_scalar(
            "SELECT project_id FROM remote_mcp_operations WHERE principal=? AND request_id=?",
        )
        .bind(&principal)
        .bind(id)
        .fetch_optional(&store.pool)
        .await?
    } else {
        None
    };
    let project = match (saved_project, a.project_id, a.issue_id) {
        (Some(p), Some(explicit), _) if p != explicit => {
            return Err(ApiError::Conflict("Operation Project mismatch".into()));
        }
        (Some(p), _, _) | (None, Some(p), _) => p,
        (None, None, Some(id)) => {
            sqlx::query_scalar("SELECT project_id FROM local_issues WHERE id=?")
                .bind(id)
                .fetch_one(&store.pool)
                .await?
        }
        _ => return Err(ApiError::BadRequest("Project required".into())),
    };
    let guarantees = json!({"request_deduplication":a.request_id.is_some(),"observed_revision_check":a.expected_revision.is_some(),"receipt_owner":"shared_internal_principal_not_run_isolation"});
    let pool = store.pool.clone();
    let request_id = a.request_id;
    let action = name.clone();
    let targets = json!({"project_id":project,"issue_id":a.issue_id});
    let execute = move |id| async move {
        let mutation = match prepare(&pool, &action, project, a).await {
            Ok(value) => value,
            Err(error) => {
                return super::issue_mutations::domain_failure(
                    crate::routes::local_remote::IssueMutationError::Failed(error),
                );
            }
        };
        mutation.apply(&pool, id).await
    };
    let mut value = if let Some(request) = request_id {
        let receipt = store
            .submit_and_wait(principal, request, project, name, input, targets, execute)
            .await
            .map_err(|_| {
                ApiError::Conflict("Inspect original operation and state before any retry".into())
            })?;
        receipt_value(receipt, |_| String::new())
    } else {
        let (status, result) = execute(Uuid::new_v4()).await;
        json!({"status":status,"result":result})
    };
    value["guarantees"] = guarantees;
    Ok(value)
}

#[derive(Deserialize)]
struct WorkspaceStart {
    issue_id: Option<Uuid>,
    request_id: Option<Uuid>,
    project_id: Option<Uuid>,
    expected_revision: Option<String>,
    request: db::models::requests::CreateAndStartWorkspaceRequest,
}
async fn start_workspace(
    State(d): State<DeploymentImpl>,
    Extension(actor): Extension<server_access::Principal>,
    Json(input): Json<Value>,
) -> Result<Json<ApiResponse<Value>>, ApiError> {
    use super::launch::LaunchActions;
    let principal = owner(&actor, true)?;
    let mut args: WorkspaceStart = serde_json::from_value(input.clone())
        .map_err(|_| ApiError::BadRequest("Invalid Workspace start".into()))?;
    if let Some(id) = args.request_id
        && let Some(receipt) = store(&d)
            .replay(&principal, id, "start_workspace", &input)
            .await
            .map_err(|_| ApiError::Conflict("Inspect original operation before retrying".into()))?
    {
        let mut value = receipt_value(receipt, |_| String::new());
        value["guarantees"] = json!({"request_deduplication":true});
        value["workspace_id"] = value["result"]["workspace_id"].clone();
        value["guarantees"]["observed_revision_check"] = json!(args.expected_revision.is_some());
        return Ok(Json(ApiResponse::success(value)));
    }

    if let Some(issue_id) = args.issue_id {
        let issue = crate::routes::local_remote::get_local_issue(&d.db().pool, issue_id).await?;
        args.request.linked_issue = Some(db::models::requests::LinkedIssueInfo {
            remote_project_id: issue.project_id,
            issue_id,
        });
        if args.request.prompt.trim().is_empty() {
            let title = issue.title.trim();
            let description = issue.description.as_deref().unwrap_or_default().trim();
            args.request.prompt = [title, description]
                .into_iter()
                .filter(|part| !part.is_empty())
                .collect::<Vec<_>>()
                .join("\n\n");
        }
    }
    if args.request.prompt.trim().is_empty() {
        return Err(ApiError::BadRequest(
            "Prompt or non-empty linked Issue required".into(),
        ));
    }

    let project = args
        .request
        .linked_issue
        .as_ref()
        .map(|i| i.remote_project_id)
        .or(args.project_id);
    if args.request_id.is_some() && project.is_none() {
        return Err(ApiError::BadRequest(
            "request_id requires a Project context or linked Issue".into(),
        ));
    }
    let guarantees = json!({"request_deduplication":args.request_id.is_some(),"observed_revision_check":args.expected_revision.is_some()});
    let request_id = args.request_id;
    let execution = super::launch::ExistingLaunch(d.clone());
    let store = store(&d);
    let execute = move |id| async move {
        if let Some(link) = &args.request.linked_issue {
            let current: Result<String, _> = sqlx::query_scalar(
                "SELECT updated_at FROM local_issues WHERE id=? AND project_id=?",
            )
            .bind(link.issue_id)
            .bind(link.remote_project_id)
            .fetch_one(&d.db().pool)
            .await;
            let Ok(current) = current else {
                return ("failed", json!({"code":"issue_not_found"}));
            };
            if args.expected_revision.as_ref().is_some_and(|expected| {
                super::issue_mutations::parse_revision(expected).ok()
                    != super::issue_mutations::parse_revision(&current).ok()
            }) {
                return ("failed", json!({"code":"revision_conflict"}));
            }
            if crate::routes::attachment_transfer::validate_ids(
                &d.db().pool,
                link.remote_project_id,
                args.request.attachment_ids.as_deref().unwrap_or_default(),
            )
            .await
            .is_err()
            {
                return ("failed", json!({"code":"attachments_unavailable"}));
            }
        } else if args.expected_revision.is_some()
            || args
                .request
                .attachment_ids
                .as_ref()
                .is_some_and(|ids| !ids.is_empty())
        {
            return ("failed", json!({"code":"linked_issue_required"}));
        }
        let mut request = args.request;
        for id in request.attachment_ids.as_deref().unwrap_or_default() {
            let file = match db::models::file::File::find_by_id(&d.db().pool, *id).await {
                Ok(Some(file)) => file,
                _ => return ("failed", json!({"code":"attachment_missing"})),
            };
            request.prompt.push_str(&format!(
                "\n\nAttachment: `.lvk/attachments/{}`",
                file.file_path
            ));
        }
        match execution.start(request, id).await {
            Ok(value) => ("succeeded", value),
            Err(_) => (
                "uncertain",
                json!({"workspace_id":id,"execution_accepted":null}),
            ),
        }
    };
    let mut value = if let Some(id) = request_id {
        let receipt = store
            .submit_and_wait(
                principal,
                id,
                project.unwrap(),
                "start_workspace".into(),
                input,
                json!({"project_id":project}),
                execute,
            )
            .await
            .map_err(|_| ApiError::Conflict("Inspect original operation before retrying".into()))?;
        receipt_value(receipt, |_| String::new())
    } else {
        let (status, result) = execute(Uuid::new_v4()).await;
        json!({"status":status,"result":result})
    };
    if value["status"] == "succeeded" {
        value["workspace_id"] = value["result"]["workspace_id"].clone();
    }
    value["guarantees"] = guarantees;
    Ok(Json(ApiResponse::success(value)))
}
#[derive(Deserialize)]
struct SessionCreate {
    project_id: Option<Uuid>,
    request_id: Option<Uuid>,
    workspace_id: Uuid,
    executor: Option<String>,
    name: Option<String>,
}
async fn create_session(
    State(d): State<DeploymentImpl>,
    Extension(actor): Extension<server_access::Principal>,
    Json(input): Json<Value>,
) -> Result<Json<ApiResponse<Value>>, ApiError> {
    let principal = owner(&actor, true)?;
    let args: SessionCreate = serde_json::from_value(input.clone())
        .map_err(|_| ApiError::BadRequest("Invalid Session input".into()))?;
    if let Some(id) = args.request_id
        && let Some(receipt) = store(&d)
            .replay(&principal, id, "create_session", &input)
            .await
            .map_err(|_| ApiError::Conflict("Inspect original operation before retrying".into()))?
    {
        let mut value = receipt_value(receipt, |_| String::new());
        value["guarantees"] = json!({"request_deduplication":true});
        value["session"] = value["result"]["session"].clone();
        return Ok(Json(ApiResponse::success(value)));
    }

    let project = project_for_workspace(&d.db().pool, args.workspace_id, args.project_id).await?;
    if args.request_id.is_some() && project.is_none() {
        return Err(ApiError::BadRequest(
            "request_id requires Project context or linked Workspace".into(),
        ));
    }
    let request_id = args.request_id;
    let store = store(&d);
    let execute = move |_| async move {
        let result = crate::routes::sessions::create_session(
            State(d),
            Json(crate::routes::sessions::CreateSessionRequest {
                workspace_id: args.workspace_id,
                executor: args.executor,
                name: args.name,
            }),
        )
        .await;
        match result {
            Ok(value) => ("succeeded", json!({"session":value.0.into_data()})),
            Err(_) => ("uncertain", json!({"workspace_id":args.workspace_id})),
        }
    };
    let mut value = if let Some(id) = request_id {
        receipt_value(
            store
                .submit_and_wait(
                    principal,
                    id,
                    project.unwrap(),
                    "create_session".into(),
                    input,
                    json!({"workspace_id":args.workspace_id}),
                    execute,
                )
                .await
                .map_err(|_| {
                    ApiError::Conflict("Inspect original operation before retrying".into())
                })?,
            |_| String::new(),
        )
    } else {
        let (status, result) = execute(Uuid::new_v4()).await;
        json!({"status":status,"result":result})
    };
    value["guarantees"] = json!({"request_deduplication":request_id.is_some()});
    if value["status"] == "succeeded" {
        value["session"] = value["result"]["session"].clone();
    }
    Ok(Json(ApiResponse::success(value)))
}
async fn project_for_workspace(
    pool: &SqlitePool,
    workspace: Uuid,
    explicit: Option<Uuid>,
) -> Result<Option<Uuid>, ApiError> {
    let linked:Option<Uuid>=sqlx::query_scalar("SELECT l.project_id FROM local_workspace_links l JOIN local_issues i ON i.id=l.issue_id AND i.project_id=l.project_id WHERE l.workspace_id=?").bind(workspace).fetch_optional(pool).await?;
    if let (Some(a), Some(b)) = (explicit, linked)
        && a != b
    {
        return Err(ApiError::BadRequest("Workspace Project mismatch".into()));
    }
    Ok(linked.or(explicit))
}
#[derive(Deserialize)]
struct Continue {
    project_id: Option<Uuid>,
    request_id: Option<Uuid>,
    agent_run_id: Option<Uuid>,
    session_id: Uuid,
    prompt: String,
    #[serde(default)]
    attachment_ids: Vec<Uuid>,
    #[serde(default)]
    overrides: executors::profile::NextRunOverrides,
}
async fn continue_session(
    State(d): State<DeploymentImpl>,
    Extension(actor): Extension<server_access::Principal>,
    Json(input): Json<Value>,
) -> Result<Json<ApiResponse<Value>>, ApiError> {
    use super::runtime_write::RuntimeActions;
    let principal = owner(&actor, true)?;
    let args: Continue = serde_json::from_value(input.clone())
        .map_err(|_| ApiError::BadRequest("Invalid continuation".into()))?;
    if let Some(id) = args.request_id
        && let Some(receipt) = store(&d)
            .replay(&principal, id, "continue_execution", &input)
            .await
            .map_err(|_| ApiError::Conflict("Inspect original operation before retrying".into()))?
    {
        let mut value = receipt_value(receipt, |_| String::new());
        value["guarantees"] = json!({"request_deduplication":true});
        value["agent_run_id"] = value["result"]["agent_run_id"].clone();
        value["session_id"] = json!(args.session_id);
        value["guarantees"]["caller_selected_run"] = json!(args.agent_run_id.is_some());
        value["guarantees"]["latest_run_check"] = json!(true);
        return Ok(Json(ApiResponse::success(value)));
    }

    if args.prompt.trim().is_empty() {
        return Err(ApiError::BadRequest("Prompt required".into()));
    }
    let session = db::models::session::Session::find_by_id(&d.db().pool, args.session_id)
        .await?
        .ok_or_else(|| ApiError::BadRequest("Session missing".into()))?;
    let project =
        project_for_workspace(&d.db().pool, session.workspace_id, args.project_id).await?;
    if args.request_id.is_some() && project.is_none() {
        return Err(ApiError::BadRequest(
            "request_id requires Project context or linked Workspace".into(),
        ));
    }
    let request_id = args.request_id;
    let explicit_run = args.agent_run_id.is_some();
    let store = store(&d);
    let execute = move |operation| async move {
        let latest:Option<Uuid>=match sqlx::query_scalar("SELECT id FROM agent_runs WHERE session_id=? ORDER BY created_at DESC,rowid DESC LIMIT 1").bind(session.id).fetch_optional(&d.db().pool).await{Ok(v)=>v,Err(_)=>return ("failed",json!({"code":"run_unavailable"}))};
        let runtime = super::runtime_write::ExistingRuntime(d.clone());
        if let Some(run) = args.agent_run_id.or(latest) {
            let target = super::runtime_write::Target {
                project_id: project.unwrap_or(Uuid::nil()),
                workspace_id: session.workspace_id,
                session_id: session.id,
                agent_run_id: run,
            };
            if super::runtime_write::validate_run(&d.db().pool, &target, true)
                .await
                .is_err()
            {
                return (
                    "failed",
                    json!({"code":"run_conflict","message":"Continue requires the latest finished Run; use steer for running execution"}),
                );
            }
            let kind = match runtime
                .prepare_change(
                    &target,
                    super::runtime_write::Change::Continue(args.overrides),
                )
                .await
            {
                Ok(v) => v,
                Err(_) => return ("failed", json!({"code":"configuration_rejected"})),
            };
            let mut prompt = args.prompt;
            if !args.attachment_ids.is_empty() {
                let placement = crate::routes::attachment_transfer::place(
                    d.clone(),
                    crate::routes::attachment_transfer::Place {
                        project_id: target.project_id,
                        workspace_id: target.workspace_id,
                        session_id: target.session_id,
                        request_id: operation,
                        attachment_ids: args.attachment_ids,
                    },
                    json!(["attachment", actor.issuer, actor.subject]).to_string(),
                )
                .await;
                match placement {
                    Ok(v) if v["result"]["all_verified"] == true || v["all_verified"] == true => {
                        let results = if v["result"].is_object() {
                            &v["result"]["results"]
                        } else {
                            &v["results"]
                        };
                        for item in results.as_array().into_iter().flatten() {
                            if let Some(path) = item["attachment"]["file_path"].as_str() {
                                prompt.push_str(&format!("\n\nAttachment: `{path}`"));
                            }
                        }
                    }
                    _ => return ("failed", json!({"code":"attachments_not_verified"})),
                }
            }
            match runtime.change(target, operation, prompt, kind).await {
                Ok(value) => ("succeeded", value),
                Err(_) => ("uncertain", json!({"code":"delivery_unconfirmed"})),
            }
        } else {
            if !args.overrides.is_empty() || !args.attachment_ids.is_empty() {
                return (
                    "failed",
                    json!({"code":"initial_session_requires_saved_configuration"}),
                );
            }
            let config = match crate::routes::sessions::executor_config::read_executor_config(
                &d.db().pool,
                session.id,
            )
            .await
            {
                Ok(Some(c)) => c,
                Err(_) => return ("failed", json!({"code":"saved_configuration_unavailable"})),
                Ok(None) => {
                    let default: executors::profile::ExecutorConfig =
                        d.config().read().await.executor_profile.clone().into();
                    match session.executor.as_deref() {
                        Some("CLAUDE_CODE") => executors::profile::ExecutorConfig::new(
                            executors::executors::BaseCodingAgent::ClaudeCode,
                        ),
                        Some("CODEX") => executors::profile::ExecutorConfig::new(
                            executors::executors::BaseCodingAgent::Codex,
                        ),
                        _ => default,
                    }
                }
            };
            match crate::routes::sessions::start_coding_agent_execution_for_session(
                &d,
                session,
                args.prompt,
                None,
                config,
                None,
                None,
            )
            .await
            {
                Ok(run) => (
                    "succeeded",
                    json!({"agent_run_id":run.agent_run_id,"execution_accepted":true,"implementation_complete":false}),
                ),
                Err(_) => ("uncertain", json!({"code":"initial_execution_unconfirmed"})),
            }
        }
    };
    let mut value = if let Some(id) = request_id {
        receipt_value(
            store
                .submit_and_wait(
                    principal,
                    id,
                    project.unwrap(),
                    "continue_execution".into(),
                    input,
                    json!({"session_id":args.session_id}),
                    execute,
                )
                .await
                .map_err(|_| {
                    ApiError::Conflict("Inspect original operation before retrying".into())
                })?,
            |_| String::new(),
        )
    } else {
        let (status, result) = execute(Uuid::new_v4()).await;
        json!({"status":status,"result":result})
    };
    value["guarantees"] = json!({"request_deduplication":request_id.is_some(),"caller_selected_run":explicit_run,"latest_run_check":true});
    if value["status"] == "succeeded" {
        value["agent_run_id"] = value["result"]["agent_run_id"].clone();
        value["session_id"] = json!(args.session_id);
    }
    Ok(Json(ApiResponse::success(value)))
}

async fn workflow(
    State(d): State<DeploymentImpl>,
    Json(value): Json<Value>,
) -> Result<Json<ApiResponse<Value>>, ApiError> {
    let actor = server_access::ACTOR
        .try_with(Clone::clone)
        .map_err(|_| ApiError::Forbidden("Authenticated actor required".into()))?;
    let args: super::workflow::WorkflowArgs =
        serde_json::from_value(value).map_err(|e| ApiError::BadRequest(e.to_string()))?;
    // Apply the same project/Issue/Workspace authorization as the remote adapter.
    let ids = sqlx::query_scalar::<_, Uuid>("SELECT id FROM projects")
        .fetch_all(&d.db().pool)
        .await?
        .into_iter()
        .collect();
    let principal = super::auth::Principal {
        subject: actor.subject.clone(),
        scopes: actor.scopes.clone(),
    };
    let remote = super::Remote {
        pool: d.db().pool.clone(),
        runtime: None,
        launch: Some(std::sync::Arc::new(super::launch::ExistingLaunch(d))),
        environment: None,
        process_instance: Uuid::new_v4(),
        config: std::sync::Arc::new(super::config::Config {
            public_base_url: String::new(),
            issuer: actor.issuer,
            #[cfg(test)]
            introspection_endpoint: String::new(),
            #[cfg(test)]
            introspection_client_id: String::new(),
            #[cfg(test)]
            introspection_secret_env: String::new(),
            #[cfg(test)]
            owner_subject: actor.subject,
            project_ids: ids,
        }),
    };
    let result = super::workflow::call(
        &remote,
        &principal,
        serde_json::to_value(args).map_err(|e| ApiError::BadRequest(e.to_string()))?,
    )
    .await
    .map_err(|e| ApiError::BadRequest(e.to_string()))?;
    Ok(Json(ApiResponse::success(result)))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn optional_guarantees_replay_conflict_and_deleted_target_receipt() {
        let (remote, _, _, target, issue) = super::super::runtime_write_tests::fixture().await;
        let store = remote.operations();
        let principal = "internal-owner".to_string();
        let revision = |pool: SqlitePool| async move {
            sqlx::query_scalar::<_, String>("SELECT updated_at FROM local_issues WHERE id=?")
                .bind(issue)
                .fetch_one(&pool)
                .await
                .unwrap()
        };
        for (dedup, cas) in [(false, false), (true, false), (false, true), (true, true)] {
            let mut input = json!({"issue_id":issue,"title":format!("changed-{dedup}-{cas}")});
            if dedup {
                input["request_id"] = json!(Uuid::new_v4());
            }
            if cas {
                input["expected_revision"] = json!(revision(store.pool.clone()).await);
            }
            let first = issue_write(
                store.clone(),
                principal.clone(),
                "update_issue".into(),
                input.clone(),
            )
            .await
            .unwrap();
            assert_eq!(first["status"], "succeeded", "{first}");
            assert_eq!(first["guarantees"]["request_deduplication"], dedup);
            assert_eq!(first["guarantees"]["observed_revision_check"], cas);
            if dedup {
                let second = issue_write(
                    store.clone(),
                    principal.clone(),
                    "update_issue".into(),
                    input.clone(),
                )
                .await
                .unwrap();
                assert_eq!(first["operation_id"], second["operation_id"]);
                input["title"] = json!("different");
                assert!(
                    issue_write(
                        store.clone(),
                        principal.clone(),
                        "update_issue".into(),
                        input
                    )
                    .await
                    .is_err()
                );
            }
        }
        let stale = issue_write(
            store.clone(),
            principal.clone(),
            "update_issue".into(),
            json!({"issue_id":issue,"title":"stale","expected_revision":"2000-01-01 00:00:00"}),
        )
        .await
        .unwrap();
        assert_eq!(stale["status"], "failed");
        assert_eq!(stale["result"]["error"]["category"], "conflict");
        let key = Uuid::new_v4();
        let create =
            json!({"project_id":target.project_id,"request_id":key,"title":"deduplicated"});
        let (a, b) = tokio::join!(
            issue_write(
                store.clone(),
                principal.clone(),
                "create_issue".into(),
                create.clone()
            ),
            issue_write(
                store.clone(),
                principal.clone(),
                "create_issue".into(),
                create
            )
        );
        let a = a.unwrap();
        let b = b.unwrap();
        assert_eq!(a["operation_id"], b["operation_id"]);
        assert!(store.by_request("external-owner", key).await.is_err());
        let id = a["result"]["issue_id"].clone();
        let delete = json!({"issue_id":id,"request_id":Uuid::new_v4()});
        let first = issue_write(
            store.clone(),
            principal.clone(),
            "delete_issue".into(),
            delete.clone(),
        )
        .await
        .unwrap();
        let replay = issue_write(
            store.clone(),
            principal.clone(),
            "delete_issue".into(),
            delete,
        )
        .await
        .unwrap();
        assert_eq!(first["status"], "succeeded");
        assert_eq!(first["operation_id"], replay["operation_id"]);
        let missing = issue_write(
            store,
            principal,
            "delete_issue".into(),
            json!({"issue_id":id}),
        )
        .await
        .unwrap();
        assert_eq!(missing["status"], "succeeded");
    }
    #[tokio::test]
    async fn worker_id_mixing_is_rejected_without_claiming_caller_run_authentication() {
        let (remote, _, _, target, _) = super::super::runtime_write_tests::fixture().await;
        // This fixture normally stores an intentionally minimal envelope. The
        // real worker reader deserializes the complete persisted request.
        let envelope = json!({"schema_version":1,"payload_version":1,"request_id":Uuid::new_v4(),"idempotency_key":"worker-fixture","session_id":target.session_id,"agent_run_id":target.agent_run_id,"turn_id":Uuid::new_v4(),"correlation_id":Uuid::new_v4(),"intent":"initial","runtime_profile_id":"CODEX:default","provider_id":"codex","workspace":{"workspace_id":target.workspace_id,"mode":"shared_workspace","path":"/fixture"},"input":{"message_id":Uuid::new_v4(),"role":"user","content":"fixture"},"created_at":chrono::Utc::now()});
        sqlx::query("UPDATE agent_runs SET request_envelope=? WHERE id=?")
            .bind(envelope.to_string())
            .bind(target.agent_run_id)
            .execute(&remote.pool)
            .await
            .unwrap();
        let agent =
            db::models::agent_runtime::AgentRunRecord::find(&remote.pool, target.agent_run_id)
                .await
                .unwrap()
                .unwrap();
        let check = crate::routes::integrations::conversation::require_worker_ids;
        assert!(
            check(
                &remote.pool,
                agent.workspace_id,
                agent.id,
                Some(agent.session_id),
                agent.correlation_id
            )
            .await
            .is_ok()
        );
        assert!(
            check(
                &remote.pool,
                Uuid::new_v4(),
                agent.id,
                Some(agent.session_id),
                agent.correlation_id
            )
            .await
            .is_err()
        );
        assert!(
            check(
                &remote.pool,
                agent.workspace_id,
                agent.id,
                Some(Uuid::new_v4()),
                agent.correlation_id
            )
            .await
            .is_err()
        );
        assert!(
            check(
                &remote.pool,
                agent.workspace_id,
                agent.id,
                Some(agent.session_id),
                Uuid::new_v4()
            )
            .await
            .is_err()
        );
        // A consistent tuple passes this guard; no caller-Run credential is checked.
        // This test does not claim rejection of a different caller using that tuple.
    }
    #[tokio::test]
    async fn priority_null_and_omission_preserve_medium_is_explicit_and_create_defaults() {
        let (remote, _, principal, target, issue) =
            super::super::runtime_write_tests::fixture().await;
        sqlx::query("UPDATE local_issues SET priority='high' WHERE id=?")
            .bind(issue)
            .execute(&remote.pool)
            .await
            .unwrap();
        for priority in [None, Some(Value::Null), Some(json!("medium"))] {
            let revision: String =
                sqlx::query_scalar("SELECT updated_at FROM local_issues WHERE id=?")
                    .bind(issue)
                    .fetch_one(&remote.pool)
                    .await
                    .unwrap();
            let mut input = json!({"project_id":target.project_id,"issue_id":issue,"request_id":Uuid::new_v4(),"expected_revision":revision,"title":"keep requirements"});
            if let Some(value) = &priority {
                input["priority"] = value.clone();
            }
            let result =
                super::super::issue_write::call(&remote, &principal, "update_issue", input)
                    .await
                    .unwrap();
            let request: Uuid = serde_json::from_value(result["request_id"].clone()).unwrap();
            let receipt = super::super::tests::settled_operation(
                &remote,
                &principal,
                target.project_id,
                request,
            )
            .await;
            assert_eq!(receipt["status"], "succeeded", "{receipt}");
            let current: String =
                sqlx::query_scalar("SELECT priority FROM local_issues WHERE id=?")
                    .bind(issue)
                    .fetch_one(&remote.pool)
                    .await
                    .unwrap();
            assert_eq!(
                current,
                if priority == Some(json!("medium")) {
                    "medium"
                } else {
                    "high"
                }
            );
        }
        sqlx::query("UPDATE local_issues SET priority='high' WHERE id=?")
            .bind(issue)
            .execute(&remote.pool)
            .await
            .unwrap();
        for priority in [None, Some(Value::Null), Some(json!("medium"))] {
            let mut input = json!({"issue_id":issue,"title":"stdio priority"});
            if let Some(value) = &priority {
                input["priority"] = value.clone();
            }
            let result = issue_write(
                remote.operations(),
                "internal-owner".into(),
                "update_issue".into(),
                input,
            )
            .await
            .unwrap();
            assert_eq!(result["status"], "succeeded");
            let current: String =
                sqlx::query_scalar("SELECT priority FROM local_issues WHERE id=?")
                    .bind(issue)
                    .fetch_one(&remote.pool)
                    .await
                    .unwrap();
            assert_eq!(
                current,
                if priority == Some(json!("medium")) {
                    "medium"
                } else {
                    "high"
                }
            );
        }
        let created = issue_write(
            remote.operations(),
            "internal-owner".into(),
            "create_issue".into(),
            json!({"project_id":target.project_id,"title":"default priority"}),
        )
        .await
        .unwrap();
        let id: Uuid = serde_json::from_value(created["result"]["issue_id"].clone()).unwrap();
        let current: String = sqlx::query_scalar("SELECT priority FROM local_issues WHERE id=?")
            .bind(id)
            .fetch_one(&remote.pool)
            .await
            .unwrap();
        assert_eq!(current, "medium");
    }
}
