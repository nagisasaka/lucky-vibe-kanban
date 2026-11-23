use anyhow::ensure;
use api_types::*;
use db::models::saved_filter::{self as store, SavedFilterWrite};
use rmcp::model::Tool;
use serde_json::{Value, json};
use uuid::Uuid;

use super::{
    Remote,
    auth::Principal,
    read::{arguments, definition},
};
use crate::{error::ApiError, routes::saved_filters as service};

pub(super) fn tools() -> Vec<Tool> {
    let mut tools = vec![
        definition::<SavedFilterListArgs>(
            "list_saved_filters",
            "List Project-owned saved conditions, not fixed Issue sets. Follow next_offset using page.offset.",
        ),
        definition::<SavedFilterReadArgs>(
            "get_saved_filter",
            "Read current definition and revision. Name changes preserve saved_filter_id.",
        ),
        definition::<SavedFilterReadArgs>(
            "get_saved_filter_results",
            "Read current machine results and cached semantic state without starting AI. Pass evaluation_revision as page.expected_evaluation_revision with next_offset; on conflict discard pages and restart at offset 0 without the token. Unevaluated/failed is not a complete empty result. Evaluate to refresh stale evidence.",
        ),
        definition::<SavedFilterReadArgs>(
            "get_saved_filter_link",
            "Get a stable Project board link selecting the current saved definition. Does not remotely change any open screen.",
        ),
    ];
    for mut tool in [
        definition::<SavedFilterCreateArgs>(
            "create_saved_filter",
            "Save shared Project conditions. Fields combine with AND; selections within a field use OR, tags support any/all. Search is literal case-insensitive title/description substring. Relative periods use [now-seconds,now], absolute [start,end). Reuse input.request_id and identical input on retry. Returns a durable operation receipt; get_operation resolves acceptance.",
        ),
        definition::<SavedFilterUpdateArgs>(
            "update_saved_filter",
            "Replace the full definition at expected_revision, retaining its ID. Reuse input.request_id on identical retries; inspect get_operation when outcome is uncertain.",
        ),
        definition::<SavedFilterDeleteArgs>(
            "delete_saved_filter",
            "Delete only this saved definition/cache at expected_revision, preserving Issues and relationships. Reuse input.request_id and inspect get_operation.",
        ),
        definition::<SavedFilterEvaluateArgs>(
            "evaluate_saved_filter",
            "Evaluate current saved conditions against the complete Project corpus; reuses valid semantic cache. May start server-side AI. Reuse request_id on identical retries; get_operation returns the receipt, get_saved_filter_results returns current state and pages.",
        ),
    ] {
        let mut value = serde_json::to_value(&tool).expect("tool");
        value["annotations"] = json!({"readOnlyHint":false,"destructiveHint":true,"idempotentHint":true,"openWorldHint":false});
        value["_meta"] =
            json!({"securitySchemes":[{"type":"oauth2","scopes":["lvk:read","lvk:write"]}]});
        tool = serde_json::from_value(value).expect("tool");
        tools.push(tool);
    }
    tools
}
fn diagnostic(error: &ApiError) -> utils::mcp_error::Diagnostic {
    use db::models::saved_filter::SavedFilterError as E;
    use utils::mcp_error::{Category as C, Diagnostic, Outcome};
    if matches!(error, ApiError::SavedFilterEvaluationChanged) {
        let mut diagnostic = Diagnostic::new(C::Conflict, Outcome::Failed);
        diagnostic.recovery = "restart_result_pagination";
        diagnostic.message = "Saved filter evaluation changed. Discard collected pages and restart at offset 0 without expected_evaluation_revision.";
        return diagnostic;
    }
    let (category, outcome) = match error {
        ApiError::SavedFilter(E::NotFound) => (C::NotFound, Outcome::Failed),
        ApiError::SavedFilter(E::Conflict) | ApiError::Conflict(_) => {
            (C::Conflict, Outcome::Failed)
        }
        ApiError::SavedFilter(E::Invalid(_)) | ApiError::BadRequest(_) => {
            (C::Validation, Outcome::Failed)
        }
        ApiError::SavedFilter(E::CommitUnknown(_)) => (C::OutcomeUnknown, Outcome::Unknown),
        ApiError::Forbidden(_) | ApiError::Unauthorized => (C::Permission, Outcome::Failed),
        _ => (C::Internal, Outcome::Failed),
    };
    Diagnostic::new(category, outcome)
}
fn failure(error: ApiError) -> (&'static str, Value) {
    let unknown = matches!(
        error,
        ApiError::SavedFilter(store::SavedFilterError::CommitUnknown(_))
    );
    let d = diagnostic(&error);
    (
        if unknown { "uncertain" } else { "failed" },
        json!({"message":d.message,"error":d.value()}),
    )
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
    let project: Uuid = arguments(input.get("project_id").cloned().unwrap_or(Value::Null))?;
    remote.require_project(project).await?;
    if name == "list_saved_filters" {
        let a: SavedFilterListArgs = arguments(input)?;
        return store::list(&remote.pool, project, &a.page)
            .await
            .map(|v| json!(v))
            .map_err(|e| diagnostic(&e.into()).into());
    }
    if matches!(
        name,
        "get_saved_filter" | "get_saved_filter_results" | "get_saved_filter_link"
    ) {
        let a: SavedFilterReadArgs = arguments(input)?;
        let result: Result<Value, ApiError> = match name {
            "get_saved_filter" => store::get(&remote.pool, project, a.saved_filter_id)
                .await
                .map(|v| json!(v))
                .map_err(Into::into),
            "get_saved_filter_results" => service::evaluate_service(
                &remote.pool,
                remote
                    .runtime
                    .as_ref()
                    .and_then(|r| r.saved_filter_deployment()),
                project,
                a.saved_filter_id,
                &a.page,
                service::EvaluationOptions::default(),
            )
            .await
            .map(|v| json!(v)),
            _ => service::link_service(&remote.pool, project, a.saved_filter_id)
                .await
                .map(|mut v| {
                    v.url = format!("{}{}", remote.config.public_base_url, v.url);
                    json!(v)
                }),
        };
        return result.map_err(|e| diagnostic(&e).into());
    }
    ensure!(
        principal.allows("lvk:write"),
        super::errors::rejection(utils::mcp_error::Category::Permission)
    );
    let (request, write, evaluate) = match name {
        "create_saved_filter" => {
            let a: SavedFilterCreateArgs = arguments(input.clone())?;
            (
                a.input.request_id,
                Some(SavedFilterWrite::Create(a.input)),
                None,
            )
        }
        "update_saved_filter" => {
            let a: SavedFilterUpdateArgs = arguments(input.clone())?;
            (
                a.input.request_id,
                Some(SavedFilterWrite::Update {
                    id: a.saved_filter_id,
                    input: a.input,
                }),
                None,
            )
        }
        "delete_saved_filter" => {
            let a: SavedFilterDeleteArgs = arguments(input.clone())?;
            (
                a.input.request_id,
                Some(SavedFilterWrite::Delete {
                    id: a.saved_filter_id,
                    input: a.input,
                }),
                None,
            )
        }
        "evaluate_saved_filter" => {
            let a: SavedFilterEvaluateArgs = arguments(input.clone())?;
            (a.request_id, None, Some(a))
        }
        _ => anyhow::bail!("Unknown saved filter tool"),
    };
    let pool = remote.pool.clone();
    let runtime = remote.runtime.clone();
    let actor = format!("mcp:{}", remote.principal_key(principal));
    let receipt = remote
        .operations()
        .submit(
            remote.principal_key(principal),
            request,
            project,
            name.into(),
            input,
            json!({"project_id":project}),
            move |_| async move {
                let result = if let Some(write) = write {
                    store::write(&pool, project, &actor, write)
                        .await
                        .map_err(ApiError::from)
                } else {
                    let a = evaluate.expect("evaluate input");
                    service::evaluate_service(
                        &pool,
                        runtime.as_ref().and_then(|r| r.saved_filter_deployment()),
                        project,
                        a.saved_filter_id,
                        &a.page,
                        service::EvaluationOptions {
                            activate: true,
                            retry_failed: a.retry_failed,
                            ..Default::default()
                        },
                    )
                    .await
                    .map(|v| json!(v))
                };
                match result {
                    Ok(v) => ("succeeded", v),
                    Err(e) => failure(e),
                }
            },
        )
        .await?;
    Ok(remote.receipt_value(receipt))
}

#[cfg(test)]
mod tests {
    use super::{
        super::{runtime_write_tests::fixture, tests::settled_operation},
        *,
    };
    #[tokio::test]
    async fn saved_filter_remote_receipt_auth_scope_and_common_results() {
        let (remote, _, principal, target, issue) = fixture().await;
        let project = target.project_id;
        let request = Uuid::new_v4();
        let input = json!({"project_id":project,"input":{"request_id":request,"definition":{"name":"All active","machine":{},"natural_language":null,"context":null}}});
        let receipt = call(&remote, &principal, "create_saved_filter", input.clone())
            .await
            .unwrap();
        let settled = settled_operation(&remote, &principal, project, request).await;
        assert_eq!(settled["status"], "succeeded", "{settled}");
        let definition = settled["result"].clone();
        let id: Uuid = serde_json::from_value(definition["id"].clone()).unwrap();
        let replay = call(&remote, &principal, "create_saved_filter", input.clone())
            .await
            .unwrap();
        assert_eq!(receipt["operation_id"], replay["operation_id"]);
        assert_eq!(
            store::list(&remote.pool, project, &SavedFilterPage::default())
                .await
                .unwrap()
                .total_count,
            1
        );
        let mut different = input.clone();
        different["input"]["definition"]["name"] = json!("changed");
        assert!(
            call(&remote, &principal, "create_saved_filter", different)
                .await
                .is_err()
        );
        let args = json!({"project_id":project,"saved_filter_id":id});
        let from_mcp = call(
            &remote,
            &principal,
            "get_saved_filter_results",
            args.clone(),
        )
        .await
        .unwrap();
        let direct = service::evaluate_service(
            &remote.pool,
            None,
            project,
            id,
            &SavedFilterPage::default(),
            service::EvaluationOptions::default(),
        )
        .await
        .unwrap();
        assert_eq!(from_mcp, json!(direct));
        assert_eq!(from_mcp["issue_ids"], json!([issue]));
        let stale = call(&remote, &principal, "get_saved_filter_results", json!({"project_id":project,"saved_filter_id":id,"page":{"offset":1,"expected_evaluation_revision":"old"}})).await.unwrap_err();
        let diagnostic = stale
            .downcast_ref::<utils::mcp_error::Diagnostic>()
            .unwrap();
        assert_eq!(diagnostic.category, utils::mcp_error::Category::Conflict);
        assert_eq!(diagnostic.recovery, "restart_result_pagination");
        let mut readonly = principal.clone();
        readonly.scopes.remove("lvk:write");
        assert!(
            call(&remote, &readonly, "create_saved_filter", input)
                .await
                .is_err()
        );
        assert!(
            call(&remote, &readonly, "get_saved_filter", args.clone())
                .await
                .is_ok()
        );
        assert!(
            call(
                &remote,
                &principal,
                "get_saved_filter",
                json!({"project_id":Uuid::new_v4(),"saved_filter_id":id})
            )
            .await
            .is_err()
        );
        let link = call(&remote, &principal, "get_saved_filter_link", args)
            .await
            .unwrap();
        assert_eq!(
            link["url"],
            format!("https://lvk.example/team/projects/{project}?savedFilter={id}")
        );
        let delete = json!({"project_id":project,"saved_filter_id":id,"input":{"request_id":Uuid::new_v4(),"expected_revision":1}});
        let request = serde_json::from_value(delete["input"]["request_id"].clone()).unwrap();
        call(&remote, &principal, "delete_saved_filter", delete)
            .await
            .unwrap();
        assert_eq!(
            settled_operation(&remote, &principal, project, request).await["status"],
            "succeeded"
        );
        assert!(
            service::link_service(&remote.pool, project, id)
                .await
                .is_err()
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM local_issues WHERE id=?")
                .bind(issue)
                .fetch_one(&remote.pool)
                .await
                .unwrap(),
            1
        );
    }
}
