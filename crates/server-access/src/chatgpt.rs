use axum::Json;
use serde_json::{Value, json};

use super::*;

#[derive(Serialize, Deserialize)]
pub(crate) struct Credentials {
    pub issuer: String,
    pub web_client_id: String,
    pub resource: String,
    pub client_id: String,
    pub client_secret: String,
}

pub(crate) async fn save(access: &Access, credentials: &Credentials) -> Result<(), Error> {
    sqlx::query("INSERT INTO access_chatgpt_client(id,encrypted) VALUES(1,?) ON CONFLICT(id) DO UPDATE SET encrypted=excluded.encrypted")
        .bind(access.encrypt(credentials)?)
        .execute(access.pool()).await?;
    Ok(())
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum Action {
    Status,
    Reveal,
    Save,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Input {
    action: Action,
    issuer: String,
    web_client_id: String,
    #[serde(default)]
    client_id: String,
    #[serde(default)]
    client_secret: String,
}

pub(crate) async fn handle(
    State(access): State<Access>,
    headers: HeaderMap,
    Json(input): Json<Input>,
) -> Result<Response, Error> {
    let session = access.management(&headers, true).await?;
    access.allow_mode("auth0")?;
    let resource = access.resource().ok_or_else(|| {
        Error::bad("Configure a public HTTPS URL before saving ChatGPT credentials")
    })?;
    if matches!(input.action, Action::Save) {
        provider::validate_issuer(&input.issuer)?;
        if input.web_client_id.is_empty()
            || input.web_client_id.len() > 256
            || input.client_id.is_empty()
            || input.client_id.len() > 256
            || input.client_id == input.web_client_id
            || input.client_secret.is_empty()
            || input.client_secret.len() > 4096
        {
            return Err(Error::bad(
                "Enter a dedicated ChatGPT Client ID and Client Secret, separate from the Web application",
            ));
        }
        save(
            &access,
            &Credentials {
                issuer: input.issuer.clone(),
                web_client_id: input.web_client_id.clone(),
                resource: resource.clone(),
                client_id: input.client_id,
                client_secret: input.client_secret,
            },
        )
        .await?;
        access
            .audit(
                session.user_id.as_deref(),
                "management",
                "chatgpt-client-save",
                None,
                "success",
            )
            .await?;
    }
    let encrypted: Option<Vec<u8>> =
        sqlx::query_scalar("SELECT encrypted FROM access_chatgpt_client WHERE id=1")
            .fetch_optional(access.pool())
            .await?;
    let credentials = encrypted
        .map(|bytes| access.decrypt::<Credentials>(&bytes))
        .transpose()?;
    let credentials = credentials.filter(|c| {
        c.issuer == input.issuer && c.web_client_id == input.web_client_id && c.resource == resource
    });
    let value: Value = if let Some(c) = credentials {
        if matches!(input.action, Action::Reveal) {
            // Deliberate, CSRF-protected admin access; never include secrets in audit.
            access
                .audit(
                    session.user_id.as_deref(),
                    "management",
                    "chatgpt-client-reveal",
                    None,
                    "success",
                )
                .await?;
            json!({"available":true,"client_id":c.client_id,"client_secret":c.client_secret})
        } else {
            json!({"available":true,"client_id":c.client_id})
        }
    } else if matches!(input.action, Action::Reveal) {
        return Err(Error::bad(
            "No saved ChatGPT credentials match this Auth0 issuer and Web application",
        ));
    } else {
        json!({"available":false})
    };
    Ok(secure_response(Json(value).into_response()))
}
