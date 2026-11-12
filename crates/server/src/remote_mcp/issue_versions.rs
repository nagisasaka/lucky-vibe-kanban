//! Manual Issue body versions. Viewing and editing never launch or steer runs.
use anyhow::ensure;
use rmcp::model::Tool;
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};
use uuid::Uuid;

use super::{
    Remote,
    auth::Principal,
    operations::Operations,
    read::{arguments, definition},
};
use crate::{error::ApiError, routes::local_remote::issue_versions as versions};

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ReadArgs {
    project_id: Uuid,
    issue_id: Uuid,
}
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct GetArgs {
    project_id: Uuid,
    issue_id: Uuid,
    version_id: Uuid,
}
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct CreateArgs {
    project_id: Uuid,
    issue_id: Uuid,
    request_id: Uuid,
    name: String,
    source_version_id: Option<Uuid>,
}
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct EditArgs {
    project_id: Uuid,
    issue_id: Uuid,
    request_id: Uuid,
    version_id: Uuid,
    expected_revision: i64,
    name: Option<String>,
    description: Option<String>,
}
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct SelectArgs {
    project_id: Uuid,
    issue_id: Uuid,
    request_id: Uuid,
    version_id: Uuid,
    expected_current_revision: i64,
    expected_version_revision: i64,
}

pub(super) fn tools() -> Vec<Tool> {
    let mut tools = vec![
        definition::<ReadArgs>(
            "list_issue_versions",
            "List this Issue's editable manual body versions and explicit current specification. Does not change the current version.",
        ),
        definition::<GetArgs>(
            "get_issue_version",
            "Read one stable Issue version ID, including body and revision. Reading a draft never changes the current specification.",
        ),
        definition::<CreateArgs>(
            "create_issue_version",
            "Explicitly create a named empty body version, or copy source_version_id without changing its body. Does not select it or start execution. Reuse the same request_id after response loss. Receipt contains metadata only; read list_issue_versions or get_issue_version for bodies.",
        ),
        definition::<EditArgs>(
            "update_issue_version",
            "Edit one version against expected_revision; omitted fields stay unchanged, empty description clears the body. Does not create versions or steer runs. Reuse the same request_id after response loss. Receipt contains metadata only; read list_issue_versions or get_issue_version for bodies.",
        ),
        definition::<SelectArgs>(
            "set_current_issue_version",
            "Explicitly select the specification used by future execution, checking both observed current revision and target version revision. Preserves every version's ID, name and body; does not steer existing runs. Receipt contains metadata only; read list_issue_versions or get_issue_version for bodies.",
        ),
    ];
    for tool in tools.iter_mut().skip(2) {
        let mut value = serde_json::to_value(&*tool).expect("tool");
        value["annotations"] = json!({"readOnlyHint":false,"destructiveHint":true,"idempotentHint":true,"openWorldHint":false});
        value["_meta"] =
            json!({"securitySchemes":[{"type":"oauth2","scopes":["lvk:read","lvk:write"]}]});
        *tool = serde_json::from_value(value).expect("tool");
    }
    tools
}

async fn scoped(pool: &sqlx::SqlitePool, project: Uuid, issue: Uuid) -> Result<(), ApiError> {
    let exists: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM local_issues WHERE id=? AND project_id=?)")
            .bind(issue)
            .bind(project)
            .fetch_one(pool)
            .await?;
    if !exists {
        return Err(ApiError::BadRequest("Issue not found".into()));
    }
    Ok(())
}

pub(super) async fn execute(
    store: Operations,
    principal: String,
    name: &str,
    input: Value,
) -> anyhow::Result<Value> {
    let project: Uuid =
        serde_json::from_value(input.get("project_id").cloned().unwrap_or(Value::Null))
            .map_err(|_| ApiError::BadRequest("Project required".into()))?;
    let issue: Uuid = serde_json::from_value(input.get("issue_id").cloned().unwrap_or(Value::Null))
        .map_err(|_| ApiError::BadRequest("Issue required".into()))?;
    if matches!(name, "list_issue_versions" | "get_issue_version") {
        scoped(&store.pool, project, issue).await?;
        if name == "list_issue_versions" {
            let args: ReadArgs = serde_json::from_value(input)
                .map_err(|_| ApiError::BadRequest("Invalid parameters".into()))?;
            let _ = (args.project_id, args.issue_id);
            return Ok(json!(versions::list(&store.pool, issue).await?));
        }
        let args: GetArgs = serde_json::from_value(input)
            .map_err(|_| ApiError::BadRequest("Invalid parameters".into()))?;
        let _ = (args.project_id, args.issue_id);
        let state = versions::list(&store.pool, issue).await?;
        let version = state
            .versions
            .into_iter()
            .find(|v| v.id == args.version_id)
            .ok_or_else(|| ApiError::BadRequest("Version not found".into()))?;
        return Ok(
            json!({"version":version,"current_version_id":state.current_version_id,"current_revision":state.current_revision}),
        );
    }
    let request: Uuid =
        serde_json::from_value(input.get("request_id").cloned().unwrap_or(Value::Null))
            .map_err(|_| ApiError::BadRequest("Request ID required".into()))?;
    let operation = name.to_owned();
    let pool = store.pool.clone();
    let payload = input.clone();
    let receipt = store
        .submit_and_wait(
            principal,
            request,
            project,
            operation.clone(),
            input,
            json!({"project_id":project,"issue_id":issue}),
            move |_| async move {
                let result = async {
                    scoped(&pool, project, issue).await?;
                    match operation.as_str() {
                        "create_issue_version" => {
                            let a: CreateArgs = serde_json::from_value(payload)
                                .map_err(|_| ApiError::BadRequest("Invalid parameters".into()))?;
                            let _ = (a.project_id, a.issue_id, a.request_id);
                            versions::create_outcome(
                                &pool,
                                issue,
                                versions::CreateVersion {
                                    name: a.name,
                                    source_version_id: a.source_version_id,
                                },
                            )
                            .await
                        }
                        "update_issue_version" => {
                            let a: EditArgs = serde_json::from_value(payload)
                                .map_err(|_| ApiError::BadRequest("Invalid parameters".into()))?;
                            let _ = (a.project_id, a.issue_id, a.request_id);
                            versions::edit_outcome(
                                &pool,
                                issue,
                                a.version_id,
                                versions::EditVersion {
                                    expected_revision: a.expected_revision,
                                    name: a.name,
                                    description: a.description.map(Some),
                                },
                            )
                            .await
                        }
                        "set_current_issue_version" => {
                            let a: SelectArgs = serde_json::from_value(payload)
                                .map_err(|_| ApiError::BadRequest("Invalid parameters".into()))?;
                            let _ = (a.project_id, a.issue_id, a.request_id);
                            versions::select_outcome(
                                &pool,
                                issue,
                                versions::SelectVersion {
                                    version_id: a.version_id,
                                    expected_current_revision: a.expected_current_revision,
                                    expected_version_revision: a.expected_version_revision,
                                },
                            )
                            .await
                        }
                        _ => Err(ApiError::BadRequest("Unknown version operation".into()).into()),
                    }
                }
                .await;
                match result {
                    Ok(state) => {
                        // Receipts establish mutation identity and revisions, not an
                        // automatic history of every version's body on every save.
                        let metadata: Vec<Value> = state
                            .versions
                            .iter()
                            .map(|version| {
                                json!({
                                    "id":version.id,
                                    "issue_id":version.issue_id,
                                    "name":version.name,
                                    "revision":version.revision,
                                })
                            })
                            .collect();
                        (
                            "succeeded",
                            json!({"state":{
                            "versions":metadata,
                            "current_version_id":state.current_version_id,
                            "current_revision":state.current_revision,
                            "created_version_id":state.created_version_id,
                        },"implementation_started":false}),
                        )
                    }
                    Err(error) => super::issue_mutations::domain_failure(error),
                }
            },
        )
        .await?;
    Ok(super::issue_mutations::receipt_value(receipt, |p| {
        format!("/projects/{p}")
    }))
}

pub(super) async fn call(
    remote: &Remote,
    principal: &Principal,
    name: &str,
    input: Value,
) -> anyhow::Result<Value> {
    ensure!(
        principal.allows("lvk:read"),
        super::errors::rejection(utils::mcp_error::Category::Permission)
    );
    if !matches!(name, "list_issue_versions" | "get_issue_version") {
        ensure!(
            principal.allows("lvk:write"),
            super::errors::rejection(utils::mcp_error::Category::Permission)
        );
    }
    let project: Uuid =
        arguments::<ReadProject>(json!({"project_id":input.get("project_id")}))?.project_id;
    remote.require_project(project).await?;
    execute(
        remote.operations(),
        remote.principal_key(principal),
        name,
        input,
    )
    .await
}
#[derive(Deserialize)]
struct ReadProject {
    project_id: Uuid,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn manual_versions_preserve_identity_scope_cas_and_receipt_replay() {
        let (remote, _, principal, target, _) = super::super::runtime_write_tests::fixture().await;
        let issue: Uuid =
            sqlx::query_scalar("SELECT issue_id FROM local_workspace_links WHERE workspace_id=?")
                .bind(target.workspace_id)
                .fetch_one(&remote.pool)
                .await
                .unwrap();
        let base = json!({"project_id":target.project_id,"issue_id":issue});
        let initial = call(&remote, &principal, "list_issue_versions", base.clone())
            .await
            .unwrap();
        assert_eq!(initial["versions"].as_array().unwrap().len(), 1);
        let initial_id = initial["current_version_id"].clone();
        let create = json!({"project_id":target.project_id,"issue_id":issue,"request_id":Uuid::new_v4(),"name":"draft2","source_version_id":initial_id});
        let copied = call(&remote, &principal, "create_issue_version", create.clone())
            .await
            .unwrap();
        assert_eq!(copied["status"], "succeeded");
        let replay = call(&remote, &principal, "create_issue_version", create.clone())
            .await
            .unwrap();
        assert_eq!(copied["operation_id"], replay["operation_id"]);
        let mut conflicting_input = create;
        conflicting_input["name"] = json!("different payload");
        let error = call(
            &remote,
            &principal,
            "create_issue_version",
            conflicting_input,
        )
        .await
        .unwrap_err();
        let diagnostic = super::super::errors::diagnostic(&error, true);
        assert_eq!(diagnostic.category, utils::mcp_error::Category::Conflict);
        assert_eq!(diagnostic.outcome, utils::mcp_error::Outcome::Unknown);
        let state = &copied["result"]["state"];
        assert_eq!(state["versions"].as_array().unwrap().len(), 2);
        assert_eq!(state["current_version_id"], initial_id);
        let draft = state["versions"]
            .as_array()
            .unwrap()
            .iter()
            .find(|v| v["name"] == "draft2")
            .unwrap()["id"]
            .clone();
        let edited = call(&remote,&principal,"update_issue_version",json!({"project_id":target.project_id,"issue_id":issue,"request_id":Uuid::new_v4(),"version_id":draft,"expected_revision":0,"description":"Draft requirements","name":"proposal"})).await.unwrap();
        assert_eq!(edited["status"], "succeeded");
        assert!(!edited.to_string().contains("description"));
        assert!(!edited.to_string().contains("Draft requirements"));
        assert!(
            edited["result"]["state"]["versions"]
                .as_array()
                .unwrap()
                .iter()
                .all(|v| v.get("description").is_none())
        );
        let current: Option<String> =
            sqlx::query_scalar("SELECT description FROM local_issues WHERE id=?")
                .bind(issue)
                .fetch_one(&remote.pool)
                .await
                .unwrap();
        assert_eq!(current, None);
        let stale = call(&remote,&principal,"update_issue_version",json!({"project_id":target.project_id,"issue_id":issue,"request_id":Uuid::new_v4(),"version_id":draft,"expected_revision":0,"description":"lost"})).await.unwrap();
        assert_eq!(stale["status"], "failed");
        assert_eq!(stale["result"]["code"], "revision_conflict");
        let selected = call(&remote,&principal,"set_current_issue_version",json!({"project_id":target.project_id,"issue_id":issue,"request_id":Uuid::new_v4(),"version_id":draft,"expected_current_revision":0,"expected_version_revision":1})).await.unwrap();
        assert_eq!(selected["status"], "succeeded");
        let current: Option<String> =
            sqlx::query_scalar("SELECT description FROM local_issues WHERE id=?")
                .bind(issue)
                .fetch_one(&remote.pool)
                .await
                .unwrap();
        assert_eq!(current.as_deref(), Some("Draft requirements"));
        let current_issue = super::super::read::call(&remote, "get_issue", base.clone())
            .await
            .unwrap();
        assert_eq!(current_issue["description_page"], "Draft requirements");
        let saved = call(&remote, &principal, "update_issue_version", json!({"project_id":target.project_id,"issue_id":issue,"request_id":Uuid::new_v4(),"version_id":draft,"expected_revision":1,"description":"Saved current specification"})).await.unwrap();
        assert_eq!(saved["status"], "succeeded");
        assert!(!saved.to_string().contains("description"));
        assert!(!saved.to_string().contains("Saved current specification"));
        let current_issue = super::super::read::call(&remote, "get_issue", base.clone())
            .await
            .unwrap();
        assert_eq!(
            current_issue["description_page"],
            "Saved current specification"
        );
        let read = call(
            &remote,
            &principal,
            "get_issue_version",
            json!({"project_id":target.project_id,"issue_id":issue,"version_id":initial_id}),
        )
        .await
        .unwrap();
        assert_eq!(read["version"]["description"], Value::Null);
        assert_eq!(read["current_version_id"], draft);
        assert!(
            call(
                &remote,
                &principal,
                "list_issue_versions",
                json!({"project_id":Uuid::new_v4(),"issue_id":issue})
            )
            .await
            .is_err()
        );
        let other = Uuid::new_v4();
        let missing = call(
            &remote,
            &principal,
            "get_issue_version",
            json!({"project_id":target.project_id,"issue_id":issue,"version_id":other}),
        )
        .await;
        assert!(missing.is_err());
        let mut readonly = principal.clone();
        readonly.scopes.retain(|s| s != "lvk:write");
        assert!(call(&remote,&readonly,"create_issue_version",json!({"project_id":target.project_id,"issue_id":issue,"request_id":Uuid::new_v4(),"name":"denied"})).await.is_err());
    }

    #[test]
    fn version_tools_expose_required_concurrency_and_scope_contracts() {
        let tools = tools();
        assert_eq!(tools.len(), 5);
        for tool in tools.iter().skip(2) {
            let value = serde_json::to_value(tool).unwrap();
            assert_eq!(value["annotations"]["idempotentHint"], true);
            assert!(
                value["inputSchema"]["required"]
                    .as_array()
                    .unwrap()
                    .contains(&json!("request_id"))
            );
        }
        assert!(serde_json::from_value::<EditArgs>(json!({"project_id":Uuid::new_v4(),"issue_id":Uuid::new_v4(),"request_id":Uuid::new_v4(),"version_id":Uuid::new_v4(),"description":"missing revision"})).is_err());
    }
}
