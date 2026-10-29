//! Browser administrator settings. Deliberately absent from the AI MCP catalogue.

use axum::{
    Extension, Json, Router,
    extract::{DefaultBodyLimit, State},
    http::StatusCode,
    routing::get,
};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use ts_rs::TS;

use super::environment::RuntimeAccessSettings;

#[derive(Serialize, Deserialize, TS)]
pub struct RuntimeAccessView {
    #[ts(type = "number")]
    pub revision: i64,
    pub source: String,
    pub settings: RuntimeAccessSettings,
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
    if let Some((revision, text)) = sqlx::query_as::<_, (i64, String)>(
        "SELECT revision,settings FROM runtime_access_settings WHERE id=1",
    )
    .fetch_optional(pool)
    .await?
    {
        return Ok((revision, "saved".into(), serde_json::from_str(&text)?));
    }
    Ok((0, "default".into(), RuntimeAccessSettings::default()))
}
async fn read(
    State(pool): State<SqlitePool>,
    Extension(p): Extension<server_access::Principal>,
) -> Result<Json<RuntimeAccessView>, Error> {
    admin(&p)?;
    let (revision, source, settings) = load(&pool).await.map_err(unavailable)?;
    Ok(Json(RuntimeAccessView {
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
    // Revocation must work even if a saved root has disappeared.
    input.settings.validate_paths().map_err(bad)?;
    if input.settings.files_enabled {
        input.settings.validate().map_err(bad)?;
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
        revision,
        source: "saved".into(),
        settings: input.settings,
    }))
}
