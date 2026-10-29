//! Thin stdio delegation to the same authenticated, durable remote MCP operation.
use rmcp::{
    ErrorData, handler::server::wrapper::Parameters, model::CallToolResult, schemars, tool,
    tool_router,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use uuid::Uuid;

use super::McpServer;

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct Restart {
    project_id: Uuid,
    request_id: Uuid,
    purpose: String,
    bundle: String,
    no_new_work: bool,
}
#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct Status {
    project_id: Uuid,
    request_id: Uuid,
}

#[tool_router(router = self_restart_tools_router, vis = "pub")]
impl McpServer {
    #[tool(
        description = "Explicit developer-instance self restart through the existing authenticated durable engine. Requires user authorisation, no_new_work coordination and enabled runtime execution. After durable admission end this Codex Run so its children/writers can settle; admission is not restart success. The engine waits 30 seconds and rechecks once only if its initial preflight is busy. Query get_self_restart_operation with the same request_id after reconnecting. Never substitute run_process or kill."
    )]
    async fn self_restart(
        &self,
        Parameters(a): Parameters<Restart>,
    ) -> Result<CallToolResult, ErrorData> {
        self.restart_rpc("self_restart", json!(a)).await
    }
    #[tool(
        description = "Read the existing durable self restart receipt/result using the original project_id and request_id. Does not restart, retry or approve anything; admission is separate from successful cutover."
    )]
    async fn get_self_restart_operation(
        &self,
        Parameters(a): Parameters<Status>,
    ) -> Result<CallToolResult, ErrorData> {
        self.restart_rpc("get_self_restart_operation", json!(a))
            .await
    }
}
impl McpServer {
    async fn restart_rpc(&self, name: &str, arguments: Value) -> Result<CallToolResult, ErrorData> {
        // Server access verifies the existing loopback capability. The adapter
        // delegates to the remote restart implementation, with no stdio sleep.
        match self
            .send_json::<Value>(
                self.client
                    .post(self.url(&format!("/api/self-restart/{name}")))
                    .timeout(std::time::Duration::from_secs(20))
                    .json(&arguments),
            )
            .await
        {
            Ok(value) => Self::success(&value),
            Err(_) => Self::err(
                "Self restart response unavailable; query the same request_id before retrying",
                None,
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    use super::*;

    #[tokio::test]
    async fn delegates_same_request_to_formal_route_and_preserves_admission_and_errors() {
        crate::task_server::tools::tests::install_rustls_provider();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let server = McpServer::new_global(&format!("http://{}", listener.local_addr().unwrap()));
        let args = json!({"project_id":Uuid::new_v4(),"request_id":Uuid::new_v4(),"purpose":"fixture","bundle":"/fixture","no_new_work":true});
        let expected = args.clone();
        let backend = tokio::spawn(async move {
            for name in ["self_restart", "self_restart", "get_self_restart_operation"] {
                let (mut socket, _) = listener.accept().await.unwrap();
                let mut head = Vec::new();
                while !head.ends_with(b"\r\n\r\n") {
                    head.push(socket.read_u8().await.unwrap());
                }
                let head = String::from_utf8(head).unwrap();
                assert!(head.starts_with(&format!("POST /api/self-restart/{name} HTTP/1.1")));
                let size: usize = head
                    .lines()
                    .find_map(|l| {
                        l.to_lowercase()
                            .strip_prefix("content-length: ")
                            .map(str::to_owned)
                    })
                    .unwrap()
                    .parse()
                    .unwrap();
                let mut body = vec![0; size];
                socket.read_exact(&mut body).await.unwrap();
                let rpc: Value = serde_json::from_slice(&body).unwrap();

                assert_eq!(rpc["request_id"], expected["request_id"]);
                let value = if name == "self_restart" {
                    assert_eq!(rpc, expected);
                    json!({"success":true,"data":{"admission":"durable accepted","cutover_success":false}})
                } else {
                    json!({"success":false,"message":"owner denied"})
                };
                let body = value.to_string();
                socket.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{}",body.len(),body).as_bytes()).await.unwrap();
            }
        });
        for _ in 0..2 {
            assert_ne!(
                server
                    .restart_rpc("self_restart", args.clone())
                    .await
                    .unwrap()
                    .is_error,
                Some(true)
            );
        }
        assert_eq!(
            server
                .restart_rpc(
                    "get_self_restart_operation",
                    json!({"project_id":args["project_id"],"request_id":args["request_id"]})
                )
                .await
                .unwrap()
                .is_error,
            Some(true)
        );
        backend.await.unwrap();
        let mut invalid = args;
        invalid["instance"] = json!("/other");
        assert!(serde_json::from_value::<Restart>(invalid).is_err());
        for router in [
            McpServer::global_mode_router(),
            McpServer::orchestrator_mode_router(),
        ] {
            let tools = router.list_all();
            for name in ["self_restart", "get_self_restart_operation"] {
                assert!(tools.iter().any(|t| t.name == name));
            }
        }
    }
}
