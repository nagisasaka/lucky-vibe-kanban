//! Thin MCP adapter over the same repository publication service as settings.
use anyhow::ensure;
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};
use uuid::Uuid;

use super::{
    Remote,
    auth::Principal,
    read::{arguments, definition},
};
use crate::repository_push::{
    RepositoryPushPreview, RepositoryPushRequest, RepositoryPushSelection,
};
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct PreviewArgs {
    repository_id: Uuid,
    #[serde(flatten)]
    selection: RepositoryPushSelection,
}
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct PushArgs {
    request_id: Uuid,
    preview: RepositoryPushPreview,
}
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct VerifyArgs {
    project_id: Uuid,
    repository_id: Uuid,
    request_id: Uuid,
}
pub(super) fn tools() -> Vec<rmcp::model::Tool> {
    let preview = definition::<PreviewArgs>(
        "preview_repository_push",
        "Preview explicit publication of a registered repository local branch (main/dev etc.) to the same-named branch on one configured GitHub remote. Requires read and write permissions, separate from merge. Returns source SHA, exact push URL, destination SHA and unpublished history including Issue archive paths. Never publishes or creates a Workspace.",
    );
    let mut preview = serde_json::to_value(preview).unwrap();
    preview["_meta"] =
        json!({"securitySchemes":[{"type":"oauth2","scopes":["lvk:read","lvk:write"]}]});
    let preview = serde_json::from_value(preview).unwrap();
    let mut push = serde_json::to_value(definition::<PushArgs>("push_repository_branch", "Only after explicit human approval of preview targets and full history (including Issue archives), perform an ordinary push using registered GitHub credentials. No force, arbitrary URL, PR, auto-merge or automatic retry. Pass the complete preview unchanged and retain request_id with identical input on transport retry. Receipt survives disconnect/restart and prevents replay. Use verify_repository_push for remote state; uncertain is not failure or permission to retry.")).unwrap();
    push["annotations"] = json!({"readOnlyHint":false,"destructiveHint":true,"idempotentHint":true,"openWorldHint":true});
    push["_meta"] =
        json!({"securitySchemes":[{"type":"oauth2","scopes":["lvk:read","lvk:write"]}]});
    vec![
        preview,
        serde_json::from_value(push).unwrap(),
        definition::<VerifyArgs>(
            "verify_repository_push",
            "Read the original repository push receipt and query the same configured remote branch without pushing or fetching. source_present means the remote branch currently equals the confirmed source SHA, not proof of who pushed it. A lost response must be resolved here, never by a new request ID.",
        ),
    ]
}
fn preflight_error(error: anyhow::Error) -> anyhow::Error {
    let code = error
        .downcast_ref::<crate::repository_push::NotAdmitted>()
        .map(|rejection| rejection.0)
        .unwrap_or_else(|| git::repository_push::error_code(&error));
    let category = match code {
        "authentication_rejected" | "permission_denied" => utils::mcp_error::Category::Permission,
        "source_drift" | "remote_drift" | "target_changed" => utils::mcp_error::Category::Conflict,
        _ => utils::mcp_error::Category::Validation,
    };
    super::errors::rejection_with_message(category, code)
}
pub(super) async fn call(
    remote: &Remote,
    principal: &Principal,
    lease: Option<server_access::Lease>,
    name: &str,
    input: Value,
) -> anyhow::Result<Value> {
    ensure!(principal.allows("lvk:read"), "Read permission required");
    let actor = server_access::ACTOR
        .try_with(Clone::clone)
        .map_err(|_| anyhow::anyhow!("Authenticated member required"))?;
    match name {
        "preview_repository_push" => {
            ensure!(principal.allows("lvk:write"), "Write permission required");
            let args: PreviewArgs = arguments(input)?;
            remote.require_project(args.selection.project_id).await?;
            crate::repository_push::authorize(&actor, true)?;
            Ok(serde_json::to_value(
                crate::repository_push::preview(&remote.pool, args.repository_id, args.selection)
                    .await
                    .map_err(preflight_error)?,
            )?)
        }
        "push_repository_branch" => {
            ensure!(principal.allows("lvk:write"), "Write permission required");
            let args: PushArgs = arguments(input)?;
            remote.require_project(args.preview.project_id).await?;
            Ok(serde_json::to_value(
                crate::repository_push::push(
                    &remote.pool,
                    &actor,
                    lease.ok_or_else(|| {
                        super::errors::rejection(utils::mcp_error::Category::Permission)
                    })?,
                    args.preview.repository_id,
                    RepositoryPushRequest {
                        request_id: args.request_id,
                        preview: args.preview,
                    },
                )
                .await
                .map_err(|error| {
                    if error
                        .downcast_ref::<crate::repository_push::NotAdmitted>()
                        .is_some()
                    {
                        preflight_error(error)
                    } else {
                        error
                    }
                })?,
            )?)
        }
        "verify_repository_push" => {
            let args: VerifyArgs = arguments(input)?;
            remote.require_project(args.project_id).await?;
            let result = crate::repository_push::verify(
                &remote.pool,
                &actor,
                args.repository_id,
                args.request_id,
            )
            .await?;
            ensure!(
                result.receipt.preview.project_id == args.project_id,
                "Project target mismatch"
            );
            Ok(serde_json::to_value(result)?)
        }
        _ => anyhow::bail!("Unknown repository publication operation"),
    }
}

#[cfg(all(test, unix))]
mod tests {
    use axum::{Extension, Router, body::Body, http::Request, routing::get};
    use tower::ServiceExt;

    use super::*;
    async fn actor() -> (
        tempfile::TempDir,
        sqlx::SqlitePool,
        server_access::Principal,
        server_access::Lease,
    ) {
        let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
        let temp = tempfile::tempdir().unwrap();
        let access = server_access::test_support::access(
            pool.clone(),
            temp.path().to_owned(),
            json!({"keys":[]}),
        )
        .await;
        let cookie = server_access::test_support::web_session(&access).await;
        let (sender, receiver) = tokio::sync::oneshot::channel();
        let sender = std::sync::Arc::new(std::sync::Mutex::new(Some(sender)));
        let app = access.layer(Router::new().route(
            "/api/actor",
            get(
                move |Extension(actor): Extension<server_access::Principal>,
                      Extension(lease): Extension<server_access::Lease>| {
                    let sender = sender.clone();
                    async move {
                        assert!(
                            sender
                                .lock()
                                .unwrap()
                                .take()
                                .unwrap()
                                .send((actor, lease))
                                .is_ok()
                        );
                        "ok"
                    }
                },
            ),
        ));
        assert!(
            app.oneshot(
                Request::builder()
                    .uri("/api/actor")
                    .header("host", "203.0.113.7:8443")
                    .header("cookie", cookie)
                    .body(Body::empty())
                    .unwrap()
            )
            .await
            .unwrap()
            .status()
            .is_success()
        );
        let (actor, lease) = receiver.await.unwrap();
        (temp, pool, actor, lease)
    }
    #[tokio::test]
    async fn repository_push_authority_targets_receipts_and_unknown_are_shared() {
        let (_temp, remote, ids) = super::super::code_read::tests::fixture().await;
        let (_auth_temp, auth_pool, actor, lease) = actor().await;
        let project: Uuid = serde_json::from_value(ids["project_id"].clone()).unwrap();
        let repository: Uuid = serde_json::from_value(ids["repository_id"].clone()).unwrap();
        let preview = RepositoryPushPreview {
            repository_id: repository,
            repository_name: "fixture".into(),
            project_id: project,
            target: git::repository_push::PushObservation {
                remote: "origin".into(),
                remote_url: "/fixture/not-a-github-remote".into(),
                branch: "main".into(),
                source_sha: "0".repeat(40),
                destination_branch: "main".into(),
                remote_sha: None,
                unpublished_commits: 1,
                issue_archive_paths: vec![],
            },
        };
        let request = RepositoryPushRequest {
            request_id: Uuid::new_v4(),
            preview,
        };
        let round_trip: RepositoryPushRequest =
            serde_json::from_value(serde_json::to_value(&request).unwrap()).unwrap();
        assert_eq!(round_trip.preview, request.preview);
        let mut read_only = actor.clone();
        read_only.scopes.remove("lvk:write");
        assert!(
            crate::repository_push::push(
                &remote.pool,
                &read_only,
                lease.clone(),
                repository,
                request.clone()
            )
            .await
            .is_err()
        );
        assert!(
            crate::repository_push::push(
                &remote.pool,
                &actor,
                lease.clone(),
                Uuid::new_v4(),
                request.clone()
            )
            .await
            .is_err()
        );
        assert!(
            crate::repository_push::push(
                &remote.pool,
                &actor,
                lease.clone(),
                repository,
                request.clone()
            )
            .await
            .is_err()
        );
        let count: i64 = sqlx::query_scalar("SELECT count(*) FROM remote_mcp_operations")
            .fetch_one(&remote.pool)
            .await
            .unwrap();
        assert_eq!(count, 0); // No untrusted preview URL or identity is persisted.
        // Seed a trusted domain receipt: replay/recovery use the same store as
        // HTTP/MCP without opening an external connection in this fixture.
        let key = json!([actor.issuer, actor.subject]).to_string();
        let (seeded, owned) = remote
            .operations()
            .admit(
                &key,
                request.request_id,
                project,
                "push_repository_branch",
                &serde_json::to_value(&request).unwrap(),
                &serde_json::to_value(&request.preview).unwrap(),
            )
            .await
            .unwrap();
        assert!(owned);
        assert!(remote.operations().start(seeded.id).await.unwrap());
        remote
            .operations()
            .finish(seeded.id, "failed", &json!({"outcome":"protected_branch"}))
            .await
            .unwrap();
        let replay = crate::repository_push::push(
            &remote.pool,
            &actor,
            lease.clone(),
            repository,
            request.clone(),
        )
        .await
        .unwrap();
        assert_eq!(replay.status, "failed");
        let verification =
            crate::repository_push::verify(&remote.pool, &actor, repository, request.request_id)
                .await
                .unwrap();
        assert_eq!(
            verification.receipt.result.unwrap().outcome,
            "protected_branch"
        );
        assert_eq!(verification.source_present, None);
        assert!(verification.verification_error.is_some());
        let mut changed = request.clone();
        changed.preview.target.source_sha = "1".repeat(40);
        assert!(
            crate::repository_push::push(&remote.pool, &actor, lease.clone(), repository, changed)
                .await
                .is_err()
        );
        let count: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM remote_mcp_operations WHERE operation='push_repository_branch'",
        )
        .fetch_one(&remote.pool)
        .await
        .unwrap();
        assert_eq!(count, 1);
        // A response lost around process restart remains unknown; the original
        // request must not execute a second push, even with unchanged arguments.
        sqlx::query("UPDATE remote_mcp_operations SET status='running', result=NULL, process_instance=? WHERE request_id=?").bind(Uuid::new_v4()).bind(request.request_id).execute(&remote.pool).await.unwrap();
        assert_eq!(
            crate::repository_push::push(
                &remote.pool,
                &actor,
                lease.clone(),
                repository,
                request.clone()
            )
            .await
            .unwrap()
            .status,
            "uncertain"
        );
        let mut other = actor.clone();
        other.subject = "another-member".into();
        assert!(
            crate::repository_push::receipt(&remote.pool, &other, repository, request.request_id)
                .await
                .is_err()
        );
        sqlx::query("UPDATE access_sessions SET expires=0")
            .execute(&auth_pool)
            .await
            .unwrap();
        let mut expired = request.clone();
        expired.request_id = Uuid::new_v4();
        let rejected =
            crate::repository_push::push(&remote.pool, &actor, lease.clone(), repository, expired)
                .await
                .err()
                .unwrap();
        assert_eq!(
            rejected
                .downcast_ref::<crate::repository_push::NotAdmitted>()
                .unwrap()
                .0,
            "permission_denied"
        );
        sqlx::query("DELETE FROM workspace_repos WHERE repo_id=?")
            .bind(repository)
            .execute(&remote.pool)
            .await
            .unwrap();
        assert!(
            crate::repository_push::receipt(&remote.pool, &actor, repository, request.request_id)
                .await
                .is_err()
        );
    }
    #[test]
    fn repository_push_catalogue_keeps_merge_separate_and_fixed_input() {
        let input = json!({"repository_id":Uuid::new_v4(), "project_id":Uuid::new_v4(), "remote":"origin", "branch":"dev"});
        let selection: PreviewArgs = arguments(input).unwrap();
        assert_eq!(selection.selection.branch, "dev");
        let tools = tools();
        assert_eq!(tools.len(), 3);
        let push = serde_json::to_value(&tools[1]).unwrap();
        assert_eq!(push["annotations"]["readOnlyHint"], false);
        assert_eq!(push["annotations"]["idempotentHint"], true);
        assert_eq!(
            super::super::operation_scope("push_repository_branch"),
            Some("lvk:write")
        );
        assert_eq!(
            super::super::operation_scope("verify_repository_push"),
            None
        );
        let rejection = preflight_error(anyhow::anyhow!("authentication_rejected"));
        let diagnostic = rejection
            .downcast_ref::<utils::mcp_error::Diagnostic>()
            .unwrap();
        assert_eq!(diagnostic.message, "authentication_rejected");
        assert_eq!(diagnostic.category, utils::mcp_error::Category::Permission);
        assert_eq!(diagnostic.outcome, utils::mcp_error::Outcome::Failed);
        let schema = serde_json::to_value(schemars::schema_for!(PushArgs)).unwrap();
        assert!(schema.to_string().contains("source_sha"));
        assert!(schema.to_string().contains("destination_branch"));
    }
}
