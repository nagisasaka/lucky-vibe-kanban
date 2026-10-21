//! Lightweight provider-reported counters, never billing or context occupancy.
use serde::Serialize;
use sqlx::{FromRow, SqlitePool};
use ts_rs::TS;
use uuid::Uuid;

#[derive(Debug, Serialize, TS)]
pub struct WorkspaceTokenUsage {
    pub workspace_id: Uuid,
    pub total_tokens: Option<i64>,
    pub input_tokens: Option<i64>,
    pub output_tokens: Option<i64>,
    pub cached_input_tokens: Option<i64>,
    pub reasoning_tokens: Option<i64>,
    pub cached_input_observed_threads: i64,
    pub reasoning_observed_threads: i64,
    /// Unknown without verified reports; otherwise partial: unreported work is unknowable.
    pub completeness: String,
    pub scope: String,
    pub thread_count: i64,
    pub attempt_count: i64,
    pub observed_attempt_count: i64,
    pub providers: Vec<String>,
    /// Requested models are not proof of the model actually used for a counter.
    pub requested_models: Vec<String>,
    pub first_observed_at: Option<String>,
    pub updated_at: Option<String>,
}

#[derive(FromRow)]
struct Totals {
    total: Option<i64>,
    input: Option<i64>,
    output: Option<i64>,
    cached: Option<i64>,
    reasoning: Option<i64>,
    threads: i64,
    cached_threads: i64,
    reasoning_threads: i64,
    first: Option<String>,
    updated: Option<String>,
}

impl WorkspaceTokenUsage {
    pub async fn get(pool: &SqlitePool, workspace_id: Uuid) -> Result<Self, sqlx::Error> {
        let mut tx = pool.begin().await?;
        let totals: Totals = sqlx::query_as(
            "SELECT SUM(total_tokens) AS total, SUM(input_tokens) AS input,
             SUM(output_tokens) AS output,
             SUM(cached_input_tokens) AS cached, COUNT(cached_input_tokens) AS cached_threads,
             SUM(reasoning_tokens) AS reasoning, COUNT(reasoning_tokens) AS reasoning_threads,
             COUNT(*) AS threads, MIN(first_observed_at) AS first, MAX(updated_at) AS updated
             FROM workspace_token_usage WHERE workspace_id=?",
        )
        .bind(workspace_id)
        .fetch_one(&mut *tx)
        .await?;
        let (attempt_count, observed_attempt_count): (i64, i64) = sqlx::query_as(
            "SELECT COUNT(*), COUNT(CASE WHEN json_extract(u.event_json,'$.payload.data.cumulative_usage.provider_id')='codex' THEN 1 END)
             FROM agent_run_attempts a JOIN agent_runs r ON r.id=a.agent_run_id
             LEFT JOIN agent_run_usage_snapshots u ON u.run_attempt_id=a.id WHERE r.workspace_id=?"
        ).bind(workspace_id).fetch_one(&mut *tx).await?;
        let providers = sqlx::query_scalar("SELECT DISTINCT provider_id FROM workspace_token_usage WHERE workspace_id=? ORDER BY provider_id")
            .bind(workspace_id).fetch_all(&mut *tx).await?;
        let requested_models = sqlx::query_scalar("SELECT DISTINCT json_extract(a.request_envelope,'$.executor_config.model_id') FROM agent_run_attempts a JOIN agent_runs r ON r.id=a.agent_run_id WHERE r.workspace_id=? AND json_extract(a.request_envelope,'$.executor_config.model_id') IS NOT NULL ORDER BY 1")
            .bind(workspace_id).fetch_all(&mut *tx).await?;
        tx.commit().await?;
        Ok(Self {
            workspace_id, total_tokens: totals.total, input_tokens: totals.input,
            output_tokens: totals.output, cached_input_tokens: totals.cached,
            reasoning_tokens: totals.reasoning,
            cached_input_observed_threads: totals.cached_threads, reasoning_observed_threads: totals.reasoning_threads,
            completeness: if totals.threads == 0 { "unknown" } else { "partial" }.into(),
            scope: "Codex parent-thread cumulative totals observed since collection began (a first report may include earlier usage in that thread); cached input is included in input, reasoning in output. Unreported usage, child-thread usage not included by the provider, unsupported providers and unmeasured history are unknown. Requested models are not actual model attribution. Not a cost estimate.".into(),
            thread_count: totals.threads, attempt_count, observed_attempt_count, providers, requested_models,
            first_observed_at: totals.first, updated_at: totals.updated,
        })
    }
}
