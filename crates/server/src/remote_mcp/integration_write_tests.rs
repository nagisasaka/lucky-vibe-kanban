use serde_json::{Value, json};

use super::{integration_read, integration_write};
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
    assert!(!schema.to_string().contains("pending_gates"));
    assert_eq!(integration_write::tools().len(), 1);
    assert_eq!(integration_write::tools()[0].name, "cancel_integration");
}

/// Runs only from the fresh isolated conversation fixture; no live operation.
pub(crate) async fn exercise_cancel(
    deployment: &crate::DeploymentImpl,
    run: &db::models::integration::IntegrationRun,
) -> anyhow::Result<()> {
    use std::sync::Arc;

    use deployment::Deployment;
    use uuid::Uuid;

    use super::{Remote, auth::Principal, launch::ExistingLaunch};
    let mut config = super::tests::config();
    config.project_ids = [run.project_id].into();
    let remote = Remote {
        environment: None,
        runtime: None,
        launch: Some(Arc::new(ExistingLaunch(deployment.clone()))),
        process_instance: Uuid::new_v4(),
        pool: deployment.db().pool.clone(),
        config: Arc::new(config),
    };
    let principal = Principal {
        subject: "fixture-owner".into(),
        scopes: ["lvk:read".into(), "lvk:merge".into()].into(),
    };
    let input =
        json!({"project_id":run.project_id,"integration_id":run.id,"request_id":Uuid::new_v4()});
    let no_merge = Principal {
        subject: principal.subject.clone(),
        scopes: ["lvk:read".into(), "lvk:write".into()].into(),
    };
    assert!(
        integration_write::call(&remote, &no_merge, "cancel_integration", input.clone())
            .await
            .is_err()
    );
    let mut foreign = input.clone();
    foreign["project_id"] = json!(Uuid::new_v4());
    assert!(
        integration_write::call(&remote, &principal, "cancel_integration", foreign)
            .await
            .is_err()
    );
    let before = integration_write::observation(&remote, run.project_id, run.id).await?;
    let a =
        integration_write::call(&remote, &principal, "cancel_integration", input.clone()).await?;
    let b =
        integration_write::call(&remote, &principal, "cancel_integration", input.clone()).await?;
    assert_eq!(a["operation_id"], b["operation_id"]);
    assert_eq!(a["status"], "succeeded");
    assert_eq!(a["result"]["cancellation_requested"], true);
    assert_eq!(a["result"]["cancellation_complete"], false);
    assert_eq!(
        a["result"]["reservations_released"],
        before["reservations_released"]
    );
    let mut changed = input;
    changed["integration_id"] = json!(Uuid::new_v4());
    assert!(
        integration_write::call(&remote, &principal, "cancel_integration", changed)
            .await
            .is_err()
    );
    Ok(())
}
