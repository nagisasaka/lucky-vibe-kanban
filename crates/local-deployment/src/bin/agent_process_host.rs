#[tokio::main(flavor = "current_thread")]
async fn main() {
    if std::env::args().nth(1).as_deref() == Some("runtime-payload") {
        println!(
            "{}",
            serde_json::json!({"component": "agent-process-host", "protocol": 1})
        );
        return;
    }
    if let Err(error) = local_deployment::process_host::run_from_stdin().await {
        eprintln!("agent process host failed: {error}");
        std::process::exit(1);
    }
}
