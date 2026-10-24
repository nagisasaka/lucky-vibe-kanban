use anyhow::ensure;
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::Row;
use uuid::Uuid;

use super::{
    Remote,
    read::{arguments, definition, page},
};

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct WorkspaceArgs {
    project_id: Uuid,
    workspace_id: Uuid,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct RunListArgs {
    project_id: Uuid,
    workspace_id: Uuid,
    #[serde(default = "super::default_limit")]
    #[schemars(range(min = 1, max = 50))]
    limit: u32,
    cursor: Option<Uuid>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
enum ExecutionSection {
    FinalResponse,
    Input,
    Goal,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ExecutionArgs {
    project_id: Uuid,
    workspace_id: Uuid,
    agent_run_id: Uuid,
    section: ExecutionSection,
    #[serde(default)]
    offset: u32,
    #[serde(default = "super::read::text_limit")]
    #[schemars(range(min = 1, max = 16000))]
    limit: u32,
    expected_revision: Option<String>,
}

pub(super) fn tools() -> Vec<rmcp::model::Tool> {
    vec![
        definition::<WorkspaceArgs>(
            "get_workspace",
            "Read Workspace identity, session/run counts, cumulative token_usage (reported scope, not cost) and review link. Use list_workspace_runs for current execution IDs. An agent turn ending does not prove tests, merge or Wiki completion.",
        ),
        definition::<RunListArgs>(
            "list_workspace_runs",
            "Read canonical runs newest first, including session IDs, is_current_run within each Session, lifecycle, projection and Goal status. Page older runs with next_cursor; refresh first page to observe direct Codex changes.",
        ),
        definition::<ExecutionArgs>(
            "get_execution",
            "Read one run's final response, initial instruction or Goal objective in bounded character pages. Returns canonical lifecycle separately from the text. An agent claim is not independent test evidence.",
        ),
    ]
}

pub(super) async fn call(remote: &Remote, name: &str, args: Value) -> anyhow::Result<Value> {
    match name {
        "get_workspace" => remote.workspace(arguments(args)?).await,
        "list_workspace_runs" => remote.runs(arguments(args)?).await,
        "get_execution" => remote.execution(arguments(args)?).await,
        _ => anyhow::bail!("Unknown runtime read tool"),
    }
}

impl Remote {
    async fn workspace(&self, args: WorkspaceArgs) -> anyhow::Result<Value> {
        let issue = self
            .require_workspace(args.project_id, args.workspace_id)
            .await?;
        let row = sqlx::query("SELECT substr(name,1,500) AS name,length(coalesce(name,''))>500 AS name_truncated,archived,worktree_deleted,pinned,updated_at,(SELECT count(*) FROM sessions WHERE workspace_id=workspaces.id) AS session_count,(SELECT count(*) FROM agent_runs WHERE workspace_id=workspaces.id) AS run_count FROM workspaces WHERE id=?")
            .bind(args.workspace_id).fetch_one(&self.pool).await?;
        Ok(
            json!({"token_usage":db::models::workspace_tokens::WorkspaceTokenUsage::get(&self.pool,args.workspace_id).await?,"workspace_id":args.workspace_id,"project_id":args.project_id,"issue_id":issue,"name":row.try_get::<Option<String>,_>("name")?,"name_truncated":row.try_get::<bool,_>("name_truncated")?,"archived":row.try_get::<bool,_>("archived")?,"worktree_deleted":row.try_get::<bool,_>("worktree_deleted")?,"pinned":row.try_get::<bool,_>("pinned")?,"updated_at":row.try_get::<String,_>("updated_at")?,"session_count":row.try_get::<i64,_>("session_count")?,"run_count":row.try_get::<i64,_>("run_count")?,"url":self.config.workspace_url(args.project_id,issue,args.workspace_id),"verification":{"status":"unknown","reason":"Inspect recorded verification and Integration results; run completion alone is not proof"}}),
        )
    }

    async fn runs(&self, args: RunListArgs) -> anyhow::Result<Value> {
        let issue = self
            .require_workspace(args.project_id, args.workspace_id)
            .await?;
        ensure!(
            (1..=50).contains(&args.limit),
            "limit must be between 1 and 50"
        );
        let before: Option<(String, i64)> = match args.cursor {
            Some(cursor) => Some(
                sqlx::query_as(
                    "SELECT created_at,rowid FROM agent_runs WHERE id=? AND workspace_id=?",
                )
                .bind(cursor)
                .bind(args.workspace_id)
                .fetch_optional(&self.pool)
                .await?
                .ok_or_else(|| anyhow::anyhow!("Stale or foreign run cursor; restart listing"))?,
            ),
            None => None,
        };
        let rows = sqlx::query("SELECT r.id,r.session_id,r.status,r.projection_status,r.created_at,r.updated_at,s.last_event_sequence,NOT EXISTS (SELECT 1 FROM agent_runs newer WHERE newer.session_id=r.session_id AND (newer.created_at>r.created_at OR (newer.created_at=r.created_at AND newer.rowid>r.rowid))) AS is_current_run,json_extract(s.state_json,'$.goal.status') AS goal_status FROM agent_runs r LEFT JOIN agent_run_state s ON s.agent_run_id=r.id WHERE r.workspace_id=? AND (? IS NULL OR r.created_at<? OR (r.created_at=? AND r.rowid<?)) ORDER BY r.created_at DESC,r.rowid DESC LIMIT ?")
            .bind(args.workspace_id).bind(before.as_ref().map(|v| &v.0)).bind(before.as_ref().map(|v| &v.0)).bind(before.as_ref().map(|v| &v.0)).bind(before.as_ref().map(|v| v.1)).bind(args.limit+1).fetch_all(&self.pool).await?;
        let items = rows.into_iter().map(|row| -> anyhow::Result<Value> {
            Ok(json!({"id":row.try_get::<Uuid,_>("id")?,"session_id":row.try_get::<Uuid,_>("session_id")?,"workspace_id":args.workspace_id,"status":row.try_get::<String,_>("status")?,"projection_status":row.try_get::<String,_>("projection_status")?,"is_current_run":row.try_get::<bool,_>("is_current_run")?,"goal_status":row.try_get::<Option<String>,_>("goal_status")?,"last_event_sequence":row.try_get::<Option<i64>,_>("last_event_sequence")?,"updated_at":row.try_get::<String,_>("updated_at")?,"url":self.config.workspace_url(args.project_id,issue,args.workspace_id)}))
        }).collect::<anyhow::Result<Vec<_>>>()?;
        Ok(page(items, args.limit))
    }

    async fn execution(&self, args: ExecutionArgs) -> anyhow::Result<Value> {
        let issue = self
            .require_workspace(args.project_id, args.workspace_id)
            .await?;
        ensure!(
            (1..=16000).contains(&args.limit),
            "limit must be between 1 and 16000 characters"
        );
        ensure!(
            args.offset == 0 || args.expected_revision.is_some(),
            "Supply expected_revision for subsequent execution pages"
        );
        let expression = match args.section {
            ExecutionSection::FinalResponse => {
                "json_extract(s.state_json,'$.terminal_output.content')"
            }
            ExecutionSection::Input => "json_extract(r.request_envelope,'$.input.content')",
            ExecutionSection::Goal => "json_extract(s.state_json,'$.goal.objective')",
        };
        // Only the closed enum selects a SQL expression; caller text is bound.
        let sql = format!(
            "SELECT r.id,r.session_id,r.status,r.projection_status,r.updated_at,s.last_event_sequence,NOT EXISTS (SELECT 1 FROM agent_runs newer WHERE newer.session_id=r.session_id AND (newer.created_at>r.created_at OR (newer.created_at=r.created_at AND newer.rowid>r.rowid))) AS is_current_run,json_extract(s.state_json,'$.goal.status') AS goal_status,json_extract(s.state_json,'$.goal.token_budget') AS goal_token_budget,json_extract(s.state_json,'$.goal.tokens_used') AS goal_tokens_used,json_extract(s.state_json,'$.goal.time_used_seconds') AS goal_time_used_seconds,json_extract(s.state_json,'$.last_error.code') AS error_code,substr(coalesce({expression},''),?,?) AS content,length(coalesce({expression},'')) AS total FROM agent_runs r LEFT JOIN agent_run_state s ON s.agent_run_id=r.id WHERE r.id=? AND r.workspace_id=?"
        );
        let row = sqlx::query(&sql)
            .bind(i64::from(args.offset) + 1)
            .bind(args.limit)
            .bind(args.agent_run_id)
            .bind(args.workspace_id)
            .fetch_optional(&self.pool)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Run does not exist in this Workspace"))?;
        let sequence = row.try_get::<Option<i64>, _>("last_event_sequence")?;
        let revision = format!(
            "{}:{:?}:{}:{}",
            args.agent_run_id,
            args.section,
            row.try_get::<String, _>("updated_at")?,
            sequence
                .map(|s| s.to_string())
                .unwrap_or_else(|| "missing".into())
        );
        ensure!(
            args.expected_revision
                .as_ref()
                .is_none_or(|expected| expected == &revision),
            "Run changed; restart reading this section"
        );
        let total: i64 = row.try_get("total")?;
        ensure!(
            i64::from(args.offset) <= total,
            "Page offset is outside this section"
        );
        let next = i64::from(args.offset) + i64::from(args.limit);
        Ok(
            json!({"agent_run_id":args.agent_run_id,"session_id":row.try_get::<Uuid,_>("session_id")?,"workspace_id":args.workspace_id,"status":row.try_get::<String,_>("status")?,"projection_status":row.try_get::<String,_>("projection_status")?,"is_current_run":row.try_get::<bool,_>("is_current_run")?,"projection_available":sequence.is_some(),"goal_status":row.try_get::<Option<String>,_>("goal_status")?,"goal_usage":{"token_budget":row.try_get::<Option<i64>,_>("goal_token_budget")?,"tokens_used":row.try_get::<Option<i64>,_>("goal_tokens_used")?,"time_used_seconds":row.try_get::<Option<i64>,_>("goal_time_used_seconds")?},"error_code":row.try_get::<Option<String>,_>("error_code")?,"revision":revision,"content":row.try_get::<String,_>("content")?,"offset":args.offset,"total_characters":total,"has_more":next<total,"next_offset":if next<total {Some(next)} else {None},"url":self.config.workspace_url(args.project_id,issue,args.workspace_id),"verification":{"status":"unknown","reason":"Output is an agent report, not independent verification evidence"}}),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::remote_mcp::{
        runtime_write,
        runtime_write_tests::{fixture, insert_run},
    };

    #[tokio::test]
    async fn current_runs_follow_session_admission_order_across_equal_timestamp_pages() {
        let (remote, _, _, mut target, _) = fixture().await;
        let original = target.agent_run_id;
        let mut expected = vec![original];
        // Reverse UUID order deliberately disagrees with admission order.
        for n in (1..=105).rev() {
            let run = Uuid::from_u128(n);
            insert_run(&remote.pool, target.session_id, target.workspace_id, run).await;
            expected.push(run);
        }
        let other_session = Uuid::new_v4();
        db::models::session::Session::create(
            &remote.pool,
            &db::models::session::CreateSession {
                executor: Some("CODEX".into()),
                name: None,
            },
            other_session,
            target.workspace_id,
        )
        .await
        .unwrap();
        let other_run = Uuid::new_v4();
        insert_run(&remote.pool, other_session, target.workspace_id, other_run).await;
        expected.push(other_run);
        sqlx::query("UPDATE agent_runs SET created_at='2026-01-01T00:00:00Z',updated_at='2026-01-01T00:00:00Z'")
            .execute(&remote.pool).await.unwrap();
        expected.reverse();
        let mut args =
            json!({"project_id":target.project_id,"workspace_id":target.workspace_id,"limit":20});
        let mut actual = Vec::new();
        loop {
            let result = call(&remote, "list_workspace_runs", args.clone())
                .await
                .unwrap();
            let items = result["items"].as_array().unwrap();
            assert!(items.len() <= 20);
            for item in items {
                let run: Uuid = serde_json::from_value(item["id"].clone()).unwrap();
                assert_eq!(
                    item["is_current_run"],
                    run == other_run || run == Uuid::from_u128(1)
                );
                actual.push(run);
            }
            if result["has_more"] == false {
                break;
            }
            args["cursor"] = result["next_cursor"].clone();
        }
        assert_eq!(actual, expected);
        assert!(
            runtime_write::validate_run(&remote.pool, &target, false)
                .await
                .is_err()
        );
        target.agent_run_id = Uuid::from_u128(1);
        runtime_write::validate_run(&remote.pool, &target, false)
            .await
            .unwrap();
        let detail_args = json!({"project_id":target.project_id,"workspace_id":target.workspace_id,"agent_run_id":original,"section":"input"});
        let detail = call(&remote, "get_execution", detail_args.clone())
            .await
            .unwrap();
        assert_eq!(detail["is_current_run"], false);
        assert_eq!(detail["goal_usage"]["tokens_used"], Value::Null);
        let mut foreign_revision = detail_args.clone();
        foreign_revision["agent_run_id"] = json!(target.agent_run_id);
        foreign_revision["expected_revision"] = detail["revision"].clone();
        assert!(
            call(&remote, "get_execution", foreign_revision)
                .await
                .is_err()
        );
        let mut wrong_section = detail_args;
        wrong_section["section"] = json!("goal");
        wrong_section["expected_revision"] = detail["revision"].clone();
        assert!(call(&remote, "get_execution", wrong_section).await.is_err());
        sqlx::query("DELETE FROM agent_runs WHERE id=?")
            .bind(args["cursor"].as_str().unwrap().parse::<Uuid>().unwrap())
            .execute(&remote.pool)
            .await
            .unwrap();
        assert!(call(&remote, "list_workspace_runs", args).await.is_err());
    }
}

#[cfg(test)]
mod token_tests {
    use super::*;

    #[tokio::test]
    async fn workspace_tokens_share_api_aggregate_and_enforce_project_scope() {
        let (remote, _, _, target, _) = crate::remote_mcp::runtime_write_tests::fixture().await;
        let args = json!({"project_id":target.project_id,"workspace_id":target.workspace_id});
        let initial = call(&remote, "get_workspace", args.clone()).await.unwrap();
        assert_eq!(initial["token_usage"]["completeness"], "unknown");
        assert!(initial["token_usage"]["total_tokens"].is_null());
        sqlx::query("INSERT INTO workspace_token_usage VALUES(?,'codex','fixture',42,30,12,10,4,'2026-10-21T00:00:00Z','2026-10-21T00:00:00Z')")
            .bind(target.workspace_id).execute(&remote.pool).await.unwrap();
        let result = call(&remote, "get_workspace", args).await.unwrap();
        let shared = db::models::workspace_tokens::WorkspaceTokenUsage::get(
            &remote.pool,
            target.workspace_id,
        )
        .await
        .unwrap();
        assert_eq!(result["token_usage"], serde_json::to_value(shared).unwrap());
        assert_eq!(result["token_usage"]["total_tokens"], 42);
        assert!(
            call(
                &remote,
                "get_workspace",
                json!({"project_id":Uuid::new_v4(),"workspace_id":target.workspace_id})
            )
            .await
            .is_err()
        );
        assert!(
            call(
                &remote,
                "get_workspace",
                json!({"project_id":target.project_id,"workspace_id":Uuid::new_v4()})
            )
            .await
            .is_err()
        );
    }
}
