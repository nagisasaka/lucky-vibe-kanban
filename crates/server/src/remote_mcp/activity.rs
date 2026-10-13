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
struct ListArgs {
    project_id: Uuid,
    workspace_id: Uuid,
    agent_run_id: Uuid,
    #[serde(default = "super::default_limit")]
    #[schemars(range(min = 1, max = 50))]
    limit: u32,
    cursor: Option<Uuid>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct CommandArgs {
    project_id: Uuid,
    workspace_id: Uuid,
    agent_run_id: Uuid,
    command_id: Uuid,
    #[serde(default)]
    offset: u32,
    #[serde(default = "super::read::text_limit")]
    #[schemars(range(min = 1, max = 16000))]
    limit: u32,
}

pub(super) fn tools() -> Vec<rmcp::model::Tool> {
    vec![
        definition::<ListArgs>(
            "list_run_commands",
            "Read accepted instructions/control actions from any LVK entry point, newest first. Delivery is not implementation completion. Refresh the first page to observe direct Codex steering; get_run_command reads the selected instruction.",
        ),
        definition::<CommandArgs>(
            "get_run_command",
            "Read one immutable steering, question-answer or Goal instruction in character pages. Runtime envelopes, native handles and execution-approval contents are not exported.",
        ),
    ]
}

pub(super) async fn call(remote: &Remote, name: &str, args: Value) -> anyhow::Result<Value> {
    match name {
        "list_run_commands" => remote.commands(arguments(args)?).await,
        "get_run_command" => remote.command(arguments(args)?).await,
        _ => anyhow::bail!("Unknown activity tool"),
    }
}

impl Remote {
    pub(super) async fn require_run(
        &self,
        project: Uuid,
        workspace: Uuid,
        run: Uuid,
    ) -> anyhow::Result<Uuid> {
        let issue = self.require_workspace(project, workspace).await?;
        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM agent_runs WHERE id=? AND workspace_id=?)",
        )
        .bind(run)
        .bind(workspace)
        .fetch_one(&self.pool)
        .await?;
        ensure!(exists, "Run does not exist in this Workspace");
        Ok(issue)
    }

    async fn commands(&self, args: ListArgs) -> anyhow::Result<Value> {
        let issue = self
            .require_run(args.project_id, args.workspace_id, args.agent_run_id)
            .await?;
        ensure!(
            (1..=50).contains(&args.limit),
            "limit must be between 1 and 50"
        );
        let before:Option<String>=match args.cursor {
            Some(cursor)=>Some(sqlx::query_scalar("SELECT created_at FROM agent_run_commands WHERE command_id=? AND agent_run_id=?").bind(cursor).bind(args.agent_run_id).fetch_optional(&self.pool).await?.ok_or_else(||anyhow::anyhow!("Stale or foreign command cursor; refresh activity"))?),
            None=>None,
        };
        let rows=sqlx::query("SELECT command_id,delivery_status,created_at,updated_at,json_extract(command_envelope,'$.command.type') AS kind FROM agent_run_commands WHERE agent_run_id=? AND (? IS NULL OR created_at<? OR (created_at=? AND command_id<?)) ORDER BY created_at DESC,command_id DESC LIMIT ?")
            .bind(args.agent_run_id).bind(&before).bind(&before).bind(&before).bind(args.cursor).bind(args.limit+1).fetch_all(&self.pool).await?;
        let items=rows.into_iter().map(|row|->anyhow::Result<Value>{
            let kind:Option<String>=row.try_get("kind")?;
            Ok(json!({"id":row.try_get::<Uuid,_>("command_id")?,"agent_run_id":args.agent_run_id,"workspace_id":args.workspace_id,"kind":kind,"delivery_status":row.try_get::<String,_>("delivery_status")?,"created_at":row.try_get::<String,_>("created_at")?,"updated_at":row.try_get::<String,_>("updated_at")?,"instruction_readable":matches!(kind.as_deref(),Some("steer"|"submit_input"|"goal_update"|"update_plan_goal_draft")),"implementation_unverified":true,"url":self.config.workspace_url(args.project_id,issue,args.workspace_id)}))
        }).collect::<anyhow::Result<Vec<_>>>()?;
        Ok(page(items, args.limit))
    }

    async fn command(&self, args: CommandArgs) -> anyhow::Result<Value> {
        let issue = self
            .require_run(args.project_id, args.workspace_id, args.agent_run_id)
            .await?;
        ensure!(
            (1..=16000).contains(&args.limit),
            "limit must be between 1 and 16000 characters"
        );
        let row=sqlx::query("SELECT delivery_status,json_extract(command_envelope,'$.command.type') AS kind,CASE json_extract(command_envelope,'$.command.type') WHEN 'steer' THEN json_extract(command_envelope,'$.command.data.content') WHEN 'submit_input' THEN json_extract(command_envelope,'$.command.data.content') WHEN 'goal_update' THEN json_extract(command_envelope,'$.command.data.objective') WHEN 'update_plan_goal_draft' THEN json_extract(command_envelope,'$.command.data.objective') ELSE NULL END AS content FROM agent_run_commands WHERE command_id=? AND agent_run_id=?")
            .bind(args.command_id).bind(args.agent_run_id).fetch_optional(&self.pool).await?.ok_or_else(||anyhow::anyhow!("Command does not exist in this run"))?;
        let text: Option<String> = row.try_get("content")?;
        let available = text.is_some();
        let text = text.unwrap_or_default();
        let total = text.chars().count();
        ensure!(
            args.offset as usize <= total,
            "Instruction offset is outside the content"
        );
        let end = (args.offset as usize + args.limit as usize).min(total);
        Ok(
            json!({"command_id":args.command_id,"agent_run_id":args.agent_run_id,"kind":row.try_get::<Option<String>,_>("kind")?,"delivery_status":row.try_get::<String,_>("delivery_status")?,"instruction_available":available,"content":text.chars().skip(args.offset as usize).take(args.limit as usize).collect::<String>(),"offset":args.offset,"has_more":end<total,"next_offset":if end<total {Some(end)} else {None},"implementation_unverified":true,"url":self.config.workspace_url(args.project_id,issue,args.workspace_id)}),
        )
    }
}
