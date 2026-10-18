//! Optional, setup-time Management API access. Credentials live only in a request;
//! provisioning never commits LVK authentication settings or promotes a member.
use axum::{Json, extract::State};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use serde_json::{Value, json};

use super::*;

const MAX_MANAGEMENT_TOKEN_BYTES: usize = 64 * 1024;

const CLIENT_FIELDS: &str = "client_id,name,app_type,client_metadata,callbacks,allowed_logout_urls,jwt_configuration,grant_types,token_endpoint_auth_method,is_first_party,oidc_conformant,refresh_token";
const RETRY: &str = "Auth0 setup did not finish. Existing LVK authentication is unchanged. Check Auth0 permissions, then preview again; created applications will be reused.";

// Provider messages may echo credentials or input values. Emit only our own
// finite vocabulary; never concatenate any provider-controlled text.
pub(super) fn rejection_detail(bytes: &[u8]) -> String {
    let value: Value = serde_json::from_slice(bytes).unwrap_or(Value::Null);
    let message = value["message"]
        .as_str()
        .unwrap_or_default()
        .to_ascii_lowercase();
    let words: Vec<_> = message
        .split(|c: char| !c.is_ascii_alphanumeric() && c != '_')
        .collect();
    let fields: Vec<_> = [
        "name",
        "app_type",
        "client_metadata",
        "callbacks",
        "allowed_logout_urls",
        "jwt_configuration",
        "alg",
        "lifetime_in_seconds",
        "secret_encoded",
        "scopes",
        "grant_types",
        "token_endpoint_auth_method",
        "client_authentication_methods",
        "is_first_party",
        "oidc_conformant",
        "refresh_token",
        "rotation_type",
        "expiration_type",
        "token_lifetime",
        "idle_token_lifetime",
        "leeway",
        "infinite_token_lifetime",
        "infinite_idle_token_lifetime",
        "organization_usage",
        "organization_require_behavior",
        "resource_parameter_profile",
        "flags",
    ]
    .into_iter()
    .filter(|field| words.contains(field))
    .collect();
    let reasons: Vec<_> = [
        "payload validation error",
        "additional properties not allowed",
        "invalid type",
        "expected type",
        "invalid grant types",
        "not allowed",
        "must be",
        "cannot",
        "required",
        "too long",
        "too short",
        "invalid uri",
        "invalid url",
    ]
    .into_iter()
    .filter(|reason| message.contains(reason))
    .collect();
    format!(
        "Provider validation hints: fields=[{}]; categories=[{}]. {}",
        fields.join(", "),
        reasons.join(", "),
        RETRY
    )
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Input {
    issuer: String,
    management_token: String,
    step: Step,
    #[serde(default)]
    connection_id: String,
    #[serde(default)]
    web_client_id: String,
    #[serde(default)]
    chatgpt_client_id: String,
    #[serde(default)]
    chatgpt_callback: String,
    #[serde(default)]
    mcp_enabled: bool,
    #[serde(default)]
    refresh_tokens: bool,
    #[serde(default)]
    confirmation: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum Step {
    Inspect,
    Preview,
    Apply,
}

#[derive(Serialize, Deserialize)]
struct Receipt {
    digest: String,
    session: String,
    expires: i64,
}

struct Management<'a> {
    input: &'a Input,
    http: reqwest::Client,
}
impl<'a> Management<'a> {
    fn new(input: &'a Input) -> Result<Self, Error> {
        provider::validate_issuer(&input.issuer)?;
        if input.management_token.is_empty() {
            return Err(Error::bad("Enter an Auth0 Management API access token"));
        }
        if input.management_token.len() > MAX_MANAGEMENT_TOKEN_BYTES {
            return Err(Error::bad(
                "Auth0 Management API access token exceeds the 65536-byte limit",
            ));
        }
        Ok(Self {
            input,
            http: reqwest::Client::builder()
                .redirect(reqwest::redirect::Policy::none())
                .connect_timeout(Duration::from_secs(5))
                .timeout(Duration::from_secs(15))
                .build()?,
        })
    }
    async fn request(
        &self,
        method: Method,
        path: &str,
        body: Option<&Value>,
    ) -> Result<Value, Error> {
        // Only endpoint templates enter diagnostics; never IDs, queries, payloads or URLs.
        let endpoint = match path.split('?').next().unwrap_or_default() {
            "clients" => "clients",
            "connections" => "connections",
            "resource-servers" => "resource-servers",
            "tenants/settings" => "tenants/settings",
            p if p.starts_with("clients/") => "clients/{id}",
            p if p.starts_with("connections/") => "connections/{id}/clients",
            p if p.starts_with("resource-servers/") => "resource-servers/{id}",
            _ => "unknown",
        };
        let failure = |stage: &str, status: Option<StatusCode>, hint: &str| {
            let code = status.map(|s| s.as_u16());
            tracing::warn!(operation = %method, endpoint, stage, http_status = ?code, "Auth0 provisioning request failed");
            let status = code.map(|s| format!(" HTTP {s}")).unwrap_or_default();
            Error {
                status: StatusCode::BAD_REQUEST,
                message: format!("Auth0 {method} /api/v2/{endpoint}:{status} {stage}. {hint}")
                    .into(),
            }
        };
        // All paths are internal constants or validated IDs, never response links.
        let mut request = self
            .http
            .request(
                method.clone(),
                format!("{}api/v2/{path}", self.input.issuer),
            )
            .bearer_auth(&self.input.management_token);
        if let Some(body) = body {
            request = request.json(body);
        }
        let mut response = request.send().await.map_err(|e| {
            let stage = if e.is_timeout() {
                "request timeout"
            } else if e.is_connect() {
                "connection failed"
            } else if e.is_builder() {
                "request construction failed"
            } else {
                "request transport failed"
            };
            failure(stage, None, RETRY)
        })?;
        let status = response.status();
        match response.status() {
            StatusCode::UNAUTHORIZED => {
                return Err(failure(
                    "HTTP rejection",
                    Some(status),
                    "Auth0 management token is invalid or expired; enter a new token and preview again",
                ));
            }
            StatusCode::FORBIDDEN => {
                return Err(failure(
                    "HTTP rejection",
                    Some(status),
                    "Auth0 management token lacks required permissions; check the setup guide and preview again",
                ));
            }
            StatusCode::TOO_MANY_REQUESTS => {
                return Err(failure(
                    "HTTP rejection",
                    Some(status),
                    "Auth0 rate limit reached; wait before previewing again",
                ));
            }
            status if !status.is_success() => {
                // Bound error-body reads independently from successful responses.
                let mut bytes = Vec::new();
                while let Some(chunk) = response
                    .chunk()
                    .await
                    .map_err(|_| failure("error response body read failed", Some(status), RETRY))?
                {
                    if bytes.len() + chunk.len() > 16 * 1024 {
                        return Err(failure(
                            "HTTP rejection (error response exceeds 16 KiB)",
                            Some(status),
                            RETRY,
                        ));
                    }
                    bytes.extend_from_slice(&chunk);
                }
                let detail = rejection_detail(&bytes);
                tracing::warn!(operation = %method, endpoint, http_status = status.as_u16(), detail, "Auth0 rejected provisioning settings");
                return Err(failure("HTTP rejection", Some(status), &detail));
            }
            StatusCode::NO_CONTENT => return Ok(Value::Null),
            _ => {}
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(|e| {
            failure(
                if e.is_timeout() {
                    "response body timeout"
                } else {
                    "response body read failed"
                },
                Some(status),
                RETRY,
            )
        })? {
            if bytes.len() + chunk.len() > 1024 * 1024 {
                return Err(failure("response exceeds 1 MiB", Some(status), RETRY));
            }
            bytes.extend_from_slice(&chunk);
        }
        serde_json::from_slice(&bytes)
            .map_err(|_| failure("invalid JSON response", Some(status), RETRY))
    }
    async fn list(&self, path: &str, fields: &str) -> Result<Vec<Value>, Error> {
        let mut items = Vec::new();
        for page in 0..100 {
            let result = self.request(Method::GET, &format!("{path}?page={page}&per_page=100&include_totals=false&fields={fields}&include_fields=true"), None).await?;
            let batch = result
                .as_array()
                .ok_or_else(|| Error::bad("Unexpected Auth0 list response"))?;
            items.extend(batch.iter().cloned());
            if batch.len() < 100 {
                return Ok(items);
            }
        }
        Err(Error::bad(
            "Auth0 tenant has too many entries for automatic setup",
        ))
    }
}

fn id(value: &str) -> Result<&str, Error> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
    {
        return Err(Error::bad("Invalid Auth0 object ID"));
    }
    Ok(value)
}
fn string<'a>(v: &'a Value, key: &str) -> Result<&'a str, Error> {
    v.get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| Error::bad("Incomplete Auth0 response"))
}
fn callback(value: &str) -> Result<(), Error> {
    let url = Url::parse(value)
        .map_err(|_| Error::bad("Enter the exact HTTPS callback URL shown by ChatGPT"))?;
    if url.scheme() != "https"
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
        || value.contains('*')
        || value.len() > 2048
    {
        return Err(Error::bad(
            "Enter the exact HTTPS callback URL shown by ChatGPT",
        ));
    }
    Ok(())
}
fn add_to_array(old: &Value, value: &str) -> Value {
    let mut values = old.as_array().cloned().unwrap_or_default();
    if !values.iter().any(|v| v == value) {
        values.push(json!(value));
    }
    json!(values)
}

#[derive(Serialize)]
struct ObjectPlan {
    id: Option<String>,
    previous: Value,
    settings: Value,
}
#[derive(Serialize)]
struct Plan {
    issuer: String,
    public_url: String,
    connection_id: String,
    connections: Vec<Value>,
    web: ObjectPlan,
    chatgpt: Option<ObjectPlan>,
    resource: Option<ObjectPlan>,
    tenant: Value,
}
fn client_plan(
    clients: &[Value],
    selected: &str,
    public_url: &str,
    role: &str,
    callback: &str,
    refresh: bool,
) -> Result<ObjectPlan, Error> {
    let owner = store::hash(public_url);
    let candidates: Vec<_> = clients
        .iter()
        .filter(|c| {
            if selected.is_empty() {
                c["client_metadata"]["lvk_owner"] == owner
                    && c["client_metadata"]["lvk_role"] == role
            } else {
                c["client_id"] == selected
            }
        })
        .collect();
    if candidates.len() > 1 || (!selected.is_empty() && candidates.is_empty()) {
        return Err(Error::bad(
            "Select exactly one existing application or create a new one",
        ));
    }
    let old = candidates.first().copied().cloned().unwrap_or(json!({}));
    if !old.is_object() || (!old["app_type"].is_null() && old["app_type"] != "regular_web") {
        return Err(Error::bad("Only Regular Web Applications can be reused"));
    }
    if let Some(old_owner) = old["client_metadata"]["lvk_owner"].as_str()
        && (old_owner != owner || old["client_metadata"]["lvk_role"] != role)
    {
        return Err(Error::bad(
            "Application belongs to another LVK URL or purpose; select a different application",
        ));
    }
    let mut metadata = old["client_metadata"]
        .as_object()
        .cloned()
        .unwrap_or_default();
    metadata.insert("lvk_owner".into(), json!(owner));
    metadata.insert("lvk_role".into(), json!(role));
    let mut jwt = old["jwt_configuration"]
        .as_object()
        .cloned()
        .unwrap_or_default();
    jwt.insert("alg".into(), json!("RS256"));
    let mut settings = json!({
        "name":old["name"].as_str().map(str::to_owned).unwrap_or_else(|| format!("LVK {role} ({public_url})")),
        "app_type":"regular_web", "is_first_party":true, "oidc_conformant":true,
        "token_endpoint_auth_method":"client_secret_basic", "jwt_configuration":jwt,
        "grant_types":if refresh { vec!["authorization_code","refresh_token"] } else { vec!["authorization_code"] },
        "callbacks":add_to_array(&old["callbacks"],callback), "client_metadata":metadata
    });
    if role == "web" {
        settings["allowed_logout_urls"] =
            add_to_array(&old["allowed_logout_urls"], &format!("{public_url}/"));
    }
    if refresh {
        settings["refresh_token"] = json!({"rotation_type":"rotating","expiration_type":"expiring","token_lifetime":2592000,"idle_token_lifetime":604800,"infinite_token_lifetime":false,"infinite_idle_token_lifetime":false,"leeway":0});
    }
    let client_id = old["client_id"].as_str().map(str::to_owned);
    if let Some(ref client_id) = client_id {
        id(client_id)?;
    }
    Ok(ObjectPlan {
        id: client_id,
        previous: old,
        settings,
    })
}

async fn plan(
    api: &Management<'_>,
    public_url: &str,
    clients: &[Value],
    mut connections: Vec<Value>,
) -> Result<Plan, Error> {
    let input = api.input;
    id(&input.connection_id)?;
    if !connections.iter().any(|c| {
        c["id"] == input.connection_id
            && matches!(c["strategy"].as_str(), Some("github" | "google-oauth2"))
    }) {
        return Err(Error::bad(
            "Select an existing GitHub or Google social connection",
        ));
    }
    for c in &connections {
        id(string(c, "id")?)?;
    }
    connections.sort_by_key(|c| c["id"].as_str().unwrap_or_default().to_owned());
    let web = client_plan(
        clients,
        &input.web_client_id,
        public_url,
        "web",
        &format!("{public_url}/api/access/callback"),
        false,
    )?;
    let chatgpt = if input.mcp_enabled {
        callback(&input.chatgpt_callback)?;
        Some(client_plan(
            clients,
            &input.chatgpt_client_id,
            public_url,
            "chatgpt",
            &input.chatgpt_callback,
            input.refresh_tokens,
        )?)
    } else {
        None
    };
    if chatgpt
        .as_ref()
        .is_some_and(|c| c.id.is_some() && c.id == web.id)
    {
        return Err(Error::bad("Web and ChatGPT must use separate applications"));
    }
    let mut tenant = json!({"authorization_response_iss_parameter_supported":true,"resource_parameter_profile":"compatibility"});
    let old_tenant = api.request(Method::GET, "tenants/settings", None).await?;
    tenant
        .as_object_mut()
        .unwrap()
        .retain(|key, value| old_tenant.get(key) != Some(value));
    let resource = if input.mcp_enabled {
        let identifier = format!("{public_url}/mcp");
        let resources = api.list("resource-servers","id,identifier,name,scopes,signing_alg,allow_offline_access,skip_consent_for_verifiable_first_party_clients").await?;
        let old = resources
            .into_iter()
            .find(|r| r["identifier"] == identifier)
            .unwrap_or(json!({}));
        if !old["signing_alg"].is_null() && old["signing_alg"] != "RS256" {
            return Err(Error::bad(
                "Existing MCP API is not RS256; review it in Auth0 before continuing",
            ));
        }
        let mut scopes = old["scopes"].as_array().cloned().unwrap_or_default();
        for scope in SCOPES {
            if !scopes.iter().any(|s| s["value"] == *scope) {
                scopes.push(json!({"value":scope,"description":scope}));
            }
        }
        let resource_id = old["id"].as_str().map(str::to_owned);
        if let Some(ref resource_id) = resource_id {
            id(resource_id)?;
        }
        let mut settings = json!({"name":old["name"].as_str().unwrap_or("LVK MCP"),"signing_alg":"RS256","scopes":scopes,"skip_consent_for_verifiable_first_party_clients":false});
        if resource_id.is_none() {
            settings["identifier"] = json!(identifier);
        }
        // Do not disable refresh access used by other clients of an existing API.
        if input.refresh_tokens || resource_id.is_none() {
            settings["allow_offline_access"] = json!(input.refresh_tokens);
        }
        Some(ObjectPlan {
            id: resource_id,
            previous: old,
            settings,
        })
    } else {
        None
    };
    Ok(Plan {
        issuer: input.issuer.clone(),
        public_url: public_url.into(),
        connection_id: input.connection_id.clone(),
        connections,
        web,
        chatgpt,
        resource,
        tenant,
    })
}

async fn save(
    api: &Management<'_>,
    collection: &str,
    object: &ObjectPlan,
    id_key: &str,
) -> Result<String, Error> {
    let result = if let Some(ref object_id) = object.id {
        api.request(
            Method::PATCH,
            &format!("{collection}/{}", id(object_id)?),
            Some(&object.settings),
        )
        .await?;
        object_id.clone()
    } else {
        let response = api
            .request(Method::POST, collection, Some(&object.settings))
            .await?;
        string(&response, id_key)?.to_owned()
    };
    id(&result)?;
    Ok(result)
}
async fn apply(api: &Management<'_>, plan: &Plan) -> Result<Value, Error> {
    if plan.tenant.as_object().is_some_and(|o| !o.is_empty()) {
        api.request(Method::PATCH, "tenants/settings", Some(&plan.tenant))
            .await?;
    }
    let web = save(api, "clients", &plan.web, "client_id").await?;
    if let Some(ref resource) = plan.resource {
        save(api, "resource-servers", resource, "id").await?;
    }
    let chatgpt = if let Some(ref chatgpt) = plan.chatgpt {
        Some(save(api, "clients", chatgpt, "client_id").await?)
    } else {
        None
    };
    // Per-client changes preserve every other application's enabled connections.
    for connection in &plan.connections {
        let connection_id = string(connection, "id")?;
        let changes: Vec<_> = std::iter::once(&web).chain(chatgpt.iter()).map(|client_id| json!({"client_id":client_id,"status":connection_id == plan.connection_id})).collect();
        api.request(
            Method::PATCH,
            &format!("connections/{}/clients", id(connection_id)?),
            Some(&json!(changes)),
        )
        .await?;
    }
    let mut result = json!({"issuer":plan.issuer,"mcp_enabled":plan.chatgpt.is_some(),"resource":format!("{}/mcp",plan.public_url)});
    for (role, client_id) in
        std::iter::once(("web", &web)).chain(chatgpt.as_ref().map(|c| ("chatgpt", c)))
    {
        let credentials = api
            .request(
                Method::GET,
                &format!(
                    "clients/{}?fields=client_id,client_secret&include_fields=true",
                    id(client_id)?
                ),
                None,
            )
            .await?;
        let secret = string(&credentials, "client_secret")?;
        if secret.is_empty() {
            return Err(Error::bad(
                "Auth0 did not return the client secret; check read:client_keys permission",
            ));
        }
        result[role] = json!({"client_id":client_id,"client_secret":secret});
    }
    Ok(result)
}

pub(crate) async fn handle(
    State(access): State<Access>,
    headers: HeaderMap,
    Json(input): Json<Input>,
) -> Result<Response, Error> {
    let session = access.management(&headers, true).await?;
    access.allow_mode("auth0")?;
    access.rate_limit().await?;
    let public_url = access
        .public_url()
        .ok_or_else(|| Error::bad("Configure a public HTTPS URL before Auth0 setup"))?;
    let api = Management::new(&input)?;
    let _lock = access
        .0
        .provisioning
        .try_lock()
        .map_err(|_| Error::bad("Another Auth0 setup is running; retry after it finishes"))?;
    let action = async {
        let clients = api.list("clients", CLIENT_FIELDS).await?;
        let connections = api.list("connections", "id,name,strategy").await?;
        if matches!(input.step, Step::Inspect) {
            let choices: Vec<_> = clients
                .iter()
                .filter(|c| c["app_type"] == "regular_web")
                .map(|c| json!({"client_id":c["client_id"],"name":c["name"]}))
                .collect();
            return Ok(Json(json!({"clients":choices,"connections":connections.into_iter().filter(|c| matches!(c["strategy"].as_str(),Some("github" | "google-oauth2"))).collect::<Vec<_>>()})).into_response());
        }
        let plan = plan(&api, public_url, &clients, connections).await?;
        let digest = store::hash(&serde_json::to_string(&plan)?);
        if matches!(input.step, Step::Preview) {
            let receipt = Receipt {
                digest,
                session: session.hash.clone(),
                expires: now() + TRANSACTION_LIFETIME,
            };
            return Ok(Json(json!({"plan":plan,"confirmation":URL_SAFE_NO_PAD.encode(access.encrypt(&receipt)?)})).into_response());
        }
        let receipt: Receipt = access
            .decrypt(
                &URL_SAFE_NO_PAD
                    .decode(&input.confirmation)
                    .map_err(|_| Error::bad("Preview the Auth0 changes before applying"))?,
            )
            .map_err(|_| Error::bad("Preview the Auth0 changes before applying"))?;
        if receipt.digest != digest || receipt.session != session.hash || receipt.expires <= now() {
            return Err(Error::bad(
                "Settings changed or preview expired; preview the Auth0 changes again",
            ));
        }
        access.management(&headers, true).await?;
        access
            .audit(
                session.user_id.as_deref(),
                "setup",
                "auth0-provision",
                None,
                "started",
            )
            .await?;
        let result = apply(&api, &plan).await;
        access
            .audit(
                session.user_id.as_deref(),
                "setup",
                "auth0-provision",
                None,
                if result.is_ok() {
                    "success"
                } else {
                    "partial-failure"
                },
            )
            .await?;
        Ok(Json(result?).into_response())
    };
    let response = tokio::time::timeout(Duration::from_secs(120), action)
        .await
        .map_err(|_| {
            tracing::warn!("Auth0 provisioning overall timeout (120 seconds)");
            Error::bad("Auth0 setup exceeded the overall 120-second timeout; preview again before retrying")
        })??;
    Ok(secure_response(response))
}
