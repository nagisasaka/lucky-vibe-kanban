//! Project navigation over existing Board membership and canonical question state.
use anyhow::ensure;
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::Row;
use uuid::Uuid;

use super::{
    Remote,
    questions::Kind,
    read::{arguments, definition, page},
};

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct ListArgs {
    pub project_id: Uuid,
    #[serde(default = "super::default_limit")]
    #[schemars(range(min = 1, max = 50))]
    pub limit: u32,
    pub cursor: Option<Uuid>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct SessionArgs {
    project_id: Uuid,
    #[serde(default = "super::default_limit")]
    #[schemars(range(min = 1, max = 50))]
    limit: u32,
    cursor: Option<Uuid>,
    /// Include only Sessions with a non-terminal run when true.
    #[serde(default)]
    active_only: bool,
}

// Use actual Issue membership as well as the link's Project. The latest-run
// ordering matches the canonical write validator; UUID order is not admission order.
const SESSIONS: &str = "SELECT s.id,s.workspace_id,l.issue_id,r.id AS agent_run_id,r.status,r.projection_status,w.archived,EXISTS(SELECT 1 FROM agent_runs a WHERE a.session_id=s.id AND a.status NOT IN ('succeeded','failed','cancelled','crashed','audit_failed')) AS active FROM sessions s JOIN workspaces w ON w.id=s.workspace_id JOIN local_workspace_links l ON l.workspace_id=w.id JOIN local_issues i ON i.id=l.issue_id AND i.project_id=l.project_id LEFT JOIN agent_runs r ON r.id=(SELECT id FROM agent_runs latest WHERE latest.session_id=s.id ORDER BY latest.created_at DESC,latest.rowid DESC LIMIT 1) WHERE l.project_id=?";

pub(super) fn tools() -> Vec<rmcp::model::Tool> {
    vec![
        definition::<ListArgs>(
            "list_project_statuses",
            "Read all Issue status IDs, names and counts in bounded pages. Use these actual IDs when creating or updating Issues.",
        ),
        definition::<SessionArgs>(
            "list_project_sessions",
            "Find Session, Workspace, Issue and current run IDs within one Project. Returns unresolved question and approval counts without bodies. Use active_only for ongoing work; use list_questions on a selected Session.",
        ),
    ]
}

pub(super) async fn call(remote: &Remote, name: &str, args: Value) -> anyhow::Result<Value> {
    match name {
        "list_project_statuses" => remote.project_statuses(arguments(args)?).await,
        "list_project_sessions" => remote.project_sessions(arguments(args)?).await,
        _ => anyhow::bail!("Unknown Project read tool"),
    }
}

impl Remote {
    pub(super) async fn project_statuses(&self, args: ListArgs) -> anyhow::Result<Value> {
        self.require_project(args.project_id).await?;
        ensure!(
            (1..=50).contains(&args.limit),
            "limit must be between 1 and 50"
        );
        if let Some(cursor) = args.cursor {
            let exists: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM local_project_statuses WHERE id=? AND project_id=?)",
            )
            .bind(cursor)
            .bind(args.project_id)
            .fetch_one(&self.pool)
            .await?;
            ensure!(exists, "Stale or foreign status cursor; restart listing");
        }
        let rows = sqlx::query("SELECT s.id,substr(s.name,1,100) AS name,length(s.name)>100 AS name_truncated,s.hidden,count(i.id) AS issue_count FROM local_project_statuses s LEFT JOIN local_issues i ON i.status_id=s.id AND i.project_id=s.project_id WHERE s.project_id=? AND (? IS NULL OR s.id>?) GROUP BY s.id ORDER BY s.id LIMIT ?")
            .bind(args.project_id).bind(args.cursor).bind(args.cursor).bind(args.limit+1).fetch_all(&self.pool).await?;
        let items = rows.into_iter().map(|row| -> anyhow::Result<Value> {
            Ok(json!({"id":row.try_get::<Uuid,_>("id")?,"name":row.try_get::<String,_>("name")?,"name_truncated":row.try_get::<bool,_>("name_truncated")?,"hidden":row.try_get::<bool,_>("hidden")?,"issue_count":row.try_get::<i64,_>("issue_count")?,"url":self.config.project_url(args.project_id)}))
        }).collect::<anyhow::Result<Vec<_>>>()?;
        Ok(page(items, args.limit))
    }

    async fn session_summary(
        &self,
        project: Uuid,
        row: &sqlx::sqlite::SqliteRow,
    ) -> anyhow::Result<Value> {
        let session: Uuid = row.try_get("id")?;
        let workspace: Uuid = row.try_get("workspace_id")?;
        let issue: Uuid = row.try_get("issue_id")?;
        let (_, questions) = self.pending_questions(project, workspace, session).await?;
        let approvals = questions
            .iter()
            .filter(|q| q.kind == Kind::Approval)
            .count();
        Ok(
            json!({"id":session,"session_id":session,"workspace_id":workspace,"issue_id":issue,"agent_run_id":row.try_get::<Option<Uuid>,_>("agent_run_id")?,"status":row.try_get::<Option<String>,_>("status")?,"projection_status":row.try_get::<Option<String>,_>("projection_status")?,"workspace_archived":row.try_get::<bool,_>("archived")?,"has_nonterminal_run":row.try_get::<bool,_>("active")?,"pending_specification_questions":questions.len()-approvals,"pending_approvals":approvals,"url":self.config.workspace_url(project,issue,workspace)}),
        )
    }

    async fn project_sessions(&self, args: SessionArgs) -> anyhow::Result<Value> {
        self.require_project(args.project_id).await?;
        ensure!(
            (1..=50).contains(&args.limit),
            "limit must be between 1 and 50"
        );
        if let Some(cursor) = args.cursor {
            let sql =
                format!("SELECT EXISTS(SELECT 1 FROM ({SESSIONS}) WHERE id=? AND (?=0 OR active))");
            let exists: bool = sqlx::query_scalar(&sql)
                .bind(args.project_id)
                .bind(cursor)
                .bind(args.active_only)
                .fetch_one(&self.pool)
                .await?;
            ensure!(exists, "Stale or foreign Session cursor; restart listing");
        }
        let sql = format!(
            "SELECT * FROM ({SESSIONS}) WHERE (?=0 OR active) AND (? IS NULL OR id>?) ORDER BY id LIMIT ?"
        );
        let rows = sqlx::query(&sql)
            .bind(args.project_id)
            .bind(args.active_only)
            .bind(args.cursor)
            .bind(args.cursor)
            .bind(args.limit + 1)
            .fetch_all(&self.pool)
            .await?;
        let mut items = Vec::new();
        for row in rows {
            items.push(self.session_summary(args.project_id, &row).await?);
        }
        Ok(page(items, args.limit))
    }

    pub(super) async fn project_activity(&self, project: Uuid) -> anyhow::Result<Value> {
        // Aggregate scoped Session identities and canonical question state. Only
        // counts and the first 20 active summaries leave this boundary.
        let sql = format!("{SESSIONS} ORDER BY s.id");
        let rows = sqlx::query(&sql)
            .bind(project)
            .fetch_all(&self.pool)
            .await?;
        let mut current_run_statuses = std::collections::BTreeMap::<String, usize>::new();
        let mut active_sessions = 0;
        let mut questions = 0;
        let mut approvals = 0;
        let mut items = Vec::new();
        for row in &rows {
            let status: Option<String> = row.try_get("status")?;
            *current_run_statuses
                .entry(status.unwrap_or_else(|| "not_started".into()))
                .or_default() += 1;
            if !row.try_get::<bool, _>("active")? {
                continue;
            }
            active_sessions += 1;
            let item = self.session_summary(project, row).await?;
            questions += item["pending_specification_questions"]
                .as_u64()
                .unwrap_or(0);
            approvals += item["pending_approvals"].as_u64().unwrap_or(0);
            if items.len() < 20 {
                items.push(item);
            }
        }
        let more = active_sessions > items.len();
        let cursor = if more {
            items.last().map(|item| item["id"].clone())
        } else {
            None
        };
        let workspace_count: i64 = sqlx::query_scalar("SELECT count(*) FROM local_workspace_links l JOIN workspaces w ON w.id=l.workspace_id JOIN local_issues i ON i.id=l.issue_id AND i.project_id=l.project_id WHERE l.project_id=?")
            .bind(project).fetch_one(&self.pool).await?;
        Ok(
            json!({"workspace_count":workspace_count,"session_count":rows.len(),"current_run_statuses":current_run_statuses,"active_session_count":active_sessions,"pending_specification_questions":questions,"pending_approvals":approvals,"active_sessions":{"items":items,"has_more":more,"next_cursor":cursor,"next_tool":"list_project_sessions","active_only":true},"consistency":"live; refresh after concurrent changes"}),
        )
    }
}

#[cfg(test)]
mod tests {
    use db::models::session::{CreateSession, Session};

    use super::*;
    use crate::remote_mcp::{
        read,
        runtime_write::RuntimeActions,
        runtime_write_tests::{emit_questions, fixture, insert_run},
    };

    #[tokio::test]
    async fn project_navigation_is_bounded_scoped_and_observes_direct_answers() {
        let (remote, dispatch, _, target, _) = fixture().await;
        let question = emit_questions(&remote.pool, &target).await;
        for n in 1..=104 {
            let session = Uuid::from_u128(1000 + n);
            Session::create(
                &remote.pool,
                &CreateSession {
                    executor: Some("CODEX".into()),
                    name: None,
                },
                session,
                target.workspace_id,
            )
            .await
            .unwrap();
            let run = Uuid::new_v4();
            insert_run(&remote.pool, session, target.workspace_id, run).await;
            if n % 2 == 1 {
                sqlx::query("UPDATE agent_runs SET status='succeeded' WHERE id=?")
                    .bind(run)
                    .execute(&remote.pool)
                    .await
                    .unwrap();
            }
            sqlx::query("INSERT INTO local_project_statuses(id,project_id,name,color,sort_order) VALUES(?,?,'Status','blue',?)")
                .bind(Uuid::from_u128(2000+n)).bind(target.project_id).bind(n as i64).execute(&remote.pool).await.unwrap();
        }
        let project_args = json!({"project_id":target.project_id});
        let summary = read::call(&remote, "get_project", project_args.clone())
            .await
            .unwrap();
        assert_eq!(summary["activity"]["session_count"], 105);
        assert_eq!(summary["activity"]["workspace_count"], 1);
        assert_eq!(summary["activity"]["active_session_count"], 53);
        assert_eq!(
            summary["activity"]["current_run_statuses"],
            json!({"running":53,"succeeded":52})
        );
        assert_eq!(summary["activity"]["pending_specification_questions"], 2);
        assert_eq!(summary["activity"]["pending_approvals"], 1);
        assert_eq!(
            summary["activity"]["active_sessions"]["items"]
                .as_array()
                .unwrap()
                .len(),
            20
        );
        assert_eq!(summary["activity"]["active_sessions"]["has_more"], true);
        assert_eq!(summary["statuses_incomplete"], true);
        assert_eq!(summary["statuses"].as_array().unwrap().len(), 50);
        assert!(!summary.to_string().contains("Choose a colour"));
        for tool in ["list_project_statuses", "list_project_sessions"] {
            let mut args = project_args.clone();
            let mut ids = std::collections::HashSet::new();
            loop {
                let result = call(&remote, tool, args.clone()).await.unwrap();
                let items = result["items"].as_array().unwrap();
                assert!(items.len() <= 20);
                for item in items {
                    assert!(ids.insert(item["id"].as_str().unwrap().to_owned()));
                }
                if result["has_more"] == false {
                    break;
                }
                args["cursor"] = result["next_cursor"].clone();
            }
            assert_eq!(ids.len(), 105);
            args["cursor"] = json!(Uuid::new_v4());
            assert!(call(&remote, tool, args.clone()).await.is_err());
            args.as_object_mut().unwrap().remove("cursor");
            args["project_id"] = json!(Uuid::new_v4());
            assert!(call(&remote, tool, args).await.is_err());
            let mut too_large = project_args.clone();
            too_large["limit"] = json!(51);
            assert!(call(&remote, tool, too_large).await.is_err());
        }
        let next = call(&remote,"list_project_statuses",json!({"project_id":target.project_id,"cursor":summary["statuses_next_cursor"],"limit":50})).await.unwrap();
        assert_eq!(next["items"].as_array().unwrap().len(), 50);
        assert_eq!(next["has_more"], true);
        let next = call(&remote,"list_project_sessions",json!({"project_id":target.project_id,"cursor":summary["activity"]["active_sessions"]["next_cursor"],"active_only":true,"limit":50})).await.unwrap();
        assert_eq!(next["items"].as_array().unwrap().len(), 33);
        assert_eq!(next["has_more"], false);
        for table in ["remote_mcp_operations", "agent_run_commands"] {
            assert_eq!(
                sqlx::query_scalar::<_, i64>(&format!("SELECT count(*) FROM {table}"))
                    .fetch_one(&remote.pool)
                    .await
                    .unwrap(),
                0
            );
        }
        // Direct UI and MCP both use this canonical answer service; only its
        // final provider delivery is replaced by the existing runtime fixture.
        dispatch
            .answer(
                target.clone(),
                Uuid::new_v4(),
                question,
                "Blue".into(),
                Kind::AsyncSpecification,
                "local-user".into(),
                None,
            )
            .await
            .unwrap();
        let refreshed = read::call(&remote, "get_project", project_args.clone())
            .await
            .unwrap();
        assert_eq!(refreshed["activity"]["pending_specification_questions"], 1);
        assert_eq!(refreshed["activity"]["pending_approvals"], 1);
        sqlx::query("UPDATE agent_runs SET status='succeeded' WHERE id=?")
            .bind(target.agent_run_id)
            .execute(&remote.pool)
            .await
            .unwrap();
        let finished = read::call(&remote, "get_project", project_args)
            .await
            .unwrap();
        assert_eq!(finished["activity"]["pending_specification_questions"], 0);
        assert_eq!(finished["activity"]["pending_approvals"], 0);
        assert_eq!(finished["activity"]["active_session_count"], 52);
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT count(*) FROM agent_run_commands")
                .fetch_one(&remote.pool)
                .await
                .unwrap(),
            1
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT count(*) FROM agent_runs")
                .fetch_one(&remote.pool)
                .await
                .unwrap(),
            105
        );
    }
    #[tokio::test]
    async fn foreign_membership_and_expired_active_cursors_are_rejected() {
        let (remote, _, _, target, issue) = fixture().await;
        let project2 = Uuid::new_v4();
        let status2 = Uuid::new_v4();
        sqlx::query("INSERT INTO projects(id,name) VALUES(?,'Foreign')")
            .bind(project2)
            .execute(&remote.pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO local_project_statuses(id,project_id,name,color,sort_order) VALUES(?,?,'Foreign','red',0)").bind(status2).bind(project2).execute(&remote.pool).await.unwrap();
        assert!(
            call(
                &remote,
                "list_project_statuses",
                json!({"project_id":target.project_id,"cursor":status2})
            )
            .await
            .is_err()
        );
        let args =
            json!({"project_id":target.project_id,"cursor":target.session_id,"active_only":true});
        sqlx::query("UPDATE agent_runs SET status='cancelled' WHERE id=?")
            .bind(target.agent_run_id)
            .execute(&remote.pool)
            .await
            .unwrap();
        assert!(call(&remote, "list_project_sessions", args).await.is_err());
        // A stale local link alone cannot make a moved Issue part of this Project.
        sqlx::query("UPDATE local_issues SET project_id=?,status_id=? WHERE id=?")
            .bind(project2)
            .bind(status2)
            .bind(issue)
            .execute(&remote.pool)
            .await
            .unwrap();
        let summary = read::call(
            &remote,
            "get_project",
            json!({"project_id":target.project_id}),
        )
        .await
        .unwrap();
        assert_eq!(summary["activity"]["session_count"], 0);
        assert_eq!(summary["activity"]["workspace_count"], 0);
        assert_eq!(summary["issue_count"], 0);
        assert!(
            call(
                &remote,
                "list_project_sessions",
                json!({"project_id":target.project_id,"cursor":target.session_id})
            )
            .await
            .is_err()
        );
        let sessions = call(
            &remote,
            "list_project_sessions",
            json!({"project_id":target.project_id}),
        )
        .await
        .unwrap();
        assert_eq!(sessions["items"], json!([]));
    }
}
