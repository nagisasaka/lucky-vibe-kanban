//! Downstream test fixture. No environment switch, HTTP bypass, or production
//! issuer-validation relaxation is provided. Normal server builds omit this module.
use super::*;
pub async fn access(pool: SqlitePool, directory: PathBuf, jwks: serde_json::Value) -> Access {
    let issuer = "https://fixture.auth0.com/";
    provider::validate_issuer(issuer).unwrap();
    sqlx::raw_sql(include_str!(
        "../../db/migrations/20261018010000_chatgpt_client_credentials.sql"
    ))
    .execute(&pool)
    .await
    .unwrap();
    sqlx::raw_sql(include_str!(
        "../../db/migrations/20261015000000_server_access.sql"
    ))
    .execute(&pool)
    .await
    .unwrap();
    sqlx::raw_sql(include_str!(
        "../../db/migrations/20261015010000_preview_access.sql"
    ))
    .execute(&pool)
    .await
    .unwrap();
    let key_path = directory.join("access.key");
    store::ensure_key(&key_path).unwrap();
    let access = Access(Arc::new(Inner {
        pool,
        preview_domain: Some("preview.example".into()),
        public: true,
        public_url: Some("https://203.0.113.7:8443".into()),
        forced_mode: None,
        key_path,
        internal_token: store::random(),
        provider: Mutex::new(provider::fixture_cache(issuer, jwks)),
        attempts: Mutex::new((now(), 0)),
        password_slots: tokio::sync::Semaphore::new(2),
        provisioning: Mutex::new(()),
    }));
    let config = Config::Auth0 {
        issuer: issuer.into(),
        client_id: "web-client".into(),
        client_secret: "fixture-only".into(),
        mcp_enabled: true,
    };
    sqlx::query("INSERT INTO access_settings(id,revision,encrypted) VALUES(1,1,?)")
        .bind(access.encrypt(&config).unwrap())
        .execute(access.pool())
        .await
        .unwrap();
    access.identity(issuer, "member", None).await.unwrap();
    sqlx::query("UPDATE access_members SET state='active'")
        .execute(access.pool())
        .await
        .unwrap();
    access
}

pub async fn web_session(access: &Access) -> String {
    let member = access
        .identity("https://fixture.auth0.com/", "member", None)
        .await
        .unwrap();
    let (token, _) = access
        .create_session(Some(&member.id), "web", 1)
        .await
        .unwrap();
    format!("lvk_session={token}")
}
