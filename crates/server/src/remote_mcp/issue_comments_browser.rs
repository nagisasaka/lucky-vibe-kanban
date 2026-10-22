//! Opt-in browser bridge: real JWT middleware and MCP handlers against the
//! disposable UI server's database. Never included in a production build.
use std::{fs::OpenOptions, io::Write, os::unix::fs::OpenOptionsExt, time::Duration};

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use rsa::{
    RsaPrivateKey,
    pkcs8::{EncodePrivateKey, LineEnding},
    traits::PublicKeyParts,
};
use serde_json::json;

#[tokio::test]
#[ignore = "Requires the isolated issue-comments browser runner"]
async fn issue_comments_browser_bridge() {
    let root = std::fs::canonicalize(std::env::var("LVK_INSTANCE_DIR").expect("isolated instance"))
        .unwrap();
    assert_eq!(
        root.parent().unwrap(),
        std::fs::canonicalize("../../.build")
            .or_else(|_| std::fs::canonicalize(".build"))
            .unwrap()
    );
    assert!(
        root.file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .starts_with("issue-comments-instance-")
    );
    assert!(root.join("issue-comments-e2e-owner").is_file());
    let database = root.join("data/db.v2.sqlite");
    assert_eq!(std::fs::canonicalize(&database).unwrap(), database);
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(2)
        .connect(&format!("sqlite:{}", database.display()))
        .await
        .unwrap();
    let auth_pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    let directory = tempfile::tempdir_in(&root).unwrap();
    let key = RsaPrivateKey::new(&mut rand::rngs::OsRng, 2048).unwrap();
    let jwks = json!({"keys":[{"kty":"RSA","kid":"browser-fixture","use":"sig","alg":"RS256","n":URL_SAFE_NO_PAD.encode(key.n().to_bytes_be()),"e":URL_SAFE_NO_PAD.encode(key.e().to_bytes_be())}]});
    let access =
        server_access::test_support::access(auth_pool, directory.path().into(), jwks).await;
    let mut header = jsonwebtoken::Header::new(jsonwebtoken::Algorithm::RS256);
    header.kid = Some("browser-fixture".into());
    let pem = key.to_pkcs8_pem(LineEnding::LF).unwrap();
    let token = |scopes: &str| {
        jsonwebtoken::encode(&header,&json!({"iss":"https://fixture.auth0.com/","sub":"member","aud":access.resource(),"exp":server_access::now()+600,"scope":scopes}),&jsonwebtoken::EncodingKey::from_rsa_pem(pem.as_bytes()).unwrap()).unwrap()
    };
    let app = access.layer(super::access_router(
        pool.clone(),
        None,
        None,
        access.clone(),
        None,
    ));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}/mcp", listener.local_addr().unwrap());
    let (stop_tx, stop_rx) = tokio::sync::oneshot::channel::<()>();
    let mut server = tokio::spawn(async move {
        axum::serve(listener, app)
            .with_graceful_shutdown(async {
                let _ = stop_rx.await;
            })
            .await
            .unwrap();
    });
    let credentials = json!({"endpoint":endpoint,"host":"203.0.113.7:8443","writer":token("lvk:read lvk:write"),"reader":token("lvk:read")});
    let path = root.join("mcp-fixture.json");
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&path)
        .unwrap();
    file.write_all(credentials.to_string().as_bytes()).unwrap();
    drop(file);
    let completed = tokio::time::timeout(Duration::from_secs(300), async {
        while !root.join("mcp-fixture-stop").exists() {
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await
    .is_ok();
    let _ = stop_tx.send(());
    if tokio::time::timeout(Duration::from_secs(10), &mut server)
        .await
        .is_err()
    {
        server.abort();
        let _ = server.await;
    }
    std::fs::remove_file(path).unwrap();
    pool.close().await;
    assert!(completed, "Browser bridge timed out without cleanup signal");
}
