//! The same byte-stream adapter used by `serve(stdio())`, with isolated pipes
//! and a disposable HTTP API. No running LVK instance or model is contacted.
use std::time::Duration;

use rmcp::ServiceExt;
use serde_json::{Value, json};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    net::TcpListener,
};
use uuid::Uuid;

use super::McpServer;

#[tokio::test]
async fn stdio_codec_initialises_lists_existing_tools_and_calls_the_existing_api() {
    super::tests::install_rustls_provider();
    tokio::time::timeout(Duration::from_secs(10), async {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let project = Uuid::new_v4();
        let organization = Uuid::new_v4();
        let backend = tokio::spawn(async move {
            let mut paths = Vec::new();
            for _ in 0..2 {
                let (mut socket, _) = listener.accept().await.unwrap();
                let mut request = Vec::new();
                while !request.ends_with(b"\r\n\r\n") {
                    request.push(socket.read_u8().await.unwrap());
                    assert!(request.len() < 8192);
                }
                let request = String::from_utf8(request).unwrap();
                let line = request.lines().next().unwrap();
                assert!(line.starts_with("GET "), "{line}");
                paths.push(line.to_owned());
                let (status, body) = if line.starts_with("GET /api/containers/attempt-context?") {
                    ("404 Not Found", "{}".to_owned())
                } else {
                    assert_eq!(line, format!("GET /api/remote/projects?organization_id={organization} HTTP/1.1"));
                    ("200 OK", json!({"success":true,"data":{"projects":[{
                        "id":project,"organization_id":organization,"name":"Stdio fixture",
                        "color":"blue","sort_order":0,"created_at":"2026-10-13T00:00:00Z",
                        "updated_at":"2026-10-13T00:00:00Z"
                    }]},"message":null}).to_string())
                };
                socket.write_all(format!("HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).as_bytes()).await.unwrap();
            }
            paths
        });
        let server = McpServer::new_global(&format!("http://{address}"))
            .init().await.unwrap();
        let (client, transport) = tokio::io::duplex(65536);
        let (read, write) = tokio::io::split(transport);
        let serving = tokio::spawn(async move {
            server.serve((read, write)).await.unwrap().waiting().await.unwrap();
        });
        let (read, mut write) = tokio::io::split(client);
        let mut lines = BufReader::new(read).lines();
        for (id, method, params) in [
            (1, "initialize", json!({"protocolVersion":"2025-03-26","capabilities":{},"clientInfo":{"name":"stdio-compatibility-fixture","version":"1"}})),
            (2, "tools/list", json!({})),
            (3, "tools/call", json!({"name":"list_projects","arguments":{"organization_id":organization}})),
        ] {
            write.write_all(format!("{}\n",json!({"jsonrpc":"2.0","id":id,"method":method,"params":params})).as_bytes()).await.unwrap();
            let reply: Value = serde_json::from_str(&lines.next_line().await.unwrap().unwrap()).unwrap();
            assert_eq!(reply["id"], id);
            assert!(reply.get("error").is_none(), "{reply}");
            match id {
                1 => {
                    assert_eq!(reply["result"]["serverInfo"]["name"], "vibe-kanban-mcp");
                    assert_eq!(reply["result"]["protocolVersion"], "2025-03-26");
                    write.write_all(b"{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}\n").await.unwrap();
                }
                2 => {
                    let tools=reply["result"]["tools"].as_array().unwrap();
                    for name in ["list_projects","list_workspaces","delete_workspace","run_resource_operation"] {
                        assert!(tools.iter().any(|tool| tool["name"]==name), "missing {name}");
                    }
                    for name in ["get_context","get_connection"] {
                        assert!(!tools.iter().any(|tool| tool["name"]==name), "unexpected {name}");
                    }
                }
                3 => {
                    assert_ne!(reply["result"]["isError"], true);
                    let body:Value=serde_json::from_str(reply["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
                    assert_eq!(body["count"],1);
                    assert_eq!(body["projects"][0]["id"],project.to_string());
                    assert_eq!(body["projects"][0]["name"],"Stdio fixture");
                }
                _ => unreachable!(),
            }
        }
        drop(write);
        drop(lines);
        serving.await.unwrap();
        assert_eq!(backend.await.unwrap().len(),2);
    }).await.expect("stdio compatibility fixture timed out");
}
