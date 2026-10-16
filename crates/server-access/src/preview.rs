//! One-use, browser-bound handoff to an optional isolated preview origin.
//! Preview credentials never authenticate LVK APIs and never reach project code.
use axum::{Form, extract::Query, response::Html};

use super::{
    store::{check_csrf, hash, random},
    *,
};

#[derive(Deserialize)]
pub(crate) struct Target {
    origin: String,
    browser: String,
}
#[derive(Deserialize)]
pub(crate) struct GrantInput {
    origin: String,
    browser: String,
    csrf: String,
}
#[derive(Deserialize)]
struct Exchange {
    grant: String,
}

impl Access {
    fn preview_origin(&self, value: &str) -> Result<(), Error> {
        validate_public_url(value).map_err(|_| Error::bad("Invalid preview origin"))?;
        let url = Url::parse(value)?;
        let domain = self
            .0
            .preview_domain
            .as_deref()
            .ok_or(Error::bad("Preview domain is not configured"))?;
        let suffix = format!(".{domain}");
        let label = url
            .host_str()
            .and_then(|h| h.strip_suffix(&suffix))
            .ok_or(Error::bad("Invalid preview host"))?;
        let port = label
            .parse::<u16>()
            .map_err(|_| Error::bad("Invalid preview port"))?;
        let app = Url::parse(self.public_url().ok_or_else(Error::unavailable)?)?;
        if port < 1024
            || [3000, 3001, 4020, 4021, 8443, 8444].contains(&port)
            || label != port.to_string()
            || url.port() != app.port()
            || app
                .host_str()
                .is_some_and(|h| h == domain || h.ends_with(&suffix))
        {
            return Err(Error::forbidden(
                "Preview origin is not isolated or uses a reserved port",
            ));
        }
        Ok(())
    }
    pub fn preview_layer(&self, router: Router) -> Router {
        router.layer(axum::middleware::from_fn_with_state(self.clone(), gate))
    }
}
fn page(access: &Access, html: String, action: &str) -> Response {
    let mut response = Html(html).into_response();
    let csp = format!(
        "default-src 'none'; style-src 'unsafe-inline'; form-action {action}; frame-ancestors {}",
        access.public_url().unwrap_or("'none'")
    );
    response.headers_mut().insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_str(&csp).unwrap(),
    );
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response.headers_mut().insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("no-referrer"),
    );
    response
}
fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('\'', "&#39;")
}
fn preview_cookie(response: &mut Response, name: &str, value: &str, age: i64) {
    response.headers_mut().append(
        header::SET_COOKIE,
        HeaderValue::from_str(&format!(
            "{name}={value}; Path=/; Secure; HttpOnly; SameSite=None; Max-Age={age}"
        ))
        .unwrap(),
    );
}
pub(crate) async fn authorize(
    State(access): State<Access>,
    headers: HeaderMap,
    Query(input): Query<Target>,
) -> Result<Response, Error> {
    access.preview_origin(&input.origin)?;
    if input.browser.len() != 43 {
        return Err(Error::bad("Invalid preview browser binding"));
    }
    let session = access.session(&headers).await?;
    if session.kind != "web" {
        return Err(Error::forbidden("Web login required"));
    }
    let action = format!(
        "{}/api/access/preview/authorize",
        access.public_url().unwrap()
    );
    Ok(page(
        &access,
        format!(
            "<!doctype html><meta charset=utf-8><title>LVK preview</title><p>Allow this browser to view {} using your LVK session?</p><form method=post action=\"{}\"><input type=hidden name=origin value=\"{}\"><input type=hidden name=browser value=\"{}\"><input type=hidden name=csrf value=\"{}\"><button>Open preview</button></form>",
            escape(&input.origin),
            escape(&action),
            escape(&input.origin),
            escape(&input.browser),
            escape(&session.csrf)
        ),
        &action,
    ))
}
pub(crate) async fn grant(
    State(access): State<Access>,
    headers: HeaderMap,
    Form(input): Form<GrantInput>,
) -> Result<Response, Error> {
    access.preview_origin(&input.origin)?;
    if input.browser.len() != 43 {
        return Err(Error::bad("Invalid browser binding"));
    }
    let session = access.session(&headers).await?;
    if session.kind != "web" {
        return Err(Error::forbidden("Web login required"));
    }
    let mut csrf_headers = HeaderMap::new();
    csrf_headers.insert(
        "x-lvk-csrf",
        HeaderValue::from_str(&input.csrf).map_err(|_| Error::unauthorized())?,
    );
    check_csrf(&csrf_headers, &session.csrf)?;
    let token = random();
    let mut tx = access.pool().begin().await?;
    sqlx::query("DELETE FROM access_preview_grants WHERE expires<=?")
        .bind(now())
        .execute(&mut *tx)
        .await?;
    sqlx::query("INSERT INTO access_preview_grants(hash,browser_hash,origin,session_hash,expires) VALUES(?,?,?,?,?)").bind(hash(&token)).bind(&input.browser).bind(&input.origin).bind(&session.hash).bind(now()+60).execute(&mut *tx).await?;
    tx.commit().await?;
    access
        .audit(
            session.user_id.as_deref(),
            "web",
            "preview-authorize",
            Some(&input.origin),
            "success",
        )
        .await?;
    let action = format!("{}/.lvk-preview/exchange", input.origin);
    // Explicit form submission avoids putting the one-use capability in URLs,
    // browser history, Referer headers or project-controlled JavaScript.
    Ok(page(
        &access,
        format!(
            "<!doctype html><meta charset=utf-8><title>LVK preview</title><form method=post action=\"{}\"><input type=hidden name=grant value=\"{}\"><button>Continue to preview</button></form>",
            escape(&action),
            token
        ),
        &action,
    ))
}
async fn preview_principal(
    access: &Access,
    headers: &HeaderMap,
    origin: &str,
) -> Result<Principal, Error> {
    let token = cookie(headers, "__Host-lvk_preview").ok_or_else(Error::unauthorized)?;
    let parent: Option<String> = sqlx::query_scalar(
        "SELECT session_hash FROM access_preview_sessions WHERE hash=? AND origin=? AND expires>?",
    )
    .bind(hash(&token))
    .bind(origin)
    .bind(now())
    .fetch_optional(access.pool())
    .await?;
    let parent = parent.ok_or_else(Error::unauthorized)?;
    let row=sqlx::query_as::<_,store::Session>("SELECT hash,user_id,kind,csrf,expires,revision FROM access_sessions WHERE hash=? AND kind='web' AND expires>? AND touched>?").bind(&parent).bind(now()).bind(now()-WEB_IDLE).fetch_optional(access.pool()).await?.ok_or_else(Error::unauthorized)?;
    let member = sqlx::query_as::<_, Member>(
        "SELECT id,issuer,subject,display_name,state,role FROM access_members WHERE id=?",
    )
    .bind(row.user_id)
    .fetch_one(access.pool())
    .await?;
    let mut p = Principal::member(&member, row.revision, "web", row.expires)?;
    p.session_hash = Some(parent);
    if !access.valid(&p).await {
        return Err(Error::unauthorized());
    }
    Ok(p)
}
async fn gate(State(access): State<Access>, mut request: Request, next: Next) -> Response {
    match access.load().await {
        Ok((Some(Config::Local), _)) => return next.run(request).await,
        Ok((Some(_), _)) => {}
        _ => return Error::unavailable().into_response(),
    }
    let host = request
        .headers()
        .get(header::HOST)
        .and_then(|h| h.to_str().ok())
        .unwrap_or("");
    let origin = format!("https://{host}");
    if let Err(e) = access.preview_origin(&origin) {
        return e.into_response();
    }
    if request.uri().path() == "/.lvk-preview/exchange" {
        if request.method() != Method::POST
            || request
                .headers()
                .get(header::ORIGIN)
                .and_then(|v| v.to_str().ok())
                != access.public_url()
        {
            return Error::forbidden("Invalid preview exchange origin").into_response();
        }
        let browser = match cookie(request.headers(), "__Host-lvk_preview_browser") {
            Some(v) => v,
            None => return Error::unauthorized().into_response(),
        };
        let form =
            match <Form<Exchange> as axum::extract::FromRequest<()>>::from_request(request, &())
                .await
            {
                Ok(v) => v.0,
                Err(_) => return Error::bad("Invalid preview grant").into_response(),
            };
        return match exchange(&access, &origin, &browser, &form.grant).await {
            Ok(r) => r,
            Err(e) => e.into_response(),
        };
    }
    // Project requests may originate from their own isolated origin or the LVK
    // iframe host. Cross-origin unsafe requests must not use ambient cookies.
    if let Some(sent) = request.headers().get(header::ORIGIN)
        && sent.to_str().ok() != Some(origin.as_str())
    {
        return Error::forbidden("Invalid preview origin").into_response();
    }
    let principal = match preview_principal(&access, request.headers(), &origin).await {
        Ok(p) => p,
        Err(_) => {
            if request.method() != Method::GET || request.headers().contains_key(header::UPGRADE) {
                return Error::unauthorized().into_response();
            }
            let browser = cookie(request.headers(), "__Host-lvk_preview_browser")
                .filter(|v| v.len() == 43)
                .unwrap_or_else(random);
            let mut url = Url::parse(&format!(
                "{}/api/access/preview/authorize",
                access.public_url().unwrap()
            ))
            .unwrap();
            url.query_pairs_mut()
                .append_pair("origin", &origin)
                .append_pair("browser", &hash(&browser));
            let mut r = page(
                &access,
                format!(
                    "<!doctype html><meta charset=utf-8><title>LVK preview login</title><p>Log in to LVK first, then <a target=_blank rel=noopener href=\"{}\">authorise this preview</a>. If your browser blocks third-party cookies, open the preview in its own tab before authorising.</p>",
                    escape(url.as_str())
                ),
                access.public_url().unwrap(),
            );
            preview_cookie(&mut r, "__Host-lvk_preview_browser", &browser, 300);
            return r;
        }
    };
    let lease = Lease {
        access: access.clone(),
        principal: principal.clone(),
    };
    request.extensions_mut().insert(lease.clone());
    request.extensions_mut().insert(principal.clone());
    request.headers_mut().remove(header::AUTHORIZATION);
    request.headers_mut().remove("x-lvk-internal-token");
    let remaining = request
        .headers()
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(';'))
        .filter(|v| {
            !v.trim().starts_with("__Host-lvk_preview")
                && !v.trim().starts_with("lvk_session=")
                && !v.trim().starts_with("lvk_transaction=")
        })
        .collect::<Vec<_>>()
        .join("; ");
    request.headers_mut().remove(header::COOKIE);
    if let Ok(value) = HeaderValue::from_str(&remaining) {
        request.headers_mut().insert(header::COOKIE, value);
    }
    let mut response = ACTOR.scope(principal, next.run(request)).await;
    if response
        .headers()
        .get(header::CONTENT_TYPE)
        .is_some_and(|v| v.as_bytes().starts_with(b"text/event-stream"))
    {
        use futures_util::StreamExt;
        let (parts, body) = response.into_parts();
        response = Response::from_parts(
            parts,
            axum::body::Body::from_stream(
                body.into_data_stream()
                    .take_until(async move { lease.until_revoked().await }),
            ),
        );
    }
    response
}
async fn exchange(
    access: &Access,
    origin: &str,
    browser: &str,
    grant: &str,
) -> Result<Response, Error> {
    let token = random();
    let mut tx = access.pool().begin_with("BEGIN IMMEDIATE").await?;
    let parent:Option<String>=sqlx::query_scalar("DELETE FROM access_preview_grants WHERE hash=? AND browser_hash=? AND origin=? AND expires>? RETURNING session_hash").bind(hash(grant)).bind(hash(browser)).bind(origin).bind(now()).fetch_optional(&mut *tx).await?;
    let parent = parent.ok_or_else(Error::unauthorized)?;
    let valid:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM access_sessions s JOIN access_members m ON m.id=s.user_id JOIN access_settings c ON c.id=1 WHERE s.hash=? AND s.kind='web' AND s.expires>? AND s.touched>? AND m.state='active' AND s.revision=c.revision)").bind(&parent).bind(now()).bind(now()-WEB_IDLE).fetch_one(&mut *tx).await?;
    if !valid {
        return Err(Error::unauthorized());
    }
    sqlx::query("DELETE FROM access_preview_sessions WHERE expires<=?")
        .bind(now())
        .execute(&mut *tx)
        .await?;
    sqlx::query(
        "INSERT INTO access_preview_sessions(hash,origin,session_hash,expires) VALUES(?,?,?,?)",
    )
    .bind(hash(&token))
    .bind(origin)
    .bind(parent)
    .bind(now() + WEB_LIFETIME)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    let mut response = axum::response::Redirect::to("/").into_response();
    preview_cookie(&mut response, "__Host-lvk_preview", &token, WEB_LIFETIME);
    preview_cookie(&mut response, "__Host-lvk_preview_browser", "", 0);
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    Ok(response)
}
