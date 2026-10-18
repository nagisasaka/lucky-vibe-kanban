use super::*;

const PATH: &str = "/api/access/auth0/chatgpt-credentials";

#[tokio::test]
async fn chatgpt_secret_survives_activation_and_restart_with_admin_only_access() {
    let mut f = Fixture::new(true).await;
    let idp = Idp::new().await;
    let (setup_cookie, setup_csrf) = f.setup().await;
    let mut input = json!({"action":"save","issuer":idp.state.issuer,
        "web_client_id":"web-client","client_id":"chat-client","client_secret":"retained-chat-secret"});
    let response = f
        .call(
            "POST",
            PATH,
            &[("cookie", &setup_cookie), ("x-lvk-csrf", &setup_csrf)],
            input.clone(),
        )
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    assert_eq!(
        body(response).await,
        json!({"available":true,"client_id":"chat-client"})
    );
    let encrypted: Vec<u8> =
        sqlx::query_scalar("SELECT encrypted FROM access_chatgpt_client WHERE id=1")
            .fetch_one(f.access.pool())
            .await
            .unwrap();
    assert!(!String::from_utf8_lossy(&encrypted).contains("retained-chat-secret"));
    assert!(!String::from_utf8_lossy(&encrypted).contains("chat-client"));
    let (cookie, csrf) = activate(&f, &idp, &setup_cookie, &setup_csrf).await;
    // Recreate all process-local auth state, keeping only the database and key.
    f.access = Access(Arc::new(Inner {
        pool: f.access.pool().clone(),
        preview_domain: f.access.0.preview_domain.clone(),
        public: true,
        public_url: f.access.0.public_url.clone(),
        forced_mode: None,
        key_path: f.access.0.key_path.clone(),
        internal_token: store::random(),
        provider: Mutex::new(provider::Cache::default()),
        attempts: Mutex::new((now(), 0)),
        password_slots: tokio::sync::Semaphore::new(2),
        provisioning: Mutex::new(()),
    }));
    input["action"] = json!("status");
    let headers = [("cookie", cookie.as_str()), ("x-lvk-csrf", csrf.as_str())];
    assert_eq!(
        body(f.call("POST", PATH, &headers, input.clone()).await).await,
        json!({"available":true,"client_id":"chat-client"})
    );
    input["action"] = json!("reveal");
    let response = f.call("POST", PATH, &headers, input.clone()).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    assert_eq!(
        body(response).await["client_secret"],
        "retained-chat-secret"
    );
    for forbidden_headers in [
        vec![],
        vec![("cookie", cookie.as_str())],
        vec![("cookie", &setup_cookie), ("x-lvk-csrf", &setup_csrf)],
        vec![("x-lvk-internal-token", f.access.0.internal_token.as_str())],
    ] {
        assert!(
            !f.call("POST", PATH, &forbidden_headers, input.clone())
                .await
                .status()
                .is_success()
        );
    }
    let member = f
        .access
        .identity(&idp.state.issuer, "ordinary-member", None)
        .await
        .unwrap();
    sqlx::query("UPDATE access_members SET state='active' WHERE id=?")
        .bind(&member.id)
        .execute(f.access.pool())
        .await
        .unwrap();
    let (_, revision) = f.access.load().await.unwrap();
    let (member_token, member_csrf) = f
        .access
        .create_session(Some(&member.id), "web", revision)
        .await
        .unwrap();
    let member_cookie = format!("lvk_session={member_token}");
    for action in ["status", "reveal", "save"] {
        input["action"] = json!(action);
        assert_eq!(
            f.call(
                "POST",
                PATH,
                &[("cookie", &member_cookie), ("x-lvk-csrf", &member_csrf)],
                input.clone()
            )
            .await
            .status(),
            StatusCode::FORBIDDEN
        );
    }
    let audits: String = sqlx::query_scalar(
        "SELECT group_concat(action || coalesce(target,'') || outcome) FROM access_audit",
    )
    .fetch_one(f.access.pool())
    .await
    .unwrap();
    assert!(audits.contains("chatgpt-client-reveal"));
    assert!(!audits.contains("retained-chat-secret"));
}

#[tokio::test]
async fn chatgpt_credentials_are_bound_to_configuration_and_reject_web_secret_reuse() {
    let f = Fixture::new(true).await;
    let (cookie, csrf) = f.setup().await;
    let headers = [("cookie", cookie.as_str()), ("x-lvk-csrf", csrf.as_str())];
    let input = json!({"action":"save","issuer":"https://fixture.auth0.com/","web_client_id":"web",
        "client_id":"chat","client_secret":"saved-secret"});
    assert_eq!(
        f.call("POST", PATH, &headers, input.clone()).await.status(),
        StatusCode::OK
    );
    let mut invalid = input.clone();
    invalid["client_id"] = json!("web");
    assert_eq!(
        f.call("POST", PATH, &headers, invalid).await.status(),
        StatusCode::BAD_REQUEST
    );
    for field in ["issuer", "web_client_id"] {
        let mut changed = input.clone();
        changed[field] = json!("different");
        changed["action"] = json!("status");
        assert_eq!(
            body(f.call("POST", PATH, &headers, changed.clone()).await).await,
            json!({"available":false})
        );
        changed["action"] = json!("reveal");
        assert_eq!(
            f.call("POST", PATH, &headers, changed).await.status(),
            StatusCode::BAD_REQUEST
        );
    }
    // Simulate a database restored under a different public URL.
    let bytes: Vec<u8> =
        sqlx::query_scalar("SELECT encrypted FROM access_chatgpt_client WHERE id=1")
            .fetch_one(f.access.pool())
            .await
            .unwrap();
    let mut credentials: crate::chatgpt::Credentials = f.access.decrypt(&bytes).unwrap();
    assert_eq!(credentials.client_secret, "saved-secret");
    credentials.resource = "https://other.example/mcp".into();
    crate::chatgpt::save(&f.access, &credentials).await.unwrap();
    let mut status = input;
    status["action"] = json!("status");
    assert_eq!(
        body(f.call("POST", PATH, &headers, status).await).await,
        json!({"available":false})
    );
}
