use anyhow::ensure;
use db::models::integration::IntegrationRun;
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
    #[serde(default = "super::default_limit")]
    #[schemars(range(min = 1, max = 50))]
    limit: u32,
    cursor: Option<Uuid>,
}
#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
enum Section {
    Summary,
    Sources,
    Validation,
}
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct DetailArgs {
    project_id: Uuid,
    integration_id: Uuid,
    section: Section,
    #[serde(default)]
    offset: u32,
    #[serde(default = "super::default_limit")]
    #[schemars(range(min = 1, max = 50))]
    limit: u32,
    expected_revision: Option<String>,
}

pub(super) fn tools() -> Vec<rmcp::model::Tool> {
    vec![
        definition::<ListArgs>(
            "list_integrations",
            "List Auto-merge records for a Project, newest first. Publication and Wiki are separate from the Auto-merge lifecycle; inspect selected records for evidence.",
        ),
        definition::<DetailArgs>(
            "get_integration",
            "Read an Auto-merge summary, frozen sources or code-SHA verification/skip evidence in bounded pages. Use the assigned Session for questions and instructions. Publication, verification and Wiki commits are distinct facts.",
        ),
    ]
}
pub(super) async fn call(remote: &Remote, name: &str, args: Value) -> anyhow::Result<Value> {
    match name {
        "list_integrations" => remote.integrations(arguments(args)?).await,
        "get_integration" => remote.integration(arguments(args)?).await,
        _ => anyhow::bail!("Unknown Auto-merge read tool"),
    }
}

impl Remote {
    async fn integrations(&self, args: ListArgs) -> anyhow::Result<Value> {
        self.require_project(args.project_id).await?;
        ensure!(
            (1..=50).contains(&args.limit),
            super::errors::rejection_with_message(
                utils::mcp_error::Category::Validation,
                "limit must be between 1 and 50"
            )
        );
        let before: Option<String> = match args.cursor {
            Some(cursor) => Some(
                sqlx::query_scalar(
                    "SELECT created_at FROM integration_runs WHERE id=? AND project_id=?",
                )
                .bind(cursor)
                .bind(args.project_id)
                .fetch_optional(&self.pool)
                .await?
                .ok_or_else(|| {
                    anyhow::anyhow!("Stale or foreign Auto-merge cursor; restart listing")
                })?,
            ),
            None => None,
        };
        let rows=sqlx::query("SELECT id,repository_id,status,updated_at,json_extract(payload,'$.published') AS published,json_array_length(payload,'$.sources') AS source_count FROM integration_runs WHERE project_id=? AND (? IS NULL OR created_at<? OR (created_at=? AND id<?)) ORDER BY created_at DESC,id DESC LIMIT ?")
            .bind(args.project_id).bind(&before).bind(&before).bind(&before).bind(args.cursor).bind(args.limit+1).fetch_all(&self.pool).await?;
        let items=rows.into_iter().map(|row|->anyhow::Result<Value>{Ok(json!({"id":row.try_get::<Uuid,_>("id")?,"repository_id":row.try_get::<Uuid,_>("repository_id")?,"status":row.try_get::<String,_>("status")?,"published":row.try_get::<Option<bool>,_>("published")?,"source_count":row.try_get::<Option<i64>,_>("source_count")?,"revision":row.try_get::<String,_>("updated_at")?,"wiki_evidence":"not inspected; use get_integration","url":self.config.project_url(args.project_id)}))}).collect::<anyhow::Result<Vec<_>>>()?;
        Ok(page(items, args.limit))
    }

    async fn integration(&self, args: DetailArgs) -> anyhow::Result<Value> {
        self.require_project(args.project_id).await?;
        ensure!(
            (1..=50).contains(&args.limit),
            super::errors::rejection_with_message(
                utils::mcp_error::Category::Validation,
                "limit must be between 1 and 50"
            )
        );
        let run: IntegrationRun =
            sqlx::query_as("SELECT * FROM integration_runs WHERE id=? AND project_id=?")
                .bind(args.integration_id)
                .bind(args.project_id)
                .fetch_optional(&self.pool)
                .await?
                .ok_or_else(|| anyhow::anyhow!("Auto-merge does not exist in this Project"))?;
        ensure!(
            args.offset == 0 || args.expected_revision.is_some(),
            super::errors::rejection_with_message(
                utils::mcp_error::Category::Validation,
                "Supply expected_revision for subsequent Auto-merge pages"
            )
        );
        ensure!(
            args.expected_revision
                .as_ref()
                .is_none_or(|expected| expected == &run.updated_at),
            super::errors::rejection_with_message(
                utils::mcp_error::Category::Conflict,
                "Auto-merge changed; restart reading this section"
            )
        );
        let mut result = json!({"integration_id":run.id,"project_id":run.project_id,"revision":run.updated_at,"status":run.status,"published":run.payload.published,"skip_validation":run.payload.skip_validation,"url":self.config.project_url(run.project_id)});
        result["conversation"] = json!({"issue_id":run.payload.card_id,"workspace_id":run.workspace_id,"session_id":run.session_id,"agent_run_id":run.agent_run_id,"candidate_commit":run.payload.result_commit,"evidence_count":run.payload.evidence.len(),"publication_request":run.payload.publication_request,"wiki_enabled":run.payload.wiki_enabled,"stop":run.payload.stop});
        if matches!(args.section, Section::Summary) {
            ensure!(
                args.offset == 0,
                super::errors::rejection_with_message(
                    utils::mcp_error::Category::Validation,
                    "Summary has no further pages"
                )
            );
            result["lifecycle"] =
                super::integration_write::observation(self, run.project_id, run.id).await?;
            result["source_count"] = json!(run.payload.sources.len());
            result["repository_id"] = json!(run.repository_id);
            result["target_ref"] = json!(run.target_ref);
            result["result_commit"] = json!(run.payload.result_commit);
            result["error_present"] = json!(run.error.is_some());
            result["wiki"] = json!({"enabled":run.payload.wiki_enabled,"result":run.payload.wiki_result,"commit":run.payload.publication_request.as_ref().and_then(|r|r.wiki_commit.as_ref())});
            result["validation"] = json!({"recorded":run.payload.evidence.len(),"details_tool":"get_integration(section=validation)","scope":"code_commit on each evidence; never final Wiki SHA"});
            return Ok(result);
        }
        if matches!(args.section, Section::Validation) {
            let start = args.offset as usize;
            ensure!(
                start <= run.payload.evidence.len(),
                super::errors::rejection_with_message(
                    utils::mcp_error::Category::Validation,
                    "Evidence offset out of range"
                )
            );
            let end = (start + args.limit as usize).min(run.payload.evidence.len());
            result["items"] = json!(&run.payload.evidence[start..end]);
            result["offset"] = json!(start);
            result["has_more"] = json!(end < run.payload.evidence.len());
            result["next_offset"] = json!(if end < run.payload.evidence.len() {
                Some(end)
            } else {
                None
            });
            return Ok(result);
        }
        let total = match args.section {
            Section::Sources => run.payload.sources.len(),
            Section::Validation => unreachable!(),
            Section::Summary => unreachable!(),
        };
        ensure!(
            args.offset as usize <= total,
            super::errors::rejection_with_message(
                utils::mcp_error::Category::Validation,
                "Page offset is outside this Auto-merge section"
            )
        );
        let end = (args.offset as usize + args.limit as usize).min(total);
        let items=(args.offset as usize..end).map(|index|match args.section {
            Section::Sources=>{
                let source=&run.payload.sources[index];
                json!({"index":index,"issue_id":source.selection.card_id,"workspace_id":source.selection.workspace_id,"expected_commit":source.selection.expected_commit,"frozen_commit":source.commit,"requirements_revision":source.requirements_revision,"done_result_truncated":source.done_result.as_ref().is_some_and(|s|s.chars().count()>500),"done_result":source.done_result.as_deref().map(|s|s.chars().take(500).collect::<String>())})
            }
            Section::Validation=>unreachable!(),
            Section::Summary=>unreachable!(),
        }).collect::<Vec<_>>();
        result["items"] = json!(items);
        result["offset"] = json!(args.offset);
        result["has_more"] = json!(end < total);
        result["next_offset"] = json!(if end < total { Some(end) } else { None });
        Ok(result)
    }
}
