use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};

use super::*;

async fn backend(status: u16, body: &str) -> (McpServer, tokio::task::JoinHandle<()>) {
    super::tests::install_rustls_provider();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let body = body.to_owned();
    let worker = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut request = Vec::new();
        while !request.ends_with(b"\r\n\r\n") {
            request.push(stream.read_u8().await.unwrap());
            assert!(request.len() < 8192);
        }
        stream.write_all(format!("HTTP/1.1 {status} Fixture\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).as_bytes()).await.unwrap();
    });
    (McpServer::new_global(&format!("http://{address}")), worker)
}

#[tokio::test]
async fn http_errors_classify_without_echoing_backend_or_request_data() {
    for (status, category) in [
        (400, Category::Validation),
        (401, Category::Permission),
        (403, Category::Permission),
        (404, Category::NotFound),
        (409, Category::Conflict),
        (423, Category::Busy),
        (429, Category::Busy),
        (500, Category::Internal),
        (503, Category::Unavailable),
    ] {
        for read in [true, false] {
            let (server, worker) =
                backend(status, "SELECT credential FROM /private/input-SECRET").await;
            let url = server.url("/private/request-SECRET");
            let request = if read {
                server.client.get(url)
            } else {
                server.client.post(url)
            };
            let error = server
                .send_json::<serde_json::Value>(request)
                .await
                .unwrap_err();
            assert_eq!(error.diagnostic.category, category);
            assert_eq!(
                error.diagnostic.outcome,
                if read || matches!(status, 401 | 403) {
                    Outcome::Failed
                } else {
                    Outcome::Unknown
                }
            );
            let result = McpServer::tool_error(error);
            assert_eq!(result.is_error, Some(true));
            let public = serde_json::to_string(&result).unwrap();
            for marker in ["SELECT", "credential", "/private", "SECRET"] {
                assert!(!public.contains(marker), "{public}");
            }
            worker.await.unwrap();
        }
    }
}

#[tokio::test]
async fn response_loss_and_invalid_envelopes_do_not_prove_a_mutation_failed() {
    for body in [
        "malformed SECRET /private/sql",
        r#"{"success":false,"data":null,"message":"SECRET /private/sql"}"#,
        r#"{"success":true,"data":null}"#,
    ] {
        for read in [true, false] {
            let (server, worker) = backend(200, body).await;
            let request = if read {
                server.client.get(server.url("/fixture"))
            } else {
                server.client.post(server.url("/fixture"))
            };
            let error = server
                .send_json::<serde_json::Value>(request)
                .await
                .unwrap_err();
            assert_eq!(
                error.diagnostic.outcome,
                if read {
                    Outcome::Failed
                } else {
                    Outcome::Unknown
                }
            );
            let value = McpServer::tool_error(error);
            assert!(!serde_json::to_string(&value).unwrap().contains("SECRET"));
            worker.await.unwrap();
        }
    }
}

#[tokio::test]
async fn successful_and_pending_payloads_are_preserved() {
    for data in [
        serde_json::json!({"status":"running","request_id":Uuid::new_v4()}),
        serde_json::json!({"id":"unchanged","status":"succeeded"}),
    ] {
        let body = serde_json::json!({"success":true,"data":data}).to_string();
        let (server, worker) = backend(200, &body).await;
        let actual: serde_json::Value = server
            .send_json(server.client.post(server.url("/fixture")))
            .await
            .unwrap();
        assert_eq!(actual, data);
        worker.await.unwrap();
    }
}

#[tokio::test]
async fn read_only_posts_do_not_report_unknown_side_effects() {
    let (server, worker) = backend(503, "SECRET /private/query").await;
    let error = server
        .send_read_json::<serde_json::Value>(server.client.post(server.url("/fixture")))
        .await
        .unwrap_err();
    assert_eq!(error.diagnostic.category, Category::Unavailable);
    assert_eq!(error.diagnostic.outcome, Outcome::Failed);
    worker.await.unwrap();
}

#[tokio::test]
async fn accepted_request_with_disconnected_response_is_unknown_without_automatic_retry() {
    super::tests::install_rustls_provider();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let worker = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut request = Vec::new();
        while !request.ends_with(b"\r\n\r\n") {
            request.push(stream.read_u8().await.unwrap());
        }
        // The backend accepted this invocation; its response is lost.
        drop(stream);
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(100), listener.accept())
                .await
                .is_err(),
            "MCP must not automatically replay the request"
        );
    });
    let server = McpServer::new_global(&format!("http://{address}"));
    let error = server
        .send_json::<serde_json::Value>(server.client.post(server.url("/fixture")))
        .await
        .unwrap_err();
    assert_eq!(error.diagnostic.outcome, Outcome::Unknown);
    assert_eq!(error.diagnostic.recovery, "inspect_receipt_and_state");
    worker.await.unwrap();
}
