//! Admission adapters for new Workspaces and the existing formal Integration.
use std::collections::HashSet;

use anyhow::ensure;
use async_trait::async_trait;
use db::models::{
    integration::{IntegrationRun, IntegrationSelection},
    repo::Repo,
    requests::{
        CreateAndStartWorkspaceRequest, CreateWorkspaceMode, LinkedIssueInfo, WorkspaceRepoInput,
    },
    workspace::Workspace,
    workspace_repo::WorkspaceRepo,
};
use deployment::Deployment;
use executors::profile::{ExecutionMode, ExecutorConfig};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::SqlitePool;
use uuid::Uuid;

use super::{
    Remote,
    auth::Principal,
    read::{arguments, definition, page},
};
use crate::{
    DeploymentImpl,
    error::ApiError,
    routes::{integrations, local_remote, workspaces},
};

#[derive(Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct RepositorySelection {
    repository_id: Uuid,
    target_branch: String,
}
#[derive(Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct StartArgs {
    project_id: Uuid,
    issue_id: Uuid,
    request_id: Uuid,
    expected_revision: String,
    #[schemars(length(min = 1, max = 10))]
    repositories: Vec<RepositorySelection>,
    /// Omit to preserve the normal default execution mode. Goal uses native controls.
    execution_mode: Option<ExecutionMode>,
    #[schemars(range(min = 1))]
    goal_token_budget: Option<u32>,
}
#[derive(Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Selection {
    issue_id: Uuid,
    workspace_id: Uuid,
    expected_commit: String,
}
#[derive(Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct MergeArgs {
    project_id: Uuid,
    request_id: Uuid,
    repository_id: Uuid,
    target_branch: String,
    expected_target_commit: String,
    #[schemars(length(min = 1, max = 100))]
    selections: Vec<Selection>,
    /// true skips additional Integration validation (unverified); false runs it.
    /// Omit or null to inherit the server integration_skip_validation preference.
    skip_validation: Option<bool>,
}
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct RepositoriesArgs {
    project_id: Uuid,
    #[serde(default = "super::default_limit")]
    #[schemars(range(min = 1, max = 50))]
    limit: u32,
    cursor: Option<Uuid>,
}
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct BranchArgs {
    project_id: Uuid,
    repository_id: Uuid,
    #[serde(default = "super::default_limit")]
    #[schemars(range(min = 1, max = 50))]
    limit: u32,
    cursor: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct CandidateArgs {
    project_id: Uuid,
    repository_id: Uuid,
    workspace_id: Uuid,
    target_branch: String,
}
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct DefaultsArgs {
    project_id: Uuid,
}

#[async_trait]
pub(super) trait LaunchActions: Send + Sync {
    async fn decide_integration(
        &self,
        _id: Uuid,
        _request: integrations::GateDecisionRequest,
        _expected: integrations::DecisionExpectation,
    ) -> anyhow::Result<()> {
        anyhow::bail!("Integration decision service unavailable")
    }
    async fn cancel_integration(&self, _id: Uuid) -> anyhow::Result<()> {
        anyhow::bail!("Integration cancellation service unavailable")
    }

    async fn defaults(&self) -> ExecutorConfig;
    async fn start(
        &self,
        request: CreateAndStartWorkspaceRequest,
        workspace_id: Uuid,
    ) -> Result<Value, ApiError>;
    async fn integration(
        &self,
        request: integrations::CreateIntegrationRequest,
        expected_target: String,
    ) -> Result<Value, integrations::AdmissionFailure>;
}
pub(super) struct ExistingLaunch(pub DeploymentImpl);
#[async_trait]
impl LaunchActions for ExistingLaunch {
    async fn decide_integration(
        &self,
        id: Uuid,
        request: integrations::GateDecisionRequest,
        expected: integrations::DecisionExpectation,
    ) -> anyhow::Result<()> {
        integrations::decide_gate_checked(&self.0, id, request, Some(expected)).await
    }
    async fn cancel_integration(&self, id: Uuid) -> anyhow::Result<()> {
        let _ = integrations::cancel(
            axum::extract::State(self.0.clone()),
            axum::extract::Path(id),
        )
        .await?;
        Ok(())
    }

    async fn defaults(&self) -> ExecutorConfig {
        self.0.config().read().await.executor_profile.clone().into()
    }
    async fn start(
        &self,
        request: CreateAndStartWorkspaceRequest,
        workspace_id: Uuid,
    ) -> Result<Value, ApiError> {
        let result = workspaces::create::create_and_start_workspace_admitted(
            self.0.clone(),
            request,
            Some(workspace_id),
        )
        .await?;
        let result = result
            .0
            .into_data()
            .ok_or_else(|| ApiError::Conflict("Workspace acceptance receipt unavailable".into()))?;
        Ok(
            json!({"workspace_id":result.workspace.id,"session_id":result.agent_run.state.session_id,"agent_run_id":result.agent_run.agent_run_id,"execution_accepted":true,"implementation_complete":false}),
        )
    }
    async fn integration(
        &self,
        request: integrations::CreateIntegrationRequest,
        expected_target: String,
    ) -> Result<Value, integrations::AdmissionFailure> {
        let run =
            integrations::create_with_expected_target(&self.0, request, &expected_target).await?;
        Ok(integration_result(&run))
    }
}
pub(super) fn integration_result(run: &IntegrationRun) -> Value {
    json!({"integration_id":run.id,"status":run.status,"integration_accepted":true,"published":run.payload.published,"skip_validation":run.payload.skip_validation,"wiki":"not_verified"})
}

fn branch(value: &str) -> anyhow::Result<()> {
    ensure!(
        value.len() <= 1024
            && !value.starts_with("refs/")
            && git::GitService::new().is_branch_name_valid(value),
        "Select a valid local branch name"
    );
    Ok(())
}
fn commit(value: &str) -> anyhow::Result<()> {
    ensure!(
        value.len() == 40 && value.bytes().all(|b| b.is_ascii_hexdigit()),
        "Expected a full commit ID from the current candidate"
    );
    Ok(())
}

pub(super) fn tools() -> Vec<rmcp::model::Tool> {
    let mut start=serde_json::to_value(definition::<StartArgs>("start_workspace","Start agreed Issue requirements in a new Worktree Workspace using the configured default executor profile. Pin the Issue revision and explicit repositories/branches. Goal uses native execution_mode. Returns existing execution admission IDs, not completion; use the original request_id after disconnection.")).unwrap();
    start["annotations"] = json!({"readOnlyHint":false,"destructiveHint":true,"idempotentHint":true,"openWorldHint":true});
    start["_meta"] =
        json!({"securitySchemes":[{"type":"oauth2","scopes":["lvk:read","lvk:write"]}]});
    let mut merge=serde_json::to_value(definition::<MergeArgs>("start_integration","Start existing formal Integration for an explicitly frozen selection and target HEAD. Requires merge permission; preserves existing validation, writer settlement, publication and Wiki rules. Does not directly merge branches or mark Issues Done.")).unwrap();
    merge["annotations"] = json!({"readOnlyHint":false,"destructiveHint":true,"idempotentHint":true,"openWorldHint":true});
    merge["_meta"] =
        json!({"securitySchemes":[{"type":"oauth2","scopes":["lvk:read","lvk:merge"]}]});
    vec![
        serde_json::from_value(start).unwrap(),
        serde_json::from_value(merge).unwrap(),
        definition::<RepositoriesArgs>(
            "list_project_repositories",
            "List current Project repository IDs and default branches in bounded pages, including configured Worktree defaults. Host paths are omitted. Integration membership may be narrower than creation defaults.",
        ),
        definition::<BranchArgs>(
            "list_repository_branches",
            "List current local branches and exact commit IDs for a permitted repository in bounded pages. Use these names for Workspace creation and target selection; no remote branch fetch occurs.",
        ),
        definition::<CandidateArgs>(
            "get_integration_candidate",
            "Read exact source and target commit IDs for one linked Workspace/repository. This is selection evidence, not merge eligibility: formal admission still checks idle writers, cleanliness, manifests and all related Workspaces.",
        ),
        definition::<DefaultsArgs>(
            "get_execution_defaults",
            "Read the same default executor identity used by normal new-Workspace creation. Profile model/authentication settings remain on the server; no model is started.",
        ),
    ]
}

async fn prepare_start(
    pool: &SqlitePool,
    args: &StartArgs,
    mut executor: ExecutorConfig,
    workspace_id: Uuid,
) -> anyhow::Result<CreateAndStartWorkspaceRequest> {
    ensure!(
        !args.repositories.is_empty() && args.repositories.len() <= 10,
        "Select 1–10 repositories"
    );
    let issue = local_remote::get_local_issue(pool, args.issue_id)
        .await
        .map_err(|_| anyhow::anyhow!("Issue unavailable"))?;
    ensure!(
        issue.project_id == args.project_id,
        "Issue is outside this Project"
    );
    let revision: String = sqlx::query_scalar("SELECT updated_at FROM local_issues WHERE id=?")
        .bind(args.issue_id)
        .fetch_one(pool)
        .await?;
    ensure!(
        revision == args.expected_revision,
        "Issue changed; read current requirements before starting"
    );
    let description = issue.description.as_deref().unwrap_or("");
    ensure!(
        !description.trim().is_empty(),
        "Issue needs self-contained agreed requirements before starting"
    );
    ensure!(
        description.len() <= 256000 && issue.title.len() <= 2000,
        "Issue text exceeds execution admission limit"
    );
    let permitted = local_remote::project_creation_repository_ids(pool, args.project_id).await?;
    let mut seen = HashSet::new();
    for selection in &args.repositories {
        ensure!(
            seen.insert(selection.repository_id) && permitted.contains(&selection.repository_id),
            "Repository duplicated or outside this Project's configured repositories"
        );
        branch(&selection.target_branch)?;
        let repo = Repo::find_by_id(pool, selection.repository_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Repository no longer exists"))?;
        let target = selection.target_branch.clone();
        tokio::task::spawn_blocking(move || {
            git::GitService::new().get_branch_oid(&repo.path, &target)
        })
        .await?
        .map_err(|_| anyhow::anyhow!("Target branch unavailable in selected repository"))?;
    }
    if let Some(mode) = args.execution_mode {
        executor.execution_mode = Some(mode);
    }
    if let Some(budget) = args.goal_token_budget {
        ensure!(
            budget > 0
                && matches!(
                    executor.execution_mode,
                    Some(ExecutionMode::Goal | ExecutionMode::PlanWithGoal)
                ),
            "A positive token budget requires Goal or PlanWithGoal mode"
        );
        executor.goal_token_budget = Some(budget);
    }
    Ok(CreateAndStartWorkspaceRequest {
        mode: CreateWorkspaceMode::Worktree,
        name: Some(issue.title.clone()),
        repos: args
            .repositories
            .iter()
            .map(|r| WorkspaceRepoInput {
                repo_id: r.repository_id,
                target_branch: r.target_branch.clone(),
            })
            .collect(),
        directory_path: None,
        linked_issue: Some(LinkedIssueInfo {
            remote_project_id: args.project_id,
            issue_id: args.issue_id,
        }),
        executor_config: executor,
        prompt: format!(
            "{}\n\n{}\n\nLVK MCP operation: {workspace_id}",
            issue.title, description
        ),
        selected_skills: None,
        resume_session_id: None,
        resume_scope_path: None,
        attachment_ids: None,
    })
}

impl Remote {
    async fn repositories(&self, args: RepositoriesArgs) -> anyhow::Result<Value> {
        self.require_project(args.project_id).await?;
        ensure!(
            (1..=50).contains(&args.limit),
            "limit must be between 1 and 50"
        );
        let mut ids =
            local_remote::project_creation_repository_ids(&self.pool, args.project_id).await?;
        ids.sort();
        if let Some(cursor) = args.cursor {
            ensure!(
                ids.contains(&cursor),
                "Stale or foreign repository cursor; restart listing"
            );
        }
        let integration_ids =
            integrations::project_repository_ids(&self.pool, args.project_id).await?;
        let mut items = Vec::new();
        for id in ids
            .into_iter()
            .filter(|id| args.cursor.is_none_or(|cursor| *id > cursor))
            .take(args.limit as usize + 1)
        {
            if let Some(repo) = Repo::find_by_id(&self.pool, id).await? {
                items.push(json!({"id":repo.id,"name":repo.display_name.chars().take(500).collect::<String>(),"name_truncated":repo.display_name.chars().count()>500,"default_target_branch":repo.default_target_branch,"integration_member":integration_ids.contains(&id),"url":self.config.project_url(args.project_id)}));
            }
        }
        Ok(page(items, args.limit))
    }
    async fn branches(&self, args: BranchArgs) -> anyhow::Result<Value> {
        self.require_project(args.project_id).await?;
        ensure!(
            (1..=50).contains(&args.limit),
            "limit must be between 1 and 50"
        );
        ensure!(
            local_remote::project_creation_repository_ids(&self.pool, args.project_id)
                .await?
                .contains(&args.repository_id),
            "Repository is not in this Project"
        );
        let repo = Repo::find_by_id(&self.pool, args.repository_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Repository missing"))?;
        let prefix = format!("{}:", args.repository_id);
        let after = args
            .cursor
            .as_deref()
            .map(|cursor| {
                cursor
                    .strip_prefix(&prefix)
                    .map(str::to_owned)
                    .ok_or_else(|| anyhow::anyhow!("Foreign branch cursor; restart listing"))
            })
            .transpose()?;
        let items = tokio::task::spawn_blocking(move || -> anyhow::Result<Vec<Value>> {
            let git = git::GitService::new();
            let mut branches = git.get_all_branches(&repo.path).map_err(|_|anyhow::anyhow!("Repository branches unavailable"))?;
            branches.retain(|b| !b.is_remote); branches.sort_by(|a,b|a.name.cmp(&b.name));
            ensure!(after.as_ref().is_none_or(|name|branches.iter().any(|b| &b.name == name)), "Stale branch cursor; restart listing");
            branches.into_iter().filter(|b|after.as_ref().is_none_or(|after|b.name > *after)).take(args.limit as usize+1).map(|b| {
                let head=git.get_branch_oid(&repo.path,&b.name).map_err(|_|anyhow::anyhow!("Branch changed while reading; restart listing"))?;
                Ok(json!({"id":format!("{prefix}{}",b.name),"name":b.name,"commit":head,"is_current":b.is_current}))
            }).collect()
        }).await??;
        Ok(page(items, args.limit))
    }

    async fn candidate(&self, args: CandidateArgs) -> anyhow::Result<Value> {
        let issue = self
            .require_workspace(args.project_id, args.workspace_id)
            .await?;
        branch(&args.target_branch)?;
        let workspace = Workspace::find_by_id(&self.pool, args.workspace_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Workspace missing"))?;
        ensure!(
            WorkspaceRepo::find_by_workspace_and_repo_id(
                &self.pool,
                args.workspace_id,
                args.repository_id
            )
            .await?
            .is_some(),
            "Repository is not in this Workspace"
        );
        let repo = Repo::find_by_id(&self.pool, args.repository_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Repository missing"))?;
        let target = args.target_branch.clone();
        let (head, target_head) = tokio::task::spawn_blocking(move || {
            let git = git::GitService::new();
            Ok::<_, git::GitServiceError>((
                git.get_branch_oid(&repo.path, &workspace.branch)?,
                git.get_branch_oid(&repo.path, &target)?,
            ))
        })
        .await?
        .map_err(|_| {
            anyhow::anyhow!("Candidate commits unavailable; inspect the Workspace in LVK")
        })?;
        Ok(
            json!({"project_id":args.project_id,"issue_id":issue,"workspace_id":args.workspace_id,"repository_id":args.repository_id,"source_commit":head,"target_branch":args.target_branch,"target_commit":target_head,"eligibility":"not_validated","formal_validation_on_start":true,"url":self.config.workspace_url(args.project_id,issue,args.workspace_id)}),
        )
    }
}
fn validation_error(error: anyhow::Error) -> (&'static str, Value) {
    let message = super::errors::message(&error);
    (
        "failed",
        json!({"code":"admission_validation_failed","message":message}),
    )
}
pub(super) async fn call(
    remote: &Remote,
    principal: &Principal,
    name: &str,
    input: Value,
) -> anyhow::Result<Value> {
    ensure!(principal.allows("lvk:read"), "Read permission required");
    match name {
        "list_project_repositories" => return remote.repositories(arguments(input)?).await,
        "get_integration_candidate" => return remote.candidate(arguments(input)?).await,
        "list_repository_branches" => return remote.branches(arguments(input)?).await,
        "get_execution_defaults" => {
            let args: DefaultsArgs = arguments(input)?;
            remote.require_project(args.project_id).await?;
            let launch = remote
                .launch
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("Launch service unavailable"))?;
            let config = launch.defaults().await;
            return Ok(
                json!({"executor":config.executor,"variant":config.variant,"execution_mode":config.execution_mode.unwrap_or_default(),"model":"configured_profile","url":remote.config.project_url(args.project_id)}),
            );
        }
        _ => {}
    }
    let required = if name == "start_integration" {
        "lvk:merge"
    } else {
        "lvk:write"
    };
    ensure!(
        principal.allows(required),
        "Required operation scope is missing"
    );
    let launch = remote
        .launch
        .clone()
        .ok_or_else(|| anyhow::anyhow!("Launch service unavailable"))?;
    let owned = remote.clone();
    let actor = remote.principal_key(principal);
    let receipt = match name {
        "start_workspace" => {
            let args: StartArgs = arguments(input.clone())?;
            remote.require_issue(args.project_id, args.issue_id).await?;
            remote.operations().submit_and_wait(actor,args.request_id,args.project_id,name.into(),input,json!({"issue_id":args.issue_id}),move|operation|async move {
                let request=match prepare_start(&owned.pool,&args,launch.defaults().await,operation).await {Ok(request)=>request,Err(error)=>return validation_error(error)};
                let url=owned.config.workspace_url(args.project_id,args.issue_id,operation);
                match launch.start(request,operation).await {
                    Ok(mut value)=>{value["url"]=json!(url);("succeeded",value)},
                    Err(_)=>("uncertain",json!({"workspace_id":operation,"url":url,"message":"Inspect this Workspace and its runs before any new start request"})),
                }
            }).await?
        }
        "start_integration" => {
            let args: MergeArgs = arguments(input.clone())?;
            remote.require_project(args.project_id).await?;
            branch(&args.target_branch)?;
            commit(&args.expected_target_commit)?;
            ensure!(
                !args.selections.is_empty() && args.selections.len() <= 100,
                "Select 1–100 Issue/Workspace pairs"
            );
            let mut issues = HashSet::new();
            let mut workspaces = HashSet::new();
            for selection in &args.selections {
                commit(&selection.expected_commit)?;
                ensure!(
                    issues.insert(selection.issue_id) && workspaces.insert(selection.workspace_id),
                    "Select exactly one adopted Workspace per Issue"
                );
            }
            remote.operations().submit_and_wait(actor,args.request_id,args.project_id,name.into(),input,json!({"repository_id":args.repository_id,"workspace_ids":args.selections.iter().map(|s|s.workspace_id).collect::<Vec<_>>()}),move|operation|async move {
                let permitted = integrations::project_repository_ids(&owned.pool,args.project_id).await;
                if !permitted.is_ok_and(|ids|ids.contains(&args.repository_id)) {return ("failed",json!({"code":"repository_scope_changed"}));}
                for selection in &args.selections {
                    if !WorkspaceRepo::find_by_workspace_and_repo_id(&owned.pool,selection.workspace_id,args.repository_id).await.is_ok_and(|repo|repo.is_some()) {return ("failed",json!({"code":"workspace_repository_changed"}));}
                    match owned.require_workspace(args.project_id,selection.workspace_id).await {Ok(issue) if issue==selection.issue_id=>{},_=>return ("failed",json!({"code":"selection_scope_changed"}))}
                }
                let request=integrations::CreateIntegrationRequest {request_id:operation,project_id:args.project_id,repository_id:args.repository_id,target_branch:args.target_branch,selections:args.selections.into_iter().map(|s|IntegrationSelection {card_id:s.issue_id,workspace_id:s.workspace_id,expected_commit:s.expected_commit}).collect(),executor_config:None,skip_validation:args.skip_validation};
                let url=owned.config.project_url(args.project_id);
                match launch.integration(request,args.expected_target_commit).await {
                    Ok(mut value)=>{value["url"]=json!(url);("succeeded",value)},
                    Err(integrations::AdmissionFailure::Rejected {code,message})=>("failed",json!({"code":code,"message":message,"integration_request_id":operation,"integration_accepted":false,"url":url})),
                    Err(integrations::AdmissionFailure::Uncertain)=>("uncertain",json!({"integration_request_id":operation,"url":url,"message":"Inspect Integration records before any new request; admission may have completed"})),
                }
            }).await?
        }
        _ => anyhow::bail!("Unknown launch tool"),
    };
    Ok(remote.receipt_value(receipt))
}
