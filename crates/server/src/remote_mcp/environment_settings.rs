//! Browser administrator settings. Deliberately absent from the AI MCP catalogue.
use std::io::Read;

use axum::{
    Extension, Json, Router,
    extract::{DefaultBodyLimit, State},
    http::StatusCode,
    routing::get,
};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use ts_rs::TS;

use super::environment::{Environment, RuntimeAccessSettings};

#[derive(Serialize, Deserialize, TS)]
pub struct RuntimeAccessView {
    pub projects: Vec<RuntimeAccessProject>,
    #[ts(type = "number")]
    pub revision: i64,
    pub source: String,
    pub settings: RuntimeAccessSettings,
}
#[derive(Serialize, Deserialize, TS, sqlx::FromRow)]
pub struct RuntimeAccessProject {
    pub id: uuid::Uuid,
    pub name: String,
}
async fn projects(pool: &SqlitePool) -> Result<Vec<RuntimeAccessProject>, Error> {
    sqlx::query_as("SELECT id,name FROM projects ORDER BY name,id")
        .fetch_all(pool)
        .await
        .map_err(unavailable)
}
#[derive(Deserialize, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct SaveRuntimeAccess {
    #[ts(type = "number")]
    pub revision: i64,
    pub settings: RuntimeAccessSettings,
}
type Error = (StatusCode, String);
fn bad(e: impl std::fmt::Display) -> Error {
    (StatusCode::BAD_REQUEST, e.to_string())
}
fn unavailable(e: impl std::fmt::Display) -> Error {
    tracing::error!(%e, "Runtime settings storage failed");
    (
        StatusCode::SERVICE_UNAVAILABLE,
        "Runtime settings unavailable".into(),
    )
}
fn admin(p: &server_access::Principal) -> Result<(), Error> {
    if p.admin && matches!(p.channel, "web" | "basic" | "local") {
        Ok(())
    } else {
        Err((
            StatusCode::FORBIDDEN,
            "Browser administrator required".into(),
        ))
    }
}
pub fn router(pool: SqlitePool) -> Router {
    Router::new()
        .route("/api/runtime-access-settings", get(read).put(save))
        .layer(DefaultBodyLimit::max(65536))
        .with_state(pool)
}
pub(super) async fn load(
    pool: &SqlitePool,
) -> anyhow::Result<(i64, String, RuntimeAccessSettings)> {
    load_with_legacy(
        pool,
        std::env::var_os("LVK_RUNTIME_ACCESS_CONFIG").map(std::path::PathBuf::from),
    )
    .await
}
pub(super) async fn load_with_legacy(
    pool: &SqlitePool,
    legacy: Option<std::path::PathBuf>,
) -> anyhow::Result<(i64, String, RuntimeAccessSettings)> {
    if let Some((revision, text)) = sqlx::query_as::<_, (i64, String)>(
        "SELECT revision,settings FROM runtime_access_settings WHERE id=1",
    )
    .fetch_optional(pool)
    .await?
    {
        return Ok((revision, "saved".into(), serde_json::from_str(&text)?));
    }
    let Some(path) = legacy else {
        return Ok((0, "default".into(), RuntimeAccessSettings::default()));
    };
    let mut bytes = vec![];
    std::fs::File::open(path)?
        .take(65537)
        .read_to_end(&mut bytes)?;
    anyhow::ensure!(
        bytes.len() <= 65536,
        "Runtime configuration exceeds size limit"
    );
    let mut settings: RuntimeAccessSettings = serde_json::from_slice(&bytes)?;
    if settings.enabled && !settings.grants.is_empty() {
        settings.grants = Environment::new(settings.grants)?.grants;
    }
    anyhow::ensure!(
        !settings.enabled || !settings.grants.is_empty(),
        "Enabled runtime requires grants"
    );
    Ok((0, "legacy_file".into(), settings))
}
async fn read(
    State(pool): State<SqlitePool>,
    Extension(p): Extension<server_access::Principal>,
) -> Result<Json<RuntimeAccessView>, Error> {
    admin(&p)?;
    let (revision, source, settings) = load(&pool).await.map_err(unavailable)?;
    Ok(Json(RuntimeAccessView {
        projects: projects(&pool).await?,
        revision,
        source,
        settings,
    }))
}
async fn save(
    State(pool): State<SqlitePool>,
    Extension(p): Extension<server_access::Principal>,
    Json(mut input): Json<SaveRuntimeAccess>,
) -> Result<Json<RuntimeAccessView>, Error> {
    admin(&p)?;
    if input.settings.enabled && input.settings.grants.is_empty() {
        return Err(bad("Enabled runtime requires a grant"));
    }
    let prior: Option<String> =
        sqlx::query_scalar("SELECT settings FROM runtime_access_settings WHERE id=1")
            .fetch_optional(&pool)
            .await
            .map_err(unavailable)?;
    let disabling_unchanged = !input.settings.enabled
        && prior
            .and_then(|text| serde_json::from_str::<RuntimeAccessSettings>(&text).ok())
            .is_some_and(|old| {
                serde_json::to_value(old.grants).ok()
                    == serde_json::to_value(&input.settings.grants).ok()
            });
    if !input.settings.grants.is_empty() && !disabling_unchanged {
        input.settings.grants = Environment::new(input.settings.grants).map_err(bad)?.grants;
    }
    let text = serde_json::to_string(&input.settings).map_err(bad)?;
    if text.len() > 65536 {
        return Err(bad("Runtime configuration exceeds size limit"));
    }
    let mut tx = pool
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(unavailable)?;
    if p.channel != "local" {
        let active: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM access_members WHERE id=? AND role='admin' AND state='active')").bind(p.user_id.to_string()).fetch_one(&mut *tx).await.map_err(unavailable)?;
        if !active {
            return Err((
                StatusCode::FORBIDDEN,
                "Active administrator required".into(),
            ));
        }
    }
    // Revocation must remain possible when a previously saved path or Project disappeared.
    for grant in input
        .settings
        .grants
        .iter()
        .filter(|_| !disabling_unchanged)
    {
        let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM projects WHERE id=?)")
            .bind(grant.project_id)
            .fetch_one(&mut *tx)
            .await
            .map_err(unavailable)?;
        if !exists {
            return Err(bad("Unknown Project"));
        }
    }
    let revision: Option<i64> =
        sqlx::query_scalar("SELECT revision FROM runtime_access_settings WHERE id=1")
            .fetch_optional(&mut *tx)
            .await
            .map_err(unavailable)?;
    if revision.unwrap_or(0) != input.revision {
        return Err((
            StatusCode::CONFLICT,
            "Settings changed; reload before saving".into(),
        ));
    }
    let revision = input
        .revision
        .checked_add(1)
        .ok_or_else(|| bad("Invalid revision"))?;
    sqlx::query("INSERT INTO runtime_access_settings(id,revision,settings) VALUES(1,?,?) ON CONFLICT(id) DO UPDATE SET revision=excluded.revision,settings=excluded.settings").bind(revision).bind(text).execute(&mut *tx).await.map_err(unavailable)?;
    sqlx::query("INSERT INTO access_audit(at,actor,channel,action,target,outcome) VALUES(strftime('%s','now'),?,'web','runtime-access-save',?,'success')").bind(p.user_id.to_string()).bind(revision.to_string()).execute(&mut *tx).await.map_err(unavailable)?;
    tx.commit().await.map_err(unavailable)?;
    Ok(Json(RuntimeAccessView {
        projects: projects(&pool).await?,
        revision,
        source: "saved".into(),
        settings: input.settings,
    }))
}
