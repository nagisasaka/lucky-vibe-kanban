//! Optional browser Web Push. Canonical DB triggers own event admission; this
//! bounded dispatcher only delivers committed, still-actionable notifications.
use std::{sync::Arc, time::Duration};

use axum::{
    Extension, Json, Router,
    extract::{Path, State},
    http::StatusCode,
    routing::{get, post},
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use deployment::Deployment;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::SqlitePool;
use uuid::Uuid;
use web_push::{ContentEncoding, SubscriptionInfo, VapidSignatureBuilder, WebPushMessageBuilder};

use crate::DeploymentImpl;

type ApiResult<T> = Result<Json<T>, StatusCode>;

struct PushConfig {
    pem: Vec<u8>,
    public_key: String,
    subject: String,
}
#[derive(Clone)]
struct PushService {
    access: server_access::Access,
    config: Option<Arc<PushConfig>>,
    delivery_gate: Arc<tokio::sync::Mutex<()>>,
}

// Only recognised public push services are destinations, never client-selected
// arbitrary URLs, credentials, ports or redirects. DNS/TLS remain platform-owned.
fn valid_endpoint(endpoint: &str) -> bool {
    let Ok(u) = url::Url::parse(endpoint) else {
        return false;
    };
    if endpoint.len() > 4096
        || u.scheme() != "https"
        || !u.username().is_empty()
        || u.password().is_some()
        || u.port().is_some()
        || u.fragment().is_some()
    {
        return false;
    }
    match u.host_str().unwrap_or("") {
        "fcm.googleapis.com" => u.path().starts_with("/fcm/send/") || u.path().starts_with("/wp/"),
        "updates.push.services.mozilla.com" => u.path().starts_with("/wpush/"),
        "web.push.apple.com" => u.path().starts_with('/'),
        host if host.ends_with(".notify.windows.com") => u.path().starts_with("/w/"),
        _ => false,
    }
}
fn valid_subscription(s: &SubscriptionInfo) -> bool {
    valid_endpoint(&s.endpoint)
        && URL_SAFE_NO_PAD
            .decode(&s.keys.p256dh)
            .is_ok_and(|k| k.len() == 65 && k[0] == 4)
        && URL_SAFE_NO_PAD
            .decode(&s.keys.auth)
            .is_ok_and(|k| k.len() == 16)
}
fn config() -> Option<PushConfig> {
    let path = std::env::var("LVK_WEB_PUSH_PRIVATE_KEY_FILE").ok()?;
    let pem = std::fs::read(path).ok()?;
    let subject = std::env::var("LVK_WEB_PUSH_SUBJECT").ok()?;
    if !subject.starts_with("mailto:") && !subject.starts_with("https://") {
        return None;
    }
    let builder = VapidSignatureBuilder::from_pem_no_sub(pem.as_slice()).ok()?;
    let public_key = URL_SAFE_NO_PAD.encode(builder.get_public_key());
    Some(PushConfig {
        pem,
        public_key,
        subject,
    })
}
fn actor() -> Result<Uuid, StatusCode> {
    server_access::ACTOR
        .try_with(|p| {
            if matches!(p.channel, "local" | "basic" | "web") {
                Ok(p.user_id)
            } else {
                Err(StatusCode::FORBIDDEN)
            }
        })
        .unwrap_or(Err(StatusCode::UNAUTHORIZED))
}

pub(super) fn router(
    deployment: &DeploymentImpl,
    access: server_access::Access,
) -> Router<DeploymentImpl> {
    let service = PushService {
        access,
        config: config().map(Arc::new),
        delivery_gate: Arc::new(tokio::sync::Mutex::new(())),
    };
    {
        let worker = service.clone();
        let pool = deployment.db().pool.clone();
        tokio::spawn(async move {
            let client = reqwest::Client::builder()
                .redirect(reqwest::redirect::Policy::none())
                .timeout(Duration::from_secs(15))
                .build()
                .expect("Web Push HTTP client");
            loop {
                if dispatch(&pool, &worker, &client).await.is_err() {
                    // Never log HTTP errors: they can contain endpoint capabilities.
                    tracing::warn!("Web Push dispatch unavailable; retrying later");
                }
                tokio::time::sleep(Duration::from_secs(if worker.config.is_some() {
                    5
                } else {
                    60
                }))
                .await;
            }
        });
    }
    subscription_router(service).with_state(deployment.db().pool.clone())
}

fn subscription_router(service: PushService) -> Router<SqlitePool> {
    Router::new()
        .route("/web-push", get(status).post(subscribe))
        .route("/web-push/{id}", post(unsubscribe))
        .route("/web-push/context/{workspace}", get(context))
        .layer(Extension(service))
}

async fn context(State(pool): State<SqlitePool>, Path(workspace): Path<Uuid>) -> ApiResult<Value> {
    actor()?;
    let link: Option<(Uuid, Uuid)> = sqlx::query_as(
        "SELECT project_id,issue_id FROM local_workspace_links WHERE workspace_id=?",
    )
    .bind(workspace)
    .fetch_optional(&pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(
        link.map(|(project_id, issue_id)| json!({"project_id":project_id,"issue_id":issue_id}))
            .unwrap_or(Value::Null),
    ))
}

#[derive(Serialize, sqlx::FromRow)]
struct Registration {
    id: Uuid,
    project_id: Uuid,
    endpoint_hash: String,
}
async fn status(
    State(pool): State<SqlitePool>,
    Extension(s): Extension<PushService>,
) -> ApiResult<Value> {
    let owner = actor()?;
    let registrations = sqlx::query_as::<_, Registration>(
        "SELECT id,project_id,endpoint_hash FROM web_push_subscriptions WHERE owner=?",
    )
    .bind(owner)
    .fetch_all(&pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    // Same shared local Board access policy as /api/board/projects.
    let projects: Vec<(Uuid, String)> =
        sqlx::query_as("SELECT id,name FROM projects ORDER BY name")
            .fetch_all(&pool)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(
        json!({"public_key":s.config.as_ref().map(|c| &c.public_key), "registrations":registrations,
        "projects":projects.into_iter().map(|(id,name)| json!({"id":id,"name":name})).collect::<Vec<_>>()}),
    ))
}
#[derive(Deserialize)]
struct Subscribe {
    project_id: Uuid,
    subscription: SubscriptionInfo,
}
async fn subscribe(
    State(pool): State<SqlitePool>,
    Extension(s): Extension<PushService>,
    Json(r): Json<Subscribe>,
) -> ApiResult<Registration> {
    let owner = actor()?;
    let _guard = s.delivery_gate.lock().await;
    if s.config.is_none() {
        return Err(StatusCode::SERVICE_UNAVAILABLE);
    }
    if !valid_subscription(&r.subscription) {
        return Err(StatusCode::BAD_REQUEST);
    }
    let pool = &pool;
    let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM projects WHERE id=?)")
        .bind(r.project_id)
        .fetch_one(pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if !exists {
        return Err(StatusCode::NOT_FOUND);
    }
    let hash = format!("{:x}", Sha256::digest(r.subscription.endpoint.as_bytes()));
    let sealed = s
        .access
        .seal_push_registration(
            serde_json::to_value(r.subscription).map_err(|_| StatusCode::BAD_REQUEST)?,
        )
        .map_err(|_| StatusCode::FORBIDDEN)?;
    let mut tx = pool
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    // An endpoint may never be silently transferred between accounts, including
    // registration for a different Project on a shared browser.
    let foreign: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM web_push_subscriptions WHERE endpoint_hash=? AND owner<>?)",
    )
    .bind(&hash)
    .bind(owner)
    .fetch_one(&mut *tx)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if foreign {
        return Err(StatusCode::CONFLICT);
    }
    let count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM web_push_subscriptions WHERE owner=?")
            .bind(owner)
            .fetch_one(&mut *tx)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let existing: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM web_push_subscriptions WHERE owner=? AND endpoint_hash=? AND project_id=?)")
        .bind(owner).bind(&hash).bind(r.project_id).fetch_one(&mut *tx).await.map_err(|_|StatusCode::INTERNAL_SERVER_ERROR)?;
    if count >= 64 && !existing {
        return Err(StatusCode::TOO_MANY_REQUESTS);
    }
    let id: Uuid = sqlx::query_scalar("INSERT INTO web_push_subscriptions(id,owner,project_id,endpoint_hash,sealed) VALUES(?,?,?,?,?) ON CONFLICT(endpoint_hash,project_id) DO UPDATE SET sealed=excluded.sealed,created_at=unixepoch() RETURNING id")
        .bind(Uuid::new_v4()).bind(owner).bind(r.project_id).bind(&hash).bind(sealed)
        .fetch_one(&mut *tx).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    tx.commit()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(Registration {
        id,
        project_id: r.project_id,
        endpoint_hash: hash,
    }))
}
async fn unsubscribe(
    State(pool): State<SqlitePool>,
    Extension(s): Extension<PushService>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, StatusCode> {
    let owner = actor()?;
    let _guard = s.delivery_gate.lock().await;
    sqlx::query("DELETE FROM web_push_subscriptions WHERE id=? AND owner=?")
        .bind(id)
        .bind(owner)
        .execute(&pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(sqlx::FromRow)]
struct Delivery {
    id: i64,
    subscription_id: Uuid,
    owner: Uuid,
    sealed: Vec<u8>,
    agent_run_id: Uuid,
    attempt_id: Option<Uuid>,
    reference_id: Option<String>,
    kind: String,
    tries: i64,
    workspace_id: Uuid,
    session_id: Uuid,
    project_id: Uuid,
    issue_id: Uuid,
}

async fn actionable(pool: &SqlitePool, item: &Delivery) -> anyhow::Result<bool> {
    if matches!(item.kind.as_str(), "input_requested" | "approval_requested") {
        let pending = super::sessions::questions::pending_blocking(pool, item.workspace_id).await?;
        Ok(pending.iter().any(|e| {
            e["agent_run_id"].as_str() == Some(&item.agent_run_id.to_string())
                && e["run_attempt_id"].as_str()
                    == item.attempt_id.as_ref().map(Uuid::to_string).as_deref()
                && e["payload"]["type"] == item.kind
                && (e["payload"]["data"]["input_id"]
                    .as_str()
                    .or_else(|| e["payload"]["data"]["approval_id"].as_str()))
                    == item.reference_id.as_deref()
        }))
    } else {
        // A retry must not receive the previous attempt's completion notice.
        Ok(sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM agent_runs r WHERE r.id=? AND r.status=? AND (SELECT id FROM agent_run_attempts WHERE agent_run_id=r.id ORDER BY attempt_number DESC LIMIT 1) IS ?)")
            .bind(item.agent_run_id).bind(&item.kind).bind(item.attempt_id).fetch_one(pool).await?)
    }
}
fn body(kind: &str) -> &'static str {
    match kind {
        "input_requested" => "Codex needs an answer. Open LVK to respond.",
        "approval_requested" => "Codex is waiting for approval. Review in LVK.",
        "succeeded" => {
            "Codex run ended successfully. Tests, Integration and publication are separate."
        }
        "cancelled" => "Codex run was cancelled. Open LVK for details.",
        _ => "Codex run failed or ended abnormally. Open LVK for details.",
    }
}
async fn send(
    client: &reqwest::Client,
    config: &PushConfig,
    sub: &SubscriptionInfo,
    payload: &[u8],
) -> anyhow::Result<u16> {
    anyhow::ensure!(valid_subscription(sub), "Invalid subscription");
    let mut signature = VapidSignatureBuilder::from_pem(config.pem.as_slice(), sub)?;
    signature.add_claim("sub", config.subject.clone());
    let mut builder = WebPushMessageBuilder::new(sub);
    builder.set_vapid_signature(signature.build()?);
    builder.set_payload(ContentEncoding::Aes128Gcm, payload);
    builder.set_ttl(300);
    builder.set_topic(URL_SAFE_NO_PAD.encode(&Sha256::digest(payload)[..24]));
    let message = builder.build()?;
    let request = web_push::request_builder::build_request::<reqwest::Body>(message);
    let (parts, bytes) = request.into_parts();
    let mut request = client.post(parts.uri.to_string()).body(bytes);
    for (key, value) in &parts.headers {
        request = request.header(key.as_str(), value.as_bytes());
    }
    Ok(request.send().await?.status().as_u16())
}
async fn dispatch(
    pool: &SqlitePool,
    service: &PushService,
    client: &reqwest::Client,
) -> anyhow::Result<()> {
    sqlx::query("DELETE FROM web_push_subscriptions WHERE created_at<unixepoch()-7776000")
        .execute(pool)
        .await?;
    sqlx::query("UPDATE web_push_deliveries SET finished=1 WHERE finished=0 AND created_at<unixepoch()-3600").execute(pool).await?;
    // Expired jobs are not replayed after long outages; retained receipts bound
    // duplicate delivery. Canonical event replay is already idempotent.
    sqlx::query("DELETE FROM web_push_deliveries WHERE created_at<unixepoch()-604800")
        .execute(pool)
        .await?;
    let Some(config) = service.config.as_ref() else {
        return Ok(());
    };
    let jobs = sqlx::query_as::<_, Delivery>("SELECT d.id,d.subscription_id,s.owner,s.sealed,d.agent_run_id,d.attempt_id,d.reference_id,d.kind,d.tries,r.workspace_id,r.session_id,l.project_id,l.issue_id FROM web_push_deliveries d JOIN web_push_subscriptions s ON s.id=d.subscription_id JOIN agent_runs r ON r.id=d.agent_run_id JOIN local_workspace_links l ON l.workspace_id=r.workspace_id AND l.project_id=s.project_id WHERE d.finished=0 AND d.next_at<=unixepoch() ORDER BY d.id LIMIT 32")
        .fetch_all(pool).await?;
    for item in jobs {
        // Serialise OFF/renewal with each bounded send, not with the whole batch.
        // Once OFF returns there is no in-flight sender for that subscription.
        let _guard = service.delivery_gate.lock().await;
        let current: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM web_push_subscriptions WHERE id=? AND sealed=?)",
        )
        .bind(item.subscription_id)
        .bind(&item.sealed)
        .fetch_one(pool)
        .await?;
        if !current {
            continue;
        }
        let Some(value) = service
            .access
            .open_push_registration(&item.sealed, item.owner)
            .await
        else {
            sqlx::query("DELETE FROM web_push_subscriptions WHERE id=?")
                .bind(item.subscription_id)
                .execute(pool)
                .await?;
            continue;
        };
        if !actionable(pool, &item).await? || item.tries >= 5 {
            sqlx::query("UPDATE web_push_deliveries SET finished=1 WHERE id=?")
                .bind(item.id)
                .execute(pool)
                .await?;
            continue;
        }
        // Reserve before network I/O: restart can retry only after this lease.
        let claimed = sqlx::query("UPDATE web_push_deliveries SET tries=tries+1,next_at=unixepoch()+120 WHERE id=? AND finished=0 AND next_at<=unixepoch() AND created_at>unixepoch()-3600")
            .bind(item.id).execute(pool).await?.rows_affected();
        if claimed == 0 {
            continue;
        }
        let sub: SubscriptionInfo = serde_json::from_value(value)?;
        let url = format!(
            "/projects/{}/issues/{}/workspaces/{}?session_id={}&agent_run_id={}",
            item.project_id, item.issue_id, item.workspace_id, item.session_id, item.agent_run_id
        );
        let payload = serde_json::to_vec(
            &json!({"title":"LVK", "body":body(&item.kind), "tag":format!("lvk:{}",item.id), "url":url}),
        )?;
        let result = send(client, config, &sub, &payload).await;
        match result {
            Ok(404 | 410) => {
                sqlx::query("DELETE FROM web_push_subscriptions WHERE id=?")
                    .bind(item.subscription_id)
                    .execute(pool)
                    .await?;
            }
            Ok(code) if (200..500).contains(&code) && code != 429 => {
                sqlx::query("UPDATE web_push_deliveries SET finished=1 WHERE id=?")
                    .bind(item.id)
                    .execute(pool)
                    .await?;
            }
            _ => {
                sqlx::query("UPDATE web_push_deliveries SET next_at=unixepoch()+? WHERE id=?")
                    .bind(30_i64 * (1 << item.tries))
                    .bind(item.id)
                    .execute(pool)
                    .await?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn endpoint_allowlist_rejects_ssrf_and_redirect_tricks() {
        assert!(valid_endpoint("https://fcm.googleapis.com/fcm/send/token"));
        assert!(valid_endpoint("https://fcm.googleapis.com/wp/token"));
        assert!(valid_endpoint(
            "https://updates.push.services.mozilla.com/wpush/v2/token"
        ));
        assert!(valid_endpoint("https://web.push.apple.com/token"));
        for endpoint in [
            "http://fcm.googleapis.com/fcm/send/x",
            "https://127.0.0.1/fcm/send/x",
            "https://evil.example/x",
            "https://fcm.googleapis.com.evil.example/fcm/send/x",
            "https://user@fcm.googleapis.com/fcm/send/x",
            "https://fcm.googleapis.com:444/fcm/send/x",
            "https://fcm.googleapis.com/fcm/send/x#y",
            "https://fcm.googleapis.com/other",
        ] {
            assert!(!valid_endpoint(endpoint), "{endpoint}");
        }
    }
    #[test]
    fn lock_screen_contains_no_project_or_question_content() {
        assert!(body("succeeded").contains("separate"));
        assert!(body("approval_requested").contains("Review in LVK"));
        assert!(body("crashed").contains("abnormally"));
    }
}

#[cfg(test)]
mod lifecycle_tests {
    use super::*;
    async fn fixture() -> (SqlitePool, Uuid, Uuid, Uuid) {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        sqlx::raw_sql("PRAGMA foreign_keys=ON;
            CREATE TABLE projects(id BLOB PRIMARY KEY);
            CREATE TABLE agent_runs(id BLOB PRIMARY KEY,workspace_id BLOB,session_id BLOB,status TEXT,provider_id TEXT);
            CREATE TABLE local_workspace_links(workspace_id BLOB,project_id BLOB,issue_id BLOB);
            CREATE TABLE agent_run_attempts(id BLOB,agent_run_id BLOB,attempt_number INTEGER,status TEXT);
            CREATE TABLE agent_run_state(agent_run_id BLOB,last_run_attempt_number INTEGER);
            CREATE TABLE agent_events(event_id BLOB PRIMARY KEY,agent_run_id BLOB,run_attempt_id BLOB,run_attempt_number INTEGER,sequence INTEGER,event_envelope TEXT,created_at TEXT);
            CREATE TABLE agent_run_commands(agent_run_id BLOB,created_at TEXT,command_envelope TEXT);")
            .execute(&pool).await.unwrap();
        sqlx::raw_sql(include_str!(
            "../../../db/migrations/20261018180000_web_push.sql"
        ))
        .execute(&pool)
        .await
        .unwrap();
        let project = Uuid::new_v4();
        let run = Uuid::new_v4();
        let attempt = Uuid::new_v4();
        sqlx::query("INSERT INTO projects VALUES(?)")
            .bind(project)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO agent_runs VALUES(?,?,?,'running','codex')")
            .bind(run)
            .bind(run)
            .bind(run)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO local_workspace_links VALUES(?,?,?)")
            .bind(run)
            .bind(project)
            .bind(Uuid::new_v4())
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO agent_run_attempts VALUES(?,?,1,'running')")
            .bind(attempt)
            .bind(run)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO agent_run_state VALUES(?,1)")
            .bind(run)
            .execute(&pool)
            .await
            .unwrap();
        for i in 0..2 {
            sqlx::query("INSERT INTO web_push_subscriptions(id,owner,project_id,endpoint_hash,sealed) VALUES(?,?,?,?,X'00')")
            .bind(Uuid::new_v4()).bind(Uuid::new_v4()).bind(project).bind(i.to_string()).execute(&pool).await.unwrap();
        }
        (pool, project, run, attempt)
    }
    async fn event(pool: &SqlitePool, run: Uuid, attempt: Uuid, sequence: i64, kind: &str) {
        let envelope = json!({"agent_run_id":run,"run_attempt_id":attempt,"payload":{"type":kind,"data":{"input_id":"question-1","approval_id":"approval-1","status":"succeeded"}}});
        sqlx::query("INSERT INTO agent_events VALUES(?,?,?,1,?,?,datetime('now'))")
            .bind(Uuid::new_v4())
            .bind(run)
            .bind(attempt)
            .bind(sequence)
            .bind(envelope.to_string())
            .execute(pool)
            .await
            .unwrap();
    }
    async fn count(pool: &SqlitePool) -> i64 {
        sqlx::query_scalar("SELECT count(*) FROM web_push_deliveries")
            .fetch_one(pool)
            .await
            .unwrap()
    }
    async fn job(pool: &SqlitePool) -> Delivery {
        sqlx::query_as("SELECT d.*,s.owner,s.sealed,r.workspace_id,r.session_id,l.project_id,l.issue_id FROM web_push_deliveries d JOIN web_push_subscriptions s ON s.id=d.subscription_id JOIN agent_runs r ON r.id=d.agent_run_id JOIN local_workspace_links l ON l.workspace_id=r.workspace_id ORDER BY d.id LIMIT 1")
            .fetch_one(pool).await.unwrap()
    }
    #[tokio::test]
    async fn canonical_sources_scope_fanout_terminal_and_unsubscribe() {
        let (pool, project, run, attempt) = fixture().await;
        // A different Project never receives this run's notifications.
        let other = Uuid::new_v4();
        sqlx::query("INSERT INTO projects VALUES(?)")
            .bind(other)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO web_push_subscriptions(id,owner,project_id,endpoint_hash,sealed) VALUES(?,?,?,'other',X'00')")
            .bind(Uuid::new_v4()).bind(Uuid::new_v4()).bind(other).execute(&pool).await.unwrap();
        for (sequence, kind) in [
            "message",
            "tool_call_completed",
            "provider_extension",
            "lifecycle_changed",
        ]
        .iter()
        .enumerate()
        {
            event(&pool, run, attempt, sequence as i64 + 1, kind).await;
        }
        assert_eq!(count(&pool).await, 0);
        event(&pool, run, attempt, 5, "input_requested").await;
        event(&pool, run, attempt, 6, "approval_requested").await;
        assert_eq!(count(&pool).await, 4);
        // Projection updates that stay running and repeated terminal updates do
        // not create duplicates. Completion is not a provider message event.
        sqlx::query("UPDATE agent_runs SET status='running'")
            .execute(&pool)
            .await
            .unwrap();
        assert_eq!(count(&pool).await, 4);
        sqlx::query("UPDATE agent_runs SET status='succeeded'")
            .execute(&pool)
            .await
            .unwrap();
        assert_eq!(count(&pool).await, 6);
        sqlx::query("UPDATE agent_runs SET status='succeeded'")
            .execute(&pool)
            .await
            .unwrap();
        assert_eq!(count(&pool).await, 6);
        sqlx::query("DELETE FROM web_push_subscriptions WHERE project_id=?")
            .bind(project)
            .execute(&pool)
            .await
            .unwrap();
        assert_eq!(count(&pool).await, 0);
    }
    #[tokio::test]
    async fn resolved_expired_and_retried_questions_are_not_actionable() {
        for resolution in ["resolved", "terminal", "retry", "submitted"] {
            let (pool, _, run, attempt) = fixture().await;
            event(&pool, run, attempt, 1, "input_requested").await;
            let item = job(&pool).await;
            assert!(actionable(&pool, &item).await.unwrap());
            match resolution {
                "resolved" => event(&pool, run, attempt, 2, "input_resolved").await,
                "terminal" => {
                    sqlx::query("UPDATE agent_runs SET status='crashed'")
                        .execute(&pool)
                        .await
                        .unwrap();
                }
                "retry" => {
                    sqlx::query("INSERT INTO agent_run_attempts VALUES(?,?,2,'running')")
                        .bind(Uuid::new_v4())
                        .bind(run)
                        .execute(&pool)
                        .await
                        .unwrap();
                }
                _ => {
                    sqlx::query("INSERT INTO agent_run_commands VALUES(?,datetime('now'),?)").bind(run)
                    .bind(json!({"command":{"type":"submit_input","data":{"input_id":"question-1"}}}).to_string()).execute(&pool).await.unwrap();
                }
            }
            assert!(!actionable(&pool, &item).await.unwrap(), "{resolution}");
        }
    }
    #[tokio::test]
    async fn terminal_variants_and_non_codex_are_precise() {
        for status in [
            "succeeded",
            "failed",
            "cancelled",
            "crashed",
            "audit_failed",
        ] {
            let (pool, _, run, _) = fixture().await;
            sqlx::query("UPDATE agent_runs SET status=?")
                .bind(status)
                .execute(&pool)
                .await
                .unwrap();
            let item = job(&pool).await;
            assert_eq!(item.kind, status);
            assert!(actionable(&pool, &item).await.unwrap());
            sqlx::query("UPDATE agent_runs SET status='running' WHERE id=?")
                .bind(run)
                .execute(&pool)
                .await
                .unwrap();
            assert!(!actionable(&pool, &item).await.unwrap());
        }
        let (pool, _, run, attempt) = fixture().await;
        sqlx::query("UPDATE agent_runs SET provider_id='claude'")
            .execute(&pool)
            .await
            .unwrap();
        event(&pool, run, attempt, 1, "input_requested").await;
        sqlx::query("UPDATE agent_runs SET status='failed'")
            .execute(&pool)
            .await
            .unwrap();
        assert_eq!(count(&pool).await, 0);
    }
    #[tokio::test]
    async fn event_and_delivery_rollback_together_and_replay_is_deduplicated() {
        let (pool, _, run, attempt) = fixture().await;
        event(&pool, run, attempt, 1, "approval_requested").await;
        sqlx::query("INSERT OR IGNORE INTO agent_events SELECT * FROM agent_events")
            .execute(&pool)
            .await
            .unwrap();
        assert_eq!(count(&pool).await, 2);
        let mut tx = pool.begin().await.unwrap();
        sqlx::query("UPDATE agent_runs SET status='failed'")
            .execute(&mut *tx)
            .await
            .unwrap();
        tx.rollback().await.unwrap();
        assert_eq!(count(&pool).await, 2);
    }
}

#[cfg(test)]
mod api_tests {
    use axum::body::Body;
    use tower::ServiceExt;

    use super::*;

    #[tokio::test]
    async fn authenticated_registration_is_owner_scoped_renewable_and_removable() {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        sqlx::raw_sql("CREATE TABLE projects(id BLOB PRIMARY KEY,name TEXT); CREATE TABLE agent_runs(id BLOB PRIMARY KEY,status TEXT,provider_id TEXT,workspace_id BLOB); CREATE TABLE agent_events(event_id BLOB,agent_run_id BLOB,run_attempt_id BLOB,event_envelope TEXT); CREATE TABLE agent_run_attempts(id BLOB,agent_run_id BLOB,attempt_number INTEGER); CREATE TABLE local_workspace_links(workspace_id BLOB,project_id BLOB,issue_id BLOB);").execute(&pool).await.unwrap();
        sqlx::raw_sql(include_str!(
            "../../../db/migrations/20261018180000_web_push.sql"
        ))
        .execute(&pool)
        .await
        .unwrap();
        let dir = tempfile::tempdir().unwrap();
        let access = server_access::test_support::access(
            pool.clone(),
            dir.path().to_owned(),
            json!({"keys":[]}),
        )
        .await;
        let cookie = server_access::test_support::web_session(&access).await;
        let service = PushService {
            access: access.clone(),
            config: Some(Arc::new(PushConfig {
                pem: vec![],
                public_key: "fixture".into(),
                subject: "mailto:test@example.com".into(),
            })),
            delivery_gate: Arc::new(tokio::sync::Mutex::new(())),
        };
        let app = access.layer(
            Router::new()
                .nest(
                    "/api",
                    subscription_router(service).with_state(pool.clone()),
                )
                .merge(server_access::router(access.clone())),
        );
        let response = app
            .clone()
            .oneshot(
                axum::http::Request::builder()
                    .uri("/api/access/status")
                    .header("host", "203.0.113.7:8443")
                    .header("cookie", &cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let status: Value = serde_json::from_slice(
            &axum::body::to_bytes(response.into_body(), 10000)
                .await
                .unwrap(),
        )
        .unwrap();
        let csrf = status["csrf"].as_str().unwrap();
        let project = Uuid::new_v4();
        sqlx::query("INSERT INTO projects VALUES(?,'Test')")
            .bind(project)
            .execute(&pool)
            .await
            .unwrap();
        let mut key = vec![0; 65];
        key[0] = 4;
        let payload = json!({"project_id":project,"subscription":{"endpoint":"https://fcm.googleapis.com/fcm/send/test","keys":{"p256dh":URL_SAFE_NO_PAD.encode(key),"auth":URL_SAFE_NO_PAD.encode([0;16])}}});
        let request = |method: &str, path: &str, value: &Value, authorized: bool| {
            let mut request = axum::http::Request::builder()
                .method(method)
                .uri(path)
                .header("host", "203.0.113.7:8443")
                .header("content-type", "application/json");
            if authorized {
                request = request.header("cookie", &cookie).header("x-lvk-csrf", csrf);
            }
            request.body(Body::from(value.to_string())).unwrap()
        };
        assert_eq!(
            app.clone()
                .oneshot(request("POST", "/api/web-push", &payload, false))
                .await
                .unwrap()
                .status(),
            StatusCode::UNAUTHORIZED
        );
        let no_csrf = axum::http::Request::builder()
            .method("POST")
            .uri("/api/web-push")
            .header("host", "203.0.113.7:8443")
            .header("cookie", &cookie)
            .header("content-type", "application/json")
            .body(Body::from(payload.to_string()))
            .unwrap();
        assert_eq!(
            app.clone().oneshot(no_csrf).await.unwrap().status(),
            StatusCode::FORBIDDEN
        );
        let mut invalid = payload.clone();
        invalid["subscription"]["endpoint"] = json!("https://127.0.0.1/private");
        assert_eq!(
            app.clone()
                .oneshot(request("POST", "/api/web-push", &invalid, true))
                .await
                .unwrap()
                .status(),
            StatusCode::BAD_REQUEST
        );
        invalid = payload.clone();
        invalid["project_id"] = json!(Uuid::new_v4());
        assert_eq!(
            app.clone()
                .oneshot(request("POST", "/api/web-push", &invalid, true))
                .await
                .unwrap()
                .status(),
            StatusCode::NOT_FOUND
        );
        let response = app
            .clone()
            .oneshot(request("POST", "/api/web-push", &payload, true))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let saved: Value = serde_json::from_slice(
            &axum::body::to_bytes(response.into_body(), 10000)
                .await
                .unwrap(),
        )
        .unwrap();
        let response = app
            .clone()
            .oneshot(request("POST", "/api/web-push", &payload, true))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let count: i64 = sqlx::query_scalar("SELECT count(*) FROM web_push_subscriptions")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 1);
        // Tamper ownership to represent another member's registration; endpoint
        // transfer, list disclosure and deletion must all be refused/scoped.
        sqlx::query("UPDATE web_push_subscriptions SET owner=?")
            .bind(Uuid::new_v4())
            .execute(&pool)
            .await
            .unwrap();
        assert_eq!(
            app.clone()
                .oneshot(request("POST", "/api/web-push", &payload, true))
                .await
                .unwrap()
                .status(),
            StatusCode::CONFLICT
        );
        let response = app
            .clone()
            .oneshot(request("GET", "/api/web-push", &json!({}), true))
            .await
            .unwrap();
        let listed: Value = serde_json::from_slice(
            &axum::body::to_bytes(response.into_body(), 10000)
                .await
                .unwrap(),
        )
        .unwrap();
        assert_eq!(listed["registrations"], json!([]));
        let path = format!("/api/web-push/{}", saved["id"].as_str().unwrap());
        assert_eq!(
            app.clone()
                .oneshot(request("POST", &path, &json!({}), true))
                .await
                .unwrap()
                .status(),
            StatusCode::NO_CONTENT
        );
        let count: i64 = sqlx::query_scalar("SELECT count(*) FROM web_push_subscriptions")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 1);
        let owner = Uuid::parse_str(status["user"]["id"].as_str().unwrap()).unwrap();
        sqlx::query("UPDATE web_push_subscriptions SET owner=?")
            .bind(owner)
            .execute(&pool)
            .await
            .unwrap();
        assert_eq!(
            app.oneshot(request("POST", &path, &json!({}), true))
                .await
                .unwrap()
                .status(),
            StatusCode::NO_CONTENT
        );
        let count: i64 = sqlx::query_scalar("SELECT count(*) FROM web_push_subscriptions")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 0);
    }
}
