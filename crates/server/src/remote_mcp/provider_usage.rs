use serde_json::Value;

use super::{
    Remote,
    read::{arguments, definition},
};

pub(super) fn tools() -> Vec<rmcp::model::Tool> {
    let mut tool = definition::<executors::account_usage::Request>(
        "read_provider_usage",
        "Read official Codex/Claude quota and optional usage in an explicit server_default account context (administrator or authenticated LVK internal agent). Works without a Workspace using a bounded short-lived official CLI control connection. No model turns, transcript scans, login or execution records. Existing-session usage requires project/workspace/session IDs; never pass a native ID. Unknown stays unknown. Snapshots are cumulative; estimated USD is not a subscription invoice.",
    );
    if let Some(annotations) = &mut tool.annotations {
        annotations.open_world_hint = Some(true);
    }
    vec![tool]
}
pub(super) async fn call(remote: &Remote, args: Value) -> anyhow::Result<Value> {
    let request: executors::account_usage::Request = arguments(args)?;
    // Check delegation before even resolving the server-global launcher/auth.
    let admin = server_access::ACTOR
        .try_with(|p| p.admin && p.scopes.contains("lvk:read"))
        .unwrap_or(false);
    anyhow::ensure!(
        admin,
        super::errors::rejection_with_message(
            utils::mcp_error::Category::Permission,
            "Administrator required for the server-managed account context"
        )
    );
    if let Some(project) = request.project_id {
        remote.require_project(project).await?;
        if let Some(workspace) = request.workspace_id {
            remote.require_workspace(project, workspace).await?;
        }
    }
    crate::provider_usage::read(&remote.pool, request, admin).await
}

#[cfg(test)]
mod tests {
    use executors::{
        account_usage::Request, executors::BaseCodingAgent, profile::ExecutorProfileId,
    };
    use serde_json::json;
    use uuid::Uuid;

    use super::*;
    #[tokio::test]
    async fn provider_usage_authorizes_account_and_native_session_before_launch_without_writes() {
        let (remote, _, _, target, _) = super::super::runtime_write_tests::fixture().await;
        let account = json!({"provider":"codex","account_context":"server_default"});
        // Absence of an authenticated administrator never falls back to local admin.
        assert!(call(&remote, account.clone()).await.is_err());
        assert!(
            crate::provider_usage::read(
                &remote.pool,
                serde_json::from_value(account.clone()).unwrap(),
                false
            )
            .await
            .is_err()
        );
        let profile = ExecutorProfileId::new(BaseCodingAgent::Codex);
        let account: Request = serde_json::from_value(account).unwrap();
        assert!(
            crate::provider_usage::resolve_native(&remote.pool, &account, &profile)
                .await
                .unwrap()
                .is_none()
        );
        let mut args = json!({"provider":"codex","account_context":"server_default","include_usage":true,"session_id":target.session_id,"workspace_id":target.workspace_id,"project_id":target.project_id});
        sqlx::query("INSERT INTO agent_provider_sessions(id,session_id,schema_version,provider_id,runtime_profile_id,provider_session_id,session_reference,observed_at) VALUES(?,?,1,'codex','CODEX:DEFAULT','native-not-a-run-id','{}',datetime('now','subsec'))")
            .bind(Uuid::new_v4()).bind(target.session_id).execute(&remote.pool).await.unwrap();
        let request: Request = serde_json::from_value(args.clone()).unwrap();
        assert_eq!(
            crate::provider_usage::resolve_native(&remote.pool, &request, &profile)
                .await
                .unwrap()
                .as_deref(),
            Some("native-not-a-run-id")
        );
        for field in ["project_id", "workspace_id", "session_id"] {
            let mut foreign = args.clone();
            foreign[field] = json!(Uuid::new_v4());
            assert!(
                crate::provider_usage::resolve_native(
                    &remote.pool,
                    &serde_json::from_value(foreign).unwrap(),
                    &profile
                )
                .await
                .is_err()
            );
        }
        args["provider"] = json!("claude_code");
        assert!(
            crate::provider_usage::resolve_native(
                &remote.pool,
                &serde_json::from_value(args).unwrap(),
                &profile
            )
            .await
            .is_err()
        );
        sqlx::query("UPDATE agent_provider_sessions SET runtime_profile_id='CODEX:OTHER'")
            .execute(&remote.pool)
            .await
            .unwrap();
        assert!(
            crate::provider_usage::resolve_native(&remote.pool, &request, &profile)
                .await
                .is_err()
        );
        let runs: i64 = sqlx::query_scalar("SELECT count(*) FROM agent_runs")
            .fetch_one(&remote.pool)
            .await
            .unwrap();
        let commands: i64 = sqlx::query_scalar("SELECT count(*) FROM agent_run_commands")
            .fetch_one(&remote.pool)
            .await
            .unwrap();
        assert_eq!(runs, 1);
        assert_eq!(commands, 0);
    }
}
