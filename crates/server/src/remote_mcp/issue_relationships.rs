//! Additive Board relationships with durable remote receipts.
use anyhow::ensure;
use api_types::{CreateIssueRelationshipRequest, IssueRelationshipType};
use rmcp::model::Tool;
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::Row;
use utils::mcp_error::Category;
use uuid::Uuid;

use super::{
    Remote,
    auth::Principal,
    issue_write::{domain_failure, parse_revision},
    read::{arguments, definition, page},
};
use crate::routes::local_remote::issue_relations::{self, Guard};

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ListArgs {
    project_id: Uuid,
    issue_id: Uuid,
    #[serde(default = "super::default_limit")]
    #[schemars(range(min = 1, max = 50))]
    limit: u32,
    cursor: Option<Uuid>,
}
#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
enum Kind {
    Related,
    Blocking,
}
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct CreateArgs {
    project_id: Uuid,
    /// Source: for blocking, this Issue blocks related_issue_id.
    issue_id: Uuid,
    related_issue_id: Uuid,
    relationship_type: Kind,
    /// Revision from get_issue for the source Issue.
    expected_revision: String,
    request_id: Uuid,
}
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct DeleteArgs {
    project_id: Uuid,
    /// Either endpoint, as read with list_issue_relationships.
    issue_id: Uuid,
    relationship_id: Uuid,
    expected_revision: String,
    request_id: Uuid,
}
pub(super) fn tools() -> Vec<Tool> {
    let mut tools = vec![definition::<ListArgs>(
        "list_issue_relationships",
        "Read incoming and outgoing structured Issue relationships, including the other Issue ID/simple_id, source/target direction and relationship ID for deletion. Follow next_cursor until has_more is false. Errors are not empty successful lists.",
    )];
    for tool in [
        definition::<CreateArgs>(
            "create_issue_relationship",
            "Add related or blocking using the source Issue revision from get_issue. For A blocks B use issue_id=A, related_issue_id=B once; B reads the same row as target/blocked_by. Related is undirected and reverse duplicates reuse the existing row. Blocking cycles between different Issues are allowed. Preserves other relationships and never starts implementation. Read relations before/after and reuse request_id/get_operation after uncertainty.",
        ),
        definition::<DeleteArgs>(
            "delete_issue_relationship",
            "Explicitly remove only the specified relationship_id from either endpoint, using its get_issue revision. Preserves Issues, parents, other relationships and Workspaces. Read list_issue_relationships before/after; use stable request_id/get_operation after uncertainty.",
        ),
    ] {
        let mut value = serde_json::to_value(tool).expect("relationship tool");
        value["annotations"] = json!({"readOnlyHint":false,"destructiveHint":true,"idempotentHint":true,"openWorldHint":false});
        value["_meta"] =
            json!({"securitySchemes":[{"type":"oauth2","scopes":["lvk:read","lvk:write"]}]});
        tools.push(serde_json::from_value(value).expect("relationship tool"));
    }
    tools
}
pub(super) async fn call(
    remote: &Remote,
    principal: &Principal,
    name: &str,
    input: Value,
) -> anyhow::Result<Value> {
    ensure!(
        principal.allows("lvk:read"),
        super::errors::rejection(Category::Permission)
    );
    if name == "list_issue_relationships" {
        let args: ListArgs = arguments(input)?;
        remote.require_issue(args.project_id, args.issue_id).await?;
        ensure!(
            (1..=50).contains(&args.limit),
            super::errors::rejection(Category::Validation)
        );
        // LEFT JOIN prevents dangling/cross-Project endpoints from silently
        // disappearing. Such a page fails instead of claiming completeness.
        let rows=sqlx::query("SELECT r.*,a.project_id AS source_project,b.project_id AS target_project,a.simple_id AS source_simple_id,b.simple_id AS target_simple_id FROM local_issue_relationships r LEFT JOIN local_issues a ON a.id=r.issue_id LEFT JOIN local_issues b ON b.id=r.related_issue_id WHERE (r.issue_id=? OR r.related_issue_id=?) AND (? IS NULL OR r.id>?) ORDER BY r.id LIMIT ?")
            .bind(args.issue_id).bind(args.issue_id).bind(args.cursor).bind(args.cursor).bind(args.limit+1).fetch_all(&remote.pool).await?;
        let mut items = Vec::new();
        for r in rows {
            ensure!(
                r.try_get::<Option<Uuid>, _>("source_project")? == Some(args.project_id)
                    && r.try_get::<Option<Uuid>, _>("target_project")? == Some(args.project_id),
                super::errors::rejection(Category::Unavailable)
            );
            let source: Uuid = r.try_get("issue_id")?;
            let target: Uuid = r.try_get("related_issue_id")?;
            let is_source = source == args.issue_id;
            let kind: String = r.try_get("relationship_type")?;
            ensure!(
                matches!(kind.as_str(), "related" | "blocking" | "has_duplicate"),
                super::errors::rejection(Category::Unavailable)
            );
            items.push(json!({"id":r.try_get::<Uuid,_>("id")?,"issue_id":source,"source_issue_id":source,"target_issue_id":target,"related_issue_id":if is_source {target} else {source},"related_simple_id":r.try_get::<String,_>(if is_source {"target_simple_id"} else {"source_simple_id"})?,"relationship_type":kind,"direction":if is_source {"source"} else {"target"}}));
        }
        return Ok(page(items, args.limit));
    }
    ensure!(
        principal.allows("lvk:write"),
        super::errors::rejection(Category::Permission)
    );
    let owned = remote.clone();
    let receipt = match name {
        "create_issue_relationship" => {
            let args: CreateArgs = arguments(input.clone())?;
            let revision = parse_revision(&args.expected_revision)?;
            remote.require_project(args.project_id).await?;
            remote
                .operations()
                .submit(
                    remote.principal_key(principal),
                    args.request_id,
                    args.project_id,
                    name.into(),
                    input,
                    json!({"issue_id":args.issue_id,"related_issue_id":args.related_issue_id}),
                    move |id| async move {
                        let request = CreateIssueRelationshipRequest {
                            id: Some(id),
                            issue_id: args.issue_id,
                            related_issue_id: args.related_issue_id,
                            relationship_type: match args.relationship_type {
                                Kind::Related => IssueRelationshipType::Related,
                                Kind::Blocking => IssueRelationshipType::Blocking,
                            },
                        };
                        match issue_relations::create(
                            &owned.pool,
                            &request,
                            Some(Guard {
                                project: args.project_id,
                                issue: args.issue_id,
                                revision,
                            }),
                        )
                        .await
                        {
                            Ok(relation) => (
                                "succeeded",
                                json!({"relationship":relation,"implementation_started":false}),
                            ),
                            Err(error) => domain_failure(error),
                        }
                    },
                )
                .await?
        }
        "delete_issue_relationship" => {
            let args: DeleteArgs = arguments(input.clone())?;
            let revision = parse_revision(&args.expected_revision)?;
            remote.require_project(args.project_id).await?;
            remote.operations().submit(remote.principal_key(principal),args.request_id,args.project_id,name.into(),input,json!({"issue_id":args.issue_id,"relationship_id":args.relationship_id}),move |_| async move {
                match issue_relations::delete(&owned.pool,args.relationship_id,Some(Guard {project:args.project_id,issue:args.issue_id,revision})).await {
                    Ok(())=>("succeeded",json!({"relationship_id":args.relationship_id,"present":false,"implementation_started":false})),
                    Err(error)=>domain_failure(error),
                }
            }).await?
        }
        _ => return Err(super::errors::rejection(Category::Validation)),
    };
    Ok(remote.receipt_value(receipt))
}

#[cfg(test)]
mod tests {
    use super::{
        super::{issue_write, read, tests::settled_operation},
        *,
    };
    use crate::routes::local_remote;

    async fn read_issue(remote: &Remote, project: Uuid, issue: Uuid) -> Value {
        read::call(
            remote,
            "get_issue",
            json!({"project_id":project,"issue_id":issue}),
        )
        .await
        .unwrap()
    }
    async fn write(remote: &Remote, principal: &Principal, name: &str, input: Value) -> Value {
        let request = serde_json::from_value(input["request_id"].clone()).unwrap();
        let project = serde_json::from_value(input["project_id"].clone()).unwrap();
        if matches!(name, "create_issue" | "update_issue") {
            issue_write::call(remote, principal, name, input)
                .await
                .unwrap();
        } else {
            call(remote, principal, name, input).await.unwrap();
        }
        settled_operation(remote, principal, project, request).await
    }
    async fn make(
        remote: &Remote,
        principal: &Principal,
        project: Uuid,
        status: Uuid,
        parent: Option<Uuid>,
    ) -> Uuid {
        let value=write(remote,principal,"create_issue",json!({"project_id":project,"status_id":status,"request_id":Uuid::new_v4(),"title":"child","description":"agreed","parent_issue_id":parent})).await;
        assert_eq!(value["status"], "succeeded", "{value}");
        serde_json::from_value(value["result"]["issue_id"].clone()).unwrap()
    }
    async fn change_parent(
        remote: &Remote,
        principal: &Principal,
        project: Uuid,
        issue: Uuid,
        changes: Value,
    ) -> Value {
        let current = read_issue(remote, project, issue).await;
        let mut input = json!({"project_id":project,"issue_id":issue,"request_id":Uuid::new_v4(),"expected_revision":current["revision"]});
        input
            .as_object_mut()
            .unwrap()
            .extend(changes.as_object().unwrap().clone());
        write(remote, principal, "update_issue", input).await
    }
    #[tokio::test]
    async fn parents_create_reparent_clear_preserve_and_reject_cycles_atomically() {
        let (remote, runtime, principal, target, root) =
            super::super::runtime_write_tests::fixture().await;
        let project = target.project_id;
        let status: Uuid = sqlx::query_scalar("SELECT status_id FROM local_issues WHERE id=?")
            .bind(root)
            .fetch_one(&remote.pool)
            .await
            .unwrap();
        let child = make(&remote, &principal, project, status, Some(root)).await;
        assert_eq!(
            read_issue(&remote, project, child).await["parent_issue_id"],
            json!(root)
        );
        let grandchild = make(&remote, &principal, project, status, Some(child)).await;
        for parent in [root, grandchild, Uuid::new_v4()] {
            let before = read_issue(&remote, project, root).await;
            let result = change_parent(
                &remote,
                &principal,
                project,
                root,
                json!({"parent_issue_id":parent,"description":"must roll back"}),
            )
            .await;
            assert_eq!(result["status"], "failed");
            assert_eq!(read_issue(&remote, project, root).await, before);
        }
        // The UI/stdio shared update path must reject the same descendant cycle.
        assert!(
            local_remote::update_local_issue(
                &remote.pool,
                root,
                serde_json::from_value(json!({"parent_issue_id":grandchild})).unwrap()
            )
            .await
            .is_err()
        );
        let second = make(&remote, &principal, project, status, None).await;
        assert_eq!(
            change_parent(
                &remote,
                &principal,
                project,
                child,
                json!({"parent_issue_id":second})
            )
            .await["status"],
            "succeeded"
        );
        for changes in [
            json!({"title":"renamed"}),
            json!({"parent_issue_id":null,"description":"kept parent"}),
        ] {
            assert_eq!(
                change_parent(&remote, &principal, project, child, changes).await["status"],
                "succeeded"
            );
            assert_eq!(
                read_issue(&remote, project, child).await["parent_issue_id"],
                json!(second)
            );
        }
        let before = read_issue(&remote, project, child).await;
        assert_eq!(
            change_parent(
                &remote,
                &principal,
                project,
                child,
                json!({"clear_parent":true})
            )
            .await["status"],
            "succeeded"
        );
        let after = read_issue(&remote, project, child).await;
        assert!(after["parent_issue_id"].is_null());
        assert_eq!(before["description_page"], after["description_page"]);
        assert_eq!(before["status_id"], after["status_id"]);
        assert_eq!(
            local_remote::get_local_issue(&remote.pool, child)
                .await
                .unwrap()
                .parent_issue_id,
            None
        );
        let invalid = json!({"project_id":project,"issue_id":child,"expected_revision":after["revision"],"request_id":Uuid::new_v4(),"parent_issue_id":root,"clear_parent":true});
        assert!(
            issue_write::call(&remote, &principal, "update_issue", invalid)
                .await
                .is_err()
        );
        assert!(runtime.calls.lock().await.is_empty());
        let workspaces: i64 = sqlx::query_scalar("SELECT count(*) FROM workspaces")
            .fetch_one(&remote.pool)
            .await
            .unwrap();
        assert_eq!(workspaces, 1);
    }

    #[tokio::test]
    async fn relationships_receipts_concurrency_direction_and_targeted_delete() {
        // Real separate connections exercise writer serialization, not a single
        // connection queue. The temp directory owns and cleans the isolated DB.
        let directory = tempfile::tempdir().unwrap();
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(4)
            .connect_with(
                sqlx::sqlite::SqliteConnectOptions::new()
                    .filename(directory.path().join("relations.sqlite"))
                    .create_if_missing(true)
                    .busy_timeout(std::time::Duration::from_secs(5)),
            )
            .await
            .unwrap();
        let (remote, runtime, principal, target, a) =
            super::super::runtime_write_tests::fixture_with_pool(pool).await;
        let project = target.project_id;
        sqlx::query("UPDATE projects SET key='REL' WHERE id=?")
            .bind(project)
            .execute(&remote.pool)
            .await
            .unwrap();
        let status: Uuid = sqlx::query_scalar("SELECT status_id FROM local_issues WHERE id=?")
            .bind(a)
            .fetch_one(&remote.pool)
            .await
            .unwrap();
        let b = make(&remote, &principal, project, status, Some(a)).await;
        let c = make(&remote, &principal, project, status, None).await;
        let original = read_issue(&remote, project, b).await;
        let revision = read_issue(&remote, project, a).await["revision"].clone();
        let add = json!({"project_id":project,"issue_id":a,"related_issue_id":b,"relationship_type":"blocking","expected_revision":revision,"request_id":Uuid::new_v4()});
        let first = write(
            &remote,
            &principal,
            "create_issue_relationship",
            add.clone(),
        )
        .await;
        assert_eq!(first["status"], "succeeded", "{first}");
        assert_eq!(
            write(
                &remote,
                &principal,
                "create_issue_relationship",
                add.clone()
            )
            .await["result"],
            first["result"]
        );
        let mut different = add.clone();
        different["related_issue_id"] = json!(c);
        assert!(
            call(&remote, &principal, "create_issue_relationship", different)
                .await
                .is_err(),
            "a request_id cannot be reused for different input"
        );
        let relation = first["result"]["relationship"]["id"].clone();
        let b_revision = original["revision"].clone();
        let related = json!({"project_id":project,"issue_id":a,"related_issue_id":b,"relationship_type":"related","expected_revision":revision,"request_id":Uuid::new_v4()});
        let mut reverse = related.clone();
        reverse["issue_id"] = json!(b);
        reverse["related_issue_id"] = json!(a);
        reverse["expected_revision"] = b_revision.clone();
        reverse["request_id"] = json!(Uuid::new_v4());
        let (left, right) = tokio::join!(
            write(
                &remote,
                &principal,
                "create_issue_relationship",
                related.clone()
            ),
            write(&remote, &principal, "create_issue_relationship", reverse)
        );
        assert_eq!(left["status"], "succeeded", "{left}");
        assert_eq!(right["status"], "succeeded", "{right}");
        assert_eq!(
            left["result"]["relationship"]["id"],
            right["result"]["relationship"]["id"]
        );
        // Shared Board path also deduplicates reverse related requests.
        let board = issue_relations::create(
            &remote.pool,
            &CreateIssueRelationshipRequest {
                id: None,
                issue_id: b,
                related_issue_id: a,
                relationship_type: IssueRelationshipType::Related,
            },
            None,
        )
        .await
        .unwrap();
        assert_eq!(json!(board.id), left["result"]["relationship"]["id"]);
        assert!(
            issue_relations::create(
                &remote.pool,
                &CreateIssueRelationshipRequest {
                    id: Some(board.id),
                    issue_id: a,
                    related_issue_id: c,
                    relationship_type: IssueRelationshipType::Related,
                },
                None
            )
            .await
            .is_err(),
            "a primary-key collision is not a successful replay"
        );
        // Intentional blocking cycles, including three nodes, are accepted.
        for (source, target) in [(b, a), (b, c), (c, a)] {
            let rev = read_issue(&remote, project, source).await["revision"].clone();
            let result=write(&remote,&principal,"create_issue_relationship",json!({"project_id":project,"issue_id":source,"related_issue_id":target,"relationship_type":"blocking","expected_revision":rev,"request_id":Uuid::new_v4()})).await;
            assert_eq!(result["status"], "succeeded");
        }
        for (issue, other, direction) in [(a, b, "source"), (b, a, "target")] {
            let mut cursor = Value::Null;
            let mut found = false;
            loop {
                let page = call(
                    &remote,
                    &principal,
                    "list_issue_relationships",
                    json!({"project_id":project,"issue_id":issue,"limit":1,"cursor":cursor}),
                )
                .await
                .unwrap();
                for item in page["items"].as_array().unwrap() {
                    if item["id"] == relation {
                        assert_eq!(item["related_issue_id"], json!(other));
                        assert!(!item["related_simple_id"].as_str().unwrap().is_empty());
                        assert_eq!(item["direction"], direction);
                        found = true;
                    }
                }
                if page["has_more"] == false {
                    break;
                }
                cursor = page["next_cursor"].clone();
            }
            assert!(found);
        }
        let remove = json!({"project_id":project,"issue_id":b,"relationship_id":relation,"expected_revision":b_revision,"request_id":Uuid::new_v4()});
        let removed = write(
            &remote,
            &principal,
            "delete_issue_relationship",
            remove.clone(),
        )
        .await;
        assert_eq!(removed["status"], "succeeded");
        assert_eq!(
            write(&remote, &principal, "delete_issue_relationship", remove).await["result"],
            removed["result"]
        );
        for issue in [a, b] {
            let page = call(
                &remote,
                &principal,
                "list_issue_relationships",
                json!({"project_id":project,"issue_id":issue}),
            )
            .await
            .unwrap();
            assert!(
                !page["items"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|v| v["id"] == relation)
            );
        }
        let count: i64 = sqlx::query_scalar("SELECT count(*) FROM local_issue_relationships")
            .fetch_one(&remote.pool)
            .await
            .unwrap();
        assert_eq!(count, 4);
        assert_eq!(read_issue(&remote, project, b).await, original);
        assert!(runtime.calls.lock().await.is_empty());
        remote.pool.close().await;
    }

    #[tokio::test]
    async fn relationship_and_parent_scope_conflicts_failures_are_not_empty_successes() {
        let (remote, _, principal, target, a) = super::super::runtime_write_tests::fixture().await;
        let project = target.project_id;
        let status: Uuid = sqlx::query_scalar("SELECT status_id FROM local_issues WHERE id=?")
            .bind(a)
            .fetch_one(&remote.pool)
            .await
            .unwrap();
        let b = make(&remote, &principal, project, status, None).await;
        let foreign_project = Uuid::new_v4();
        let foreign = Uuid::new_v4();
        sqlx::query("INSERT INTO projects(id,name,key) VALUES(?,'foreign','FOREIGN')")
            .bind(foreign_project)
            .execute(&remote.pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO local_issues(id,project_id,issue_number,simple_id,status_id,title,sort_order) VALUES(?,?,1,'FOREIGN-1',?,'foreign',0)").bind(foreign).bind(foreign_project).bind(status).execute(&remote.pool).await.unwrap();
        assert_eq!(
            change_parent(
                &remote,
                &principal,
                project,
                a,
                json!({"parent_issue_id":foreign})
            )
            .await["status"],
            "failed"
        );
        let failed_create=write(&remote,&principal,"create_issue",json!({"project_id":project,"status_id":status,"parent_issue_id":foreign,"request_id":Uuid::new_v4(),"title":"invalid child","description":"agreed"})).await;
        assert_eq!(failed_create["status"], "failed");
        let revision = read_issue(&remote, project, a).await["revision"].clone();
        for target in [a, foreign, Uuid::new_v4()] {
            let result=write(&remote,&principal,"create_issue_relationship",json!({"project_id":project,"issue_id":a,"related_issue_id":target,"relationship_type":"related","expected_revision":revision,"request_id":Uuid::new_v4()})).await;
            assert_eq!(result["status"], "failed");
        }
        let input = json!({"project_id":project,"issue_id":a,"related_issue_id":b,"relationship_type":"related","expected_revision":revision,"request_id":Uuid::new_v4()});
        let mut readonly = principal.clone();
        readonly.scopes = ["lvk:read".into()].into_iter().collect();
        assert!(
            call(
                &remote,
                &readonly,
                "create_issue_relationship",
                input.clone()
            )
            .await
            .is_err()
        );
        let mut stale = input.clone();
        stale["expected_revision"] = json!("2000-01-01T00:00:00Z");
        stale["request_id"] = json!(Uuid::new_v4());
        assert_eq!(
            write(&remote, &principal, "create_issue_relationship", stale).await["result"]["code"],
            "revision_conflict"
        );
        let result = write(
            &remote,
            &principal,
            "create_issue_relationship",
            input.clone(),
        )
        .await;
        let id = result["result"]["relationship"]["id"].clone();
        let c = make(&remote, &principal, project, status, None).await;
        let c_revision = read_issue(&remote, project, c).await["revision"].clone();
        let remove = json!({"project_id":project,"issue_id":c,"relationship_id":id,"expected_revision":c_revision,"request_id":Uuid::new_v4()});
        assert!(
            call(
                &remote,
                &readonly,
                "delete_issue_relationship",
                remove.clone()
            )
            .await
            .is_err()
        );
        assert_eq!(
            write(&remote, &principal, "delete_issue_relationship", remove).await["status"],
            "failed"
        );
        // Deliberately corrupt an endpoint's scope to represent historical data.
        sqlx::query("UPDATE local_issues SET project_id=? WHERE id=?")
            .bind(foreign_project)
            .bind(b)
            .execute(&remote.pool)
            .await
            .unwrap();
        assert!(
            call(
                &remote,
                &principal,
                "list_issue_relationships",
                json!({"project_id":project,"issue_id":a})
            )
            .await
            .is_err()
        );
        // Receipt replay returns its committed result, even after domain changes.
        assert_eq!(
            write(&remote, &principal, "create_issue_relationship", input).await["result"],
            result["result"]
        );
        assert!(
            call(
                &remote,
                &principal,
                "list_issue_relationships",
                json!({"project_id":foreign_project,"issue_id":foreign})
            )
            .await
            .is_err()
        );
        sqlx::query("DROP TABLE local_issue_relationships")
            .execute(&remote.pool)
            .await
            .unwrap();
        assert!(
            call(
                &remote,
                &principal,
                "list_issue_relationships",
                json!({"project_id":project,"issue_id":a})
            )
            .await
            .is_err()
        );
        for name in ["create_issue_relationship", "delete_issue_relationship"] {
            assert_eq!(super::super::operation_scope(name), Some("lvk:write"));
        }
    }
}

#[cfg(test)]
mod http_tests {
    use super::{
        super::{router, runtime_write_tests, tests},
        *,
    };
    async fn invoke(client: &reqwest::Client, base: &str, name: &str, input: Value) -> Value {
        let response=client.post(format!("{base}/mcp")).header("Host","lvk.example")
            .header("Accept","application/json, text/event-stream").header("MCP-Protocol-Version","2025-03-26")
            .bearer_auth("valid").json(&json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":name,"arguments":input}})).send().await.unwrap();
        assert_eq!(response.status(), 200);
        let value: Value = response.json().await.unwrap();
        assert_ne!(value["result"]["isError"], true, "{value}");
        value["result"]["structuredContent"].clone()
    }
    #[tokio::test]
    async fn public_http_parent_and_relationship_operations_roundtrip() {
        let (_, auth, provider) = tests::fixture().await;
        let (remote, _, principal, target, a) = runtime_write_tests::fixture().await;
        let project = target.project_id;
        let status: Uuid = sqlx::query_scalar("SELECT status_id FROM local_issues WHERE id=?")
            .bind(a)
            .fetch_one(&remote.pool)
            .await
            .unwrap();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let running = remote.clone();
        let server =
            tokio::spawn(
                async move { axum::serve(listener, router(running, auth)).await.unwrap() },
            );
        let client = reqwest::Client::new();
        let request = Uuid::new_v4();
        invoke(&client,&base,"create_issue",json!({"project_id":project,"status_id":status,"title":"http child","description":"agreed","parent_issue_id":a,"request_id":request})).await;
        let receipt = tests::settled_operation(&remote, &principal, project, request).await;
        assert_eq!(receipt["status"], "succeeded");
        let b = receipt["result"]["issue_id"].clone();
        let detail = invoke(
            &client,
            &base,
            "get_issue",
            json!({"project_id":project,"issue_id":b}),
        )
        .await;
        assert_eq!(detail["parent_issue_id"], json!(a));
        let request = Uuid::new_v4();
        invoke(&client,&base,"update_issue",json!({"project_id":project,"issue_id":b,"clear_parent":true,"expected_revision":detail["revision"],"request_id":request})).await;
        assert_eq!(
            tests::settled_operation(&remote, &principal, project, request).await["status"],
            "succeeded"
        );
        let detail = invoke(
            &client,
            &base,
            "get_issue",
            json!({"project_id":project,"issue_id":b}),
        )
        .await;
        assert!(detail["parent_issue_id"].is_null());
        let a_detail = invoke(
            &client,
            &base,
            "get_issue",
            json!({"project_id":project,"issue_id":a}),
        )
        .await;
        let request = Uuid::new_v4();
        let input = json!({"project_id":project,"issue_id":a,"related_issue_id":b,"relationship_type":"blocking","expected_revision":a_detail["revision"],"request_id":request});
        invoke(&client, &base, "create_issue_relationship", input.clone()).await;
        let receipt = tests::settled_operation(&remote, &principal, project, request).await;
        assert_eq!(receipt["status"], "succeeded", "{receipt}");
        let relation = receipt["result"]["relationship"]["id"].clone();
        let replay = invoke(&client, &base, "create_issue_relationship", input).await;
        assert_eq!(replay["result"], receipt["result"]);
        let outgoing = invoke(
            &client,
            &base,
            "list_issue_relationships",
            json!({"project_id":project,"issue_id":a}),
        )
        .await;
        let incoming = invoke(
            &client,
            &base,
            "list_issue_relationships",
            json!({"project_id":project,"issue_id":b}),
        )
        .await;
        assert_eq!(outgoing["items"][0]["direction"], "source");
        assert_eq!(incoming["items"][0]["direction"], "target");
        assert_eq!(incoming["items"][0]["related_issue_id"], json!(a));
        assert_eq!(outgoing["items"][0]["id"], incoming["items"][0]["id"]);
        let request = Uuid::new_v4();
        invoke(&client,&base,"delete_issue_relationship",json!({"project_id":project,"issue_id":b,"relationship_id":relation,"expected_revision":detail["revision"],"request_id":request})).await;
        assert_eq!(
            tests::settled_operation(&remote, &principal, project, request).await["status"],
            "succeeded"
        );
        for issue in [json!(a), b] {
            let page = invoke(
                &client,
                &base,
                "list_issue_relationships",
                json!({"project_id":project,"issue_id":issue}),
            )
            .await;
            assert_eq!(page["items"], json!([]));
            assert_eq!(page["has_more"], false);
        }
        server.abort();
        server.await.unwrap_err();
        provider.abort();
        provider.await.unwrap_err();
    }
}
