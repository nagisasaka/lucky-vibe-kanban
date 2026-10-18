use axum::{
    Json,
    extract::{DefaultBodyLimit, Query, State},
    response::Redirect,
    routing::{get, post},
};
use serde_json::{Value, json};
use sqlx::Row;

use super::{
    store::{check_csrf, hash, random},
    *,
};

pub fn router(access: Access) -> Router {
    Router::new()
        .route("/api/access/status", get(status))
        .route("/api/access/setup", post(exchange_key))
        .route("/api/access/login", get(login))
        .route("/api/access/basic-login", post(basic_login))
        .route("/api/access/logout", post(logout))
        .route("/api/access/configuration", get(configuration))
        .route("/api/access/basic", post(basic))
        .route("/api/access/local", post(local))
        .route("/api/access/auth0/test", post(test_auth0))
        .route(
            "/api/access/auth0/provision",
            post(super::provisioning::handle),
        )
        .route("/api/access/callback", get(callback))
        .route("/api/access/confirmation", get(confirmation))
        .route("/api/access/activate", post(activate))
        .route("/api/access/members", get(members).post(update_member))
        .route("/api/access/audit", get(audit))
        .route(
            "/api/access/preview/authorize",
            get(super::preview::authorize).post(super::preview::grant),
        )
        .route("/.well-known/oauth-protected-resource/mcp", get(metadata))
        .layer(DefaultBodyLimit::max(16384))
        .with_state(access)
}
async fn status(State(access): State<Access>, headers: HeaderMap) -> Json<Value> {
    let state = access.load().await;
    let mode = state
        .as_ref()
        .ok()
        .and_then(|(c, _)| c.as_ref())
        .map(Config::mode)
        .unwrap_or("locked");
    let session = access.session(&headers).await.ok();
    // Interactive Basic login exchanges credentials for a browser session. A
    // cached proxy Basic header must not force expensive password hashing on
    // every request after mounting the application.
    let principal = access
        .web_principal(&headers)
        .await
        .ok()
        .filter(|p| p.channel != "basic");
    let management = session.as_ref().is_some_and(|s| s.kind != "web")
        || principal.as_ref().is_some_and(|p| p.admin);
    let csrf = session
        .as_ref()
        .map(|s| s.csrf.clone())
        .or_else(|| principal.as_ref().map(|p| p.channel.to_string()));
    Json(
        json!({"mode":mode,"deployment":if access.0.public{"public"}else{"local"},"public_url":access.public_url(),"authenticated":principal.is_some(),"management":management,"limited":session.as_ref().is_some_and(|s|s.kind!="web"),"csrf":csrf,"user":principal.map(|p|json!({"id":p.user_id,"admin":p.admin})),"forced_mode":access.0.forced_mode,"configuration_error":state.is_err(),"web_absolute_seconds":WEB_LIFETIME,"web_idle_seconds":WEB_IDLE}),
    )
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct KeyInput {
    key: String,
}
async fn exchange_key(
    State(access): State<Access>,
    headers: HeaderMap,
    Json(input): Json<KeyInput>,
) -> Result<Response, Error> {
    check_csrf(&headers, "setup")?;
    access.rate_limit().await?;
    if input.key.len() > 128 {
        return Err(Error::unauthorized());
    }
    let mut tx = access.pool().begin_with("BEGIN IMMEDIATE").await?;
    let kind: Option<String> =
        sqlx::query_scalar("DELETE FROM access_keys WHERE hash=? AND expires>? RETURNING kind")
            .bind(hash(&input.key))
            .bind(now())
            .fetch_optional(&mut *tx)
            .await?;
    let kind = kind.ok_or_else(Error::unauthorized)?;
    let revision: Option<i64> =
        sqlx::query_scalar("SELECT revision FROM access_settings WHERE id=1")
            .fetch_optional(&mut *tx)
            .await?;
    if kind == "setup" && revision.is_some() {
        return Err(Error::forbidden("Setup already completed"));
    }
    let token = random();
    let csrf = random();
    sqlx::query(
        "INSERT INTO access_sessions(hash,kind,csrf,expires,touched,revision) VALUES(?,?,?,?,?,?)",
    )
    .bind(hash(&token))
    .bind(&kind)
    .bind(&csrf)
    .bind(now() + SETUP_LIFETIME)
    .bind(now())
    .bind(revision.unwrap_or(0))
    .execute(&mut *tx)
    .await?;
    sqlx::query("INSERT INTO access_audit(at,channel,action,outcome) VALUES(?,'setup','key-exchange','success')").bind(now()).execute(&mut *tx).await?;
    tx.commit().await?;
    let mut response =
        Json(json!({"csrf":csrf,"kind":kind,"expires_in":SETUP_LIFETIME})).into_response();
    set_cookie(
        &mut response,
        &access,
        "lvk_session",
        &token,
        SETUP_LIFETIME,
    );
    Ok(response)
}
async fn basic_login(State(access): State<Access>, headers: HeaderMap) -> Result<Response, Error> {
    check_csrf(&headers, "basic")?;
    // Reauthentication must verify the supplied password, never renew an old
    // browser session merely because its cookie is still valid.
    let mut credentials = headers.clone();
    credentials.remove(header::COOKIE);
    let principal = access.web_principal(&credentials).await?;
    if !matches!(access.load().await?, (Some(Config::Basic { .. }), _)) {
        return Err(Error::forbidden("Basic login is disabled"));
    }
    let (token, csrf) = access
        .create_session(
            Some(&principal.user_id.to_string()),
            "web",
            principal.revision,
        )
        .await?;
    if let Some(old) = cookie(&headers, "lvk_session") {
        sqlx::query("DELETE FROM access_sessions WHERE hash=?")
            .bind(hash(&old))
            .execute(access.pool())
            .await?;
    }
    access
        .audit(
            Some(&principal.user_id.to_string()),
            "web",
            "login",
            None,
            "success",
        )
        .await?;
    let mut r = Json(json!({"csrf":csrf})).into_response();
    set_cookie(&mut r, &access, "lvk_session", &token, WEB_LIFETIME);
    Ok(r)
}
async fn logout(State(access): State<Access>, headers: HeaderMap) -> Result<Response, Error> {
    let session = access.session(&headers).await?;
    check_csrf(&headers, &session.csrf)?;
    sqlx::query("DELETE FROM access_sessions WHERE hash=?")
        .bind(&session.hash)
        .execute(access.pool())
        .await?;
    access
        .audit(session.user_id.as_deref(), "web", "logout", None, "success")
        .await?;
    let mut r = StatusCode::NO_CONTENT.into_response();
    set_cookie(&mut r, &access, "lvk_session", "", 0);
    Ok(r)
}
async fn configuration(
    State(access): State<Access>,
    headers: HeaderMap,
) -> Result<Json<Value>, Error> {
    access.management(&headers, false).await?;
    let value = match access.load().await {
        Ok((
            Some(Config::Auth0 {
                issuer,
                client_id,
                mcp_enabled,
                ..
            }),
            revision,
        )) => {
            json!({"mode":"auth0","issuer":issuer,"client_id":client_id,"secret_configured":true,"mcp_enabled":mcp_enabled,"revision":revision})
        }
        Ok((Some(Config::Basic { username, .. }), revision)) => {
            json!({"mode":"basic","username":username,"revision":revision})
        }
        Ok((config, revision)) => json!({"mode":config.map(|c|c.mode()),"revision":revision}),
        Err(_) => json!({"mode":"locked","configuration_error":true}),
    };
    let base = access.public_url().unwrap_or("");
    Ok(Json(
        json!({"current":value,"forced_mode":access.0.forced_mode,"public_url":base,"callback_url":format!("{base}/api/access/callback"),"logout_url":format!("{base}/"),"resource":access.resource(),"scopes":SCOPES,"chatgpt":"unverified"}),
    ))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BasicInput {
    username: String,
    password: String,
}
async fn basic(
    State(access): State<Access>,
    headers: HeaderMap,
    Json(input): Json<BasicInput>,
) -> Result<Response, Error> {
    let session = access.management(&headers, true).await?;
    access.allow_mode("basic")?;
    access.rate_limit().await?;
    if access.public_url().is_none() {
        return Err(Error::bad(
            "Restart with the public HTTPS deployment preset first",
        ));
    }
    if input.username.is_empty()
        || input.username.len() > 128
        || input.username.contains(':')
        || input.username.chars().any(char::is_control)
    {
        return Err(Error::bad("Invalid Basic username"));
    }
    let _slot = access.0.password_slots.try_acquire().map_err(|_| Error {
        status: StatusCode::TOO_MANY_REQUESTS,
        message: "Authentication busy",
    })?;
    let config = Config::Basic {
        username: input.username,
        password_hash: store::password_hash(input.password).await?,
    };
    let revision = sqlx::query_scalar::<_, i64>("SELECT revision FROM access_settings WHERE id=1")
        .fetch_optional(access.pool())
        .await?
        .unwrap_or(0);
    access
        .commit_config(&session, &config, None, revision)
        .await?;
    let mut r = Json(json!({"mode":"basic","login_required":true})).into_response();
    set_cookie(&mut r, &access, "lvk_session", "", 0);
    Ok(r)
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Auth0Input {
    issuer: String,
    client_id: String,
    client_secret: String,
    mcp_enabled: bool,
}
#[derive(Serialize, Deserialize)]
struct Transaction {
    config: Config,
    nonce: String,
    verifier: String,
    grant: Option<String>,
    revision: i64,
}
#[derive(Serialize, Deserialize)]
struct Confirmation {
    config: Config,
    member: Member,
    revision: i64,
}
async fn begin(
    access: &Access,
    config: Config,
    grant: Option<String>,
    revision: i64,
) -> Result<Response, Error> {
    let (url, state, nonce, verifier) = access.start_oidc(&config).await?;
    let browser = random();
    let encrypted = access.encrypt(&Transaction {
        config,
        nonce,
        verifier,
        grant,
        revision,
    })?;
    sqlx::query("DELETE FROM access_transactions WHERE expires<=?")
        .bind(now())
        .execute(access.pool())
        .await?;
    sqlx::query("INSERT INTO access_transactions(state_hash,browser_hash,encrypted,expires) VALUES(?,?,?,?)").bind(hash(&state)).bind(hash(&browser)).bind(encrypted).bind(now()+TRANSACTION_LIFETIME).execute(access.pool()).await?;
    let mut response = Json(json!({"authorize_url":url})).into_response();
    set_cookie(
        &mut response,
        access,
        "lvk_transaction",
        &browser,
        TRANSACTION_LIFETIME,
    );
    Ok(response)
}
async fn login(State(access): State<Access>) -> Result<Response, Error> {
    access.rate_limit().await?;
    let (config, revision) = access.load().await?;
    let Some(config @ Config::Auth0 { .. }) = config else {
        return Err(Error::bad("Auth0 login is disabled"));
    };
    // Browser navigates to Auth0 itself; URL is never derived from Host/Forwarded.
    begin(&access, config, None, revision).await
}
async fn test_auth0(
    State(access): State<Access>,
    headers: HeaderMap,
    Json(mut input): Json<Auth0Input>,
) -> Result<Response, Error> {
    let session = access.management(&headers, true).await?;
    access.allow_mode("auth0")?;
    access.rate_limit().await?;
    provider::validate_issuer(&input.issuer)?;
    if input.client_secret.is_empty()
        && let Ok((
            Some(Config::Auth0 {
                issuer,
                client_id,
                client_secret,
                ..
            }),
            _,
        )) = access.load().await
        && issuer == input.issuer
        && client_id == input.client_id
    {
        input.client_secret = client_secret;
    }
    if input.client_id.is_empty()
        || input.client_id.len() > 256
        || !input
            .client_id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        || input.client_secret.is_empty()
        || input.client_secret.len() > 4096
        || access.public_url().is_none()
    {
        return Err(Error::bad(
            "Public HTTPS URL, Web Client ID and Web Client Secret are required",
        ));
    }
    let config = Config::Auth0 {
        issuer: input.issuer,
        client_id: input.client_id,
        client_secret: input.client_secret,
        mcp_enabled: input.mcp_enabled,
    };
    let checks = match access.check_provider(&config).await {
        Ok(c) => c,
        Err(e) => {
            access
                .audit(
                    session.user_id.as_deref(),
                    "settings",
                    "auth0-check",
                    None,
                    "failed",
                )
                .await?;
            return Err(e);
        }
    };
    if checks["client_secret_basic"] != true {
        return Err(Error::bad(
            "Auth0 discovery must advertise client_secret_basic for the configured clients",
        ));
    }
    if checks["pkce_s256"] != true {
        return Err(Error::bad(
            "Auth0 discovery does not advertise PKCE S256; check tenant settings",
        ));
    }
    let revision = sqlx::query_scalar::<_, i64>("SELECT revision FROM access_settings WHERE id=1")
        .fetch_optional(access.pool())
        .await?
        .unwrap_or(0);
    access
        .audit(
            session.user_id.as_deref(),
            "settings",
            "auth0-check",
            None,
            "discovery-passed-login-pending",
        )
        .await?;
    begin(&access, config, Some(session.hash), revision).await
}
#[derive(Deserialize)]
struct Callback {
    state: Option<String>,
    code: Option<String>,
    iss: Option<String>,
    error: Option<String>,
}
async fn callback(
    State(access): State<Access>,
    headers: HeaderMap,
    Query(query): Query<Callback>,
) -> Result<Response, Error> {
    let browser = cookie(&headers, "lvk_transaction").ok_or_else(Error::unauthorized)?;
    let state = query
        .state
        .filter(|s| s.len() <= 256)
        .ok_or_else(Error::unauthorized)?;
    let bytes:Option<Vec<u8>>=sqlx::query_scalar("DELETE FROM access_transactions WHERE state_hash=? AND browser_hash=? AND expires>? RETURNING encrypted").bind(hash(&state)).bind(hash(&browser)).bind(now()).fetch_optional(access.pool()).await?;
    let transaction: Transaction = access.decrypt(&bytes.ok_or_else(Error::unauthorized)?)?;
    let code = query
        .code
        .filter(|s| s.len() <= 8192)
        .ok_or_else(Error::unauthorized)?;
    if query.error.is_some() || query.iss.as_deref() != transaction.config.issuer() {
        return Err(Error::unauthorized());
    }
    let grant = if let Some(expected) = &transaction.grant {
        let session = access.management(&headers, false).await?;
        if &session.hash != expected {
            return Err(Error::unauthorized());
        }
        Some(session)
    } else {
        None
    };
    let verified = access
        .finish_oidc(
            &transaction.config,
            code,
            transaction.nonce,
            transaction.verifier,
        )
        .await;
    let (subject, name) = match verified {
        Ok(v) => v,
        Err(e) => {
            access
                .audit(None, "web", "oidc-callback", None, "rejected")
                .await?;
            return Err(e);
        }
    };
    let issuer = transaction
        .config
        .issuer()
        .ok_or_else(Error::unauthorized)?;
    let member = access.identity(issuer, &subject, Some(&name)).await?;
    let mut response = if let Some(grant) = grant {
        let confirmed = Confirmation {
            config: transaction.config,
            member,
            revision: transaction.revision,
        };
        sqlx::query("INSERT INTO access_confirmations(session_hash,encrypted,expires) VALUES(?,?,?) ON CONFLICT(session_hash) DO UPDATE SET encrypted=excluded.encrypted,expires=excluded.expires").bind(grant.hash).bind(access.encrypt(&confirmed)?).bind(now()+TRANSACTION_LIFETIME).execute(access.pool()).await?;
        Redirect::to("/?access=confirm").into_response()
    } else {
        let (_, revision) = access.load().await?;
        if revision != transaction.revision {
            return Err(Error::unauthorized());
        }
        if member.state != "active" {
            access
                .audit(Some(&member.id), "web", "login", None, "membership-denied")
                .await?;
            Redirect::to("/?access=pending").into_response()
        } else {
            let (token, _) = access
                .create_session(Some(&member.id), "web", revision)
                .await?;
            if let Some(old) = cookie(&headers, "lvk_session") {
                sqlx::query("DELETE FROM access_sessions WHERE hash=?")
                    .bind(hash(&old))
                    .execute(access.pool())
                    .await?;
            }
            let mut response = Redirect::to("/").into_response();
            set_cookie(&mut response, &access, "lvk_session", &token, WEB_LIFETIME);
            access
                .audit(Some(&member.id), "web", "login", None, "success")
                .await?;
            response
        }
    };
    set_cookie(&mut response, &access, "lvk_transaction", "", 0);
    Ok(response)
}
async fn confirmation(
    State(access): State<Access>,
    headers: HeaderMap,
) -> Result<Json<Value>, Error> {
    let session = access.management(&headers, false).await?;
    let bytes: Option<Vec<u8>> = sqlx::query_scalar(
        "SELECT encrypted FROM access_confirmations WHERE session_hash=? AND expires>?",
    )
    .bind(session.hash)
    .bind(now())
    .fetch_optional(access.pool())
    .await?;
    let Some(bytes) = bytes else {
        return Ok(Json(json!({"ready":false})));
    };
    let c: Confirmation = access.decrypt(&bytes)?;
    Ok(Json(
        json!({"ready":true,"identity":c.member,"mode":"auth0","web_login":"verified","chatgpt":"unverified","resource":access.resource()}),
    ))
}
async fn activate(State(access): State<Access>, headers: HeaderMap) -> Result<Response, Error> {
    let session = access.management(&headers, true).await?;
    let bytes: Option<Vec<u8>> = sqlx::query_scalar(
        "SELECT encrypted FROM access_confirmations WHERE session_hash=? AND expires>?",
    )
    .bind(&session.hash)
    .bind(now())
    .fetch_optional(access.pool())
    .await?;
    let confirmed: Confirmation = access.decrypt(&bytes.ok_or_else(Error::unauthorized)?)?;
    access
        .commit_config(
            &session,
            &confirmed.config,
            Some(&confirmed.member),
            confirmed.revision,
        )
        .await?;
    let (token, csrf) = access
        .create_session(Some(&confirmed.member.id), "web", confirmed.revision + 1)
        .await?;
    let mut r = Json(json!({"csrf":csrf,"mode":"auth0"})).into_response();
    set_cookie(&mut r, &access, "lvk_session", &token, WEB_LIFETIME);
    Ok(r)
}
async fn require_admin(
    access: &Access,
    headers: &HeaderMap,
    write: bool,
) -> Result<Principal, Error> {
    let p = access.web_principal(headers).await?;
    if !p.admin {
        return Err(Error::forbidden("Administrator required"));
    }
    if write {
        access.csrf(headers, &p).await?;
    }
    Ok(p)
}
async fn members(State(access): State<Access>, headers: HeaderMap) -> Result<Json<Value>, Error> {
    require_admin(&access, &headers, false).await?;
    let (config, _) = access.load().await?;
    let issuer = config
        .as_ref()
        .and_then(Config::issuer)
        .unwrap_or("urn:lvk:basic");
    let members=sqlx::query_as::<_,Member>("SELECT id,issuer,subject,display_name,state,role FROM access_members WHERE issuer=? ORDER BY created_at,id LIMIT 1000").bind(issuer).fetch_all(access.pool()).await?;
    Ok(Json(json!({"members":members})))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MemberUpdate {
    id: String,
    state: String,
    role: String,
}
async fn update_member(
    State(access): State<Access>,
    headers: HeaderMap,
    Json(input): Json<MemberUpdate>,
) -> Result<Json<Value>, Error> {
    let actor = require_admin(&access, &headers, true).await?;
    if !matches!(input.state.as_str(), "active" | "disabled")
        || !matches!(input.role.as_str(), "admin" | "member")
    {
        return Err(Error::bad("Invalid member state or role"));
    }
    let mut tx = access.pool().begin_with("BEGIN IMMEDIATE").await?;
    let current = sqlx::query_as::<_, Member>(
        "SELECT id,issuer,subject,display_name,state,role FROM access_members WHERE id=?",
    )
    .bind(&input.id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(Error::bad("Unknown member"))?;
    let still_admin:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM access_members WHERE id=? AND role='admin' AND state='active')").bind(actor.user_id.to_string()).fetch_one(&mut *tx).await?;
    if !still_admin {
        return Err(Error::forbidden("Administrator required"));
    }
    let revision: Option<i64> =
        sqlx::query_scalar("SELECT revision FROM access_settings WHERE id=1")
            .fetch_optional(&mut *tx)
            .await?;
    if revision != Some(actor.revision) || current.issuer != actor.issuer {
        return Err(Error::forbidden(
            "Member belongs to an inactive authentication provider",
        ));
    }
    if current.role == "admin"
        && current.state == "active"
        && (input.role != "admin" || input.state != "active")
    {
        let admins:i64=sqlx::query_scalar("SELECT COUNT(*) FROM access_members WHERE issuer=? AND role='admin' AND state='active'").bind(&current.issuer).fetch_one(&mut *tx).await?;
        if admins <= 1 {
            return Err(Error::forbidden(
                "Cannot disable or demote the last active administrator",
            ));
        }
    }
    sqlx::query("UPDATE access_members SET state=?,role=? WHERE id=?")
        .bind(&input.state)
        .bind(&input.role)
        .bind(&input.id)
        .execute(&mut *tx)
        .await?;
    // Demotion also revokes existing browser sessions. Bearer permissions are
    // resolved afresh for every request; streams recheck within two seconds.
    sqlx::query("DELETE FROM access_sessions WHERE user_id=?")
        .bind(&input.id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("INSERT INTO access_audit(at,actor,channel,action,target,outcome) VALUES(?,?,'settings',?,?,'success')").bind(now()).bind(actor.user_id.to_string()).bind(format!("member-{}-{}",input.state,input.role)).bind(&input.id).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(Json(json!({"updated":true})))
}
async fn audit(State(access): State<Access>, headers: HeaderMap) -> Result<Json<Value>, Error> {
    require_admin(&access, &headers, false).await?;
    let rows=sqlx::query("SELECT id,at,actor,channel,action,target,outcome FROM access_audit ORDER BY id DESC LIMIT 200").fetch_all(access.pool()).await?;
    Ok(Json(
        json!({"events":rows.iter().map(|r|json!({"id":r.get::<i64,_>("id"),"at":r.get::<i64,_>("at"),"actor":r.get::<Option<String>,_>("actor"),"channel":r.get::<String,_>("channel"),"action":r.get::<String,_>("action"),"target":r.get::<Option<String>,_>("target"),"outcome":r.get::<String,_>("outcome")})).collect::<Vec<_>>()}),
    ))
}
async fn metadata(State(access): State<Access>) -> Result<Json<Value>, Error> {
    let (config, _) = access.load().await?;
    let Some(Config::Auth0 {
        issuer,
        mcp_enabled: true,
        ..
    }) = config
    else {
        return Err(Error {
            status: StatusCode::NOT_FOUND,
            message: "Remote MCP is disabled",
        });
    };
    Ok(Json(
        json!({"resource":access.resource(),"authorization_servers":[issuer],"scopes_supported":SCOPES,"bearer_methods_supported":["header"]}),
    ))
}

async fn local(State(access): State<Access>, headers: HeaderMap) -> Result<Json<Value>, Error> {
    let session = access.management(&headers, true).await?;
    let revision = sqlx::query_scalar::<_, i64>("SELECT revision FROM access_settings WHERE id=1")
        .fetch_optional(access.pool())
        .await?
        .unwrap_or(0);
    access
        .commit_config(&session, &Config::Local, None, revision)
        .await?;
    Ok(Json(json!({"mode":"local"})))
}
