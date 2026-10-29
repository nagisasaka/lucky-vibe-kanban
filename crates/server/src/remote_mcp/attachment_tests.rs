//! Real HTTP MCP + real stdio codecs and production attachment service.
//! Native coding-agent execution is intentionally not invoked by this fixture.
use anyhow::ensure;
use base64::{Engine, engine::general_purpose::STANDARD};
use rmcp::ServiceExt;
use sha2::{Digest, Sha256};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

use super::*;
use crate::routes::attachment_transfer;

struct NativeFixture {
    deployment: DeploymentImpl,
    root: std::path::PathBuf,
}
#[async_trait::async_trait]
impl attachment_transfer::Execution for NativeFixture {
    async fn start(
        &self,
        request: db::models::requests::CreateAndStartWorkspaceRequest,
        workspace_id: Uuid,
    ) -> Result<Value, crate::error::ApiError> {
        use db::models::{
            file::WorkspaceAttachment,
            session::{CreateSession, Session},
            workspace::{CreateWorkspace, Workspace},
            workspace_repo::{CreateWorkspaceRepo, WorkspaceRepo},
        };
        let d = &self.deployment;
        let pool = &d.db().pool;
        Workspace::create(
            pool,
            &CreateWorkspace {
                branch: format!("fixture-{workspace_id}"),
                name: request.name,
            },
            workspace_id,
        )
        .await?;
        let issue = request.linked_issue.unwrap();
        sqlx::query(
            "INSERT INTO local_workspace_links(workspace_id,project_id,issue_id) VALUES(?,?,?)",
        )
        .bind(workspace_id)
        .bind(issue.remote_project_id)
        .bind(issue.issue_id)
        .execute(pool)
        .await?;
        WorkspaceRepo::create_many(
            pool,
            workspace_id,
            &request
                .repos
                .iter()
                .map(|r| CreateWorkspaceRepo {
                    repo_id: r.repo_id,
                    target_branch: r.target_branch.clone(),
                })
                .collect::<Vec<_>>(),
        )
        .await?;
        let work = self.root.join(workspace_id.to_string());
        std::fs::create_dir_all(&work)?;
        for repo in WorkspaceRepo::find_repos_for_workspace(pool, workspace_id).await? {
            std::fs::create_dir_all(work.join(repo.name))?;
        }
        Workspace::update_container_ref(pool, workspace_id, work.to_str().unwrap()).await?;
        WorkspaceAttachment::associate_many_dedup(
            pool,
            workspace_id,
            request.attachment_ids.as_deref().unwrap_or_default(),
        )
        .await?;
        for id in request.attachment_ids.as_deref().unwrap_or_default() {
            let file = db::models::file::File::find_by_id(pool, *id)
                .await?
                .unwrap();
            assert!(request.prompt.contains(&file.file_path));
        }
        let session = Session::create(
            pool,
            &CreateSession {
                executor: Some("CODEX".into()),
                name: None,
            },
            Uuid::new_v4(),
            workspace_id,
        )
        .await?;
        d.file()
            .copy_files_by_workspace_to_worktree(
                &work,
                workspace_id,
                session.agent_working_dir.as_deref(),
            )
            .await?;
        let run = Uuid::new_v4();
        runtime_write_tests::insert_run(pool, session.id, workspace_id, run).await;
        Ok(
            json!({"workspace_id":workspace_id,"session_id":session.id,"agent_run_id":run,"execution_accepted":true}),
        )
    }
    async fn continue_session(
        &self,
        p: &attachment_transfer::Place,
        prompt: String,
        latest: Uuid,
    ) -> Result<Value, crate::error::ApiError> {
        for id in &p.attachment_ids {
            let file = db::models::file::File::find_by_id(&self.deployment.db().pool, *id)
                .await?
                .unwrap();
            assert!(prompt.contains(&file.file_path));
        }
        let current:Uuid=sqlx::query_scalar("SELECT id FROM agent_runs WHERE session_id=? ORDER BY created_at DESC,rowid DESC LIMIT 1").bind(p.session_id).fetch_one(&self.deployment.db().pool).await?;
        assert_eq!(current, latest);
        let v = attachment_transfer::place_now(&self.deployment, p)
            .await
            .map_err(|_| crate::error::ApiError::BadRequest("Fixture placement failed".into()))?;
        assert_eq!(v["all_verified"], true);
        let run = Uuid::new_v4();
        runtime_write_tests::insert_run(
            &self.deployment.db().pool,
            p.session_id,
            p.workspace_id,
            run,
        )
        .await;
        Ok(json!({"agent_run_id":run,"execution_accepted":true}))
    }
}
struct FixtureLaunch {
    native: Arc<NativeFixture>,
}
#[async_trait::async_trait]
impl launch::LaunchActions for FixtureLaunch {
    async fn defaults(&self) -> executors::profile::ExecutorConfig {
        executors::profile::ExecutorConfig::new(executors::executors::BaseCodingAgent::Codex)
    }
    async fn start(
        &self,
        request: db::models::requests::CreateAndStartWorkspaceRequest,
        id: Uuid,
    ) -> Result<Value, crate::error::ApiError> {
        attachment_transfer::Execution::start(self.native.as_ref(), request, id).await
    }
    async fn integration(
        &self,
        _request: crate::routes::integrations::CreateIntegrationRequest,
        _expected: String,
    ) -> Result<Value, crate::routes::integrations::AdmissionFailure> {
        unreachable!("not used by attachment tests")
    }
    async fn attachment_transfer(
        &self,
        name: &str,
        input: Value,
        actor: String,
    ) -> anyhow::Result<Value> {
        launch::LaunchActions::attachment_transfer(
            &launch::ExistingLaunch(self.native.deployment.clone()),
            name,
            input,
            actor,
        )
        .await
    }
    async fn place_for_execution(&self, args: attachment_transfer::Place) -> anyhow::Result<Value> {
        attachment_transfer::place_now(&self.native.deployment, &args).await
    }
}

#[test]
fn attachment_catalog_declares_host_file_contract_and_write_scope() {
    let catalog = attachments::tools();
    let t = serde_json::to_value(&catalog[0]).unwrap();
    assert_eq!(t["_meta"]["openai/fileParams"], json!(["file"]));
    let file = &t["inputSchema"]["properties"]["file"];
    assert_eq!(file["required"], json!(["download_url", "file_id"]));
    for name in ["download_url", "file_id", "mime_type", "file_name"] {
        assert_eq!(file["properties"][name]["type"], "string");
    }
    for name in ["upload_attachment", "place_attachments"] {
        assert_eq!(operation_scope(name), Some("lvk:write"));
    }
}

#[tokio::test]
#[ignore = "requires a fresh task-owned LVK_INSTANCE_DIR named attachment-fixture-*"]
async fn isolated_attachment_remote_stdio_e2e() -> anyhow::Result<()> {
    let root = std::path::PathBuf::from(std::env::var("LVK_INSTANCE_DIR")?);
    ensure!(
        root.file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("attachment-fixture-")
    );
    ensure!(
        !root.join("data/db.v2.sqlite").exists(),
        "fresh fixture required"
    );
    ensure!(
        std::path::PathBuf::from(std::env::var("XDG_CACHE_HOME")?).starts_with(&root),
        "fixture cache must be isolated too"
    );
    let shutdown = tokio_util::sync::CancellationToken::new();
    let deployment = DeploymentImpl::new(shutdown.clone()).await?;
    let (mut remote, runtime, _, target, issue) =
        runtime_write_tests::fixture_with_pool(deployment.db().pool.clone()).await;
    let native = Arc::new(NativeFixture {
        deployment: deployment.clone(),
        root: root.join("native-workspaces"),
    });
    remote.launch = Some(Arc::new(FixtureLaunch {
        native: native.clone(),
    }));
    let work = root.join("workspace");
    std::fs::create_dir_all(&work)?;
    sqlx::query("UPDATE workspaces SET container_ref=? WHERE id=?")
        .bind(work.to_string_lossy().as_ref())
        .bind(target.workspace_id)
        .execute(&remote.pool)
        .await?;
    let repo_root = root.join("repository");
    std::fs::create_dir(&repo_root)?;
    for args in [
        vec!["init", "-b", "main"],
        vec![
            "-c",
            "user.name=Attachment Fixture",
            "-c",
            "user.email=fixture@example.invalid",
            "commit",
            "--allow-empty",
            "-m",
            "fixture",
        ],
    ] {
        ensure!(
            std::process::Command::new("git")
                .args(args)
                .current_dir(&repo_root)
                .output()?
                .status
                .success()
        );
    }
    let repo = db::models::repo::Repo::find_or_create(&remote.pool, &repo_root, "material-fixture")
        .await?;
    db::models::workspace_repo::WorkspaceRepo::create_many(
        &remote.pool,
        target.workspace_id,
        &[db::models::workspace_repo::CreateWorkspaceRepo {
            repo_id: repo.id,
            target_branch: "main".into(),
        }],
    )
    .await?;
    sqlx::query("UPDATE local_issues SET description='Inspect attached material without changing it' WHERE id=?").bind(issue).execute(&remote.pool).await?;
    let (_, auth, provider) = tests::fixture().await;
    let http = environment_tests::serve(remote.clone(), auth).await;
    let local_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let local_base = format!("http://{}", local_listener.local_addr()?);
    let access =
        server_access::Access::from_env(deployment.db().pool.clone(), utils::assets::asset_dir())?;
    let local_router = access.layer(
        axum::Router::new()
            .nest(
                "/api",
                attachment_transfer::router_with_execution(native.clone()),
            )
            .route(
                "/api/sessions/{id}",
                axum::routing::get(
                    |axum::extract::State(d): axum::extract::State<DeploymentImpl>,
                     axum::extract::Path(id): axum::extract::Path<Uuid>| async move {
                        axum::Json(
                            utils::response::ApiResponse::<db::models::session::Session>::success(
                                db::models::session::Session::find_by_id(&d.db().pool, id)
                                    .await
                                    .unwrap()
                                    .unwrap(),
                            ),
                        )
                    },
                ),
            )
            .route(
                "/api/sessions/{id}/executor-config",
                axum::routing::get(
                    |axum::extract::State(d): axum::extract::State<DeploymentImpl>,
                     axum::extract::Path(id): axum::extract::Path<Uuid>| async move {
                        axum::Json(utils::response::ApiResponse::success(
                            crate::routes::sessions::executor_config::read_executor_config(
                                &d.db().pool,
                                id,
                            )
                            .await
                            .unwrap(),
                        ))
                    },
                ),
            )
            .nest(
                "/api/board",
                crate::routes::local_remote::board_api::router(),
            )
            .with_state(deployment.clone()),
    );
    let stop = shutdown.clone();
    let backend = tokio::spawn(async move {
        axum::serve(local_listener, local_router)
            .with_graceful_shutdown(async move { stop.cancelled().await })
            .await
    });
    let result=async {
        let count_before:i64=sqlx::query_scalar("SELECT COUNT(*) FROM agent_runs").fetch_one(&remote.pool).await?;
        let svg=b"<svg xmlns='http://www.w3.org/2000/svg'><path d='M0 0'/></svg>";
        let request=Uuid::new_v4();
        let args=json!({"project_id":target.project_id,"request_id":request,"original_name":"素材.svg","data_base64":STANDARD.encode(svg)});
        let (code,_)=http.rpc("read-only","tools/call",json!({"name":"upload_attachment","arguments":args})).await;ensure!(code==403);
        let secret="signed-url-must-not-escape";
        let refused=http.call("runtime","upload_attachment",json!({"project_id":target.project_id,"request_id":Uuid::new_v4(),"original_name":"x.png","file":{"download_url":format!("https://evil.invalid/file?secret={secret}"),"file_id":"opaque"}})).await;
        ensure!(refused["isError"]==true && !refused.to_string().contains(secret));
        let saved=http.value("upload_attachment",args.clone()).await;
        ensure!(saved["status"]=="succeeded","{saved}");
        ensure!(saved["result"]["attachment"]["size_bytes"]==svg.len());ensure!(saved["result"]["attachment"]["hash"]==format!("{:x}",Sha256::digest(svg)));
        let id=saved["result"]["attachment"]["id"].clone();
        let replay=http.value("upload_attachment",args.clone()).await;ensure!(saved["operation_id"]==replay["operation_id"]);
        // Unusable file reference models retry after a signed URL becomes unavailable.
        // Recovery reads the original receipt without any source/URL lookup.
        let mut unavailable=args.clone();unavailable.as_object_mut().unwrap().remove("data_base64");
        unavailable["file"]=json!({"download_url":"https://expired.invalid/file?signature=expired","file_id":"expired-ref"});
        ensure!(http.call("runtime","upload_attachment",unavailable).await["isError"]==true);
        let recovered=http.value("get_operation",json!({"project_id":target.project_id,"request_id":request})).await;
        ensure!(recovered["status"]=="succeeded" && recovered["operation_id"]==saved["operation_id"] && recovered["result"]["attachment"]["id"]==id);
        let mut changed=args;changed["data_base64"]=json!(STANDARD.encode(b"different"));
        ensure!(http.call("runtime","upload_attachment",changed).await["isError"]==true);
        let place=json!({"project_id":target.project_id,"workspace_id":target.workspace_id,"session_id":target.session_id,"request_id":Uuid::new_v4(),"attachment_ids":[id]});
        let mut bad=place.clone();bad["request_id"]=json!(Uuid::new_v4());bad["session_id"]=json!(Uuid::new_v4());
        let rejected=http.value("place_attachments",bad).await;ensure!(rejected["status"]=="failed");
        let placed=http.value("place_attachments",place.clone()).await;ensure!(placed["result"]["all_verified"]==true,"{placed}");
        let path=saved["result"]["attachment"]["file_path"].as_str().unwrap();
        ensure!(std::fs::read(work.join(path))?==svg);
        let mut unknown=place.clone();unknown["request_id"]=json!(Uuid::new_v4());unknown["attachment_ids"]=json!([Uuid::new_v4()]);
        ensure!(http.call("runtime","place_attachments",unknown).await["isError"]==true);
        let mut wrong_project=place.clone();wrong_project["project_id"]=json!(Uuid::new_v4());
        ensure!(http.call("runtime","place_attachments",wrong_project).await["isError"]==true);
        // Exercise stdio through the same byte-stream adapter as serve(stdio()).
        let server=mcp::task_server::McpServer::new_global(&local_base).init().await?;
        let (client,transport)=tokio::io::duplex(1024*1024);
        let (read,write)=tokio::io::split(transport);
        let serving=tokio::spawn(async move {server.serve((read,write)).await.unwrap().waiting().await.unwrap()});
        let (read,mut write)=tokio::io::split(client);let mut lines=BufReader::new(read).lines();
        let png=include_bytes!("../../../../packages/public/favicon.png");
        let stdio_request=Uuid::new_v4();
        let mut binary_id=Value::Null;
        for (n,method,params) in [
            (1,"initialize",json!({"protocolVersion":"2025-03-26","capabilities":{},"clientInfo":{"name":"attachment-fixture","version":"1"}})),
            (2,"tools/call",json!({"name":"upload_attachment","arguments":{"project_id":target.project_id,"request_id":stdio_request,"original_name":"binary.png","data_base64":STANDARD.encode(png)}})),
            (3,"tools/call",json!({"name":"get_attachment_operation","arguments":{"project_id":target.project_id,"request_id":stdio_request}})),
        ] {
            write.write_all(format!("{}\n",json!({"jsonrpc":"2.0","id":n,"method":method,"params":params})).as_bytes()).await?;
            let response:Value=serde_json::from_str(&lines.next_line().await?.unwrap())?;
            ensure!(response.get("error").is_none(),"{response}");
            if n==1 {write.write_all(b"{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}\n").await?;}
            else {
                ensure!(response["result"]["isError"]!=true,"{response}");
                let v:Value=serde_json::from_str(response["result"]["content"][0]["text"].as_str().unwrap())?;
                ensure!(v["status"]=="succeeded","{v}");binary_id=v["result"]["attachment"]["id"].clone();
            }
        }
        let stdio_place=json!({"project_id":target.project_id,"workspace_id":target.workspace_id,"session_id":target.session_id,"request_id":Uuid::new_v4(),"attachment_ids":[binary_id]});
        write.write_all(format!("{}\n",json!({"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"place_attachments","arguments":stdio_place}})).as_bytes()).await?;
        let reply:Value=serde_json::from_str(&lines.next_line().await?.unwrap())?;
        let v:Value=serde_json::from_str(reply["result"]["content"][0]["text"].as_str().unwrap())?;
        ensure!(v["result"]["all_verified"]==true,"{v}");
        let binary_path=v["result"]["results"][0]["attachment"]["file_path"].as_str().unwrap();
        ensure!(std::fs::read(work.join(binary_path))?==png);
        ensure!(v["result"]["results"][0]["attachment"]["size_bytes"]==png.len());ensure!(v["result"]["results"][0]["attachment"]["hash"]==format!("{:x}",Sha256::digest(png)));
        let count_saved:i64=sqlx::query_scalar("SELECT COUNT(*) FROM agent_runs").fetch_one(&remote.pool).await?;ensure!(count_saved==count_before);
        // New first input: real Remote admission/preflight, native boundary mocked.
        let revision:String=sqlx::query_scalar("SELECT updated_at FROM local_issues WHERE id=?").bind(issue).fetch_one(&remote.pool).await?;
        let start_request=Uuid::new_v4();
        let remote_start=json!({"project_id":target.project_id,"issue_id":issue,"request_id":start_request,"expected_revision":revision,"repositories":[{"repository_id":repo.id,"target_branch":"main"}],"attachment_ids":[id,binary_id]});
        let started=http.value("start_workspace",remote_start.clone()).await;
        let started=if started["status"]=="succeeded" {started} else {http.settle(start_request).await};
        ensure!(started["status"]=="succeeded","{started}");
        let start_replay=http.value("start_workspace",remote_start).await;ensure!(start_replay["operation_id"]==started["operation_id"]);
        let first_session:Uuid=serde_json::from_value(started["result"]["session_id"].clone())?;
        let first_workspace:Uuid=serde_json::from_value(started["result"]["workspace_id"].clone())?;
        let first_run:Uuid=serde_json::from_value(started["result"]["agent_run_id"].clone())?;
        let ws=db::models::workspace::Workspace::find_by_id(&remote.pool,first_workspace).await?.unwrap();
        let sess=db::models::session::Session::find_by_id(&remote.pool,first_session).await?.unwrap();
        let base=crate::routes::workspaces::attachments::resolve_session_base_path(&deployment,&ws,first_session).await?;
        ensure!(sess.agent_working_dir.is_some());ensure!(std::fs::read(base.join(path))?==svg);ensure!(std::fs::read(base.join(binary_path))?==png);
        let mut mismatch=place.clone();mismatch["request_id"]=json!(Uuid::new_v4());mismatch["session_id"]=json!(first_session);
        ensure!(http.value("place_attachments",mismatch).await["status"]=="failed");
        sqlx::query("UPDATE agent_runs SET status='succeeded' WHERE id=?").bind(first_run).execute(&remote.pool).await?;
        let continued=Uuid::new_v4();
        http.value("continue_execution",json!({"project_id":target.project_id,"workspace_id":first_workspace,"session_id":first_session,"agent_run_id":first_run,"request_id":continued,"prompt":"Continue with material","attachment_ids":[id,binary_id]})).await;
        ensure!(http.settle(continued).await["status"]=="succeeded");
        // Attached stdio first input and continuation use the real local API adapters.
        let stdio_start=json!({"name":"Material first input","prompt":"Inspect both attachments","executor":"CODEX","issue_id":issue,"request_id":Uuid::new_v4(),"repositories":[{"repo_id":repo.id,"branch":"main"}],"attachment_ids":[id,binary_id]});
        write.write_all(format!("{}\n",json!({"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"start_workspace","arguments":stdio_start}})).as_bytes()).await?;
        let reply:Value=serde_json::from_str(&lines.next_line().await?.unwrap())?;
        ensure!(reply["result"]["isError"]!=true,"{reply}");
        let first:Value=serde_json::from_str(reply["result"]["content"][0]["text"].as_str().unwrap())?;
        ensure!(first["status"]=="succeeded","{first}");
        let stdio_session:Uuid=serde_json::from_value(first["result"]["session_id"].clone())?;
        let stdio_run:Uuid=serde_json::from_value(first["result"]["agent_run_id"].clone())?;
        sqlx::query("UPDATE agent_runs SET status='succeeded' WHERE id=?").bind(stdio_run).execute(&remote.pool).await?;
        let follow_request=Uuid::new_v4();
        let follow=json!({"project_id":target.project_id,"session_id":stdio_session,"request_id":follow_request,"prompt":"Continue using material","attachment_ids":[id,binary_id]});
        for n in [6,7] {
            write.write_all(format!("{}\n",json!({"jsonrpc":"2.0","id":n,"method":"tools/call","params":{"name":"run_session_prompt","arguments":follow}})).as_bytes()).await?;
            let reply:Value=serde_json::from_str(&lines.next_line().await?.unwrap())?;
            ensure!(reply["result"]["isError"]!=true,"{reply}");
            let result:Value=serde_json::from_str(reply["result"]["content"][0]["text"].as_str().unwrap())?;
            ensure!(result["status"]=="succeeded","{result}");
        }
        let count:i64=sqlx::query_scalar("SELECT COUNT(*) FROM agent_runs WHERE session_id=?").bind(stdio_session).fetch_one(&remote.pool).await?;ensure!(count==2,"replay must not launch twice");
        drop(write);drop(lines);serving.await?;
        // One corrupt target yields an explicit partial result, never successful placement.
        std::fs::write(work.join(path),b"agent edit")?;
        let mut partial=place;partial["request_id"]=json!(Uuid::new_v4());partial["attachment_ids"]=json!([id,binary_id]);
        let failed=http.value("place_attachments",partial).await;
        ensure!(failed["status"]=="failed" && failed["result"]["all_verified"]==false,"{failed}");
        ensure!(failed["result"]["results"][0]["verified"]==false && failed["result"]["results"][1]["verified"]==true);
        let calls_before=runtime.calls.lock().await.len();
        let blocked=Uuid::new_v4();
        http.value("steer_execution",json!({"project_id":target.project_id,"workspace_id":target.workspace_id,"session_id":target.session_id,"agent_run_id":target.agent_run_id,"request_id":blocked,"prompt":"Must not be delivered","attachment_ids":[id]})).await;
        ensure!(http.settle(blocked).await["status"]=="failed");ensure!(runtime.calls.lock().await.len()==calls_before);
        let count_after:i64=sqlx::query_scalar("SELECT COUNT(*) FROM agent_runs").fetch_one(&remote.pool).await?;ensure!(count_before+3==count_after);
        ensure!(db::models::file::File::find_orphaned_files(&remote.pool).await?.is_empty());
        Ok::<_,anyhow::Error>(())
    }.await;
    shutdown.cancel();
    backend.await??;
    drop(http);
    provider.abort();
    result
}
