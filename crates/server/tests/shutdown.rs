//! Subprocess fixture for tests/embedded-browser-removal/shutdown.py.
//! Its environment and database must be isolated before the runtime starts.
#![cfg(unix)]

use std::path::PathBuf;

use tokio_util::sync::CancellationToken;

#[tokio::test]
#[ignore = "run through shutdown.py with a fresh LVK_INSTANCE_DIR"]
async fn library_shutdown_fixture() {
    let instance = PathBuf::from(std::env::var_os("LVK_SHUTDOWN_TEST_INSTANCE").unwrap());
    assert_eq!(
        std::env::var_os("LVK_INSTANCE_DIR").map(PathBuf::from),
        Some(instance.clone())
    );
    assert_eq!(
        std::fs::read_to_string(instance.join("shutdown-test-marker")).unwrap(),
        "isolated shutdown test"
    );
    assert!(
        instance
            .file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("local247-shutdown-")
    );
    assert!(!instance.join("data/db.v2.sqlite").exists());
    rustls::crypto::aws_lc_rs::default_provider()
        .install_default()
        .unwrap();
    tracing_subscriber::fmt().with_ansi(false).init();

    let token = CancellationToken::new();
    let handle = server::startup::start_with_bind("127.0.0.1:0", token.clone())
        .await
        .unwrap();
    let mut terminate =
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()).unwrap();
    tokio::spawn(async move {
        terminate.recv().await;
        token.cancel();
    });
    std::fs::write(instance.join("library-port"), handle.port.to_string()).unwrap();
    handle.serve().await.unwrap();
    println!("Library serve and cleanup completed");
}
