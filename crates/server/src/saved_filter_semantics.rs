//! On-demand saved-filter semantics over the existing Workspace/Session/Review runtime.
//!
//! Core owns the Project snapshot, definition authorization/revision and migration.
//! Required table: saved_filter_semantic_cache(filter_id BLOB PRIMARY KEY REFERENCES
//! saved_filters(id) ON DELETE CASCADE, generation INTEGER NOT NULL, state_json TEXT NOT NULL).
//! saved_filters supplies id, project_id and integer revision. No Issue writes occur here.
mod engine;
mod stream;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use db::models::{
    session::{CreateSession, Session},
    workspace::{CreateWorkspace, Workspace},
};
use deployment::Deployment;
use engine::{Cache, Evidence, Pending, Success};
pub use engine::{EvaluationStatus, SemanticIssue, SemanticRequest, SemanticResult};
use executors::{
    profile::{ExecutionMode, ExecutorConfig},
    runtime::AgentRunStatus,
};
use services::services::agent_runtime::AgentRuntimeReadService;
use sqlx::SqlitePool;
use uuid::Uuid;

use crate::{DeploymentImpl, error::ApiError};

/// `activate=false` observes/reconciles durable results but never starts AI work.
/// Polling an actively used filter may pass true. Failed identical input is not retried
/// unless `retry_failed` is explicitly true, so provider errors cannot create a hot loop.
pub async fn evaluate(
    deployment: &DeploymentImpl,
    request: SemanticRequest,
    activate: bool,
    retry_failed: bool,
) -> Result<SemanticResult, ApiError> {
    // Reuse the existing scoped lock so a slow launch cannot be mistaken for an
    // abandoned reservation by another request in the same server process.
    let _lock = deployment
        .queued_message_service()
        .lock_session(request.filter_id)
        .await;
    evaluate_with(
        &deployment.db().pool,
        &Runtime(deployment),
        &request,
        activate,
        retry_failed,
        Utc::now(),
    )
    .await
}

enum Progress {
    Running,
    Complete(String),
    Failed(String),
}

#[async_trait]
trait EvaluationRuntime: Sync {
    async fn launch(&self, pending: &Pending, prompt: String) -> Result<(), ApiError>;
    async fn poll(&self, pending: &Pending, now: DateTime<Utc>) -> Result<Progress, ApiError>;
}

struct Runtime<'a>(&'a DeploymentImpl);

#[async_trait]
impl EvaluationRuntime for Runtime<'_> {
    async fn launch(&self, pending: &Pending, prompt: String) -> Result<(), ApiError> {
        let deployment = self.0;
        let pool = &deployment.db().pool;
        let path = utils::assets::asset_dir()
            .join("saved-filter-evaluations")
            .join(pending.workspace_id.to_string());
        tokio::fs::create_dir_all(&path).await?;
        if deployment.git().is_inside_repository(&path) {
            return Err(ApiError::BadRequest(
                "Filter evaluation needs LVK_INSTANCE_DIR outside a Git repository".into(),
            ));
        }
        let profile = deployment.config().read().await.executor_profile.clone();
        let mut config = ExecutorConfig::new(profile.executor);
        config.variant = profile.variant;
        config.execution_mode = Some(ExecutionMode::Review);
        Workspace::create_direct_folder(
            pool,
            &CreateWorkspace {
                branch: "direct-folder".into(),
                name: Some("Saved filter evaluation".into()),
            },
            pending.workspace_id,
            &path.to_string_lossy(),
        )
        .await?;
        let session = Session::create(
            pool,
            &CreateSession {
                executor: Some(config.executor.to_string()),
                name: Some("Saved filter evaluation".into()),
            },
            pending.session_id,
            pending.workspace_id,
        )
        .await?;
        let _lock = deployment
            .queued_message_service()
            .lock_session(session.id)
            .await;
        let response = crate::routes::sessions::review::launch_review(
            deployment.clone(),
            session,
            crate::routes::sessions::review::StartReviewRequest {
                executor_config: config,
                additional_prompt: None,
                use_all_workspace_commits: false,
            },
            Some(prompt),
        )
        .await?;
        if !response.0.is_success() {
            return Err(ApiError::Conflict(
                "Saved filter agent launch was not accepted".into(),
            ));
        }
        Ok(())
    }

    async fn poll(&self, pending: &Pending, now: DateTime<Utc>) -> Result<Progress, ApiError> {
        let runs = AgentRuntimeReadService::new(&self.0.db().pool)
            .list_for_session(pending.session_id)
            .await
            .map_err(runtime_error)?;
        let Some(run) = runs.last() else {
            // A crash between durable reservation and launch is never silently retried:
            // the old process may still have dispatched. The user can retry a failed result.
            return Ok(
                if now.signed_duration_since(pending.started_at).num_seconds() > 120 {
                    Progress::Failed(
                        "Evaluation launch interrupted before a durable Run was found".into(),
                    )
                } else {
                    Progress::Running
                },
            );
        };
        if !run.state.status.is_terminal() {
            return Ok(Progress::Running);
        }
        if run.state.status != AgentRunStatus::Succeeded {
            return Ok(Progress::Failed(format!(
                "Evaluation Run ended with {:?}",
                run.state.status
            )));
        }
        Ok(match &run.state.terminal_output {
            Some(message) => Progress::Complete(message.content.clone()),
            None => Progress::Failed("Successful evaluation Run has no final response".into()),
        })
    }
}

fn runtime_error(error: impl std::fmt::Display) -> ApiError {
    ApiError::BadGateway(error.to_string())
}
fn invalid(error: impl std::fmt::Display) -> ApiError {
    ApiError::BadRequest(error.to_string())
}

async fn load(pool: &SqlitePool, request: &SemanticRequest) -> Result<(i64, Cache), ApiError> {
    let revision: Option<i64> =
        sqlx::query_scalar("SELECT revision FROM saved_filters WHERE id=? AND project_id=?")
            .bind(request.filter_id)
            .bind(request.project_id)
            .fetch_optional(pool)
            .await?;
    if revision != Some(request.definition_revision) {
        return Err(ApiError::Conflict(
            "Saved filter was deleted or its definition revision changed".into(),
        ));
    }
    let row: Option<(i64, String)> = sqlx::query_as(
        "SELECT generation,state_json FROM saved_filter_semantic_cache WHERE filter_id=?",
    )
    .bind(request.filter_id)
    .fetch_optional(pool)
    .await?;
    let Some((generation, json)) = row else {
        return Ok((0, Cache::new(request)));
    };
    let cache: Cache = serde_json::from_str(&json).map_err(invalid)?;
    if cache.schema_version != 1 || cache.project_id != request.project_id {
        return Err(ApiError::Conflict(
            "Unsupported or mismatched semantic cache".into(),
        ));
    }
    Ok((generation, cache))
}

/// A single SQL compare-and-swap also checks the authoritative definition revision.
/// Neither a competing process nor a late completion can overwrite a newer snapshot.
async fn save(
    pool: &SqlitePool,
    request: &SemanticRequest,
    generation: i64,
    cache: &Cache,
) -> Result<bool, ApiError> {
    let json = serde_json::to_string(cache).map_err(invalid)?;
    let changed = if generation == 0 {
        sqlx::query("INSERT INTO saved_filter_semantic_cache(filter_id,generation,state_json) SELECT ?,1,? WHERE EXISTS(SELECT 1 FROM saved_filters WHERE id=? AND project_id=? AND revision=?) ON CONFLICT(filter_id) DO NOTHING")
            .bind(request.filter_id).bind(json).bind(request.filter_id).bind(request.project_id).bind(request.definition_revision)
            .execute(pool).await?
    } else {
        sqlx::query("UPDATE saved_filter_semantic_cache SET generation=generation+1,state_json=? WHERE filter_id=? AND generation=? AND EXISTS(SELECT 1 FROM saved_filters WHERE id=? AND project_id=? AND revision=?)")
            .bind(json).bind(request.filter_id).bind(generation).bind(request.filter_id).bind(request.project_id).bind(request.definition_revision)
            .execute(pool).await?
    };
    Ok(changed.rows_affected() == 1)
}

async fn evaluate_with(
    pool: &SqlitePool,
    runtime: &impl EvaluationRuntime,
    request: &SemanticRequest,
    activate: bool,
    retry_failed: bool,
    now: DateTime<Utc>,
) -> Result<SemanticResult, ApiError> {
    let evidence = Evidence::from_request(request).map_err(invalid)?;
    for _ in 0..8 {
        let (generation, mut cache) = load(pool, request).await?;
        if request.natural_language.trim().is_empty() {
            // Core normally bypasses us for machine-only filters. Still enforce revision.
            let all: std::collections::BTreeSet<_> = request.issues.iter().map(|i| i.id).collect();
            let candidates: std::collections::BTreeSet<_> =
                request.machine_candidate_ids.iter().copied().collect();
            return Ok(SemanticResult {
                status: EvaluationStatus::Complete,
                issue_ids: Some(all.intersection(&candidates).copied().collect()),
                stale: false,
                error: None,
                definition_revision: request.definition_revision,
            });
        }
        cache.definition_revision = request.definition_revision;
        if let Some(mut pending) = cache.pending.clone() {
            if !pending.dispatched {
                if !pending.plan.evidence.same(&evidence) {
                    cache.pending = None;
                    if !save(pool, request, generation, &cache).await? {
                        continue;
                    }
                    continue;
                }
                if !activate {
                    return cache.result(request, now).map_err(invalid);
                }
                pending.dispatched = true;
                pending.started_at = now;
                let prompt = pending
                    .stream
                    .as_ref()
                    .ok_or_else(|| invalid("Missing streaming checkpoint"))?
                    .prompt();
                cache.pending = Some(pending.clone());
                if !save(pool, request, generation, &cache).await? {
                    continue;
                }
                if let Err(error) = runtime.launch(&pending, prompt).await {
                    cache.failure = Some(format!("Evaluation launch failed: {error}"));
                    let _ = save(pool, request, generation + 1, &cache).await?;
                }
                return cache.result(request, now).map_err(invalid);
            }
            match runtime.poll(&pending, now).await? {
                Progress::Running => return cache.result(request, now).map_err(invalid),
                Progress::Complete(output) => {
                    // A local delta discovering a cross-Issue dependency must
                    // restart with the full population, not guess from changed rows.
                    let escalates = (!pending.plan.reused.is_empty()
                        || pending
                            .stream
                            .as_ref()
                            .is_some_and(|s| s.needs_project_scan()))
                        && serde_json::from_str::<serde_json::Value>(engine::normalize_output(
                            &output,
                        ))
                        .ok()
                        .is_some_and(|v| v["scope"] == "project");
                    if escalates {
                        if !pending.plan.reused.is_empty() {
                            if let Some(success) = &mut cache.success {
                                success.scope = engine::Scope::Project;
                            }
                            cache.pending = None;
                        } else {
                            let mut stream = pending
                                .stream
                                .clone()
                                .ok_or_else(|| invalid("Missing scan checkpoint"))?;
                            stream.restart_project(&pending.plan).map_err(invalid)?;
                            cache.pending = Some(Pending {
                                session_id: Uuid::new_v4(),
                                workspace_id: Uuid::new_v4(),
                                started_at: now,
                                plan: pending.plan.clone(),
                                stream: Some(stream),
                                dispatched: false,
                            });
                        }
                    } else if let Err(error) = cache.complete(&output, now) {
                        cache.fail(error);
                    }
                }
                Progress::Failed(error) => cache.fail(error),
            }
            if !save(pool, request, generation, &cache).await? {
                continue;
            }
            // Reconcile first. The next poll can launch updated evidence without making
            // a failed completion immediately disappear behind an automatic retry.
            return cache.result(request, now).map_err(invalid);
        }
        let Some(plan) = cache.plan(evidence.clone(), now) else {
            return cache.result(request, now).map_err(invalid);
        };
        let same_snapshot = cache
            .success
            .as_ref()
            .is_some_and(|s| s.evidence.same(&evidence));
        if !same_snapshot || retry_failed {
            cache.expired_refreshes = 0;
            cache.refresh_after = None;
        }
        if !activate
            || (same_snapshot
                && !retry_failed
                && (cache.expired_refreshes >= 2 || cache.refresh_after.is_some_and(|at| now < at)))
            || (!retry_failed
                && cache
                    .failed_evidence
                    .as_ref()
                    .is_some_and(|e| e.same(&evidence))
                && !cache.failed_time_boundary.is_some_and(|at| now >= at))
        {
            return cache.result(request, now).map_err(invalid);
        }
        if plan.requested_ids.is_empty() {
            // Empty Project, or only deletions from an independent per-Issue result.
            cache.success = Some(Success {
                evidence: plan.evidence,
                decisions: plan.reused,
                scope: engine::Scope::Local,
                valid_until: plan.reused_until,
            });
            cache.failure = None;
            cache.failed_evidence = None;
            if !save(pool, request, generation, &cache).await? {
                continue;
            }
            return cache.result(request, now).map_err(invalid);
        }
        let direct_prompt = engine::prompt(request, &plan, now);
        let stream = (direct_prompt.len() > stream::DIRECT_LIMIT)
            .then(|| stream::Stream::new(request, &plan, now));
        let prompt = stream.as_ref().map_or(direct_prompt, |work| work.prompt());
        let pending = Pending {
            session_id: Uuid::new_v4(),
            workspace_id: Uuid::new_v4(),
            started_at: now,
            plan,
            stream,
            dispatched: true,
        };
        cache.pending = Some(pending.clone());
        cache.failure = None;
        cache.failed_evidence = None;
        if !save(pool, request, generation, &cache).await? {
            continue;
        }
        // Reservation is durable before any side effect, including provider launch.
        // Cancellation/restart is recovered from this reserved Session on a later call.
        if let Err(error) = runtime.launch(&pending, prompt).await {
            cache.failure = Some(format!("Evaluation launch failed: {error}"));
            // Keep the reservation until polling determines whether dispatch happened.
            // A launch error alone is not proof that no provider process was started.
            let _ = save(pool, request, generation + 1, &cache).await?;
        }
        return cache.result(request, now).map_err(invalid);
    }
    Err(ApiError::Conflict(
        "Saved filter evaluation changed concurrently; retry the request".into(),
    ))
}

#[cfg(test)]
mod tests;
