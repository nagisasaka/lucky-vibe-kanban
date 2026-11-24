//! One domain service for UI, stdio MCP and remote MCP. Paging never narrows the
//! semantic corpus or the machine candidate set.
pub mod machine;
#[cfg(test)]
mod tests;

use api_types::*;
use axum::{
    Extension, Json, Router,
    extract::{Path, Query, State},
    routing::get,
};
use chrono::{DateTime, Utc};
use db::models::saved_filter::{self as store, SavedFilterWrite};
use deployment::Deployment;
use sha2::{Digest, Sha256};
use sqlx::SqlitePool;
use utils::response::ApiResponse;
use uuid::Uuid;

use crate::{DeploymentImpl, error::ApiError, routes::local_remote};

pub fn router() -> Router<DeploymentImpl> {
    Router::new()
        .route("/projects/{project}/saved-filters", get(list).post(create))
        .route(
            "/projects/{project}/saved-filters/{id}",
            get(read).put(update).delete(delete),
        )
        .route(
            "/projects/{project}/saved-filters/{id}/evaluate",
            axum::routing::post(evaluate),
        )
        .route(
            "/projects/{project}/saved-filters/{id}/results",
            get(results),
        )
        .route("/projects/{project}/saved-filters/{id}/link", get(link))
}
fn authorize(actor: &server_access::Principal, write: bool) -> Result<String, ApiError> {
    if !actor.scopes.contains("lvk:read") || (write && !actor.scopes.contains("lvk:write")) {
        return Err(ApiError::Forbidden(
            "Saved filter permission required".into(),
        ));
    }
    Ok(serde_json::json!([actor.channel, actor.issuer, actor.subject]).to_string())
}
pub async fn load_corpus(pool: &SqlitePool, project: Uuid) -> Result<SavedFilterCorpus, ApiError> {
    local_remote::saved_filter_snapshot::load(pool, project).await
}

pub struct EvaluationOptions {
    pub activate: bool,
    pub retry_failed: bool,
    pub now: DateTime<Utc>,
}
impl Default for EvaluationOptions {
    fn default() -> Self {
        Self {
            activate: false,
            retry_failed: false,
            now: Utc::now(),
        }
    }
}
fn semantic_request(
    definition: &SavedFilter,
    corpus: &SavedFilterCorpus,
    candidates: Vec<Uuid>,
) -> crate::saved_filter_semantics::SemanticRequest {
    use crate::saved_filter_semantics::{SemanticIssue, SemanticRequest};
    // Stable snapshot order comes from SQL. Per-Issue evidence includes both
    // incoming and outgoing edges plus parent/child relationships.
    let mut relations: std::collections::HashMap<Uuid, Vec<&IssueRelationship>> =
        std::collections::HashMap::new();
    for r in &corpus.relationships {
        relations.entry(r.issue_id).or_default().push(r);
        relations.entry(r.related_issue_id).or_default().push(r);
    }
    let mut children: std::collections::HashMap<Uuid, Vec<Uuid>> = std::collections::HashMap::new();
    for i in &corpus.issues {
        if let Some(parent) = i.parent_issue_id {
            children.entry(parent).or_default().push(i.id);
        }
    }
    let mut tags: std::collections::HashMap<Uuid, Vec<&IssueTag>> =
        std::collections::HashMap::new();
    for t in &corpus.issue_tags {
        tags.entry(t.issue_id).or_default().push(t);
    }
    let mut assignees: std::collections::HashMap<Uuid, Vec<&IssueAssignee>> =
        std::collections::HashMap::new();
    for a in &corpus.assignees {
        assignees.entry(a.issue_id).or_default().push(a);
    }
    SemanticRequest {
        filter_id:definition.id,project_id:definition.project_id,definition_revision:i64::from(definition.revision),
        natural_language:definition.natural_language.clone().unwrap_or_default(),
        context:serde_json::json!({"definition_context":definition.context,"statuses":corpus.statuses,"tags":corpus.tags,"assignee_display_names":corpus.assignee_display_names}),
        issues:corpus.issues.iter().map(|i|SemanticIssue{id:i.id,evidence:serde_json::json!({"issue":i,"issue_tags":tags.get(&i.id).cloned().unwrap_or_default(),"assignees":assignees.get(&i.id).cloned().unwrap_or_default(),"relationships":relations.get(&i.id).cloned().unwrap_or_default(),"children":children.get(&i.id).cloned().unwrap_or_default()})}).collect(),
        machine_candidate_ids:candidates,
    }
}
pub async fn evaluate_service(
    pool: &SqlitePool,
    deployment: Option<&DeploymentImpl>,
    project: Uuid,
    id: Uuid,
    page: &SavedFilterPage,
    options: EvaluationOptions,
) -> Result<SavedFilterEvaluation, ApiError> {
    use crate::saved_filter_semantics::{EvaluationStatus, SemanticResult};
    if page.offset > 0 && page.expected_evaluation_revision.is_none() {
        return Err(ApiError::BadRequest(
            "expected_evaluation_revision is required for result offset > 0".into(),
        ));
    }
    let definition = store::get(pool, project, id).await?;
    let corpus = load_corpus(pool, project).await?;
    let candidates = machine::candidate_ids(&corpus, &definition.machine, options.now);
    let semantic = if definition
        .natural_language
        .as_deref()
        .is_none_or(|s| s.trim().is_empty())
    {
        SemanticResult {
            status: EvaluationStatus::Complete,
            issue_ids: Some(candidates.clone()),
            stale: false,
            error: None,
            definition_revision: i64::from(definition.revision),
        }
    } else {
        let deployment = deployment.ok_or_else(|| {
            ApiError::BadGateway("Saved filter evaluation runtime unavailable".into())
        })?;
        crate::saved_filter_semantics::evaluate(
            deployment,
            semantic_request(&definition, &corpus, candidates.clone()),
            options.activate,
            options.retry_failed,
        )
        .await?
    };
    let state = match semantic.status {
        EvaluationStatus::Unevaluated => SavedFilterEvaluationState::Unevaluated,
        EvaluationStatus::Evaluating => SavedFilterEvaluationState::Evaluating,
        EvaluationStatus::Complete => SavedFilterEvaluationState::Complete,
        EvaluationStatus::Failed => SavedFilterEvaluationState::Failed,
    };
    // Never label an evaluation of an old definition as the current revision.
    let current = store::get(pool, project, id).await?;
    let changed = current.revision != definition.revision;
    let selected: std::collections::HashSet<_> =
        semantic.issue_ids.unwrap_or_default().into_iter().collect();
    let ids: Vec<_> = candidates
        .into_iter()
        .filter(|id| selected.contains(id))
        .collect();
    let stale = semantic.stale || changed;
    let definition_revision = i32::try_from(semantic.definition_revision)
        .map_err(|_| ApiError::Conflict("Invalid semantic definition revision".into()))?;
    let fingerprint = serde_json::json!({
        "version": 1, "project_id":project, "saved_filter_id":id,
        "definition_revision":definition_revision, "current_revision":current.revision,
        "state":state, "issue_ids":ids, "stale":stale, "error":semantic.error
    })
    .to_string();
    let evaluation_revision = format!("{:x}", Sha256::digest(fingerprint));
    if page
        .expected_evaluation_revision
        .as_ref()
        .is_some_and(|expected| expected != &evaluation_revision)
    {
        return Err(ApiError::SavedFilterEvaluationChanged);
    }
    let total_count = ids.len();
    let offset = page.offset as usize;
    let limit = page.limit.unwrap_or(100).clamp(1, 500) as usize;
    Ok(SavedFilterEvaluation {
        saved_filter_id: id,
        project_id: project,
        state,
        issue_ids: ids.into_iter().skip(offset).take(limit).collect(),
        total_count,
        offset,
        limit,
        next_offset: (offset.saturating_add(limit) < total_count)
            .then_some(offset.saturating_add(limit)),
        stale,
        error: semantic.error,
        definition_revision,
        evaluation_revision,
    })
}
pub async fn link_service(
    pool: &SqlitePool,
    project: Uuid,
    id: Uuid,
) -> Result<SavedFilterLink, ApiError> {
    store::get(pool, project, id).await?;
    Ok(SavedFilterLink {
        saved_filter_id: id,
        project_id: project,
        url: format!("/projects/{project}?savedFilter={id}"),
    })
}
async fn list(
    State(d): State<DeploymentImpl>,
    Extension(actor): Extension<server_access::Principal>,
    Path(project): Path<Uuid>,
    Query(page): Query<SavedFilterPage>,
) -> Result<Json<ApiResponse<ListSavedFiltersResponse>>, ApiError> {
    authorize(&actor, false)?;
    Ok(Json(ApiResponse::success(
        store::list(&d.db().pool, project, &page).await?,
    )))
}
async fn read(
    State(d): State<DeploymentImpl>,
    Extension(actor): Extension<server_access::Principal>,
    Path((project, id)): Path<(Uuid, Uuid)>,
) -> Result<Json<ApiResponse<SavedFilter>>, ApiError> {
    authorize(&actor, false)?;
    Ok(Json(ApiResponse::success(
        store::get(&d.db().pool, project, id).await?,
    )))
}
async fn create(
    State(d): State<DeploymentImpl>,
    Extension(actor): Extension<server_access::Principal>,
    Path(project): Path<Uuid>,
    Json(input): Json<CreateSavedFilterRequest>,
) -> Result<Json<ApiResponse<serde_json::Value>>, ApiError> {
    Ok(Json(ApiResponse::success(
        store::write(
            &d.db().pool,
            project,
            &authorize(&actor, true)?,
            SavedFilterWrite::Create(input),
        )
        .await?,
    )))
}
async fn update(
    State(d): State<DeploymentImpl>,
    Extension(actor): Extension<server_access::Principal>,
    Path((project, id)): Path<(Uuid, Uuid)>,
    Json(input): Json<UpdateSavedFilterRequest>,
) -> Result<Json<ApiResponse<serde_json::Value>>, ApiError> {
    Ok(Json(ApiResponse::success(
        store::write(
            &d.db().pool,
            project,
            &authorize(&actor, true)?,
            SavedFilterWrite::Update { id, input },
        )
        .await?,
    )))
}
async fn delete(
    State(d): State<DeploymentImpl>,
    Extension(actor): Extension<server_access::Principal>,
    Path((project, id)): Path<(Uuid, Uuid)>,
    Json(input): Json<DeleteSavedFilterRequest>,
) -> Result<Json<ApiResponse<serde_json::Value>>, ApiError> {
    Ok(Json(ApiResponse::success(
        store::write(
            &d.db().pool,
            project,
            &authorize(&actor, true)?,
            SavedFilterWrite::Delete { id, input },
        )
        .await?,
    )))
}
async fn evaluate(
    State(d): State<DeploymentImpl>,
    Extension(actor): Extension<server_access::Principal>,
    Path((project, id)): Path<(Uuid, Uuid)>,
    Json(input): Json<EvaluateSavedFilterRequest>,
) -> Result<Json<ApiResponse<SavedFilterEvaluation>>, ApiError> {
    authorize(&actor, true)?;
    Ok(Json(ApiResponse::success(
        evaluate_service(
            &d.db().pool,
            Some(&d),
            project,
            id,
            &SavedFilterPage {
                offset: input.offset,
                limit: input.limit,
                expected_evaluation_revision: input.expected_evaluation_revision,
            },
            EvaluationOptions {
                activate: true,
                retry_failed: input.retry_failed,
                ..Default::default()
            },
        )
        .await?,
    )))
}
async fn results(
    State(d): State<DeploymentImpl>,
    Extension(actor): Extension<server_access::Principal>,
    Path((project, id)): Path<(Uuid, Uuid)>,
    Query(page): Query<SavedFilterPage>,
) -> Result<Json<ApiResponse<SavedFilterEvaluation>>, ApiError> {
    authorize(&actor, false)?;
    Ok(Json(ApiResponse::success(
        evaluate_service(
            &d.db().pool,
            Some(&d),
            project,
            id,
            &page,
            EvaluationOptions::default(),
        )
        .await?,
    )))
}
async fn link(
    State(d): State<DeploymentImpl>,
    Extension(actor): Extension<server_access::Principal>,
    Path((project, id)): Path<(Uuid, Uuid)>,
) -> Result<Json<ApiResponse<SavedFilterLink>>, ApiError> {
    authorize(&actor, false)?;
    Ok(Json(ApiResponse::success(
        link_service(&d.db().pool, project, id).await?,
    )))
}
