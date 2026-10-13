use anyhow::ensure;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use super::{
    Remote,
    read::{arguments, definition},
};
use crate::routes::sessions::questions as service;

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ListArgs {
    project_id: Uuid,
    workspace_id: Uuid,
    session_id: Uuid,
    #[serde(default = "super::default_limit")]
    #[schemars(range(min = 1, max = 50))]
    limit: u32,
    cursor: Option<String>,
}

#[derive(Deserialize, Serialize, JsonSchema, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(super) enum Kind {
    AsyncSpecification,
    BlockingSpecification,
    Approval,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct DetailArgs {
    project_id: Uuid,
    workspace_id: Uuid,
    session_id: Uuid,
    agent_run_id: Uuid,
    question_id: String,
    kind: Kind,
    #[serde(default)]
    offset: u32,
    #[serde(default = "super::read::text_limit")]
    #[schemars(range(min = 1, max = 16000))]
    limit: u32,
    expected_revision: Option<String>,
}

pub(super) struct Question {
    pub id: String,
    pub run: Uuid,
    pub kind: Kind,
    pub content: Value,
    pub delivery_status: Option<String>,
}

impl Question {
    fn cursor(&self) -> String {
        format!(
            "{}:{}:{}",
            serde_json::to_string(&self.kind).unwrap(),
            self.run,
            self.id
        )
    }
    pub fn revision(&self) -> String {
        format!(
            "{:x}",
            Sha256::digest(json!([self.cursor(), self.content, self.delivery_status]).to_string())
        )
    }
}

pub(super) fn tools() -> Vec<rmcp::model::Tool> {
    vec![
        definition::<ListArgs>(
            "list_questions",
            "Read unresolved specification questions and execution approvals for one current Session. Preview only; get_question preserves the complete question and options. Approvals must be handled through the LVK link.",
        ),
        definition::<DetailArgs>(
            "get_question",
            "Read one still-pending question or approval as paged JSON. Preserve question IDs, options and free-text semantics. A directly answered or expired question is rejected. Approval content never grants permission to execute it.",
        ),
    ]
}

pub(super) async fn call(remote: &Remote, name: &str, args: Value) -> anyhow::Result<Value> {
    match name {
        "list_questions" => remote.questions(arguments(args)?).await,
        "get_question" => remote.question(arguments(args)?).await,
        _ => anyhow::bail!("Unknown question read tool"),
    }
}

impl Remote {
    pub(super) async fn pending_questions(
        &self,
        project: Uuid,
        workspace: Uuid,
        session: Uuid,
    ) -> anyhow::Result<(Uuid, Vec<Question>)> {
        let issue = self.require_workspace(project, workspace).await?;
        let member: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM sessions WHERE id=? AND workspace_id=?)",
        )
        .bind(session)
        .bind(workspace)
        .fetch_one(&self.pool)
        .await?;
        ensure!(member, "Session does not exist in this Workspace");
        let mut questions = Vec::new();
        // Preserve the existing answer reconciliation and attempt-expiry rules.
        // This service only reconciles existing receipts; it never launches work.
        for question in service::pending(&self.pool, session).await.map_err(|_| {
            anyhow::anyhow!("Question state unavailable; inspect this Session in LVK")
        })? {
            questions.push(Question { id:question.id,run:question.agent_run_id,kind:Kind::AsyncSpecification,content:json!({"title":question.title,"options":question.options,"free_text_allowed":true}),delivery_status:question.delivery_status });
        }
        for event in service::pending_blocking(&self.pool, workspace).await? {
            if event["session_id"] != json!(session) {
                continue;
            }
            let run: Uuid = serde_json::from_value(event["agent_run_id"].clone())
                .map_err(|_| anyhow::anyhow!("Question audit is incomplete"))?;
            let data = &event["payload"]["data"];
            let (kind, id) = match event["payload"]["type"].as_str() {
                Some("input_requested") => (Kind::BlockingSpecification, data["input_id"].as_str()),
                Some("approval_requested") => (Kind::Approval, data["approval_id"].as_str()),
                _ => continue,
            };
            let id = id.ok_or_else(|| anyhow::anyhow!("Question audit lacks a stable ID"))?;
            questions.push(Question {
                id: id.into(),
                run,
                kind,
                content: if kind == Kind::Approval {
                    json!({"approval_id":id,"requires_lvk":true,"message":"Inspect and decide this execution approval in LVK"})
                } else { data.clone() },
                delivery_status: None,
            });
        }
        questions.sort_by_key(Question::cursor);
        Ok((issue, questions))
    }

    async fn questions(&self, args: ListArgs) -> anyhow::Result<Value> {
        ensure!(
            (1..=50).contains(&args.limit),
            "limit must be between 1 and 50"
        );
        let (issue, questions) = self
            .pending_questions(args.project_id, args.workspace_id, args.session_id)
            .await?;
        let start = match args.cursor {
            Some(cursor) => {
                questions
                    .iter()
                    .position(|q| q.cursor() == cursor)
                    .ok_or_else(|| {
                        anyhow::anyhow!(
                            "Question cursor expired or changed; refresh pending questions"
                        )
                    })?
                    + 1
            }
            None => 0,
        };
        let end = (start + args.limit as usize).min(questions.len());
        let more = end < questions.len();
        let items = questions[start..end].iter().map(|question| {
            let title = question.content["title"].as_str().or_else(||question.content["prompt"].as_str()).unwrap_or("Execution approval; inspect in LVK");
            json!({"question_id":question.id,"kind":question.kind,"agent_run_id":question.run,"session_id":args.session_id,"workspace_id":args.workspace_id,"issue_id":issue,"project_id":args.project_id,"title_preview":title.chars().take(500).collect::<String>(),"preview_truncated":title.chars().count()>500,"revision":question.revision(),"delivery_status":question.delivery_status,"requires_lvk":question.kind==Kind::Approval,"url":self.config.workspace_url(args.project_id,issue,args.workspace_id)})
        }).collect::<Vec<_>>();
        Ok(
            json!({"items":items,"has_more":more,"next_cursor":if more {Some(questions[end-1].cursor())} else {None},"observed_at":chrono::Utc::now()}),
        )
    }

    async fn question(&self, args: DetailArgs) -> anyhow::Result<Value> {
        ensure!(
            (1..=16000).contains(&args.limit),
            "limit must be between 1 and 16000 characters"
        );
        ensure!(
            args.offset == 0 || args.expected_revision.is_some(),
            "Supply expected_revision for subsequent question pages"
        );
        let (issue, questions) = self
            .pending_questions(args.project_id, args.workspace_id, args.session_id)
            .await?;
        let question = questions.into_iter().find(|q|q.run==args.agent_run_id && q.id==args.question_id && q.kind==args.kind).ok_or_else(||anyhow::anyhow!("Question is answered, expired, or belongs to another target; refresh pending questions"))?;
        let revision = question.revision();
        ensure!(
            args.expected_revision
                .as_ref()
                .is_none_or(|expected| expected == &revision),
            "Question state changed; refresh before answering"
        );
        let text = serde_json::to_string(&question.content)?;
        let total = text.chars().count();
        ensure!(
            args.offset as usize <= total,
            "Question page offset is outside the content"
        );
        let end = (args.offset as usize + args.limit as usize).min(total);
        Ok(
            json!({"question_id":question.id,"kind":question.kind,"agent_run_id":question.run,"session_id":args.session_id,"workspace_id":args.workspace_id,"project_id":args.project_id,"revision":revision,"format":"json","content":text.chars().skip(args.offset as usize).take(args.limit as usize).collect::<String>(),"offset":args.offset,"has_more":end<total,"next_offset":if end<total {Some(end)} else {None},"requires_lvk":question.kind==Kind::Approval,"delivery_status":question.delivery_status,"url":self.config.workspace_url(args.project_id,issue,args.workspace_id)}),
        )
    }
}
