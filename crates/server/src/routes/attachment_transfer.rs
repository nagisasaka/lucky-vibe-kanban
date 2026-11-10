//! Shared Remote/stdio attachment admission. Saving and placing never start a run;
//! explicit start/continue routes delegate to existing execution admission.
use std::{
    net::{IpAddr, SocketAddr},
    time::Duration,
};

use anyhow::{Context, ensure};
use axum::{Json, Router, extract::State, routing::post};
use base64::Engine;
use db::models::{
    file::{File, WorkspaceAttachment},
    workspace::Workspace,
};
use deployment::Deployment;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::{DeploymentImpl, error::ApiError, remote_mcp::operations::Operations};

pub const INLINE_LIMIT: usize = 2 * 1024 * 1024;
pub const FILE_LIMIT: usize = 20 * 1024 * 1024;
#[derive(Clone, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct HostFile {
    pub download_url: String,
    pub file_id: String,
    pub mime_type: Option<String>,
    pub file_name: Option<String>,
}
#[derive(Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Upload {
    pub project_id: Uuid,
    pub request_id: Uuid,
    /// Display name, never a path. Use the host filename when available.
    pub original_name: String,
    #[schemars(length(max = 2796204))]
    pub data_base64: Option<String>,
    pub file: Option<HostFile>,
    pub expected_sha256: Option<String>,
}
#[derive(Clone, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Place {
    pub project_id: Uuid,
    pub workspace_id: Uuid,
    pub session_id: Uuid,
    pub request_id: Uuid,
    pub attachment_ids: Vec<Uuid>,
}
pub async fn require_project(pool: &sqlx::SqlitePool, project: Uuid) -> anyhow::Result<()> {
    ensure!(
        sqlx::query_scalar::<_, bool>("SELECT EXISTS(SELECT 1 FROM projects WHERE id=?)")
            .bind(project)
            .fetch_one(pool)
            .await?,
        "Project not found"
    );
    Ok(())
}
pub async fn validate_ids(
    pool: &sqlx::SqlitePool,
    project: Uuid,
    ids: &[Uuid],
) -> anyhow::Result<()> {
    require_project(pool, project).await?;
    ensure!(ids.len() <= 20, "At most 20 attachments per operation");
    let mut unique = std::collections::HashSet::new();
    for id in ids {
        ensure!(unique.insert(*id), "Duplicate attachment IDs");
        ensure!(sqlx::query_scalar::<_, bool>("SELECT EXISTS(SELECT 1 FROM project_attachments WHERE project_id=?1 AND attachment_id=?2 UNION ALL SELECT 1 FROM workspace_attachments wa JOIN local_workspace_links l ON l.workspace_id=wa.workspace_id JOIN local_issues i ON i.id=l.issue_id AND i.project_id=l.project_id WHERE l.project_id=?1 AND wa.attachment_id=?2)").bind(project).bind(id).fetch_one(pool).await?, "Attachment not available in this Project");
    }
    Ok(())
}
fn public_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v) => {
            !v.is_private()
                && !v.is_loopback()
                && !v.is_link_local()
                && !v.is_unspecified()
                && !v.is_broadcast()
                && !v.is_multicast()
                && !v.is_documentation()
                && v.octets()[0] != 0
                && v.octets()[0] < 224
                && !(v.octets()[0] == 192 && v.octets()[1] == 0)
                && !(v.octets()[0] == 198 && (18..=19).contains(&v.octets()[1]))
                && !(v.octets()[0] == 100 && (64..=127).contains(&v.octets()[1]))
        }
        IpAddr::V6(v) => {
            (v.segments()[0] & 0xe000) == 0x2000
                && !v.is_loopback()
                && !v.is_unspecified()
                && v.to_ipv4_mapped().is_none()
                && v.segments()[0] != 0x2002
                && !(v.segments()[0] == 0x2001 && v.segments()[1] < 0x0200)
                && !(v.segments()[0] == 0x2001 && v.segments()[1] == 0xdb8)
        }
    }
}
// This type can contain only a bounded, parsed hostname, never a URL or source error.
#[derive(Debug, thiserror::Error)]
#[error("Host file domain is not enabled (hostname: {hostname})")]
struct DisabledHost {
    hostname: String,
}

pub(crate) fn host_file_diagnostic(error: &anyhow::Error) -> Option<String> {
    error
        .downcast_ref::<DisabledHost>()
        .map(ToString::to_string)
}

fn diagnostic_hostname(url: &url::Url) -> Option<&str> {
    url.host_str().filter(|host| {
        !host.is_empty()
            && host.len() <= 253
            && host
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.'))
            && host
                .strip_suffix('.')
                .unwrap_or(host)
                .split('.')
                .all(|label| !label.is_empty() && label.len() <= 63)
    })
}

fn host_url(raw: &str) -> anyhow::Result<url::Url> {
    let url = url::Url::parse(raw).map_err(|_| anyhow::anyhow!("Invalid host file URL"))?;
    ensure!(
        url.scheme() == "https"
            && url.port_or_known_default() == Some(443)
            && url.username().is_empty()
            && url.password().is_none()
            && url.fragment().is_none(),
        "Invalid host file URL"
    );
    // Operator-owned, exact host allowlist; never accept caller-provided hosts.
    let allowed = std::env::var("LVK_ATTACHMENT_FILE_HOSTS")
        .unwrap_or_else(|_| "files.oaiusercontent.com".into());
    if !url
        .host_str()
        .is_some_and(|h| allowed.split(',').any(|a| a.trim() == h))
    {
        if let Some(hostname) = diagnostic_hostname(&url) {
            return Err(DisabledHost {
                hostname: hostname.to_owned(),
            }
            .into());
        }
        anyhow::bail!("Host file domain is not enabled");
    }
    Ok(url)
}
async fn download(file: &HostFile) -> anyhow::Result<Vec<u8>> {
    ensure!(
        !file.file_id.is_empty() && file.file_id.len() <= 256,
        "Invalid host file identity"
    );
    let url = host_url(&file.download_url)?;
    let host = url.host_str().context("Missing host")?;
    let addrs: Vec<SocketAddr> =
        tokio::time::timeout(Duration::from_secs(5), tokio::net::lookup_host((host, 443)))
            .await??
            .collect();
    ensure!(
        !addrs.is_empty() && addrs.iter().all(|a| public_ip(a.ip())),
        "Unsafe host file address"
    );
    let client = reqwest::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(30))
        .resolve_to_addrs(host, &addrs)
        .build()?;
    // Never log reqwest errors: they may contain a signed URL.
    let mut response = client
        .get(url.clone())
        .send()
        .await
        .map_err(|_| anyhow::anyhow!("Host file download failed"))?;
    ensure!(
        response.status().is_success() && response.url() == &url,
        "Host file download rejected"
    );
    ensure!(
        response
            .content_length()
            .is_none_or(|n| n <= FILE_LIMIT as u64),
        "Host file too large"
    );
    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| anyhow::anyhow!("Host file download failed"))?
    {
        ensure!(
            bytes.len() + chunk.len() <= FILE_LIMIT,
            "Host file too large"
        );
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}
fn decode_inline(encoded: &str) -> anyhow::Result<Vec<u8>> {
    ensure!(
        encoded.len() <= INLINE_LIMIT.div_ceil(3) * 4,
        "Inline attachment too large"
    );
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .context("Invalid base64")?;
    ensure!(bytes.len() <= INLINE_LIMIT, "Inline attachment too large");
    Ok(bytes)
}
fn store(deployment: &DeploymentImpl) -> Operations {
    Operations {
        pool: deployment.db().pool.clone(),
        process_instance: *crate::remote_mcp::PROCESS_INSTANCE.get_or_init(Uuid::new_v4),
    }
}
pub async fn upload(
    deployment: DeploymentImpl,
    args: Upload,
    actor: String,
) -> anyhow::Result<Value> {
    require_project(&deployment.db().pool, args.project_id).await?;
    ensure!(
        !args.original_name.is_empty()
            && args.original_name.len() <= 255
            && !args
                .original_name
                .chars()
                .any(|c| c.is_control() || matches!(c, '/' | '\\')),
        "Invalid filename"
    );
    let bytes = match (&args.data_base64, &args.file) {
        (Some(encoded), None) => decode_inline(encoded)?,
        (None, Some(file)) => download(file).await?,
        _ => anyhow::bail!("Supply exactly one of file or data_base64"),
    };
    let hash = format!("{:x}", Sha256::digest(&bytes));
    ensure!(
        args.expected_sha256.as_ref().is_none_or(|h| h == &hash),
        "Attachment hash mismatch"
    );
    // The content digest, not the expiring URL or bytes, is the durable request identity.
    let input =
        json!({"project_id":args.project_id,"original_name":args.original_name,"sha256":hash});
    let receipt=store(&deployment).submit_and_wait(actor,args.request_id,args.project_id,"upload_attachment".into(),input,json!({"project_id":args.project_id}),move |_| async move {
        let result: anyhow::Result<Value> = async {
            let file=deployment.file().store_file(&bytes,&args.original_name).await?;
            sqlx::query("INSERT OR IGNORE INTO project_attachments(project_id,attachment_id) VALUES(?,?)").bind(args.project_id).bind(file.id).execute(&deployment.db().pool).await?;
            Ok(json!({"attachment":super::attachments::AttachmentResponse::from_file(file),"saved":true,"placed":false,"execution_started":false}))
        }.await;
        match result { Ok(value)=>("succeeded",value),Err(_)=>("failed",json!({"code":"attachment_save_not_confirmed","saved":null,"placed":false,"execution_started":false})) }
    }).await?;
    Ok(receipt_value(receipt))
}
pub async fn place_now(deployment: &DeploymentImpl, args: &Place) -> anyhow::Result<Value> {
    require_project(&deployment.db().pool, args.project_id).await?;
    let belongs=sqlx::query_scalar::<_,bool>("SELECT EXISTS(SELECT 1 FROM local_workspace_links l JOIN local_issues i ON i.id=l.issue_id AND i.project_id=l.project_id WHERE l.project_id=? AND l.workspace_id=?)").bind(args.project_id).bind(args.workspace_id).fetch_one(&deployment.db().pool).await?;
    ensure!(belongs, "Workspace not in Project");
    let workspace = Workspace::find_by_id(&deployment.db().pool, args.workspace_id)
        .await?
        .context("Workspace missing")?;
    workspace.require_interactive()?;
    ensure!(!workspace.archived, "Workspace is archived");
    validate_ids(&deployment.db().pool, args.project_id, &args.attachment_ids).await?;
    let base = super::workspaces::attachments::resolve_session_base_path(
        deployment,
        &workspace,
        args.session_id,
    )
    .await?;
    let mut results = vec![];
    for id in &args.attachment_ids {
        let file = File::find_by_id(&deployment.db().pool, *id)
            .await?
            .context("Attachment missing")?;
        let mut result = json!({"attachment":super::attachments::AttachmentResponse::from_file(file),"saved":true,"associated":false,"verified":false});
        if deployment
            .file()
            .copy_files_by_ids_to_worktree(&base, &[*id])
            .await
            .is_err()
        {
            result["code"] = json!("placement_failed");
        } else {
            result["verified"] = json!(true);
            match WorkspaceAttachment::associate_many_dedup(
                &deployment.db().pool,
                workspace.id,
                &[*id],
            )
            .await
            {
                Ok(()) => result["associated"] = json!(true),
                Err(_) => result["code"] = json!("association_failed"),
            }
        }
        results.push(result);
    }
    Ok(
        json!({"all_verified":results.iter().all(|r| r["verified"]==true && r["associated"]==true),"results":results,"workspace_shared":true,"session_id":args.session_id,"execution_started":false}),
    )
}
pub async fn place(
    deployment: DeploymentImpl,
    args: Place,
    actor: String,
) -> anyhow::Result<Value> {
    // Validate ownership before admitting any filesystem work.
    require_project(&deployment.db().pool, args.project_id).await?;
    validate_ids(&deployment.db().pool, args.project_id, &args.attachment_ids).await?;
    let receipt = store(&deployment)
        .submit_and_wait(
            actor,
            args.request_id,
            args.project_id,
            "place_attachments".into(),
            serde_json::to_value(&args)?,
            json!({"workspace_id":args.workspace_id,"session_id":args.session_id}),
            move |_| async move {
                match place_now(&deployment, &args).await {
                    Ok(v) if v["all_verified"] == true => ("succeeded", v),
                    Ok(v) => ("failed", v),
                    Err(_) => (
                        "failed",
                        json!({"code":"placement_rejected","execution_started":false}),
                    ),
                }
            },
        )
        .await?;
    Ok(receipt_value(receipt))
}
#[async_trait::async_trait]
pub(crate) trait Execution: Send + Sync {
    async fn start(
        &self,
        request: db::models::requests::CreateAndStartWorkspaceRequest,
        workspace_id: Uuid,
    ) -> Result<Value, ApiError>;
    async fn continue_session(
        &self,
        placement: &Place,
        prompt: String,
        latest: Uuid,
    ) -> Result<Value, ApiError>;
}
struct ExistingExecution(DeploymentImpl);
#[async_trait::async_trait]
impl Execution for ExistingExecution {
    async fn start(
        &self,
        request: db::models::requests::CreateAndStartWorkspaceRequest,
        workspace_id: Uuid,
    ) -> Result<Value, ApiError> {
        let r = super::workspaces::create::create_and_start_workspace_admitted(
            self.0.clone(),
            request,
            Some(workspace_id),
        )
        .await?
        .0
        .into_data()
        .ok_or_else(|| ApiError::Conflict("Execution receipt unavailable".into()))?;
        Ok(
            json!({"workspace_id":r.workspace.id,"session_id":r.agent_run.state.session_id,"agent_run_id":r.agent_run.agent_run_id,"execution_accepted":true}),
        )
    }
    async fn continue_session(
        &self,
        p: &Place,
        prompt: String,
        latest: Uuid,
    ) -> Result<Value, ApiError> {
        let d = &self.0;
        let session = db::models::session::Session::find_by_id(&d.db().pool, p.session_id)
            .await?
            .ok_or_else(|| ApiError::BadRequest("Session missing".into()))?;
        let executor_config =
            super::sessions::executor_config::read_executor_config(&d.db().pool, p.session_id)
                .await?
                .ok_or_else(|| ApiError::BadRequest("Executor missing".into()))?;
        let r = super::sessions::follow_up_checked(
            d.clone(),
            session,
            super::sessions::CreateFollowUpAttempt {
                prompt,
                selected_skills: None,
                executor_config,
                resume_session_id: None,
                resume_scope_path: None,
            },
            Some((latest, Some(p.project_id))),
        )
        .await?
        .0
        .into_data()
        .ok_or_else(|| ApiError::Conflict("Execution receipt unavailable".into()))?;
        Ok(json!({"agent_run_id":r.agent_run_id,"execution_accepted":true}))
    }
}
#[derive(Deserialize, Serialize)]
struct Continue {
    #[serde(flatten)]
    placement: Place,
    prompt: String,
}
async fn continue_route(
    State(d): State<DeploymentImpl>,
    axum::Extension(execution): axum::Extension<std::sync::Arc<dyn Execution>>,
    Json(args): Json<Continue>,
) -> Result<Json<utils::response::ApiResponse<Value>>, ApiError> {
    let p = &args.placement;
    validate_ids(&d.db().pool, p.project_id, &p.attachment_ids)
        .await
        .map_err(|_| ApiError::BadRequest("Attachments unavailable".into()))?;
    let receipt=store(&d).submit_and_wait(actor(true)?,p.request_id,p.project_id,"continue_with_attachments".into(),serde_json::to_value(&args).map_err(|_|ApiError::BadRequest("Invalid input".into()))?,json!({"workspace_id":p.workspace_id,"session_id":p.session_id}),move |_|async move {
        let p=args.placement;
        let placed=match place_now(&d,&p).await {Ok(v)=>v,Err(_)=>return ("failed",json!({"code":"placement_rejected","execution_accepted":false}))};
        if placed["all_verified"]!=true {return ("failed",json!({"placement":placed,"execution_accepted":false}));}
        let latest=sqlx::query_as::<_,(Uuid,String)>("SELECT id,status FROM agent_runs WHERE session_id=? ORDER BY created_at DESC,rowid DESC LIMIT 1").bind(p.session_id).fetch_optional(&d.db().pool).await;
        let Ok(Some((latest,status)))=latest else {return ("failed",json!({"code":"latest_run_unavailable","execution_accepted":false}));};
        if !matches!(status.as_str(),"succeeded"|"failed"|"cancelled"|"crashed"|"audit_failed") {return ("failed",json!({"code":"run_not_terminal","execution_accepted":false}));}
        let mut prompt=args.prompt;
        for r in placed["results"].as_array().into_iter().flatten() {
            if let Some(path)=r["attachment"]["file_path"].as_str() {prompt.push_str(&format!("\n\nAttachment: `{path}`"));}
        }
        match execution.continue_session(&p,prompt,latest).await {
            Ok(v)=>("succeeded",v),Err(_)=>("uncertain",json!({"code":"continuation_not_confirmed","message":"Inspect the selected Session before retrying"}))
        }
    }).await.map_err(|_|ApiError::Conflict("Inspect original request before retrying".into()))?;
    Ok(Json(utils::response::ApiResponse::success(receipt_value(
        receipt,
    ))))
}
fn receipt_value(receipt: crate::remote_mcp::operations::Receipt) -> Value {
    json!({"operation_id":receipt.id,"request_id":receipt.request_id,"project_id":receipt.project_id,"status":receipt.status,"result":receipt.result.and_then(|s|serde_json::from_str::<Value>(&s).ok())})
}
#[derive(Deserialize, Serialize)]
struct Start {
    request_id: Uuid,
    request: db::models::requests::CreateAndStartWorkspaceRequest,
}
async fn start_route(
    State(d): State<DeploymentImpl>,
    axum::Extension(execution): axum::Extension<std::sync::Arc<dyn Execution>>,
    Json(args): Json<Start>,
) -> Result<Json<utils::response::ApiResponse<Value>>, ApiError> {
    let project = args
        .request
        .linked_issue
        .as_ref()
        .ok_or_else(|| ApiError::BadRequest("Attachments require a linked Issue".into()))?
        .remote_project_id;
    let issue = args.request.linked_issue.as_ref().unwrap().issue_id;
    let belongs: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM local_issues WHERE id=? AND project_id=?)")
            .bind(issue)
            .bind(project)
            .fetch_one(&d.db().pool)
            .await?;
    if !belongs {
        return Err(ApiError::BadRequest("Issue not in selected Project".into()));
    }
    validate_ids(
        &d.db().pool,
        project,
        args.request.attachment_ids.as_deref().unwrap_or_default(),
    )
    .await
    .map_err(|_| ApiError::BadRequest("Attachments unavailable".into()))?;
    let receipt=store(&d).submit_and_wait(actor(true)?,args.request_id,project,"start_with_attachments".into(),serde_json::to_value(&args).map_err(|_|ApiError::BadRequest("Invalid request".into()))?,json!({"project_id":project}),move |operation|async move {
        let mut request=args.request;
        for id in request.attachment_ids.as_deref().unwrap_or_default() {
            let file=match File::find_by_id(&d.db().pool,*id).await {Ok(Some(f))=>f,_=>return ("failed",json!({"code":"attachment_missing","execution_accepted":false}))};
            request.prompt.push_str(&format!("\n\nAttachment: `.lvk/attachments/{}`",file.file_path));
        }
        match execution.start(request,operation).await {
            Ok(v)=>("succeeded",v),
            Err(_)=>("uncertain",json!({"workspace_id":operation,"message":"Inspect this Workspace before any new start request"})),
        }
    }).await.map_err(|_|ApiError::Conflict("Inspect original request before retrying".into()))?;
    Ok(Json(utils::response::ApiResponse::success(receipt_value(
        receipt,
    ))))
}
#[derive(Deserialize)]
struct Status {
    project_id: Uuid,
    request_id: Uuid,
}
async fn status_route(
    State(d): State<DeploymentImpl>,
    Json(args): Json<Status>,
) -> Result<Json<utils::response::ApiResponse<Value>>, ApiError> {
    let receipt = store(&d)
        .by_request(&actor(false)?, args.request_id)
        .await
        .map_err(|_| ApiError::BadRequest("Operation unavailable".into()))?;
    if receipt.project_id != args.project_id {
        return Err(ApiError::BadRequest("Operation not in Project".into()));
    }
    Ok(Json(utils::response::ApiResponse::success(receipt_value(
        receipt,
    ))))
}
fn actor(write: bool) -> Result<String, ApiError> {
    server_access::ACTOR
        .try_with(|a| {
            if !a.scopes.contains("lvk:read") || (write && !a.scopes.contains("lvk:write")) {
                return Err(ApiError::Forbidden(
                    "Attachment permissions required".into(),
                ));
            }
            Ok(json!(["attachment", a.issuer, a.subject]).to_string())
        })
        .map_err(|_| ApiError::Forbidden("Authenticated attachment actor required".into()))?
}
async fn upload_route(
    State(d): State<DeploymentImpl>,
    Json(args): Json<Upload>,
) -> Result<Json<utils::response::ApiResponse<Value>>, ApiError> {
    upload(d, args, actor(true)?)
        .await
        .map(|v| Json(utils::response::ApiResponse::success(v)))
        .map_err(|error| {
            ApiError::BadRequest(
                host_file_diagnostic(&error).unwrap_or_else(|| "Attachment upload rejected".into()),
            )
        })
}
async fn place_route(
    State(d): State<DeploymentImpl>,
    Json(args): Json<Place>,
) -> Result<Json<utils::response::ApiResponse<Value>>, ApiError> {
    place(d, args, actor(true)?)
        .await
        .map(|v| Json(utils::response::ApiResponse::success(v)))
        .map_err(|_| ApiError::BadRequest("Attachment placement rejected".into()))
}
pub fn router(deployment: &DeploymentImpl) -> Router<DeploymentImpl> {
    router_with_execution(std::sync::Arc::new(ExistingExecution(deployment.clone())))
}
pub(crate) fn router_with_execution(
    execution: std::sync::Arc<dyn Execution>,
) -> Router<DeploymentImpl> {
    Router::new()
        .route("/attachment-transfer/upload", post(upload_route))
        .route("/attachment-transfer/start", post(start_route))
        .route("/attachment-transfer/status", post(status_route))
        .route("/attachment-transfer/place", post(place_route))
        .route("/attachment-transfer/continue", post(continue_route))
        .layer(axum::extract::DefaultBodyLimit::max(4 * 1024 * 1024))
        .layer(axum::Extension(execution))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Default)]
    struct DiagnosticLogs(std::sync::Arc<std::sync::Mutex<Vec<u8>>>);
    impl std::io::Write for DiagnosticLogs {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn attachment_host_diagnostic_exposes_only_bounded_normalized_hostname() {
        let logs = DiagnosticLogs::default();
        let writer = logs.clone();
        let subscriber = tracing_subscriber::fmt()
            .without_time()
            .with_ansi(false)
            .with_max_level(tracing::Level::TRACE)
            .with_writer(move || writer.clone())
            .finish();
        tracing::subscriber::with_default(subscriber, || {
            for (raw, host) in [
                (
                    "https://REJECTED.Example.invalid/private-material?signature=synthetic-secret",
                    "rejected.example.invalid",
                ),
                (
                    "https://BÜCHER.invalid/private-material?signature=synthetic-secret",
                    "xn--bcher-kva.invalid",
                ),
            ] {
                let Err(error) = host_url(raw) else {
                    panic!("fixture must be rejected");
                };
                let expected = format!("Host file domain is not enabled (hostname: {host})");
                assert!(error.to_string() == expected);
                assert!(host_file_diagnostic(&error).as_deref() == Some(expected.as_str()));
                // Even generic Debug/alternate-chain rendering has no original input.
                for rendered in [format!("{error:?}"), format!("{error:#}")] {
                    assert!(!rendered.contains("private-material"));
                    assert!(!rendered.contains("synthetic-secret"));
                    assert!(!rendered.contains("https://"));
                }
                let wrapped = error.context("synthetic-private-context");
                assert!(host_file_diagnostic(&wrapped).as_deref() == Some(expected.as_str()));
            }
        });
        assert!(logs.0.lock().unwrap().is_empty());
    }

    #[test]
    fn attachment_host_diagnostic_omits_unsafe_authorities_and_parser_errors() {
        let label = "a".repeat(64);
        let huge = (0..5).map(|_| "b".repeat(63)).collect::<Vec<_>>().join(".");
        for raw in [
            "https://synthetic-user:synthetic-password@rejected.invalid/private-material?signature=synthetic-secret".to_owned(),
            "https://rejected.invalid/private-material?signature=synthetic-secret#synthetic-fragment".to_owned(),
            "http://rejected.invalid/private-material?signature=synthetic-secret".to_owned(),
            "https://rejected.invalid:444/private-material?signature=synthetic-secret".to_owned(),
            "https://bad%0d%0ahost.invalid/private-material?signature=synthetic-secret".to_owned(),
            "https://bad%2fhost.invalid/private-material?signature=synthetic-secret".to_owned(),
            "https://[malformed/private-material?signature=synthetic-secret".to_owned(),
            format!("https://{label}.invalid/private-material?signature=synthetic-secret"),
            format!("https://{huge}/private-material?signature=synthetic-secret"),
        ] {
            let Err(error) = host_url(&raw) else { panic!("unsafe fixture must be rejected"); };
            assert!(host_file_diagnostic(&error).is_none());
            for rendered in [format!("{error:?}"), format!("{error:#}"), error.to_string()] {
                for forbidden in ["private-material", "synthetic-secret", "synthetic-user", "synthetic-password", "synthetic-fragment", "https://", "http://"] {
                    assert!(!rendered.contains(forbidden));
                }
                assert!(rendered.len() < 200);
            }
        }
    }

    #[test]
    fn attachment_inline_and_host_validation_boundaries() {
        use base64::{Engine, engine::general_purpose::STANDARD};
        assert_eq!(
            decode_inline(&STANDARD.encode(vec![255; INLINE_LIMIT]))
                .unwrap()
                .len(),
            INLINE_LIMIT
        );
        assert!(decode_inline(&STANDARD.encode(vec![0; INLINE_LIMIT + 1])).is_err());
        assert!(decode_inline("%%%bad%%").is_err());
        for url in [
            "http://files.oaiusercontent.com/a",
            "https://files.oaiusercontent.com.evil/a",
            "https://user:pass@files.oaiusercontent.com/a",
            "https://127.0.0.1/a",
            "https://files.oaiusercontent.com:444/a",
        ] {
            assert!(host_url(url).is_err());
        }
        for ip in [
            "127.0.0.1",
            "10.0.0.1",
            "169.254.169.254",
            "100.64.0.1",
            "192.168.0.1",
            "::1",
            "fc00::1",
            "::ffff:127.0.0.1",
            "2001:db8::1",
        ] {
            assert!(!public_ip(ip.parse().unwrap()), "{ip}");
        }
    }
}
