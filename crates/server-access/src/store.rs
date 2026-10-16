use std::{fs::OpenOptions, io::Write, path::Path};

use aes_gcm::{
    Aes256Gcm, KeyInit,
    aead::{Aead, AeadCore, OsRng as AeadRng},
};
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier, password_hash::SaltString};
use base64::{
    Engine,
    engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD},
};
use rand::RngCore;
use sha2::{Digest, Sha256};
use sqlx::{Row, SqlitePool};
use subtle::ConstantTimeEq;

use super::*;

pub(crate) fn random() -> String {
    let mut b = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut b);
    URL_SAFE_NO_PAD.encode(b)
}
pub(crate) fn hash(value: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(value.as_bytes()))
}
fn key(path: &Path) -> anyhow::Result<Vec<u8>> {
    let meta = std::fs::symlink_metadata(path)?;
    ensure!(
        meta.is_file() && !meta.file_type().is_symlink(),
        "Invalid authentication key file"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        ensure!(
            meta.permissions().mode() & 0o077 == 0,
            "Authentication key must be owner-only"
        );
    }
    let bytes = std::fs::read(path)?;
    ensure!(bytes.len() == 32, "Invalid authentication key");
    Ok(bytes)
}
pub(crate) fn ensure_key(path: &Path) -> anyhow::Result<()> {
    if path.exists() {
        key(path)?;
        return Ok(());
    }
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path)?;
    let mut bytes = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    file.write_all(&bytes)?;
    file.sync_all()?;
    Ok(())
}
impl Access {
    pub(crate) fn encrypt<T: Serialize>(&self, value: &T) -> Result<Vec<u8>, Error> {
        let cipher =
            Aes256Gcm::new_from_slice(&key(&self.0.key_path)?).map_err(|_| Error::unavailable())?;
        let nonce = Aes256Gcm::generate_nonce(&mut AeadRng);
        let encrypted = cipher
            .encrypt(&nonce, serde_json::to_vec(value)?.as_slice())
            .map_err(|_| Error::unavailable())?;
        let mut result = nonce.to_vec();
        result.extend(encrypted);
        Ok(result)
    }
    pub(crate) fn decrypt<T: serde::de::DeserializeOwned>(&self, bytes: &[u8]) -> Result<T, Error> {
        if bytes.len() < 28 {
            return Err(Error::unavailable());
        }
        let cipher =
            Aes256Gcm::new_from_slice(&key(&self.0.key_path)?).map_err(|_| Error::unavailable())?;
        let plain = cipher
            .decrypt(bytes[..12].into(), &bytes[12..])
            .map_err(|_| Error::unavailable())?;
        Ok(serde_json::from_slice(&plain)?)
    }
    pub(crate) async fn load(&self) -> Result<(Option<Config>, i64), Error> {
        let row = sqlx::query("SELECT revision,encrypted FROM access_settings WHERE id=1")
            .fetch_optional(self.pool())
            .await?;
        let Some(row) = row else {
            return Ok((
                if !self.0.public
                    && !self.0.key_path.with_extension("configured").exists()
                    && self.0.forced_mode.as_deref().is_none_or(|m| m == "local")
                {
                    Some(Config::Local)
                } else {
                    None
                },
                0,
            ));
        };
        let config: Config = self.decrypt(&row.get::<Vec<u8>, _>("encrypted"))?;
        self.allow_mode(config.mode())?;
        // A persisted malformed secret/configuration locks the application.
        match &config {
            Config::Basic {
                username,
                password_hash,
            } => {
                if username.is_empty() || PasswordHash::new(password_hash).is_err() {
                    return Err(Error::unavailable());
                }
            }
            Config::Auth0 {
                issuer,
                client_id,
                client_secret,
                ..
            } => {
                provider::validate_issuer(issuer)?;
                if client_id.is_empty() || client_secret.is_empty() {
                    return Err(Error::unavailable());
                }
            }
            Config::Local => {}
        }
        Ok((Some(config), row.get("revision")))
    }
    pub(crate) async fn audit(
        &self,
        actor: Option<&str>,
        channel: &str,
        action: &str,
        target: Option<&str>,
        outcome: &str,
    ) -> Result<(), Error> {
        sqlx::query(
            "INSERT INTO access_audit(at,actor,channel,action,target,outcome) VALUES(?,?,?,?,?,?)",
        )
        .bind(now())
        .bind(actor)
        .bind(channel)
        .bind(action)
        .bind(target)
        .bind(outcome)
        .execute(self.pool())
        .await?;
        Ok(())
    }
    pub(crate) async fn identity(
        &self,
        issuer: &str,
        subject: &str,
        name: Option<&str>,
    ) -> Result<Member, Error> {
        if subject.is_empty() || subject.len() > 1024 {
            return Err(Error::unauthorized());
        }
        let display = name
            .filter(|s| !s.is_empty())
            .unwrap_or(subject)
            .chars()
            .take(200)
            .collect::<String>();
        sqlx::query("INSERT INTO access_members(id,issuer,subject,display_name,state,role,created_at) VALUES(?,?,?,?,'pending','member',?) ON CONFLICT(issuer,subject) DO NOTHING")
            .bind(Uuid::new_v4().to_string()).bind(issuer).bind(subject).bind(display).bind(now()).execute(self.pool()).await?;
        Ok(sqlx::query_as("SELECT id,issuer,subject,display_name,state,role FROM access_members WHERE issuer=? AND subject=?").bind(issuer).bind(subject).fetch_one(self.pool()).await?)
    }
    pub(crate) async fn create_session(
        &self,
        user: Option<&str>,
        kind: &str,
        revision: i64,
    ) -> Result<(String, String), Error> {
        let token = random();
        let csrf = random();
        let lifetime = if kind == "web" {
            WEB_LIFETIME
        } else {
            SETUP_LIFETIME
        };
        sqlx::query("INSERT INTO access_sessions(hash,user_id,kind,csrf,expires,touched,revision) VALUES(?,?,?,?,?,?,?)").bind(hash(&token)).bind(user).bind(kind).bind(&csrf).bind(now()+lifetime).bind(now()).bind(revision).execute(self.pool()).await?;
        Ok((token, csrf))
    }
    pub(crate) async fn session(&self, headers: &HeaderMap) -> Result<Session, Error> {
        let token = cookie(headers, "lvk_session").ok_or_else(Error::unauthorized)?;
        let s=sqlx::query_as::<_,Session>("SELECT hash,user_id,kind,csrf,expires,revision FROM access_sessions WHERE hash=? AND expires>? AND touched>?")
            .bind(hash(&token)).bind(now()).bind(now()-WEB_IDLE).fetch_optional(self.pool()).await?.ok_or_else(Error::unauthorized)?;
        if s.kind == "web" {
            let (_, revision) = self.load().await?;
            if revision != s.revision {
                return Err(Error::unauthorized());
            }
            let active = sqlx::query_scalar::<_, bool>(
                "SELECT EXISTS(SELECT 1 FROM access_members WHERE id=? AND state='active')",
            )
            .bind(&s.user_id)
            .fetch_one(self.pool())
            .await?;
            if !active {
                return Err(Error::forbidden("Account is not active"));
            }
            sqlx::query("UPDATE access_sessions SET touched=? WHERE hash=? AND touched<?")
                .bind(now())
                .bind(&s.hash)
                .bind(now() - 60)
                .execute(self.pool())
                .await?;
        }
        Ok(s)
    }
    pub(crate) async fn web_principal(&self, headers: &HeaderMap) -> Result<Principal, Error> {
        let (config, revision) = self.load().await?;
        let config = config.ok_or(Error {
            status: StatusCode::LOCKED,
            message: "Server setup is incomplete",
        })?;
        if matches!(config, Config::Local) {
            let mut p = Principal::local();
            p.revision = revision;
            return Ok(p);
        }
        // Browser sessions are bound to a revision and active membership.
        if cookie(headers, "lvk_session").is_some()
            && let Ok(session) = self.session(headers).await
            && session.kind == "web"
        {
            let member = sqlx::query_as::<_, Member>(
                "SELECT id,issuer,subject,display_name,state,role FROM access_members WHERE id=?",
            )
            .bind(&session.user_id)
            .fetch_one(self.pool())
            .await?;
            let mut p = Principal::member(&member, revision, "web", session.expires)?;
            p.session_hash = Some(session.hash);
            return Ok(p);
        }
        if let Config::Basic {
            username,
            password_hash,
        } = config
        {
            let header = headers
                .get(header::AUTHORIZATION)
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.split_once(' '))
                .filter(|(scheme, _)| scheme.eq_ignore_ascii_case("Basic"))
                .map(|(_, credential)| credential)
                .filter(|v| v.len() < 4096)
                .ok_or_else(Error::unauthorized)?;
            self.rate_limit().await?;
            let decoded = STANDARD.decode(header).map_err(|_| Error::unauthorized())?;
            let decoded = String::from_utf8(decoded).map_err(|_| Error::unauthorized())?;
            let (user, password) = decoded.split_once(':').ok_or_else(Error::unauthorized)?;
            let matches_user = hash(user)
                .as_bytes()
                .ct_eq(hash(&username).as_bytes())
                .into();
            let password = password.to_owned();
            let _slot = self.0.password_slots.try_acquire().map_err(|_| Error {
                status: StatusCode::TOO_MANY_REQUESTS,
                message: "Authentication busy",
            })?;
            let valid = tokio::task::spawn_blocking(move || {
                PasswordHash::new(&password_hash).ok().is_some_and(|h| {
                    Argon2::default()
                        .verify_password(password.as_bytes(), &h)
                        .is_ok()
                })
            })
            .await?;
            if !(valid && matches_user) {
                self.audit(None, "basic", "login", None, "rejected").await?;
                return Err(Error::unauthorized());
            }
            let member=sqlx::query_as::<_,Member>("SELECT id,issuer,subject,display_name,state,role FROM access_members WHERE issuer='urn:lvk:basic' AND subject='owner'").fetch_one(self.pool()).await?;
            return Principal::member(&member, revision, "basic", now() + WEB_LIFETIME);
        }
        Err(Error::unauthorized())
    }
    pub(crate) async fn csrf(&self, headers: &HeaderMap, p: &Principal) -> Result<(), Error> {
        let expected = if let Some(hash) = &p.session_hash {
            sqlx::query_scalar::<_, String>("SELECT csrf FROM access_sessions WHERE hash=?")
                .bind(hash)
                .fetch_one(self.pool())
                .await?
        } else {
            p.channel.to_string()
        };
        check_csrf(headers, &expected)
    }
    pub(crate) async fn management(
        &self,
        headers: &HeaderMap,
        write: bool,
    ) -> Result<Session, Error> {
        let session = self.session(headers).await?;
        if write {
            check_csrf(headers, &session.csrf)?;
        }
        if session.kind == "web" {
            let admin=sqlx::query_scalar::<_,bool>("SELECT EXISTS(SELECT 1 FROM access_members WHERE id=? AND state='active' AND role='admin')").bind(&session.user_id).fetch_one(self.pool()).await?;
            if !admin {
                return Err(Error::forbidden("Administrator required"));
            }
        }
        Ok(session)
    }
    pub(crate) async fn commit_config(
        &self,
        session: &Session,
        config: &Config,
        member: Option<&Member>,
        expected: i64,
    ) -> Result<(), Error> {
        self.allow_mode(config.mode())?;
        let encrypted = self.encrypt(config)?;
        let mut tx = self.pool().begin_with("BEGIN IMMEDIATE").await?;
        let revision: Option<i64> =
            sqlx::query_scalar("SELECT revision FROM access_settings WHERE id=1")
                .fetch_optional(&mut *tx)
                .await?;
        if revision.unwrap_or(0) != expected || (session.kind == "setup" && revision.is_some()) {
            return Err(Error::forbidden("Configuration changed; restart setup"));
        }
        let still_valid:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM access_sessions WHERE hash=? AND expires>? AND (kind IN ('setup','recovery') OR user_id IN (SELECT id FROM access_members WHERE state='active' AND role='admin')))").bind(&session.hash).bind(now()).fetch_one(&mut *tx).await?;
        if !still_valid {
            return Err(Error::unauthorized());
        }
        if let Some(member) = member {
            sqlx::query("UPDATE access_members SET state='active',role='admin' WHERE id=?")
                .bind(&member.id)
                .execute(&mut *tx)
                .await?;
        }
        if let Config::Basic { username, .. } = config {
            sqlx::query("INSERT INTO access_members(id,issuer,subject,display_name,state,role,created_at) VALUES(?,'urn:lvk:basic','owner',?,'active','admin',?) ON CONFLICT(issuer,subject) DO UPDATE SET display_name=excluded.display_name,state='active',role='admin'")
                .bind(Uuid::new_v4().to_string()).bind(username).bind(now()).execute(&mut *tx).await?;
        }
        let marker = self.0.key_path.with_extension("configured");
        if !marker.exists() {
            let mut options = OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            options.open(marker)?.sync_all()?;
        }
        sqlx::query("INSERT INTO access_settings(id,revision,encrypted) VALUES(1,?,?) ON CONFLICT(id) DO UPDATE SET revision=excluded.revision,encrypted=excluded.encrypted").bind(expected+1).bind(encrypted).execute(&mut *tx).await?;
        // One atomic switch invalidates old Basic/Web credentials and all setup grants.
        sqlx::query("DELETE FROM access_confirmations")
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM access_sessions")
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM access_keys")
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM access_transactions")
            .execute(&mut *tx)
            .await?;
        sqlx::query("INSERT INTO access_audit(at,actor,channel,action,target,outcome) VALUES(?,?,'settings','activate',?,'success')").bind(now()).bind(&session.user_id).bind(config.mode()).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(())
    }
}

#[derive(sqlx::FromRow)]
pub(crate) struct Session {
    pub hash: String,
    pub user_id: Option<String>,
    pub kind: String,
    pub csrf: String,
    pub expires: i64,
    pub revision: i64,
}
pub(crate) fn check_csrf(headers: &HeaderMap, expected: &str) -> Result<(), Error> {
    let supplied = headers
        .get("x-lvk-csrf")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    if !bool::from(supplied.as_bytes().ct_eq(expected.as_bytes())) {
        return Err(Error::forbidden("CSRF check failed"));
    }
    Ok(())
}
pub(crate) async fn password_hash(password: String) -> Result<String, Error> {
    if password.len() < 12 || password.len() > 1024 {
        return Err(Error::bad("Password must be 12 to 1024 bytes"));
    }
    tokio::task::spawn_blocking(move || {
        Argon2::default()
            .hash_password(
                password.as_bytes(),
                &SaltString::generate(&mut rand::rngs::OsRng),
            )
            .map(|h| h.to_string())
            .map_err(|_| Error::unavailable())
    })
    .await?
}

/// Explicit CLI only. Never call this from an HTTP endpoint or startup logging.
pub async fn issue_key(
    pool: &SqlitePool,
    key_path: &Path,
    recovery: bool,
) -> anyhow::Result<String> {
    let mut tx = pool.begin_with("BEGIN IMMEDIATE").await?;
    let configured: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM access_settings)")
        .fetch_one(&mut *tx)
        .await?;
    ensure!(
        recovery || !configured,
        "Already configured; use recovery-token"
    );
    ensure_key(key_path)?;
    let token = random();
    let kind = if recovery { "recovery" } else { "setup" };
    sqlx::query("DELETE FROM access_confirmations WHERE session_hash IN (SELECT hash FROM access_sessions WHERE kind!='web')").execute(&mut *tx).await?;
    sqlx::query("DELETE FROM access_sessions WHERE kind!='web'")
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM access_keys")
        .execute(&mut *tx)
        .await?;
    sqlx::query("INSERT INTO access_keys(hash,kind,expires) VALUES(?,?,?)")
        .bind(hash(&token))
        .bind(kind)
        .bind(now() + SETUP_LIFETIME)
        .execute(&mut *tx)
        .await?;
    sqlx::query(
        "INSERT INTO access_audit(at,channel,action,outcome) VALUES(?,'operator-cli',?,'success')",
    )
    .bind(now())
    .bind(format!("issue-{kind}-key"))
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(token)
}

/// File capability for existing local stdio MCP. Never inherited in child env.
pub(crate) fn internal_token(path: &Path) -> anyhow::Result<String> {
    if !path.exists() {
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        match options.open(path) {
            Ok(mut file) => {
                file.write_all(random().as_bytes())?;
                file.sync_all()?;
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(e.into()),
        }
    }
    let meta = std::fs::symlink_metadata(path)?;
    ensure!(
        meta.is_file() && !meta.file_type().is_symlink(),
        "Invalid internal token file"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        ensure!(
            meta.permissions().mode() & 0o077 == 0,
            "Internal token must be owner-only"
        );
    }
    let token = std::fs::read_to_string(path)?;
    ensure!(token.len() == 43, "Invalid internal token");
    Ok(token)
}
