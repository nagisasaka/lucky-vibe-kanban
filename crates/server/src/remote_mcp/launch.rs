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
use utils::mcp_error::Category;
use uuid::Uuid;

use super::{
    Remote,
    auth::Principal,
    read::{arguments, definition, page},
};
use crate::{
    DeploymentImpl,
    error::ApiError,
    routes::{integrations, local_remote, sessions, workspaces},
};

/// Engine selection creates a fresh native conversation; it never migrates one.
#[derive(Clone, Copy, Deserialize, JsonSchema)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum Engine {
    Codex,
    ClaudeCode,
}
impl Engine {
    fn executor(self) -> executors::executors::BaseCodingAgent {
        match self {
            Self::Codex => executors::executors::BaseCodingAgent::Codex,
            Self::ClaudeCode => executors::executors::BaseCodingAgent::ClaudeCode,
        }
    }
}
fn select_engine(defaults: ExecutorConfig, engine: Option<Engine>) -> ExecutorConfig {
    match engine {
        None => defaults,
        Some(engine) if engine.executor() == defaults.executor => defaults,
        Some(engine) => ExecutorConfig::new(engine.executor()),
    }
}
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct StartSessionArgs {
    project_id: Uuid,
    workspace_id: Uuid,
    request_id: Uuid,
    /// Omit to use the configured default engine/profile. Changing engine starts a new native conversation.
    engine: Option<Engine>,
    /// Include the handoff purpose, decisions and remaining work. Native provider conversation is not transferred.
    #[schemars(length(min = 1, max = 256000))]
    prompt: String,
}
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
    /// New Session engine; omitted preserves configured default selection.
    engine: Option<Engine>,
    #[schemars(length(min = 1, max = 10))]
    repositories: Vec<RepositorySelection>,
    /// Omit to preserve the default mode. Codex review uses Code execution for bounded review/investigation: include target, scope and expected findings in the Issue. Report in conversation and stop; implementation requires a separate explicit instruction. Existing permissions still apply. Goal uses native controls.
    execution_mode: Option<ExecutionMode>,
    #[schemars(range(min = 1))]
    goal_token_budget: Option<u32>,
    #[serde(default)]
    attachment_ids: Vec<Uuid>,
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
    /// Optional user instruction to omit additional validation. The assigned agent records actual executions/skips and risk.
    /// This flag does not indicate whether validation evidence already exists.
    /// Omit or null to inherit the server integration_skip_validation preference.
    skip_validation: Option<bool>,
    instructions: Option<String>,
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
    async fn attachment_transfer(
        &self,
        _name: &str,
        _input: Value,
        _actor: String,
    ) -> anyhow::Result<Value> {
        return Err(super::errors::rejection(Category::Unavailable));
    }
    async fn place_for_execution(
        &self,
        _args: crate::routes::attachment_transfer::Place,
    ) -> anyhow::Result<Value> {
        return Err(super::errors::rejection(Category::Unavailable));
    }
    async fn cancel_integration(&self, _id: Uuid) -> anyhow::Result<()> {
        return Err(super::errors::rejection(Category::Unavailable));
    }

    async fn wiki_status(&self, _repository: Uuid) -> anyhow::Result<Value> {
        return Err(super::errors::rejection(Category::Unavailable));
    }
    async fn wiki_sync(&self, _request: super::repository_wiki::SyncArgs) -> anyhow::Result<Value> {
        return Err(super::errors::rejection(Category::Unavailable));
    }
    async fn start_session(
        &self,
        _workspace_id: Uuid,
        _prompt: String,
        _config: ExecutorConfig,
    ) -> Result<Value, ApiError> {
        Err(ApiError::BadRequest(
            "New Session launch unavailable".into(),
        ))
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
    async fn attachment_transfer(
        &self,
        name: &str,
        input: Value,
        actor: String,
    ) -> anyhow::Result<Value> {
        match name {
            "upload_attachment" => {
                crate::routes::attachment_transfer::upload(
                    self.0.clone(),
                    serde_json::from_value(input)?,
                    actor,
                )
                .await
            }
            "place_attachments" => {
                crate::routes::attachment_transfer::place(
                    self.0.clone(),
                    serde_json::from_value(input)?,
                    actor,
                )
                .await
            }
            _ => return Err(super::errors::rejection(Category::Validation)),
        }
    }
    async fn place_for_execution(
        &self,
        args: crate::routes::attachment_transfer::Place,
    ) -> anyhow::Result<Value> {
        crate::routes::attachment_transfer::place_now(&self.0, &args).await
    }
    async fn cancel_integration(&self, id: Uuid) -> anyhow::Result<()> {
        let _ = integrations::cancel(
            axum::extract::State(self.0.clone()),
            axum::extract::Path(id),
        )
        .await?;
        Ok(())
    }

    async fn wiki_status(&self, repository: Uuid) -> anyhow::Result<Value> {
        super::repository_wiki::status(&self.0, repository).await
    }
    async fn wiki_sync(&self, request: super::repository_wiki::SyncArgs) -> anyhow::Result<Value> {
        super::repository_wiki::sync(&self.0, request).await
    }
    async fn start_session(
        &self,
        workspace_id: Uuid,
        prompt: String,
        config: ExecutorConfig,
    ) -> Result<Value, ApiError> {
        let session = sessions::create_session(
            axum::extract::State(self.0.clone()),
            axum::Json(sessions::CreateSessionRequest {
                workspace_id,
                executor: Some(config.executor.to_string()),
                name: None,
            }),
        )
        .await?
        .0
        .into_data()
        .ok_or_else(|| ApiError::Conflict("Session receipt unavailable".into()))?;
        let run = sessions::start_coding_agent_execution_for_session(
            &self.0,
            session.clone(),
            prompt,
            None,
            config.clone(),
            None,
            None,
        )
        .await?;
        Ok(
            json!({"workspace_id":workspace_id,"session_id":session.id,"agent_run_id":run.agent_run_id,"execution_accepted":true,"new_session":true,"native_conversation_inherited":false,"executor_config":config,"provider_settings_verified":false,"implementation_complete":false}),
        )
    }
    async fn defaults(&self) -> ExecutorConfig {
        self.0.config().read().await.executor_profile.clone().into()
    }
    async fn start(
        &self,
        request: CreateAndStartWorkspaceRequest,
        workspace_id: Uuid,
    ) -> Result<Value, ApiError> {
        let executor_config = request.executor_config.clone();
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
            json!({"workspace_id":result.workspace.id,"session_id":result.agent_run.state.session_id,"agent_run_id":result.agent_run.agent_run_id,"execution_accepted":true,"new_session":true,"native_conversation_inherited":false,"executor_config":executor_config,"provider_settings_verified":false,"implementation_complete":false}),
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
    json!({"integration_id":run.id,"status":run.status,"integration_accepted":true,"published":run.payload.published,"skip_validation":run.payload.skip_validation,"wiki":"not_verified","issue_id":run.payload.card_id,"workspace_id":run.workspace_id,"session_id":run.session_id,"agent_run_id":run.agent_run_id})
}

fn branch(value: &str) -> anyhow::Result<()> {
    ensure!(
        value.len() <= 1024
            && !value.starts_with("refs/")
            && git::GitService::new().is_branch_name_valid(value),
        super::errors::rejection_with_message(
            utils::mcp_error::Category::Validation,
            "Select a valid local branch name"
        )
    );
    Ok(())
}
fn commit(value: &str) -> anyhow::Result<()> {
    ensure!(
        value.len() == 40 && value.bytes().all(|b| b.is_ascii_hexdigit()),
        super::errors::rejection_with_message(
            utils::mcp_error::Category::Validation,
            "Expected a full commit ID from the current candidate"
        )
    );
    Ok(())
}

pub(super) fn tools() -> Vec<rmcp::model::Tool> {
    let mut start=serde_json::to_value(definition::<StartArgs>("start_workspace","Start agreed Issue requirements in a new Worktree Workspace using the configured default executor profile unless engine is explicit. Engine selection starts a new Session and native conversation. Pin the Issue revision and explicit repositories/branches from its current Project preset; historical linked repositories do not extend creation choices. Review uses the Code path to investigate the Issue scope, report in conversation and stop; separate explicit instructions are required for product implementation and existing permissions still apply. Goal uses native execution_mode. Returns existing execution admission IDs, not completion; use the original request_id after disconnection.")).unwrap();
    start["annotations"] = json!({"readOnlyHint":false,"destructiveHint":true,"idempotentHint":true,"openWorldHint":true});
    start["_meta"] =
        json!({"securitySchemes":[{"type":"oauth2","scopes":["lvk:read","lvk:write"]}]});
    let mut merge=serde_json::to_value(definition::<MergeArgs>("start_integration","Start Auto-merge for an explicitly frozen selection and target HEAD. Call this feature 自動マージ in Japanese and Auto-merge in English in explanations, start confirmations, progress and result reports. Requires merge permission; preserves existing validation, writer settlement, publication and Wiki rules. The assigned Codex starts immediately and continues in one normal conversation; validation may be run or skipped with reasons and risk recorded. Code merge and subsequent Wiki updates are separate outcomes. Does not directly merge branches or mark Issues Done.")).unwrap();
    merge["annotations"] = json!({"readOnlyHint":false,"destructiveHint":true,"idempotentHint":true,"openWorldHint":true});
    merge["_meta"] =
        json!({"securitySchemes":[{"type":"oauth2","scopes":["lvk:read","lvk:merge"]}]});
    vec![
        serde_json::from_value(start).unwrap(),
        {
            let mut tool = serde_json::to_value(definition::<StartSessionArgs>("start_session", "Start a new Session in the specified existing Workspace using its current code. Optional engine selects CODEX or CLAUDE_CODE; omitted uses configured defaults. Include purpose, decisions and remaining work in prompt. This starts a new native conversation; it does not migrate or continue another Session. Returns admission IDs, not completion. Reuse request_id after disconnection.")).unwrap();
            tool["annotations"] = json!({"readOnlyHint":false,"destructiveHint":true,"idempotentHint":true,"openWorldHint":true});
            tool["_meta"] =
                json!({"securitySchemes":[{"type":"oauth2","scopes":["lvk:read","lvk:write"]}]});
            serde_json::from_value(tool).unwrap()
        },
        serde_json::from_value(merge).unwrap(),
        definition::<RepositoriesArgs>(
            "list_project_repositories",
            "List Project preset and linked Workspace repositories in bounded pages. creation_preset_member identifies choices permitted for new Issue Workspaces; removing a preset never invalidates existing Workspace repositories. Host paths are omitted. Auto-merge admission still uses the actual Workspace and recorded merge target.",
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
    executor: ExecutorConfig,
    workspace_id: Uuid,
) -> anyhow::Result<CreateAndStartWorkspaceRequest> {
    let mut executor = select_engine(executor, args.engine);
    ensure!(
        !args.repositories.is_empty() && args.repositories.len() <= 10,
        super::errors::rejection_with_message(
            utils::mcp_error::Category::Validation,
            "Select 1–10 repositories"
        )
    );
    let issue = local_remote::get_local_issue(pool, args.issue_id)
        .await
        .map_err(|_| super::errors::rejection(Category::Unavailable))?;
    ensure!(
        issue.project_id == args.project_id,
        super::errors::rejection_with_message(
            utils::mcp_error::Category::Permission,
            "Issue is outside this Project"
        )
    );
    let revision: String = sqlx::query_scalar("SELECT updated_at FROM local_issues WHERE id=?")
        .bind(args.issue_id)
        .fetch_one(pool)
        .await?;
    ensure!(
        revision == args.expected_revision,
        super::errors::rejection_with_message(
            utils::mcp_error::Category::Conflict,
            "Issue changed; read current requirements before starting"
        )
    );
    let description = issue.description.as_deref().unwrap_or("");
    ensure!(
        !description.trim().is_empty(),
        super::errors::rejection_with_message(
            utils::mcp_error::Category::Validation,
            "Issue needs self-contained agreed requirements before starting"
        )
    );
    ensure!(
        description.len() <= 256000 && issue.title.len() <= 2000,
        super::errors::rejection_with_message(
            utils::mcp_error::Category::Validation,
            "Issue text exceeds execution admission limit"
        )
    );
    let permitted = local_remote::project_creation_repository_ids(pool, args.project_id).await?;
    let mut seen = HashSet::new();
    for selection in &args.repositories {
        ensure!(
            seen.insert(selection.repository_id) && permitted.contains(&selection.repository_id),
            super::errors::rejection_with_message(
                utils::mcp_error::Category::Validation,
                "Repository duplicated or outside this Project's configured repositories"
            )
        );
        branch(&selection.target_branch)?;
        let repo = Repo::find_by_id(pool, selection.repository_id)
            .await?
            .ok_or_else(|| super::errors::rejection(Category::NotFound))?;
        let target = selection.target_branch.clone();
        tokio::task::spawn_blocking(move || {
            git::GitService::new().get_branch_oid(&repo.path, &target)
        })
        .await?
        .map_err(|_| super::errors::rejection(Category::Unavailable))?;
    }
    if args.execution_mode.is_some() {
        ensure!(
            executor.executor == executors::executors::BaseCodingAgent::Codex,
            super::errors::rejection_with_message(
                utils::mcp_error::Category::Validation,
                "Explicit execution_mode requires CODEX; Claude Code does not support these modes"
            )
        );
    }
    if let Some(mode) = args.execution_mode {
        executor.execution_mode = Some(mode);
        if !matches!(mode, ExecutionMode::Goal | ExecutionMode::PlanWithGoal) {
            executor.goal_token_budget = None;
            executor.goal_max_concurrent_agents = None;
        }
    }
    if let Some(budget) = args.goal_token_budget {
        ensure!(
            budget > 0
                && matches!(
                    executor.execution_mode,
                    Some(ExecutionMode::Goal | ExecutionMode::PlanWithGoal)
                ),
            super::errors::rejection_with_message(
                utils::mcp_error::Category::Validation,
                "A positive token budget requires Goal or PlanWithGoal mode"
            )
        );
        executor.goal_token_budget = Some(budget);
    }
    let mut prompt = format!(
        "{}\n\n{}\n\nLVK MCP operation: {workspace_id}",
        issue.title,
        if executors::executors::provider_adapter::DirectProvider::from_base_agent(
            executor.executor
        )
        .is_some_and(|provider| {
            executors::executors::provider_adapter::memory_source_completion_allowed(
                provider,
                executors::executors::provider_adapter::DirectIntent::Initial,
                &executor,
                description,
            )
        }) {
            executors::executors::provider_adapter::saved_issue_description_for_prompt(description)
        } else {
            description.to_owned()
        }
    );
    for id in &args.attachment_ids {
        let file = db::models::file::File::find_by_id(pool, *id)
            .await?
            .ok_or_else(|| super::errors::rejection(Category::NotFound))?;
        prompt.push_str(&format!(
            "\n\nAttachment: `.lvk/attachments/{}`",
            file.file_path
        ));
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
        prompt,
        selected_skills: None,
        resume_session_id: None,
        resume_scope_path: None,
        attachment_ids: (!args.attachment_ids.is_empty()).then(|| args.attachment_ids.clone()),
    })
}

impl Remote {
    async fn repositories(&self, args: RepositoriesArgs) -> anyhow::Result<Value> {
        self.require_project(args.project_id).await?;
        ensure!(
            (1..=50).contains(&args.limit),
            super::errors::rejection_with_message(
                utils::mcp_error::Category::Validation,
                "limit must be between 1 and 50"
            )
        );
        let mut ids =
            local_remote::project_associated_repository_ids(&self.pool, args.project_id).await?;
        ids.sort();
        if let Some(cursor) = args.cursor {
            ensure!(
                ids.contains(&cursor),
                super::errors::rejection_with_message(
                    utils::mcp_error::Category::Conflict,
                    "Stale or foreign repository cursor; restart listing"
                )
            );
        }
        let preset_ids =
            local_remote::project_creation_repository_ids(&self.pool, args.project_id).await?;
        let integration_ids =
            integrations::project_repository_ids(&self.pool, args.project_id).await?;
        let mut items = Vec::new();
        for id in ids
            .into_iter()
            .filter(|id| args.cursor.is_none_or(|cursor| *id > cursor))
            .take(args.limit as usize + 1)
        {
            if let Some(repo) = Repo::find_by_id(&self.pool, id).await? {
                items.push(json!({"id":repo.id,"name":repo.display_name.chars().take(500).collect::<String>(),"name_truncated":repo.display_name.chars().count()>500,"default_target_branch":repo.default_target_branch,"integration_member":integration_ids.contains(&id),"creation_preset_member":preset_ids.contains(&id),"url":self.config.project_url(args.project_id)}));
            }
        }
        Ok(page(items, args.limit))
    }
    async fn branches(&self, args: BranchArgs) -> anyhow::Result<Value> {
        self.require_project(args.project_id).await?;
        ensure!(
            (1..=50).contains(&args.limit),
            super::errors::rejection_with_message(
                utils::mcp_error::Category::Validation,
                "limit must be between 1 and 50"
            )
        );
        ensure!(
            local_remote::project_associated_repository_ids(&self.pool, args.project_id)
                .await?
                .contains(&args.repository_id),
            super::errors::rejection_with_message(
                utils::mcp_error::Category::Permission,
                "Repository is not in this Project"
            )
        );
        let repo = Repo::find_by_id(&self.pool, args.repository_id)
            .await?
            .ok_or_else(|| super::errors::rejection(Category::NotFound))?;
        let prefix = format!("{}:", args.repository_id);
        let after = args
            .cursor
            .as_deref()
            .map(|cursor| {
                cursor
                    .strip_prefix(&prefix)
                    .map(str::to_owned)
                    .ok_or_else(|| super::errors::rejection(Category::Validation))
            })
            .transpose()?;
        let items = tokio::task::spawn_blocking(move || -> anyhow::Result<Vec<Value>> {
            let git = git::GitService::new();
            let mut branches = git.get_all_branches(&repo.path).map_err(|_|super::errors::rejection(Category::Unavailable))?;
            branches.retain(|b| !b.is_remote); branches.sort_by(|a,b|a.name.cmp(&b.name));
            ensure!(after.as_ref().is_none_or(|name|branches.iter().any(|b| &b.name == name)), super::errors::rejection_with_message(utils::mcp_error::Category::Conflict, "Stale branch cursor; restart listing"));
            branches.into_iter().filter(|b|after.as_ref().is_none_or(|after|b.name > *after)).take(args.limit as usize+1).map(|b| {
                let head=git.get_branch_oid(&repo.path,&b.name).map_err(|_|super::errors::rejection(Category::Conflict))?;
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
            .ok_or_else(|| super::errors::rejection(Category::NotFound))?;
        ensure!(
            !workspace.is_direct_folder(),
            super::errors::rejection_with_message(
                Category::Validation,
                "Direct folders do not support Auto-merge"
            )
        );
        let membership = WorkspaceRepo::find_by_workspace_and_repo_id(
            &self.pool,
            args.workspace_id,
            args.repository_id,
        )
        .await?
        .ok_or_else(|| super::errors::rejection(Category::Permission))?;
        ensure!(
            membership.target_branch == args.target_branch.trim_start_matches("refs/heads/"),
            super::errors::rejection_with_message(
                Category::Validation,
                "Select the merge target recorded by this Workspace"
            )
        );
        let repo = Repo::find_by_id(&self.pool, args.repository_id)
            .await?
            .ok_or_else(|| super::errors::rejection(Category::NotFound))?;
        let target = args.target_branch.clone();
        let (head, target_head) = tokio::task::spawn_blocking(move || {
            let git = git::GitService::new();
            Ok::<_, git::GitServiceError>((
                git.get_branch_oid(&repo.path, &workspace.branch)?,
                git.get_branch_oid(&repo.path, &target)?,
            ))
        })
        .await?
        .map_err(|_| super::errors::rejection(Category::Unavailable))?;
        Ok(
            json!({"project_id":args.project_id,"issue_id":issue,"workspace_id":args.workspace_id,"repository_id":args.repository_id,"source_commit":head,"target_branch":args.target_branch,"target_commit":target_head,"eligibility":"not_validated","formal_validation_on_start":true,"url":self.config.workspace_url(args.project_id,issue,args.workspace_id)}),
        )
    }
}
fn validation_error(error: anyhow::Error) -> (&'static str, Value) {
    let message = super::errors::message(&error);
    (
        "failed",
        json!({"code":"admission_validation_failed","message":message,"error":super::errors::diagnostic(&error, false).value()}),
    )
}
pub(super) async fn call(
    remote: &Remote,
    principal: &Principal,
    name: &str,
    input: Value,
) -> anyhow::Result<Value> {
    ensure!(
        principal.allows("lvk:read"),
        super::errors::rejection_with_message(
            utils::mcp_error::Category::Permission,
            "Read permission required"
        )
    );
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
                .ok_or_else(|| super::errors::rejection(Category::Unavailable))?;
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
        super::errors::rejection_with_message(
            utils::mcp_error::Category::Permission,
            "Required operation scope is missing"
        )
    );
    let launch = remote
        .launch
        .clone()
        .ok_or_else(|| super::errors::rejection(Category::Unavailable))?;
    let owned = remote.clone();
    let actor = remote.principal_key(principal);
    let receipt = match name {
        "start_session" => {
            let args: StartSessionArgs = arguments(input.clone())?;
            remote
                .require_workspace(args.project_id, args.workspace_id)
                .await?;
            ensure!(
                !args.prompt.trim().is_empty() && args.prompt.len() <= 256000,
                super::errors::rejection_with_message(
                    Category::Validation,
                    "Provide a nonempty prompt of at most 256000 bytes"
                )
            );
            remote.operations().submit_and_wait(actor,args.request_id,args.project_id,name.into(),input,json!({"workspace_id":args.workspace_id}),move|_|async move {
                let issue = match owned.require_workspace(args.project_id,args.workspace_id).await { Ok(issue)=>issue,Err(error)=>return validation_error(error) };
                let config = select_engine(launch.defaults().await, args.engine);
                let url=owned.config.workspace_url(args.project_id,issue,args.workspace_id);
                match launch.start_session(args.workspace_id,args.prompt,config).await {
                    Ok(mut value)=>{value["url"]=json!(url);("succeeded",value)},
                    Err(_)=>("uncertain",json!({"workspace_id":args.workspace_id,"url":url,"message":"Inspect Workspace Sessions and runs before any new start request; Session creation or execution may have completed"})),
                }
            }).await?
        }
        "start_workspace" => {
            let args: StartArgs = arguments(input.clone())?;
            remote.require_issue(args.project_id, args.issue_id).await?;
            crate::routes::attachment_transfer::validate_ids(
                &remote.pool,
                args.project_id,
                &args.attachment_ids,
            )
            .await?;
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
                super::errors::rejection_with_message(
                    utils::mcp_error::Category::Validation,
                    "Select 1–100 Issue/Workspace pairs"
                )
            );
            let mut issues = HashSet::new();
            let mut workspaces = HashSet::new();
            for selection in &args.selections {
                commit(&selection.expected_commit)?;
                ensure!(
                    issues.insert(selection.issue_id) && workspaces.insert(selection.workspace_id),
                    super::errors::rejection_with_message(
                        utils::mcp_error::Category::Validation,
                        "Select exactly one adopted Workspace per Issue"
                    )
                );
            }
            remote.operations().submit_and_wait(actor,args.request_id,args.project_id,name.into(),input,json!({"repository_id":args.repository_id,"workspace_ids":args.selections.iter().map(|s|s.workspace_id).collect::<Vec<_>>()}),move|operation|async move {
                let permitted = integrations::project_repository_ids(&owned.pool,args.project_id).await;
                if !permitted.is_ok_and(|ids|ids.contains(&args.repository_id)) {return ("failed",json!({"code":"repository_scope_changed"}));}
                for selection in &args.selections {
                    if !WorkspaceRepo::find_by_workspace_and_repo_id(&owned.pool,selection.workspace_id,args.repository_id).await.is_ok_and(|repo|repo.is_some()) {return ("failed",json!({"code":"workspace_repository_changed"}));}
                    match owned.require_workspace(args.project_id,selection.workspace_id).await {Ok(issue) if issue==selection.issue_id=>{},_=>return ("failed",json!({"code":"selection_scope_changed"}))}
                }
                let request=integrations::CreateIntegrationRequest {request_id:operation,project_id:args.project_id,repository_id:args.repository_id,target_branch:args.target_branch,selections:args.selections.into_iter().map(|s|IntegrationSelection {card_id:s.issue_id,workspace_id:s.workspace_id,expected_commit:s.expected_commit}).collect(),instructions: args.instructions, executor_config:None,skip_validation:args.skip_validation};
                let url=owned.config.project_url(args.project_id);
                match launch.integration(request,args.expected_target_commit).await {
                    Ok(mut value)=>{value["url"]=json!(url);("succeeded",value)},
                    Err(integrations::AdmissionFailure::Rejected {code,message})=>("failed",json!({"code":code,"message":message,"integration_request_id":operation,"integration_accepted":false,"url":url})),
                    Err(integrations::AdmissionFailure::Uncertain)=>("uncertain",json!({"integration_request_id":operation,"url":url,"message":"Inspect Auto-merge records before any new request; admission may have completed"})),
                }
            }).await?
        }
        _ => return Err(super::errors::rejection(Category::Validation)),
    };
    Ok(remote.receipt_value(receipt))
}

#[cfg(test)]
mod engine_tests {
    use super::*;

    #[test]
    fn omitted_or_same_engine_preserves_default_profile() {
        let mut config = ExecutorConfig::new(executors::executors::BaseCodingAgent::Codex);
        config.variant = Some("SAVED".into());
        config.model_id = Some("saved-model".into());
        for engine in [None, Some(Engine::Codex)] {
            assert_eq!(
                serde_json::to_value(select_engine(config.clone(), engine)).unwrap(),
                serde_json::to_value(&config).unwrap()
            );
        }
    }

    #[test]
    fn switching_engine_uses_new_provider_profile_without_foreign_overrides() {
        let mut config = ExecutorConfig::new(executors::executors::BaseCodingAgent::Codex);
        config.variant = Some("SAVED".into());
        config.execution_mode = Some(ExecutionMode::Review);
        config.model_id = Some("codex-model".into());
        let selected = select_engine(config, Some(Engine::ClaudeCode));
        assert_eq!(
            serde_json::to_value(selected).unwrap(),
            serde_json::to_value(ExecutorConfig::new(
                executors::executors::BaseCodingAgent::ClaudeCode
            ))
            .unwrap()
        );
    }

    #[test]
    fn engine_contract_rejects_unknown_engines_and_existing_session_target() {
        assert!(serde_json::from_value::<Engine>(json!("OTHER")).is_err());
        assert!(
            serde_json::from_value::<StartSessionArgs>(json!({
                "project_id":Uuid::new_v4(), "workspace_id":Uuid::new_v4(),
                "request_id":Uuid::new_v4(), "prompt":"handoff", "session_id":Uuid::new_v4()
            }))
            .is_err()
        );
    }
}
