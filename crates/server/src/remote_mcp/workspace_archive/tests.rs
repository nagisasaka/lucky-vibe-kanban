//! Real authenticated HTTP /mcp -> tools/call -> shared archive/deletion.
//! Only script-store liveness is a port fixture; DB/OS guards and Git deletion
//! run normally against an owned TempDir and in-memory databases.
use std::sync::Arc;

use async_trait::async_trait;
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use db::models::workspace::{CreateWorkspace, Workspace};
use rsa::{
    RsaPrivateKey,
    pkcs8::{EncodePrivateKey, LineEnding},
    traits::PublicKeyParts,
};
use serde_json::{Value, json};
use uuid::Uuid;

use super::super::{
    questions::Kind,
    runtime_write::{Change, RuntimeActions, Target},
};
use crate::{
    error::ApiError,
    routes::workspaces::archive::{self, ConfirmationScope},
};

struct Runtime;
#[async_trait]
impl RuntimeActions for Runtime {
    async fn archive_scripts_settled(&self, _: Uuid) -> Result<bool, ApiError> {
        Ok(true)
    }
    async fn goal(
        &self,
        _: Target,
        _: Uuid,
        _: super::super::goals::Action,
        _: String,
    ) -> Result<Value, ApiError> {
        unreachable!()
    }
    async fn change(&self, _: Target, _: Uuid, _: String, _: Change) -> Result<Value, ApiError> {
        unreachable!()
    }
    async fn answer(
        &self,
        _: Target,
        _: Uuid,
        _: String,
        _: String,
        _: Kind,
        _: String,
        _: Option<Uuid>,
    ) -> Result<Value, ApiError> {
        unreachable!()
    }
}
struct Server(tokio::task::JoinHandle<()>);
impl Drop for Server {
    fn drop(&mut self) {
        self.0.abort();
    }
}

#[tokio::test]
async fn remote_archive_http_confirmation_authorization_races_and_results() {
    let (root, pool, ws, repo, session) = archive::tests::fixture().await;
    sqlx::query("UPDATE workspaces SET usage='execution_only' WHERE id=?")
        .bind(ws.id)
        .execute(&pool)
        .await
        .unwrap();
    let project = Uuid::new_v4();
    let other_project = Uuid::new_v4();
    for id in [project, other_project] {
        sqlx::query("INSERT INTO projects(id,name) VALUES(?,'archive remote fixture')")
            .bind(id)
            .execute(&pool)
            .await
            .unwrap();
    }
    let status = Uuid::new_v4();
    let issue = Uuid::new_v4();
    sqlx::query("INSERT INTO local_project_statuses(id,project_id,name,color,sort_order) VALUES(?,?,'Todo','blue',1)").bind(status).bind(project).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO local_issues(id,project_id,issue_number,simple_id,status_id,title,description,sort_order) VALUES(?,?,1,'TEST-1',?,'archive fixture','',0)").bind(issue).bind(project).bind(status).execute(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO local_workspace_links(workspace_id,project_id,issue_id) VALUES(?,?,?)",
    )
    .bind(ws.id)
    .bind(project)
    .bind(issue)
    .execute(&pool)
    .await
    .unwrap();
    let other = Workspace::create(
        &pool,
        &CreateWorkspace {
            branch: "other".into(),
            name: None,
        },
        Uuid::new_v4(),
    )
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO local_workspace_links(workspace_id,project_id,issue_id) VALUES(?,?,?)",
    )
    .bind(other.id)
    .bind(project)
    .bind(issue)
    .execute(&pool)
    .await
    .unwrap();

    let key = RsaPrivateKey::new(&mut rand::rngs::OsRng, 2048).unwrap();
    let jwks = json!({"keys":[{"kty":"RSA","kid":"archive-fixture","use":"sig","alg":"RS256","n":URL_SAFE_NO_PAD.encode(key.n().to_bytes_be()),"e":URL_SAFE_NO_PAD.encode(key.e().to_bytes_be())}]});
    // Identity fixtures have their own DB; domain data uses all real migrations.
    let identity_pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
    let access =
        server_access::test_support::access(identity_pool.clone(), root.path().into(), jwks).await;
    sqlx::query("INSERT INTO access_members(id,issuer,subject,display_name,state,role,created_at) VALUES(?,'https://fixture.auth0.com/','other-member','other','active','member',0)").bind(Uuid::new_v4().to_string()).execute(&identity_pool).await.unwrap();
    let app = access.layer(super::super::access_router(
        pool.clone(),
        Some(Arc::new(Runtime)),
        None,
        access.clone(),
        None,
    ));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/mcp", listener.local_addr().unwrap());
    let mut server = Server(tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    }));
    let pem = key.to_pkcs8_pem(LineEnding::LF).unwrap();
    let mut header = jsonwebtoken::Header::new(jsonwebtoken::Algorithm::RS256);
    header.kid = Some("archive-fixture".into());
    let token = |subject: &str, scope: &str| {
        jsonwebtoken::encode(&header,&json!({"iss":"https://fixture.auth0.com/","sub":subject,"aud":access.resource(),"exp":server_access::now()+300,"scope":scope}),&jsonwebtoken::EncodingKey::from_rsa_pem(pem.as_bytes()).unwrap()).unwrap()
    };
    let writer = token("member", "lvk:read lvk:write");
    let reader = token("member", "lvk:read");
    let foreign = token("other-member", "lvk:read lvk:write");
    let client = reqwest::Client::new();
    let request = |bearer: &str, body: Value| {
        client
            .post(&url)
            .header("host", "203.0.113.7:8443")
            .header("accept", "application/json, text/event-stream")
            .header("MCP-Protocol-Version", "2025-03-26")
            .bearer_auth(bearer)
            .json(&body)
    };
    let tool = |name: &str, args: Value| json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":name,"arguments":args}});
    let target = json!({"project_id":project,"workspace_id":ws.id});
    let prepare = tool("prepare_workspace_archive", target.clone());
    assert_eq!(
        request("invalid", prepare.clone())
            .send()
            .await
            .unwrap()
            .status(),
        401
    );
    for name in ["prepare_workspace_archive", "confirm_workspace_archive"] {
        assert_eq!(
            request(&reader, tool(name, target.clone()))
                .send()
                .await
                .unwrap()
                .status(),
            403
        );
    }
    let catalog: Value = request(
        &writer,
        json!({"jsonrpc":"2.0","id":2,"method":"tools/list"}),
    )
    .send()
    .await
    .unwrap()
    .json()
    .await
    .unwrap();
    for name in ["prepare_workspace_archive", "confirm_workspace_archive"] {
        let entry = catalog["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .find(|t| t["name"] == name)
            .unwrap();
        assert!(
            entry["inputSchema"]["required"]
                .as_array()
                .unwrap()
                .contains(&json!("project_id"))
        );
        assert!(
            entry["inputSchema"]["required"]
                .as_array()
                .unwrap()
                .contains(&json!("workspace_id"))
        );
        assert_eq!(
            entry["_meta"]["securitySchemes"][0]["scopes"],
            json!(["lvk:read", "lvk:write"])
        );
    }
    let result: Value = request(&writer, prepare.clone())
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let prepared = &result["result"]["structuredContent"];
    assert_eq!(prepared["message"], archive::CONFIRMATION);
    assert_eq!(prepared["workspace_id"], json!(ws.id));
    assert!(prepared["expires_at"].is_string());
    let confirmation: Uuid = prepared["confirmation_token"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();
    let path = std::path::Path::new(ws.container_ref.as_ref().unwrap());
    assert!(path.exists());
    assert!(
        !Workspace::find_by_id(&pool, ws.id)
            .await
            .unwrap()
            .unwrap()
            .archived
    );
    // No confirm is cancellation. The same prepared target remains untouched.
    let confirm_args = |token: Uuid| json!({"project_id":project,"workspace_id":ws.id,"confirmation_token":token,"request_id":Uuid::new_v4()});
    // A remote token cannot be consumed through the UI/stdio local boundary.
    assert!(
        archive::archive_scoped(
            &pool,
            ws.id,
            confirmation,
            ConfirmationScope::Local,
            || async { Ok(true) }
        )
        .await
        .is_err()
    );
    for (bearer, field, value) in [
        (&foreign, "workspace_id", json!(ws.id)),
        (&writer, "workspace_id", json!(other.id)),
        (&writer, "project_id", json!(other_project)),
    ] {
        let mut args = confirm_args(confirmation);
        args[field] = value;
        let refused: Value = request(bearer, tool("confirm_workspace_archive", args))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert!(
            refused["result"]["isError"] == true
                || refused["result"]["structuredContent"]["status"] == "failed",
            "{refused}"
        );
        assert!(path.exists());
    }
    for invalid_project in [other_project, Uuid::new_v4()] {
        let bad = json!({"project_id":invalid_project,"workspace_id":ws.id});
        let result: Value = request(&writer, tool("prepare_workspace_archive", bad))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(result["result"]["isError"], true);
    }
    // Membership revocation is checked at the real Access boundary again on
    // confirm, even when the caller previously obtained a valid token.
    sqlx::query("UPDATE access_members SET state='disabled' WHERE subject='member'")
        .execute(&identity_pool)
        .await
        .unwrap();
    assert_eq!(
        request(
            &writer,
            tool("confirm_workspace_archive", confirm_args(confirmation))
        )
        .send()
        .await
        .unwrap()
        .status(),
        403
    );
    sqlx::query("UPDATE access_members SET state='active' WHERE subject='member'")
        .execute(&identity_pool)
        .await
        .unwrap();

    // Local UI/stdio preparation cannot be confirmed over remote MCP either.
    let local = archive::prepare_scoped(&pool, &ws, ConfirmationScope::Local)
        .await
        .unwrap();
    let result: Value = request(
        &writer,
        tool(
            "confirm_workspace_archive",
            confirm_args(local.confirmation_token),
        ),
    )
    .send()
    .await
    .unwrap()
    .json()
    .await
    .unwrap();
    assert_eq!(result["result"]["structuredContent"]["status"], "failed");
    // Expiry is advanced only on this fixture's token; no wall-clock sleep.
    archive::expire_for_test(confirmation).await;
    let result: Value = request(
        &writer,
        tool("confirm_workspace_archive", confirm_args(confirmation)),
    )
    .send()
    .await
    .unwrap()
    .json()
    .await
    .unwrap();
    assert_eq!(result["result"]["structuredContent"]["status"], "failed");

    // Busy begins after preparation. Both the next prepare and confirm refuse.
    let prepared: Value = request(&writer, prepare.clone())
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let confirmation = prepared["result"]["structuredContent"]["confirmation_token"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();
    sqlx::query("INSERT INTO execution_processes(id,session_id,run_reason,executor_action,status) VALUES(?,?,'devserver','{}','running')").bind(Uuid::new_v4()).bind(session.id).execute(&pool).await.unwrap();
    let busy: Value = request(&writer, prepare.clone())
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(busy["result"]["isError"], true);
    let busy: Value = request(
        &writer,
        tool("confirm_workspace_archive", confirm_args(confirmation)),
    )
    .send()
    .await
    .unwrap()
    .json()
    .await
    .unwrap();
    assert_eq!(busy["result"]["structuredContent"]["status"], "failed");
    assert!(path.exists());
    assert!(
        !Workspace::find_by_id(&pool, ws.id)
            .await
            .unwrap()
            .unwrap()
            .archived
    );
    sqlx::query("UPDATE execution_processes SET status='completed'")
        .execute(&pool)
        .await
        .unwrap();

    // Exact confirmation target changes cannot silently authorize another tree.
    let prepared: Value = request(&writer, prepare.clone())
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let confirmation = prepared["result"]["structuredContent"]["confirmation_token"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();
    sqlx::query("UPDATE workspaces SET branch='changed' WHERE id=?")
        .bind(ws.id)
        .execute(&pool)
        .await
        .unwrap();
    let changed: Value = request(
        &writer,
        tool("confirm_workspace_archive", confirm_args(confirmation)),
    )
    .send()
    .await
    .unwrap()
    .json()
    .await
    .unwrap();
    assert_eq!(changed["result"]["structuredContent"]["status"], "failed");
    sqlx::query("UPDATE workspaces SET branch=? WHERE id=?")
        .bind(&ws.branch)
        .bind(ws.id)
        .execute(&pool)
        .await
        .unwrap();

    let prepared: Value = request(&writer, prepare)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let confirmation = prepared["result"]["structuredContent"]["confirmation_token"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();
    let args = confirm_args(confirmation);
    // Discard the response body, then recover the original durable result.
    drop(
        request(&writer, tool("confirm_workspace_archive", args.clone()))
            .send()
            .await
            .unwrap(),
    );
    let receipt: Value = request(
        &writer,
        tool(
            "get_operation",
            json!({"project_id":project,"request_id":args["request_id"]}),
        ),
    )
    .send()
    .await
    .unwrap()
    .json()
    .await
    .unwrap();
    let receipt = &receipt["result"]["structuredContent"];
    assert_eq!(receipt["status"], "succeeded", "{receipt}");
    assert_eq!(receipt["result"]["archived"], true);
    assert_eq!(receipt["result"]["worktree_deleted"], true);
    assert!(receipt["result"]["deletion_error"].is_null());
    let current: Value = request(&writer, tool("get_workspace", target.clone()))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(
        current["result"]["structuredContent"]["worktree_deleted"],
        true
    );
    assert!(
        Workspace::find_by_id(&pool, ws.id)
            .await
            .unwrap()
            .unwrap()
            .is_execution_only()
    );
    assert!(!path.exists());
    let retry: Value = request(&writer, tool("confirm_workspace_archive", args.clone()))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(
        retry["result"]["structuredContent"]["operation_id"],
        receipt["operation_id"]
    );
    let replay: Value = request(
        &writer,
        tool("confirm_workspace_archive", confirm_args(confirmation)),
    )
    .send()
    .await
    .unwrap()
    .json()
    .await
    .unwrap();
    assert_eq!(replay["result"]["structuredContent"]["status"], "failed");
    let foreign_receipt: Value = request(
        &foreign,
        tool(
            "get_operation",
            json!({"project_id":project,"request_id":args["request_id"]}),
        ),
    )
    .send()
    .await
    .unwrap()
    .json()
    .await
    .unwrap();
    assert_eq!(foreign_receipt["result"]["isError"], true);
    assert!(
        db::models::session::Session::find_by_id(&pool, session.id)
            .await
            .unwrap()
            .is_some()
    );
    assert!(
        std::process::Command::new("git")
            .arg("-C")
            .arg(&repo.path)
            .args(["show-ref", "--verify", &format!("refs/heads/{}", ws.branch)])
            .output()
            .unwrap()
            .status
            .success()
    );

    // Real deletion failure on another owned fixture, even when running as root.
    let invalid = root.path().join("not-a-directory");
    std::fs::write(&invalid, "retain").unwrap();
    Workspace::update_container_ref(&pool, other.id, invalid.to_str().unwrap())
        .await
        .unwrap();
    let prepared: Value = request(
        &writer,
        tool(
            "prepare_workspace_archive",
            json!({"project_id":project,"workspace_id":other.id}),
        ),
    )
    .send()
    .await
    .unwrap()
    .json()
    .await
    .unwrap();
    let failed_args = json!({"project_id":project,"workspace_id":other.id,"confirmation_token":prepared["result"]["structuredContent"]["confirmation_token"],"request_id":Uuid::new_v4()});
    let failure: Value = request(&writer, tool("confirm_workspace_archive", failed_args))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let result = &failure["result"]["structuredContent"]["result"];
    assert_eq!(result["archived"], true, "{failure}");
    assert_eq!(result["worktree_deleted"], false);
    let error = result["deletion_error"].as_str().unwrap();
    assert!(error.starts_with("ワークツリーの物理削除に失敗しました（"));
    assert!(error.ends_with("）。手動で削除してください。"));
    assert!(
        error.contains("Not a directory") || error.contains("os error"),
        "{error}"
    );
    let saved = Workspace::find_by_id(&pool, other.id)
        .await
        .unwrap()
        .unwrap();
    assert!(saved.archived && !saved.worktree_deleted);
    assert!(invalid.exists());
    println!(
        "remote archive evidence: signed JWT -> Access -> POST /mcp -> tools/list + tools/call -> common archive; catalog/auth/scope/project/subject/target/local-token/expiry/replay/busy/race/failure/history/branch/receipt PASS"
    );
    server.0.abort();
    let _ = (&mut server.0).await;
}

#[tokio::test]
async fn remote_archive_project_allowlist_is_checked_at_both_transport_calls() {
    let (mut remote, auth, provider) = super::super::tests::fixture().await;
    let mut provider = Server(provider);
    remote.runtime = Some(Arc::new(Runtime));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/mcp", listener.local_addr().unwrap());
    let mut server = Server(tokio::spawn(async move {
        axum::serve(listener, super::super::router(remote, auth))
            .await
            .unwrap();
    }));
    for tool in ["prepare_workspace_archive", "confirm_workspace_archive"] {
        // Project 106 exists in this fixture but is outside its connection's
        // allowlist. Denial must happen before target lookup or token use.
        let mut args = json!({"project_id":Uuid::from_u128(106),"workspace_id":Uuid::new_v4()});
        if tool == "confirm_workspace_archive" {
            args["confirmation_token"] = json!(Uuid::new_v4());
            args["request_id"] = json!(Uuid::new_v4());
        }
        let response:Value = reqwest::Client::new().post(&url)
            .header("host", "lvk.example").header("accept", "application/json, text/event-stream")
            .bearer_auth("valid").json(&json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":tool,"arguments":args}}))
            .send().await.unwrap().json().await.unwrap();
        assert_eq!(response["result"]["isError"], true, "{response}");
        assert!(
            response["result"]["content"][0]["text"]
                .as_str()
                .unwrap()
                .contains("Project is not available"),
            "{response}"
        );
    }
    server.0.abort();
    let _ = (&mut server.0).await;
    provider.0.abort();
    let _ = (&mut provider.0).await;
}
