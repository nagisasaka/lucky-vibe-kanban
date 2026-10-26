use std::{path::PathBuf, sync::Arc, time::Duration};

use anyhow::ensure;
use deployment::Deployment;
use serde_json::{Value, json};
use services::services::container::ContainerService;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use uuid::Uuid;

use super::dev_server::*;
use crate::DeploymentImpl;

#[tokio::test]
async fn dev_server_catalog_scopes_and_target_rejection() {
    let (remote, _, mut principal, target, _) = super::runtime_write_tests::fixture().await;
    let input = json!({"project_id":target.project_id,"workspace_id":target.workspace_id});
    assert!(
        call(&remote, &principal, "get_dev_server_status", input.clone())
            .await
            .is_err()
    );
    principal.scopes.insert("lvk:runtime:read".into());
    assert!(
        call(&remote, &principal, "start_dev_server", input.clone())
            .await
            .is_err()
    );
    for name in ["start_dev_server", "stop_dev_server"] {
        assert_eq!(super::operation_scope(name), Some("lvk:runtime:execute"));
    }
    assert_eq!(
        super::operation_scope("get_dev_server_status"),
        Some("lvk:runtime:read")
    );
    let mut invalid = input;
    invalid["workspace_id"] = json!(Uuid::new_v4());
    assert!(
        call(&remote, &principal, "get_dev_server_status", invalid)
            .await
            .is_err()
    );
    for tool in tools() {
        let value = serde_json::to_value(tool).unwrap();
        assert_eq!(
            value["annotations"]["readOnlyHint"],
            value["name"] == "get_dev_server_status"
        );
    }
}

#[tokio::test]
async fn dev_server_projection_keeps_each_repository_failure_visible() -> anyhow::Result<()> {
    use db::models::execution_process::{
        CreateExecutionProcess, ExecutionProcess, ExecutionProcessRunReason, ExecutionProcessStatus,
    };
    use executors::actions::{
        ExecutorAction, ExecutorActionType,
        script::{ScriptContext, ScriptRequest, ScriptRequestLanguage},
    };
    let (remote, _, _, target, _) = super::runtime_write_tests::fixture().await;
    let mut ids = Vec::new();
    for dir in ["a", "b", "a"] {
        let id = Uuid::new_v4();
        ExecutionProcess::create(
            &remote.pool,
            &CreateExecutionProcess {
                session_id: target.session_id,
                run_reason: ExecutionProcessRunReason::DevServer,
                executor_action: ExecutorAction::new(
                    ExecutorActionType::ScriptRequest(ScriptRequest {
                        script: "fixture".into(),
                        language: ScriptRequestLanguage::Bash,
                        context: ScriptContext::DevServer,
                        working_dir: Some(dir.into()),
                    }),
                    None,
                ),
            },
            id,
            &[],
        )
        .await?;
        if dir == "b" {
            ExecutionProcess::update_completion(
                &remote.pool,
                id,
                ExecutionProcessStatus::Failed,
                Some(7),
            )
            .await?;
        }
        if ids.is_empty() {
            ExecutionProcess::update_completion(
                &remote.pool,
                id,
                ExecutionProcessStatus::Killed,
                None,
            )
            .await?;
        }
        ids.push(id);
    }
    let records =
        crate::routes::workspaces::dev_server::process_records(&remote.pool, target.workspace_id)
            .await?;
    ensure!(records.len() == 2);
    ensure!(records.iter().any(|p| p.id == ids[1]
        && p.status == ExecutionProcessStatus::Failed
        && p.exit_code == Some(7)));
    ensure!(
        records
            .iter()
            .any(|p| p.id == ids[2] && p.status == ExecutionProcessStatus::Running)
    );
    Ok(())
}

/// Run alone with a fresh task-owned instance and this checkout's MCP binary.
/// Exercises real managed script processes; the remote IdP is a mock.
#[tokio::test]
#[ignore = "requires fresh LVK_INSTANCE_DIR with devserver-fixture- prefix and built vibe-kanban-mcp"]
async fn isolated_dev_server_stdio_remote_and_ui() -> anyhow::Result<()> {
    let root = PathBuf::from(std::env::var("LVK_INSTANCE_DIR")?);
    ensure!(
        root.file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("devserver-fixture-")
    );
    ensure!(
        !root.join("data/db.v2.sqlite").exists(),
        "fresh instance required"
    );
    let shutdown = tokio_util::sync::CancellationToken::new();
    let deployment = DeploymentImpl::new(shutdown.clone()).await?;
    let (mut remote, _, _, target, _) =
        super::runtime_write_tests::fixture_with_pool(deployment.db().pool.clone()).await;
    remote.runtime = Some(Arc::new(super::runtime_write::ExistingRuntime(
        deployment.clone(),
    )));
    let (_, auth, provider) = super::tests::fixture().await;
    let http = super::environment_tests::serve(remote.clone(), auth).await;
    let repo_root = root.join("fixture");
    std::fs::create_dir_all(&repo_root)?;
    for args in [
        vec!["init", "-b", "main"],
        vec!["config", "user.name", "Fixture"],
        vec!["config", "user.email", "fixture@localhost"],
        vec!["commit", "--allow-empty", "-m", "fixture"],
    ] {
        ensure!(
            std::process::Command::new("git")
                .args(args)
                .current_dir(&repo_root)
                .status()?
                .success()
        );
    }
    let repo = db::models::repo::Repo::find_or_create(&deployment.db().pool, &repo_root, "fixture")
        .await?;
    db::models::workspace_repo::WorkspaceRepo::create_many(
        &deployment.db().pool,
        target.workspace_id,
        &[db::models::workspace_repo::CreateWorkspaceRepo {
            repo_id: repo.id,
            target_branch: "main".into(),
        }],
    )
    .await?;
    sqlx::query("UPDATE workspaces SET container_ref=? WHERE id=?")
        .bind(root.join("workspace").to_string_lossy().as_ref())
        .bind(target.workspace_id)
        .execute(&deployment.db().pool)
        .await?;
    let workspace =
        db::models::workspace::Workspace::find_by_id(&deployment.db().pool, target.workspace_id)
            .await?
            .unwrap();
    deployment
        .container()
        .ensure_container_exists(&workspace)
        .await?;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let address = listener.local_addr()?;
    deployment.client_info().set_server_addr(address).unwrap();
    let access =
        server_access::Access::from_env(deployment.db().pool.clone(), utils::assets::asset_dir())?;
    let router = crate::routes::router(deployment.clone(), access);
    let stop = shutdown.clone();
    let server = tokio::spawn(async move {
        axum::serve(listener, router)
            .with_graceful_shutdown(async move { stop.cancelled().await })
            .await
    });
    let result = async {
            let (_, catalog) = http.rpc("runtime","tools/list",json!({})).await;
            for name in ["start_dev_server","stop_dev_server","get_dev_server_status"] {
                ensure!(catalog["result"]["tools"].as_array().unwrap().iter().any(|t|t["name"]==name));
            }
            let target_args = json!({"project_id":target.project_id,"workspace_id":target.workspace_id});
            for token in ["read-only","runtime-read"] {
                let (code,_) = http.rpc(token,"tools/call",json!({"name":"start_dev_server","arguments":target_args})).await;
                ensure!(code==403,"scope boundary: {code}");
            }
            sqlx::query("UPDATE workspaces SET archived=1 WHERE id=?").bind(target.workspace_id).execute(&deployment.db().pool).await?;
            let archived=Uuid::new_v4();
            http.value("start_dev_server",json!({"project_id":target.project_id,"workspace_id":target.workspace_id,"request_id":archived})).await;
            ensure!(http.settle(archived).await["status"]=="failed");
            sqlx::query("UPDATE workspaces SET archived=0 WHERE id=?").bind(target.workspace_id).execute(&deployment.db().pool).await?;
            let initial = http.value("get_dev_server_status",target_args.clone()).await;
            ensure!(initial["state"]=="stopped");
            let bad = http.call("runtime","get_dev_server_status",json!({"project_id":Uuid::new_v4(),"workspace_id":target.workspace_id})).await;
            ensure!(bad["isError"]==true);
            let request = Uuid::new_v4();
            let mut input=target_args.clone(); input["request_id"]=json!(request);
            http.value("start_dev_server",input.clone()).await;
            ensure!(http.settle(request).await["status"]=="failed","missing script must fail");
            sqlx::query("UPDATE repos SET dev_server_script='printf launch\\n >> launches; exec sleep 60' WHERE id=?").bind(repo.id).execute(&deployment.db().pool).await?;

            // Real stdio process and protocol, authenticated to the same UI manager.
            let binary = std::env::var("LVK_TEST_MCP_BINARY")?;
            let mut child = tokio::process::Command::new(binary).args(["--mode","global"]).env("MCP_HOST","127.0.0.1").env("MCP_PORT",address.port().to_string()).current_dir(&root).kill_on_drop(true).stdin(std::process::Stdio::piped()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::null()).spawn()?;
            let mut stdin = child.stdin.take().unwrap();
            let mut lines=BufReader::new(child.stdout.take().unwrap()).lines();
            async fn rpc(stdin:&mut tokio::process::ChildStdin,lines:&mut tokio::io::Lines<BufReader<tokio::process::ChildStdout>>,id:u32,method:&str,params:Value)->anyhow::Result<Value> {
                stdin.write_all(format!("{}\n",json!({"jsonrpc":"2.0","id":id,"method":method,"params":params})).as_bytes()).await?;
                let line=tokio::time::timeout(Duration::from_secs(30),lines.next_line()).await??.ok_or_else(||anyhow::anyhow!("stdio closed"))?;
                let reply:Value=serde_json::from_str(&line)?;
                ensure!(reply.get("error").is_none(),"{reply}");
                Ok(reply["result"].clone())
            }
            rpc(&mut stdin,&mut lines,1,"initialize",json!({"protocolVersion":"2025-03-26","capabilities":{},"clientInfo":{"name":"managed-devserver-fixture","version":"1"}})).await?;
            stdin.write_all(b"{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}\n").await?;
            let catalog=rpc(&mut stdin,&mut lines,2,"tools/list",json!({})).await?;
            for name in ["start_dev_server","stop_dev_server","get_dev_server_status"] {ensure!(catalog["tools"].as_array().unwrap().iter().any(|t|t["name"]==name));}
            let started=rpc(&mut stdin,&mut lines,3,"tools/call",json!({"name":"start_dev_server","arguments":target_args})).await?;
            ensure!(started["isError"]!=true,"{started}");
            let started:Value=serde_json::from_str(started["content"][0]["text"].as_str().unwrap())?;
            ensure!(started["accepted"]==true && started["state"]=="running","{started}");
            let id=started["processes"][0]["process_id"].clone();
            let observed=http.value("get_dev_server_status",target_args.clone()).await;
            ensure!(observed["processes"][0]["process_id"]==id,"shared state");
            let workspace=db::models::workspace::Workspace::find_by_id(&deployment.db().pool,target.workspace_id).await?.unwrap();
            let axum::Json(ui_response)=crate::routes::workspaces::execution::start_dev_server(axum::Extension(workspace),axum::extract::State(deployment.clone())).await?;
            ensure!(serde_json::to_value(ui_response)?["data"][0]["id"]==id,"UI duplicate start must reuse");
            // Model the manager's terminal-row-before-cleanup window.
            let process_id:Uuid=serde_json::from_value(id.clone())?;
            db::models::execution_process::ExecutionProcess::update_completion(&deployment.db().pool,process_id,db::models::execution_process::ExecutionProcessStatus::Killed,None).await?;
            ensure!(http.value("get_dev_server_status",target_args.clone()).await["state"]=="stopping_or_unconfirmed");
            let blocked=Uuid::new_v4();
            http.value("start_dev_server",json!({"project_id":target.project_id,"workspace_id":target.workspace_id,"request_id":blocked})).await;
            ensure!(http.settle(blocked).await["status"]=="failed","unsettled cleanup must block new starts");
            let token=std::fs::read_to_string(utils::assets::asset_dir().join("access-internal.token"))?;
            let response=reqwest::Client::builder().redirect(reqwest::redirect::Policy::none()).build()?.post(format!("http://{address}/api/execution-processes/{process_id}/stop")).header("x-lvk-internal-token",token.trim()).header("x-lvk-csrf","local").send().await?;
            ensure!(response.status().is_success(),"UI stop route failed");

            let a=Uuid::new_v4(); let b=Uuid::new_v4();
            let (ra,rb)=tokio::join!(http.value("start_dev_server",json!({"project_id":target.project_id,"workspace_id":target.workspace_id,"request_id":a})),http.value("start_dev_server",json!({"project_id":target.project_id,"workspace_id":target.workspace_id,"request_id":b})));
            ensure!(ra["operation_id"]!=Value::Null && rb["operation_id"]!=Value::Null);
            ensure!(http.settle(a).await["status"]=="succeeded");
            ensure!(http.settle(b).await["status"]=="succeeded");
            let after=http.value("get_dev_server_status",target_args.clone()).await;
            ensure!(after["processes"].as_array().unwrap().len()==1 && after["processes"][0]["process_id"]!=id,"duplicate start must reuse");
            let stop_request=Uuid::new_v4();
            let stop_args=json!({"project_id":target.project_id,"workspace_id":target.workspace_id,"request_id":stop_request});
            http.value("stop_dev_server",stop_args.clone()).await;
            ensure!(http.settle(stop_request).await["status"]=="succeeded");
            http.value("stop_dev_server",stop_args).await;
            let stopped=rpc(&mut stdin,&mut lines,4,"tools/call",json!({"name":"get_dev_server_status","arguments":target_args})).await?;
            let stopped:Value=serde_json::from_str(stopped["content"][0]["text"].as_str().unwrap())?;
            ensure!(stopped["state"]=="stopped" && stopped["process_tracking_settled"]==true,"{stopped}");
            let again=rpc(&mut stdin,&mut lines,5,"tools/call",json!({"name":"stop_dev_server","arguments":target_args})).await?;
            ensure!(again["isError"]!=true,"{again}");
            drop(stdin); child.wait().await?;
            let count:i64=sqlx::query_scalar("SELECT count(*) FROM execution_processes WHERE run_reason='devserver'").fetch_one(&deployment.db().pool).await?;
            ensure!(count==2,"duplicate start/status/stop must not create processes: {count}");
            if matches!(std::env::var("LVK_DEV_SERVER_REAL_AGENT").as_deref(),Ok("1"|"approval")) {
                let approval_only=std::env::var("LVK_DEV_SERVER_REAL_AGENT").as_deref()==Ok("approval");
                use executors::{profile::{ExecutorConfigs,ExecutorConfig},executors::{BaseCodingAgent,CodingAgent}};
                let mut profiles=ExecutorConfigs::get_cached();
                let CodingAgent::Codex(config)=profiles.executors.get_mut(&BaseCodingAgent::Codex).unwrap().configurations.get_mut("DEFAULT").unwrap() else {panic!("Codex profile")};
                config.cmd.additional_params=Some(vec!["-c".into(),format!("mcp_servers.lvk_fixture.command={}",serde_json::to_string(&std::env::var("LVK_TEST_MCP_BINARY")?)?),"-c".into(),"mcp_servers.lvk_fixture.args=[\"--mode\",\"global\"]".into(),"-c".into(),format!("mcp_servers.lvk_fixture.env={{ MCP_HOST=\"127.0.0.1\", MCP_PORT=\"{}\", LVK_INSTANCE_DIR={} }}",address.port(),serde_json::to_string(&root.to_string_lossy())?)]);
                profiles.save_overrides()?; ExecutorConfigs::reload();
                // The seeded fixture run belongs to no provider process.
                sqlx::query("UPDATE agent_runs SET status='succeeded' WHERE id=?").bind(target.agent_run_id).execute(&deployment.db().pool).await?;
                let session=db::models::session::Session::create(&deployment.db().pool,&db::models::session::CreateSession{executor:Some("CODEX".into()),name:Some("DevServer stdio acceptance".into())},Uuid::new_v4(),target.workspace_id).await?;
                let prompt=format!("これは隔離fixtureのMCP接続受入試験です。コード変更・shell・外部ツール・子agentは使用しないでください。lvk_fixture MCPのツール一覧に start_dev_server / stop_dev_server / get_dev_server_status があることを確認し、get_dev_server_status を project_id={} workspace_id={} で1回呼び出してください。起動・停止は実行しないでください。観測stateと利用可能な3ツール名だけを最終回答してください。",target.project_id,target.workspace_id);
                let run=crate::routes::sessions::start_coding_agent_execution_for_session(&deployment,session,prompt,None,ExecutorConfig::new(BaseCodingAgent::Codex),None,None).await?;
                let run_id=run.agent_run_id;
                let outcome=tokio::time::timeout(Duration::from_secs(180),async {
                    loop {
                        let status:String=sqlx::query_scalar("SELECT status FROM agent_runs WHERE id=?").bind(run_id).fetch_one(&deployment.db().pool).await?;
                        if (approval_only && status=="awaiting_approval") || matches!(status.as_str(),"succeeded"|"failed"|"cancelled"|"crashed"|"audit_failed") {return anyhow::Ok(status);}
                        tokio::time::sleep(Duration::from_secs(1)).await;
                    }
                }).await;
                if approval_only {
                    if !matches!(&outcome,Ok(Ok(status)) if status=="awaiting_approval") {
                        let _ = crate::routes::agent_runs::dispatch_control(&deployment,run_id,super::runtime_write::identity(Uuid::new_v4(),"fixture-timeout-cleanup".into()),executors::runtime::AgentRunPortCommand::Cancel{reason:"bounded approval reachability fixture timed out or ended".into()}).await;
                    }
                    ensure!(matches!(&outcome,Ok(Ok(status)) if status=="awaiting_approval"),"real agent never reached formal approval wait: {outcome:?}");
                    let raw:Vec<String>=sqlx::query_scalar("SELECT event_envelope FROM agent_events WHERE agent_run_id=? ORDER BY sequence").bind(run_id).fetch_all(&deployment.db().pool).await?;
                    let events:Vec<Value>=raw.iter().map(|v|serde_json::from_str(v).unwrap()).collect();
                    let approval=events.iter().find(|e|e["payload"]["type"]=="approval_requested").ok_or_else(||anyhow::anyhow!("approval event missing"))?;
                    let a=&approval["payload"]["data"];
                    let call=events.iter().find(|e|e["payload"]["type"]=="tool_call" && e["payload"]["data"]["tool_call_id"]==a["tool_call_id"]).ok_or_else(||anyhow::anyhow!("approval not bound to actual MCP item"))?;
                    ensure!(a["tool_name"].as_str().is_some_and(|name|name.contains("third-party") && name.contains("lvk_fixture") && name.contains("get_dev_server_status") && name.contains("one call only")));
                    ensure!(!events.iter().any(|e|e["payload"]["type"]=="approval_resolved"),"approval answered without user");
                    let evidence=json!({"agent_run_id":run_id,"state":"awaiting_approval","approval_event":approval,"bound_tool_event":call,"verification":"Formal canonical UI approval input reached; no human response submitted. Browser rendering and accepted tool execution remain unverified."});
                    std::fs::write(root.join("approval-reachability.json"),serde_json::to_vec_pretty(&evidence)?)?;
                    let _ = crate::routes::agent_runs::dispatch_control(&deployment,run_id,super::runtime_write::identity(Uuid::new_v4(),"fixture-cleanup".into()),executors::runtime::AgentRunPortCommand::Cancel{reason:"approval reachability fixture finished; no user approval impersonated".into()}).await?;
                    tokio::time::timeout(Duration::from_secs(30),async {
                        loop {let state:String=sqlx::query_scalar("SELECT status FROM agent_runs WHERE id=?").bind(run_id).fetch_one(&deployment.db().pool).await?;if matches!(state.as_str(),"cancelled"|"failed"|"crashed"|"audit_failed") {return anyhow::Ok(());}tokio::time::sleep(Duration::from_millis(100)).await;}
                    }).await??;
                    println!("PASS real LVK Codex requested formal one-call MCP approval; no permission response sent; run cancelled for fixture cleanup; agent_run_id={run_id}");
                } else {
                if !matches!(&outcome,Ok(Ok(status)) if status=="succeeded") {
                    let _=crate::routes::agent_runs::dispatch_control(&deployment,run_id,super::runtime_write::identity(Uuid::new_v4(),"fixture-cleanup".into()),executors::runtime::AgentRunPortCommand::Cancel{reason:"fixture cleanup".into()}).await;
                }
                ensure!(outcome??=="succeeded","real agent failed");
                let events:Vec<String>=sqlx::query_scalar("SELECT event_envelope FROM agent_events WHERE agent_run_id=?").bind(run_id).fetch_all(&deployment.db().pool).await?;
                let completed = events.iter().filter_map(|e|serde_json::from_str::<Value>(e).ok()).any(|e| {
                    let call=&e["payload"]["data"];
                    e["payload"]["type"]=="tool_call" && call["tool_name"].as_str().is_some_and(|name| name.contains("lvk_fixture") && name.contains("get_dev_server_status")) && call["status"]=="succeeded" && call["result"].to_string().contains("stopped")
                });
                ensure!(completed,"real agent did not successfully observe stopped through fixture stdio");
                println!("PASS real LVK Codex Session via agent-process-host called stdio get_dev_server_status; agent_run_id={run_id}");
                }
            }
            println!("PASS real stdio catalog/start/status/stop; remote HTTP catalog/scope/target/receipt/concurrent-start/shared-state; managed process count=2 across two cold starts and cleanup settled");
            anyhow::Ok(())
        }.await;
    // Always clean up our fixture's managed scripts before cancelling the host.
    let cleanup = crate::routes::workspaces::dev_server::change(
        &deployment,
        target.project_id,
        target.workspace_id,
        false,
    )
    .await;
    shutdown.cancel();
    server.await??;
    provider.abort();
    cleanup?;
    result
}
