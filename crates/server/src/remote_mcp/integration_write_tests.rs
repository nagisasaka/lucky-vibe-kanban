//! The host scenario supplies isolated Git/SQLite and real UI domain services.
use std::sync::Arc;

use db::models::integration::IntegrationRun;
use deployment::Deployment;
use serde_json::{Value, json};
use uuid::Uuid;

use super::{Remote, auth::Principal, integration_read, integration_write, launch::ExistingLaunch};

fn remote(deployment: &crate::DeploymentImpl, project: Uuid) -> Remote {
    let mut config = super::tests::config();
    config.project_ids = [project].into();
    Remote {
        environment: None,
        runtime: None,
        launch: Some(Arc::new(ExistingLaunch(deployment.clone()))),
        process_instance: Uuid::new_v4(),
        pool: deployment.db().pool.clone(),
        config: Arc::new(config),
    }
}

pub(crate) async fn exercise(
    deployment: &crate::DeploymentImpl,
    run: &IntegrationRun,
    cancel: bool,
) -> anyhow::Result<()> {
    let remote = remote(deployment, run.project_id);
    let principal = Principal {
        subject: "fixture-owner".into(),
        scopes: ["lvk:read".into(), "lvk:merge".into()].into(),
    };
    let read = integration_read::call(
        &remote,
        "get_integration",
        json!({"project_id":run.project_id,"integration_id":run.id,"section":"pending_gates"}),
    )
    .await?;
    assert_eq!(read["items"].as_array().unwrap().len(), 1);
    assert_eq!(read["items"][0]["current"], true);
    assert_eq!(read["items"][0]["kind"], "resolution_review");
    let mut input = json!({"project_id":run.project_id,"integration_id":run.id,"request_id":Uuid::new_v4(),"gate_id":read["items"][0]["gate_id"],"expected":read["expected"]});
    let expected: crate::routes::integrations::DecisionExpectation =
        serde_json::from_value(read["expected"].clone())?;
    expected.verify(run)?;
    let mut changed_source = run.clone();
    changed_source.payload.sources[0].commit = "different frozen source".into();
    assert!(expected.verify(&changed_source).is_err());
    let no_merge = Principal {
        subject: principal.subject.clone(),
        scopes: ["lvk:read".into(), "lvk:write".into()].into(),
    };
    for name in ["approve_integration", "cancel_integration"] {
        assert!(
            integration_write::call(&remote, &no_merge, name, input.clone())
                .await
                .is_err()
        );
    }
    let before: i64 = sqlx::query_scalar("SELECT count(*) FROM remote_mcp_operations")
        .fetch_one(&remote.pool)
        .await?;
    let mut foreign = input.clone();
    foreign["project_id"] = json!(Uuid::new_v4());
    assert!(
        integration_write::call(&remote, &principal, "approve_integration", foreign)
            .await
            .is_err()
    );
    assert_eq!(
        before,
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM remote_mcp_operations")
            .fetch_one(&remote.pool)
            .await?
    );
    // Every part of the frozen identity is independently required.
    for field in [
        "revision",
        "base_commit",
        "candidate_commit",
        "scope_digest",
    ] {
        let mut stale = input.clone();
        stale["request_id"] = json!(Uuid::new_v4());
        stale["expected"][field] = json!("stale");
        let rejected =
            integration_write::call(&remote, &principal, "approve_integration", stale).await?;
        assert_eq!(rejected["status"], "failed", "{rejected}");
        assert!(
            rejected["result"]["message"]
                .as_str()
                .unwrap()
                .contains("STALE_CANDIDATE")
        );
    }
    // Move only the isolated target ref. Both UI and MCP must reject it, even
    // though the database revision and candidate scope still match exactly.
    let repo = db::models::repo::Repo::find_by_id(&remote.pool, run.repository_id)
        .await?
        .unwrap();
    let update = |oid: &str| {
        let result = std::process::Command::new("git")
            .arg("-C")
            .arg(&repo.path)
            .args(["update-ref", &run.target_ref, oid])
            .output()
            .unwrap();
        assert!(result.status.success());
    };
    update(run.payload.result_commit.as_deref().unwrap());
    let rejected =
        integration_write::call(&remote, &principal, "approve_integration", input.clone()).await?;
    let ui = crate::routes::integrations::decide_gate_checked(
        deployment,
        run.id,
        crate::routes::integrations::GateDecisionRequest {
            project_id: run.project_id,
            gate_id: run.payload.reviews.last().unwrap().id,
            request_id: Uuid::new_v4(),
            decision: "continue".into(),
        },
        None,
    )
    .await;
    update(run.payload.base_commit.as_deref().unwrap());
    assert_eq!(rejected["status"], "failed", "{rejected}");
    assert!(
        rejected["result"]["message"]
            .as_str()
            .unwrap()
            .contains("TARGET_CHANGED"),
        "{rejected}"
    );
    assert!(ui.unwrap_err().to_string().contains("TARGET_CHANGED"));
    // Failed receipts cannot be repurposed after the target is restored.
    assert_eq!(
        integration_write::call(&remote, &principal, "approve_integration", input.clone()).await?["operation_id"],
        rejected["operation_id"]
    );
    input["request_id"] = json!(Uuid::new_v4());
    let name = if cancel {
        // Approval resumes the real product to integrating; cancel it before
        // the next host step. The MCP acknowledgement must retain reservations.
        let resumed =
            integration_write::call(&remote, &principal, "approve_integration", input.clone())
                .await?;
        assert_eq!(resumed["status"], "succeeded");
        assert_eq!(resumed["result"]["status"], "integrating");
        input = json!({"project_id":run.project_id,"integration_id":run.id,"request_id":Uuid::new_v4()});
        "cancel_integration"
    } else {
        "approve_integration"
    };
    let (a, b) = tokio::join!(
        integration_write::call(&remote, &principal, name, input.clone()),
        integration_write::call(&remote, &principal, name, input.clone())
    );
    let a = a?;
    let b = b?;
    assert!(a["operation_id"].is_string());
    assert_eq!(a["operation_id"], b["operation_id"]);
    let receipt = super::tests::settled_operation(
        &remote,
        &principal,
        run.project_id,
        input["request_id"].as_str().unwrap().parse()?,
    )
    .await;
    assert_eq!(receipt["status"], "succeeded", "{receipt}");
    let current = IntegrationRun::find(&remote.pool, run.id).await?;
    if cancel {
        assert!(current.cancel_requested);
        assert_eq!(receipt["result"]["cancellation_requested"], true);
        assert_eq!(receipt["result"]["cancellation_complete"], false);
        assert_eq!(receipt["result"]["reservations_released"], false);
    } else {
        assert_eq!(
            current.payload.reviews.last().unwrap().decision.as_deref(),
            Some("continue")
        );
        assert!(current.payload.reviews.last().unwrap().request_id.is_some());
        // A different explicit request for the same stale gate cannot approve again.
        let mut duplicate = input.clone();
        duplicate["request_id"] = json!(Uuid::new_v4());
        assert_eq!(
            integration_write::call(&remote, &principal, name, duplicate).await?["status"],
            "failed"
        );
        // Same request UUID with altered evidence is an idempotency conflict.
        let mut changed = input.clone();
        changed["expected"]["candidate_commit"] = json!("other");
        assert!(
            integration_write::call(&remote, &principal, name, changed)
                .await
                .is_err()
        );
    }
    Ok(())
}

pub(crate) async fn terminal(
    deployment: &crate::DeploymentImpl,
    run: &IntegrationRun,
    cancelled: bool,
) -> anyhow::Result<()> {
    let remote = remote(deployment, run.project_id);
    let read = integration_read::call(
        &remote,
        "get_integration",
        json!({"project_id":run.project_id,"integration_id":run.id,"section":"summary"}),
    )
    .await?;
    assert_eq!(read["lifecycle"]["terminal"], true);
    assert_eq!(read["lifecycle"]["reservations_released"], true);
    assert_eq!(read["lifecycle"]["cancellation_complete"], cancelled);
    let principal = Principal {
        subject: "fixture-owner".into(),
        scopes: ["lvk:read".into(), "lvk:merge".into()].into(),
    };
    let operation_name = if cancelled {
        "cancel_integration"
    } else {
        "approve_integration"
    };
    let original_request: Uuid = sqlx::query_scalar("SELECT request_id FROM remote_mcp_operations WHERE project_id=? AND operation=? AND status='succeeded' ORDER BY created_at LIMIT 1")
        .bind(run.project_id).bind(operation_name).fetch_one(&remote.pool).await?;
    let original =
        super::tests::settled_operation(&remote, &principal, run.project_id, original_request)
            .await;
    assert_eq!(original["status"], "succeeded");
    assert_eq!(original["result"]["terminal"], false);
    assert_eq!(original["result"]["cancellation_complete"], false);
    let late = integration_write::call(
        &remote,
        &principal,
        "cancel_integration",
        json!({"project_id":run.project_id,"integration_id":run.id,"request_id":Uuid::new_v4()}),
    )
    .await?;
    assert_eq!(late["result"]["published"], !cancelled);
    assert_eq!(late["result"]["cancellation_requested"], cancelled);
    Ok(())
}

#[test]
fn integration_decisions_have_strict_merge_scoped_mutation_schemas() {
    for tool in integration_write::tools() {
        let value = serde_json::to_value(tool).unwrap();
        assert_eq!(value["annotations"]["readOnlyHint"], false);
        assert_eq!(value["annotations"]["idempotentHint"], true);
        assert_eq!(value["inputSchema"]["additionalProperties"], false);
        assert_eq!(
            value["_meta"]["securitySchemes"][0]["scopes"],
            json!(["lvk:read", "lvk:merge"])
        );
        assert_eq!(
            super::operation_scope(value["name"].as_str().unwrap()),
            Some("lvk:merge")
        );
        assert!(
            super::tools()
                .iter()
                .any(|t| t.name == value["name"].as_str().unwrap())
        );
    }
    let schema: Value = serde_json::to_value(integration_read::tools().pop().unwrap()).unwrap();
    assert!(schema.to_string().contains("pending_gates"));
}
