//! UI and MCP share these admission/read/stop services and parameter types.
use axum::{
    Router,
    extract::{Json, Path, State},
    routing::{get, post},
};
use db::models::workspace_repo::CreateWorkspaceRepo;
use deployment::Deployment;
use serde_json::{Value, json};
use uuid::Uuid;
use workflow::presets::{PRESETS, compile};
pub use workflow::presets::{PresetDefaults, PresetRepository, StartPresetRequest};

use super::workflows::{self, TriggerWorkflowRequest};
use crate::{
    DeploymentImpl,
    error::ApiError,
    workflow_runtime::{
        arena::{DeploymentWorkflowArenaCreator, NoopWorkflowArenaCreator},
        runner::*,
        workspace::DeploymentWorkflowWorkspaceResolver,
    },
};

pub fn router() -> Router<DeploymentImpl> {
    Router::new()
        .route("/v1/workflow-presets", get(|| async { Json(catalog()) }))
        .route("/v1/workflow-presets/start", post(start_http))
        .route(
            "/v1/projects/{project}/workflow-preset-defaults",
            get(defaults_http).put(save_http),
        )
}
pub fn catalog() -> Value {
    json!({"presets":PRESETS.map(|p|json!({"id":p,"name":p.name(),"default_perspectives":workflow::presets::default_perspectives(),"contract":workflow::presets::CONTRACT})),"default_mode":"single_agent","guide":include_str!("../../../../assets/skills/workflow-presets/SKILL.md")})
}
async fn start_http(
    State(d): State<DeploymentImpl>,
    Json(r): Json<StartPresetRequest>,
) -> Result<Json<Value>, ApiError> {
    Ok(Json(
        serde_json::to_value(start(&d, r).await?)
            .map_err(|e| ApiError::BadRequest(e.to_string()))?,
    ))
}
async fn defaults_http(
    State(d): State<DeploymentImpl>,
    Path(project): Path<Uuid>,
) -> Result<Json<Vec<PresetDefaults>>, ApiError> {
    Ok(Json(defaults(&d.db().pool, project).await?))
}
async fn save_http(
    State(d): State<DeploymentImpl>,
    Path(project): Path<Uuid>,
    Json(r): Json<PresetDefaults>,
) -> Result<Json<Value>, ApiError> {
    save_defaults(&d.db().pool, project, r).await?;
    Ok(Json(json!({"saved":true})))
}
pub async fn defaults(
    pool: &sqlx::SqlitePool,
    project: Uuid,
) -> Result<Vec<PresetDefaults>, ApiError> {
    let mut result = Vec::new();
    for preset in PRESETS {
        let value:Option<(String,i64)>=sqlx::query_as("SELECT additions,revision FROM workflow_preset_defaults WHERE project_id=? AND preset=?").bind(project).bind(preset.key()).fetch_optional(pool).await?;
        let (additions, revision) = value.unwrap_or_default();
        result.push(PresetDefaults {
            preset,
            additions,
            revision,
        });
    }
    Ok(result)
}
pub async fn save_defaults(
    pool: &sqlx::SqlitePool,
    project: Uuid,
    r: PresetDefaults,
) -> Result<(), ApiError> {
    if r.additions.len() > 32000 {
        return Err(ApiError::BadRequest(
            "Project additions exceed 32000 bytes".into(),
        ));
    }
    let changed = if r.revision == 0 {
        sqlx::query("INSERT INTO workflow_preset_defaults(project_id,preset,additions,revision) VALUES(?,?,?,1) ON CONFLICT DO NOTHING").bind(project).bind(r.preset.key()).bind(r.additions).execute(pool).await?.rows_affected()
    } else {
        sqlx::query("UPDATE workflow_preset_defaults SET additions=?,revision=revision+1 WHERE project_id=? AND preset=? AND revision=?").bind(r.additions).bind(project).bind(r.preset.key()).bind(r.revision).execute(pool).await?.rows_affected()
    };
    if changed != 1 {
        return Err(ApiError::Conflict(
            "Preset defaults changed; reload before saving".into(),
        ));
    }
    Ok(())
}
pub async fn start(
    d: &DeploymentImpl,
    mut r: StartPresetRequest,
) -> Result<workflows::WorkflowRunResponse, ApiError> {
    // Serialize receipt admission in this process. Durable receipt and stable IDs
    // prevent a replay from becoming a second Workflow or fresh agent request.
    static ADMISSION: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
    let _admission = ADMISSION.lock().await;
    let pool = &d.db().pool;
    let request_json =
        serde_json::to_string(&r).map_err(|e| ApiError::BadRequest(e.to_string()))?;
    let receipt: Option<(String, Uuid, Option<Uuid>)> = sqlx::query_as(
        "SELECT request_json,workflow_id,run_id FROM workflow_preset_launches WHERE request_id=?",
    )
    .bind(r.request_id)
    .fetch_optional(pool)
    .await?;
    let workflow_id = if let Some((previous, workflow_id, run_id)) = receipt {
        if previous != request_json {
            return Err(ApiError::Conflict(
                "request_id belongs to different preset parameters".into(),
            ));
        }
        if let Some(id) = run_id {
            return status(d, id).await;
        }
        let existing: Option<Uuid> = sqlx::query_scalar(
            "SELECT id FROM workflow_runs WHERE workflow_id=? ORDER BY started_at LIMIT 1",
        )
        .bind(workflow_id)
        .fetch_optional(pool)
        .await?;
        if let Some(id) = existing {
            sqlx::query("UPDATE workflow_preset_launches SET run_id=? WHERE request_id=?")
                .bind(id)
                .bind(r.request_id)
                .execute(pool)
                .await?;
            return status(d, id).await;
        }
        // A crash before Run persistence may have left a prepared Workspace. Never
        // silently create another one on replay; preserve it for operator recovery.
        return Err(ApiError::Conflict(
            "Preset admission incomplete; inspect existing Workflow/Workspace before recovery"
                .into(),
        ));
    } else {
        let issue = super::local_remote::get_local_issue(pool, r.issue_id).await?;
        if issue.project_id != r.project_id {
            return Err(ApiError::BadRequest(
                "Issue belongs to another Project".into(),
            ));
        }
        super::local_remote::issue_archive::require_unarchived(pool, r.project_id, r.issue_id)
            .await?;
        if let Some(workspace) = r.workspace_id {
            let linked: Option<(Uuid, Uuid)> = sqlx::query_as(
                "SELECT project_id,issue_id FROM local_workspace_links WHERE workspace_id=?",
            )
            .bind(workspace)
            .fetch_optional(pool)
            .await?;
            if linked.is_some_and(|pair| pair != (r.project_id, r.issue_id)) {
                return Err(ApiError::Conflict(
                    "Workspace belongs to another Issue".into(),
                ));
            }
        }
        let default = defaults(pool, r.project_id)
            .await?
            .into_iter()
            .find(|v| v.preset == r.parameters.preset)
            .expect("preset default");
        if r.parameters.project_revision != default.revision {
            return Err(ApiError::Conflict(
                "Project preset additions changed; reload effective instructions before starting"
                    .into(),
            ));
        }
        r.parameters.project_additions = default.additions;
        r.parameters.project_revision = default.revision;
        let graph = compile(r.parameters.clone()).map_err(ApiError::BadRequest)?;
        let workflow_id = Uuid::new_v4();
        let graph_json =
            serde_json::to_string(&graph).map_err(|e| ApiError::BadRequest(e.to_string()))?;
        let mut tx = pool.begin().await?;
        sqlx::query("INSERT INTO workflows(id,source,project_id,name,description,graph_json) VALUES(?,'project',?,?,?,?)")
            .bind(workflow_id).bind(r.project_id).bind(r.parameters.preset.name()).bind("Fixed preset; parameters frozen at admission").bind(graph_json).execute(&mut *tx).await?;
        sqlx::query("INSERT INTO workflow_preset_launches(request_id,project_id,issue_id,request_json,workflow_id) VALUES(?,?,?,?,?)")
            .bind(r.request_id).bind(r.project_id).bind(r.issue_id).bind(&request_json).bind(workflow_id).execute(&mut *tx).await?;
        sqlx::query("INSERT INTO workflow_attempts(id,project_id,issue_id,workflow_id,name,status) VALUES(?,?,?,?,?,'ready')")
            .bind(r.request_id).bind(r.project_id).bind(r.issue_id).bind(workflow_id).bind(r.parameters.preset.name()).execute(&mut *tx).await?;
        tx.commit().await?;
        workflow_id
    };
    let issue = super::local_remote::get_local_issue(pool, r.issue_id).await?;
    let input = format!(
        "Issue {} / revision {}\n{}\n\n{}",
        issue.id,
        issue.updated_at,
        issue.title,
        issue.description.unwrap_or_default()
    );
    let result = trigger_workflow_run_for_attempt_with_repos(
        pool,
        WorkflowRunStartRequest {
            workflow_id,
            attempt_id: Some(r.request_id),
            trigger: TriggerWorkflowRequest {
                issue_id: r.issue_id,
                workspace_id: r.workspace_id,
                trigger_source: "preset".into(),
                input_text: input,
            },
            repo_overrides: r
                .repos
                .into_iter()
                .map(|v| CreateWorkspaceRepo {
                    repo_id: v.repo_id,
                    target_branch: v.target_branch,
                })
                .collect(),
        },
        &DeploymentWorkflowWorkspaceResolver::new(d.clone()),
        &DeploymentWorkflowAgentExecutor::new(d.clone()),
        &NoopWorkflowArenaCreator,
    )
    .await;
    // Persist accepted IDs even when the initial phase fails. Failure is a result,
    // never a reason to repeat admission with a new request identity.
    let run: Option<Uuid> = sqlx::query_scalar(
        "SELECT id FROM workflow_runs WHERE workflow_id=? ORDER BY started_at LIMIT 1",
    )
    .bind(workflow_id)
    .fetch_optional(pool)
    .await?;
    if let Some(id) = run {
        if let Err(error) = &result {
            let active:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM workflow_runs wr JOIN orchestration_agent_run_links l ON l.orchestration_run_id=wr.orchestration_run_id JOIN agent_run_state a ON a.agent_run_id=l.agent_run_id WHERE wr.id=? AND a.status IN ('pending','starting','running','awaiting_input','awaiting_approval','cancelling'))").bind(id).fetch_one(pool).await?;
            if active {
                cancel_workflow_run_runtime(
                    pool,
                    id,
                    &DeploymentWorkflowRunCanceller::new(d.clone()),
                )
                .await?;
            } else {
                sqlx::query("UPDATE workflow_runs SET status='failed',error_text=?,finished_at=datetime('now','subsec') WHERE id=? AND status NOT IN ('succeeded','failed','canceled')").bind(format!("Preset admission failed: {error}")).bind(id).execute(pool).await?;
            }
        }
        sqlx::query("UPDATE workflow_preset_launches SET run_id=? WHERE request_id=?")
            .bind(id)
            .bind(r.request_id)
            .execute(pool)
            .await?;
        let run = get_workflow_run_response(pool, id).await?;
        workflows::sync_attempt_from_run(pool, &run).await?;
        return Ok(run);
    }
    result
}
pub async fn status(
    d: &DeploymentImpl,
    id: Uuid,
) -> Result<workflows::WorkflowRunResponse, ApiError> {
    let run = reconcile_workflow_run_with_arena_and_boundary(
        &d.db().pool,
        id,
        &DeploymentWorkflowAgentExecutor::new(d.clone()),
        &DeploymentWorkflowArenaCreator::new(d.clone()),
        &DeploymentAgentRunReconciliationBoundary::new(d.clone()),
    )
    .await?;
    workflows::sync_attempt_from_run(&d.db().pool, &run).await?;
    Ok(run)
}
pub async fn stop(
    d: &DeploymentImpl,
    id: Uuid,
) -> Result<workflows::WorkflowRunResponse, ApiError> {
    let run = cancel_workflow_run_runtime(
        &d.db().pool,
        id,
        &DeploymentWorkflowRunCanceller::new(d.clone()),
    )
    .await?;
    workflows::sync_attempt_from_run(&d.db().pool, &run).await?;
    Ok(run)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn preset_defaults_are_revisioned_and_project_scoped() {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        db::run_migrations(&pool).await.unwrap();
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();
        for id in [a, b] {
            sqlx::query("INSERT INTO projects(id,name,key) VALUES(?,'preset fixture',?)")
                .bind(id)
                .bind(format!("P{}", &id.simple().to_string()[..6].to_uppercase()))
                .execute(&pool)
                .await
                .unwrap();
        }
        assert_eq!(defaults(&pool, a).await.unwrap().len(), 3);
        let preset = workflow::presets::Preset::InventoryReduce;
        save_defaults(
            &pool,
            a,
            PresetDefaults {
                preset,
                additions: "retain public contracts".into(),
                revision: 0,
            },
        )
        .await
        .unwrap();
        assert!(
            save_defaults(
                &pool,
                a,
                PresetDefaults {
                    preset,
                    additions: "stale".into(),
                    revision: 0
                }
            )
            .await
            .is_err()
        );
        let saved = defaults(&pool, a)
            .await
            .unwrap()
            .into_iter()
            .find(|v| v.preset == preset)
            .unwrap();
        assert_eq!(saved.revision, 1);
        assert_eq!(saved.additions, "retain public contracts");
        assert!(
            defaults(&pool, b)
                .await
                .unwrap()
                .iter()
                .all(|v| v.revision == 0 && v.additions.is_empty())
        );
    }
    #[test]
    fn public_launch_schema_rejects_arbitrary_graph() {
        assert!(serde_json::from_value::<StartPresetRequest>(json!({"request_id":Uuid::new_v4(),"project_id":Uuid::new_v4(),"issue_id":Uuid::new_v4(),"parameters":{"preset":"implement_review"},"graph_json":"{}"})).is_err());
    }
}
