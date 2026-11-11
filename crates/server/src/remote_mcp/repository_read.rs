use git::repository_read::QueryRequest;
use serde_json::Value;

use super::{
    Remote,
    read::{arguments, definition},
};

pub(super) fn tools() -> Vec<rmcp::model::Tool> {
    vec![definition::<QueryRequest>(
        "query_repository",
        "Use source:\"lvk\" for the bundled public LVK source without Project/repository IDs (omit target for the distribution commit; explicit commit targets for history; no Workspace or host paths). Result source identifies build_revision and distribution commit; source_head is the selected commit. Otherwise explore an authorised Project repository without a Workspace: commit refs resolve to full SHA. Read accepts legacy path/start_line/end_line or 1–8 ranges (maximum 200 lines each, 64 KiB item bytes); pass each item content revision as its expected_revision, and inspect per-item failures. Typed list/glob, literal/regex grep (content/files/count), first-call line read, fixed-head log and explicit direct/merge_base/parent diff. Workspace mode reads only its live HEAD/index/worktree/untracked; never overlays dirty on historical refs. Repeat original arguments with next_cursor until complete; cursor binds query, SHA and observed revision and expires on server restart. Check exclusions, stop_reason and budgets. Read-only reference data; no model or shell.",
    )]
}
pub(super) async fn call(remote: &Remote, args: Value) -> anyhow::Result<Value> {
    let request: QueryRequest = arguments(args)?;
    let project = match &request {
        QueryRequest::Project(request) => Some(request.project_id),
        QueryRequest::Bundled(_) => None,
    };
    if let Some(project) = project {
        remote.require_project(project).await?;
    }
    let value = crate::repository_read::query(&remote.pool, request).await?;
    if let Some(project) = project {
        remote.require_project(project).await?;
    }
    Ok(value)
}

#[cfg(all(test, unix))]
mod tests {
    use git::repository_read::{Filter, Operation, Request, Target};
    use serde_json::json;
    use uuid::Uuid;

    use super::*;

    #[tokio::test]
    async fn repo_ref_shared_service_authorizes_and_reproduces_without_branch_workspace() {
        let (temp, remote, ids) = super::super::code_read::tests::fixture().await;
        let request:Request=serde_json::from_value(json!({"project_id":ids["project_id"],"repository_id":ids["repository_id"],"target":{"mode":"commit","ref":"main"},"operation":{"kind":"read","path":"src/000.txt","start_line":1,"end_line":1}})).unwrap();
        let input = serde_json::to_value(&request).unwrap();
        let result = call(&remote, input.clone()).await.unwrap();
        assert_eq!(result["items"][0]["lines"][0], "before");
        assert_eq!(result["source_head"].as_str().unwrap().len(), 40);
        let shared = crate::repository_read::read(&remote.pool, request.clone())
            .await
            .unwrap();
        for key in [
            "items",
            "revision",
            "source_head",
            "base_commit",
            "end_commit",
            "snapshot",
        ] {
            assert_eq!(shared[key], result[key]);
        }
        // No main Workspace: the fixture Workspace checks out `fixture` only.
        let mut bad = request.clone();
        bad.repository_id = Uuid::new_v4();
        assert!(
            call(&remote, serde_json::to_value(&bad).unwrap())
                .await
                .is_err()
        );
        bad = request.clone();
        bad.project_id = Uuid::new_v4();
        assert!(
            call(&remote, serde_json::to_value(&bad).unwrap())
                .await
                .is_err()
        );
        bad = request.clone();
        bad.target = Target::Workspace {
            workspace_id: Uuid::new_v4(),
        };
        assert!(
            call(&remote, serde_json::to_value(&bad).unwrap())
                .await
                .is_err()
        );
        let mut list = request.clone();
        list.operation = Operation::List {
            filter: Filter::default(),
        };
        list.limit = 1;
        let first = call(&remote, serde_json::to_value(&list).unwrap())
            .await
            .unwrap();
        list.cursor = Some(first["next_cursor"].as_str().unwrap().into());
        list.operation = Operation::List {
            filter: Filter {
                directory_prefix: Some("src".into()),
                ..Default::default()
            },
        };
        assert!(
            call(&remote, serde_json::to_value(&list).unwrap())
                .await
                .is_err()
        );
        let index = std::fs::read(temp.path().join(".git/index")).unwrap();
        call(&remote, input).await.unwrap();
        assert_eq!(
            std::fs::read(temp.path().join(".git/index")).unwrap(),
            index
        );
        // Unlinking the only Project association revokes subsequent commit reads.
        sqlx::query("DELETE FROM workspace_repos WHERE repo_id=?")
            .bind(request.repository_id)
            .execute(&remote.pool)
            .await
            .unwrap();
        assert!(
            call(&remote, serde_json::to_value(&request).unwrap())
                .await
                .is_err()
        );
    }
    #[test]
    fn remote_catalogue_has_read_only_typed_tool() {
        let t = tools().remove(0);
        assert_eq!(t.name, "query_repository");
        let value = serde_json::to_value(t).unwrap();
        assert_eq!(value["annotations"]["readOnlyHint"], true);
        let schema = &value["inputSchema"]["$defs"]["Request"];
        for field in [
            "project_id",
            "repository_id",
            "target",
            "operation",
            "cursor",
            "expected_revision",
        ] {
            assert!(schema["properties"].get(field).is_some());
        }
    }
}
