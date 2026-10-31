//! Bounded tag reads and receipt-backed, additive mutations. No set replacement.
use anyhow::ensure;
use rmcp::model::Tool;
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::Row;
use uuid::Uuid;

use super::{
    Remote,
    auth::Principal,
    read::{arguments, definition, page},
};

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ListArgs {
    project_id: Uuid,
    #[serde(default = "super::default_limit")]
    #[schemars(range(min = 1, max = 50))]
    limit: u32,
    cursor: Option<Uuid>,
}
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct IssueListArgs {
    project_id: Uuid,
    issue_id: Uuid,
    #[serde(default = "super::default_limit")]
    #[schemars(range(min = 1, max = 50))]
    limit: u32,
    cursor: Option<Uuid>,
}
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct MutationArgs {
    project_id: Uuid,
    issue_id: Uuid,
    tag_id: Uuid,
    request_id: Uuid,
}
pub(super) fn tools() -> Vec<Tool> {
    let mut tools = vec![
        definition::<ListArgs>(
            "list_tags",
            "List available Project tags in pages. Exclude system from AI classification; it is managed by LVK. Follow next_cursor.",
        ),
        definition::<IssueListArgs>(
            "list_issue_tags",
            "List current Issue tag relations in pages, including protected system labels. Follow next_cursor.",
        ),
    ];
    for (name, description) in [
        (
            "add_issue_tag",
            "Idempotently add one normal tag to an Issue; preserves all other tags. Use a stable request_id and get_operation after uncertainty.",
        ),
        (
            "remove_issue_tag",
            "Idempotently remove one normal tag from an Issue by tag_id; preserves all other tags. System tags cannot be changed.",
        ),
    ] {
        let tool = definition::<MutationArgs>(name, description);
        let mut value = serde_json::to_value(tool).expect("tag tool");
        value["annotations"] = json!({"readOnlyHint":false,"destructiveHint":true,"idempotentHint":true,"openWorldHint":false});
        value["_meta"] =
            json!({"securitySchemes":[{"type":"oauth2","scopes":["lvk:read","lvk:write"]}]});
        tools.push(serde_json::from_value(value).expect("tag tool"));
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
        super::errors::rejection_with_message(
            utils::mcp_error::Category::Permission,
            "Read permission required"
        )
    );
    if name == "list_tags" {
        let args: ListArgs = arguments(input)?;
        remote.require_project(args.project_id).await?;
        ensure!(
            (1..=50).contains(&args.limit),
            super::errors::rejection_with_message(
                utils::mcp_error::Category::Validation,
                "Invalid tag page limit"
            )
        );
        let rows = sqlx::query("SELECT id,substr(name,1,500) AS name,length(name)>500 AS name_truncated,substr(color,1,100) AS color FROM local_tags WHERE project_id=? AND (? IS NULL OR id>?) ORDER BY id LIMIT ?")
            .bind(args.project_id).bind(args.cursor).bind(args.cursor).bind(args.limit+1).fetch_all(&remote.pool).await?;
        let items = rows.iter().map(|r| Ok(json!({"id":r.try_get::<Uuid,_>("id")?,"name":r.try_get::<String,_>("name")?,"name_truncated":r.try_get::<bool,_>("name_truncated")?,"color":r.try_get::<String,_>("color")?,"system":db::models::board_tag::is_system(&r.try_get::<String,_>("name")?)}))).collect::<anyhow::Result<Vec<_>>>()?;
        return Ok(page(items, args.limit));
    }
    if name == "list_issue_tags" {
        let args: IssueListArgs = arguments(input)?;
        remote.require_issue(args.project_id, args.issue_id).await?;
        ensure!(
            (1..=50).contains(&args.limit),
            super::errors::rejection_with_message(
                utils::mcp_error::Category::Validation,
                "Invalid tag page limit"
            )
        );
        let rows = sqlx::query("SELECT it.id,it.tag_id,substr(t.name,1,500) AS name FROM local_issue_tags it JOIN local_tags t ON t.id=it.tag_id AND t.project_id=? WHERE it.issue_id=? AND (? IS NULL OR it.id>?) ORDER BY it.id LIMIT ?")
            .bind(args.project_id).bind(args.issue_id).bind(args.cursor).bind(args.cursor).bind(args.limit+1).fetch_all(&remote.pool).await?;
        let items = rows.iter().map(|r| Ok(json!({"id":r.try_get::<Uuid,_>("id")?,"tag_id":r.try_get::<Uuid,_>("tag_id")?,"name":r.try_get::<String,_>("name")?}))).collect::<anyhow::Result<Vec<_>>>()?;
        return Ok(page(items, args.limit));
    }
    ensure!(
        principal.allows("lvk:write"),
        super::errors::rejection_with_message(
            utils::mcp_error::Category::Permission,
            "Write permission required (lvk:write)"
        )
    );
    let args: MutationArgs = arguments(input.clone())?;
    remote.require_issue(args.project_id, args.issue_id).await?;
    let owned = remote.clone();
    let add = name == "add_issue_tag";
    let receipt = remote.operations().submit(remote.principal_key(principal), args.request_id, args.project_id, name.into(), input,
        json!({"project_id":args.project_id,"issue_id":args.issue_id,"tag_id":args.tag_id}), move |_| async move {
            match mutate(&owned, &args, add).await {
                Ok(relation) => ("succeeded", json!({"issue_id":args.issue_id,"tag_id":args.tag_id,"issue_tag_id":relation,"present":add,"implementation_started":false})),
                Err(error) => {
                    use utils::mcp_error::{Category, Diagnostic, Outcome};
                    let (unknown, category) = match error {
                        MutationError::Commit(ref error) if db::models::todo::commit_definitely_rejected(error) => (false, super::errors::database_category(error)),
                        MutationError::Commit(_) => (true, Category::OutcomeUnknown),
                        MutationError::Domain(db::models::board_tag::TagError::Validation(_)) => (false, Category::Validation),
                        MutationError::Domain(db::models::board_tag::TagError::Database(ref error)) => (false, super::errors::database_category(error)),
                    };
                    let diagnostic = Diagnostic::new(category, if unknown { Outcome::Unknown } else { Outcome::Failed });
                    (if unknown { "uncertain" } else { "failed" }, json!({"code":"tag_update_rejected","message":diagnostic.message,"error":diagnostic.value()}))
                }
            }
        }).await?;
    Ok(remote.receipt_value(receipt))
}
#[derive(Debug, thiserror::Error)]
enum MutationError {
    #[error(transparent)]
    Domain(#[from] db::models::board_tag::TagError),
    #[error("Commit outcome unavailable")]
    Commit(sqlx::Error),
}
impl From<sqlx::Error> for MutationError {
    fn from(error: sqlx::Error) -> Self {
        Self::Domain(error.into())
    }
}
async fn mutate(
    remote: &Remote,
    args: &MutationArgs,
    add: bool,
) -> Result<Option<Uuid>, MutationError> {
    let mut tx = remote.pool.begin_with("BEGIN IMMEDIATE").await?;
    let name: Option<String> = sqlx::query_scalar("SELECT t.name FROM local_tags t JOIN local_issues i ON i.project_id=t.project_id WHERE t.id=? AND t.project_id=? AND i.id=?")
        .bind(args.tag_id).bind(args.project_id).bind(args.issue_id).fetch_optional(&mut *tx).await?;
    let name = name.ok_or(db::models::board_tag::TagError::Validation(
        "Tag not in Issue Project",
    ))?;
    if db::models::board_tag::is_system(&name) {
        return Err(
            db::models::board_tag::TagError::Validation("System tags are managed by LVK").into(),
        );
    }
    let relation = if add {
        Some(db::models::board_tag::add(&mut tx, args.issue_id, args.tag_id, None).await?)
    } else {
        let id: Option<Uuid> =
            sqlx::query_scalar("SELECT id FROM local_issue_tags WHERE issue_id=? AND tag_id=?")
                .bind(args.issue_id)
                .bind(args.tag_id)
                .fetch_optional(&mut *tx)
                .await?;
        if let Some(id) = id {
            db::models::board_tag::remove(&mut tx, id).await?;
        }
        id
    };
    tx.commit().await.map_err(MutationError::Commit)?;
    Ok(relation)
}

#[cfg(test)]
mod tests {
    use super::{
        super::{issue_write, read, tests::settled_operation},
        *,
    };

    async fn write(remote: &Remote, principal: &Principal, name: &str, input: Value) -> Value {
        let request: Uuid = serde_json::from_value(input["request_id"].clone()).unwrap();
        let project: Uuid = serde_json::from_value(input["project_id"].clone()).unwrap();
        if name.ends_with("issue_tag") {
            call(remote, principal, name, input).await.unwrap();
        } else {
            issue_write::call(remote, principal, name, input)
                .await
                .unwrap();
        }
        settled_operation(remote, principal, project, request).await
    }

    #[tokio::test]
    async fn classification_mcp_priority_tags_receipts_scope_and_atomic_create() {
        let (remote, _, principal, target, issue) =
            super::super::runtime_write_tests::fixture().await;
        let project = target.project_id;
        sqlx::query("UPDATE projects SET key='MCP' WHERE id=?")
            .bind(project)
            .execute(&remote.pool)
            .await
            .unwrap();
        let bug = Uuid::new_v4();
        let survey = Uuid::new_v4();
        let system = Uuid::new_v4();
        for (id, name) in [(bug, "bug"), (survey, "survey"), (system, "system")] {
            sqlx::query("INSERT INTO local_tags(id,project_id,name,color) VALUES(?,?,?,'blue')")
                .bind(id)
                .bind(project)
                .bind(name)
                .execute(&remote.pool)
                .await
                .unwrap();
        }
        let first_page = call(
            &remote,
            &principal,
            "list_tags",
            json!({"project_id":project,"limit":1}),
        )
        .await
        .unwrap();
        assert_eq!(first_page["items"].as_array().unwrap().len(), 1);
        assert_eq!(first_page["has_more"], true);
        let first = read::call(
            &remote,
            "get_issue",
            json!({"project_id":project,"issue_id":issue}),
        )
        .await
        .unwrap();
        assert_eq!(first["priority"], "medium");
        let high = json!({"project_id":project,"issue_id":issue,"expected_revision":first["revision"],"request_id":Uuid::new_v4(),"priority":"high"});
        assert_eq!(
            write(&remote, &principal, "update_issue", high.clone()).await["status"],
            "succeeded"
        );
        assert_eq!(
            write(&remote, &principal, "update_issue", high.clone()).await["status"],
            "succeeded"
        );
        let mut stale = high;
        stale["request_id"] = json!(Uuid::new_v4());
        stale["priority"] = json!("low");
        assert_eq!(
            write(&remote, &principal, "update_issue", stale).await["result"]["code"],
            "revision_conflict"
        );
        let current = read::call(
            &remote,
            "get_issue",
            json!({"project_id":project,"issue_id":issue}),
        )
        .await
        .unwrap();
        assert_eq!(current["priority"], "high");
        let null = json!({"project_id":project,"issue_id":issue,"expected_revision":current["revision"],"request_id":Uuid::new_v4(),"priority":null});
        assert_eq!(
            write(&remote, &principal, "update_issue", null).await["status"],
            "succeeded"
        );
        let summaries = read::call(&remote, "list_issues", json!({"project_id":project}))
            .await
            .unwrap();
        assert_eq!(summaries["items"][0]["priority"], "medium");
        let mut add =
            json!({"project_id":project,"issue_id":issue,"tag_id":bug,"request_id":Uuid::new_v4()});
        let original = write(&remote, &principal, "add_issue_tag", add.clone()).await;
        assert_eq!(original["status"], "succeeded");
        assert_eq!(
            write(&remote, &principal, "add_issue_tag", add.clone()).await["result"],
            original["result"]
        );
        add["request_id"] = json!(Uuid::new_v4());
        assert_eq!(
            write(&remote, &principal, "add_issue_tag", add.clone()).await["result"]["issue_tag_id"],
            original["result"]["issue_tag_id"]
        );
        add["request_id"] = json!(Uuid::new_v4());
        add["tag_id"] = json!(survey);
        let mut duplicate = add.clone();
        duplicate["tag_id"] = json!(bug);
        duplicate["request_id"] = json!(Uuid::new_v4());
        let (left, right) = tokio::join!(
            write(&remote, &principal, "add_issue_tag", duplicate),
            write(&remote, &principal, "add_issue_tag", add.clone())
        );
        assert_eq!(left["status"], "succeeded");
        assert_eq!(right["status"], "succeeded");
        let mut conn = remote.pool.acquire().await.unwrap();
        db::models::board_tag::attach_system(&mut conn, project, issue)
            .await
            .unwrap();
        drop(conn);
        for name in ["add_issue_tag", "remove_issue_tag"] {
            add["request_id"] = json!(Uuid::new_v4());
            add["tag_id"] = json!(system);
            assert_eq!(
                write(&remote, &principal, name, add.clone()).await["status"],
                "failed"
            );
        }
        for _ in 0..2 {
            add["request_id"] = json!(Uuid::new_v4());
            add["tag_id"] = json!(bug);
            assert_eq!(
                write(&remote, &principal, "remove_issue_tag", add.clone()).await["status"],
                "succeeded"
            );
        }
        let current = call(
            &remote,
            &principal,
            "list_issue_tags",
            json!({"project_id":project,"issue_id":issue}),
        )
        .await
        .unwrap();
        let tag_ids: Vec<_> = current["items"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v["tag_id"].clone())
            .collect();
        assert!(tag_ids.contains(&json!(system)) && tag_ids.contains(&json!(survey)));
        assert!(!tag_ids.contains(&json!(bug)));
        let status: Uuid = sqlx::query_scalar("SELECT status_id FROM local_issues WHERE id=?")
            .bind(issue)
            .fetch_one(&remote.pool)
            .await
            .unwrap();
        let create = json!({"project_id":project,"status_id":status,"request_id":Uuid::new_v4(),"title":"new issue","description":"agreed","tag_ids":[bug,survey]});
        let made = write(&remote, &principal, "create_issue", create.clone()).await;
        assert_eq!(made["status"], "succeeded");
        let created: Uuid = serde_json::from_value(made["result"]["issue_id"].clone()).unwrap();
        let count: i64 =
            sqlx::query_scalar("SELECT count(*) FROM local_issue_tags WHERE issue_id=?")
                .bind(created)
                .fetch_one(&remote.pool)
                .await
                .unwrap();
        assert_eq!(count, 2);
        assert_eq!(
            write(&remote, &principal, "create_issue", create.clone()).await["result"]["issue_id"],
            made["result"]["issue_id"]
        );
        let detail = read::call(
            &remote,
            "get_issue",
            json!({"project_id":project,"issue_id":created}),
        )
        .await
        .unwrap();
        assert_eq!(detail["simple_id"], "MCP-2");
        assert_eq!(detail["priority"], "medium");
        let mut invalid = create.clone();
        invalid["request_id"] = json!(Uuid::new_v4());
        invalid["tag_ids"] = json!([bug, system]);
        assert_ne!(
            write(&remote, &principal, "create_issue", invalid).await["status"],
            "succeeded"
        );
        let count: i64 =
            sqlx::query_scalar("SELECT count(*) FROM local_issues WHERE title='new issue'")
                .fetch_one(&remote.pool)
                .await
                .unwrap();
        assert_eq!(count, 1, "bad tags must roll back Issue creation");
        let next = write(&remote, &principal, "create_issue", json!({"project_id":project,"status_id":status,"request_id":Uuid::new_v4(),"title":"after rollback","description":"agreed","tag_ids":[survey]})).await;
        assert_eq!(next["status"], "succeeded");
        let next_issue = next["result"]["issue_id"].clone();
        let detail = read::call(
            &remote,
            "get_issue",
            json!({"project_id":project,"issue_id":next_issue}),
        )
        .await
        .unwrap();
        assert_eq!(detail["simple_id"], "MCP-3");

        let mut read_only = principal.clone();
        read_only.scopes = ["lvk:read".to_string()].into_iter().collect();
        assert!(
            call(&remote, &read_only, "add_issue_tag", add.clone())
                .await
                .is_err()
        );
        assert_eq!(
            super::super::operation_scope("add_issue_tag"),
            Some("lvk:write")
        );
        assert_eq!(
            super::super::operation_scope("remove_issue_tag"),
            Some("lvk:write")
        );
        let mut foreign = add;
        foreign["project_id"] = json!(Uuid::new_v4());
        assert!(
            call(&remote, &principal, "add_issue_tag", foreign)
                .await
                .is_err()
        );
    }
}
