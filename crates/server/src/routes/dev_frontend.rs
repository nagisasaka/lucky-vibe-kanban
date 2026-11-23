//! Opt-in external dist for the shared developer runtime only.
//! Selection is one atomic current.json with the backend release. Neither this
//! reader nor the frontend switcher changes database or process state.
use std::{
    collections::BTreeMap,
    path::Path,
    sync::{Arc, Mutex, OnceLock},
};

use anyhow::{Context, Result, ensure};
use serde::Deserialize;
use sha2::{Digest, Sha256};

pub const PREFIX: &str = "__lvk_frontend/";

#[derive(Clone, Debug, Deserialize, PartialEq)]
struct Manifest {
    format: u32,
    revision: String,
    files: BTreeMap<String, String>,
}

struct Dist {
    manifest: Manifest,
    files: BTreeMap<String, Vec<u8>>,
}

pub struct File {
    pub name: String,
    pub bytes: Vec<u8>,
    pub revision: String,
    pub immutable: bool,
}

fn valid_revision(value: &str) -> bool {
    matches!(value.len(), 40 | 64)
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn safe_path(value: &str) -> bool {
    !value.contains('\\')
        && value
            .split('/')
            .all(|p| !p.is_empty() && p != "." && p != "..")
}

fn read_regular(root: &Path, name: &str) -> Result<Vec<u8>> {
    ensure!(safe_path(name), "Invalid frontend path");
    let path = root.join(name);
    ensure!(
        path.canonicalize()? == path && path.is_file(),
        "Frontend alias refused"
    );
    Ok(std::fs::read(path)?)
}

fn load(instance: &Path, manifest: Manifest) -> Result<Dist> {
    ensure!(
        manifest.format == 1 && valid_revision(&manifest.revision),
        "Invalid frontend manifest"
    );
    let root = instance.join("frontend-releases").join(&manifest.revision);
    ensure!(root.canonicalize()? == root, "Frontend root alias refused");
    let stored: Manifest = serde_json::from_slice(&read_regular(&root, "frontend.json")?)?;
    ensure!(
        stored == manifest,
        "Frontend selection differs from artifact"
    );
    let mut files = BTreeMap::new();
    for (name, digest) in &manifest.files {
        let bytes = read_regular(&root, name)?;
        ensure!(
            format!("{:x}", Sha256::digest(&bytes)) == *digest,
            "Frontend hash mismatch"
        );
        files.insert(name.clone(), bytes);
    }
    ensure!(files.contains_key("index.html"), "Missing frontend entry");
    Ok(Dist { manifest, files })
}

fn instance() -> Option<std::path::PathBuf> {
    if std::env::var("LVK_DEV_EXTERNAL_FRONTEND").as_deref() != Ok("1") {
        return None;
    }
    // A missing instance must fail closed, never silently serve embedded bytes.
    Some(
        std::env::var_os("LVK_INSTANCE_DIR")
            .map(Into::into)
            .unwrap_or_default(),
    )
}

fn selection(instance: &Path) -> Result<Manifest> {
    ensure!(
        instance.is_absolute() && instance.canonicalize()? == instance,
        "Invalid frontend instance"
    );
    let value: serde_json::Value =
        serde_json::from_slice(&read_regular(instance, "current.json")?)?;
    serde_json::from_value(
        value
            .get("frontend")
            .context("Missing frontend selection")?
            .clone(),
    )
    .map_err(Into::into)
}

type Cache = Option<(std::path::PathBuf, Arc<Dist>)>;
static CACHE: OnceLock<Mutex<Cache>> = OnceLock::new();

fn current(instance: &Path) -> Result<Arc<Dist>> {
    let manifest = selection(instance)?;
    let mut cache = CACHE.get_or_init(|| Mutex::new(None)).lock().unwrap();
    if let Some((root, dist)) = cache.as_ref()
        && root == instance
        && dist.manifest == manifest
    {
        return Ok(dist.clone());
    }
    let dist = Arc::new(load(instance, manifest)?);
    *cache = Some((instance.to_owned(), dist.clone()));
    Ok(dist)
}

pub fn revision() -> Option<String> {
    let instance = instance()?;
    current(&instance)
        .ok()
        .map(|dist| dist.manifest.revision.clone())
}

pub fn get(path: &str) -> Option<Result<Option<File>>> {
    let instance = instance()?;
    Some(serve(&instance, path))
}

fn serve(instance: &Path, path: &str) -> Result<Option<File>> {
    ensure!(safe_path(path), "Invalid frontend request path");
    let active = current(instance)?;
    let (dist, name, immutable) = if let Some(versioned) = path.strip_prefix(PREFIX) {
        let (revision, name) = versioned
            .split_once('/')
            .context("Missing frontend asset path")?;
        ensure!(
            valid_revision(revision) && safe_path(name),
            "Invalid versioned path"
        );
        let dist = if revision == active.manifest.revision {
            active
        } else {
            let root = instance.join("frontend-releases").join(revision);
            if !root.exists() {
                return Ok(None);
            }
            let manifest: Manifest =
                serde_json::from_slice(&read_regular(&root, "frontend.json")?)?;
            ensure!(
                manifest.format == 1 && manifest.revision == revision,
                "Frontend revision mismatch"
            );
            if name.starts_with('.') || name == "frontend.json" {
                return Ok(None);
            }
            let Some(digest) = manifest.files.get(name) else {
                return Ok(None);
            };
            // Retained generations were validated at installation. Verify only
            // the requested old chunk here, rather than reading an entire dist
            // (including source maps) for every lazy import from an old tab.
            let bytes = read_regular(&root, name)?;
            ensure!(
                format!("{:x}", Sha256::digest(&bytes)) == *digest,
                "Retained frontend hash mismatch"
            );
            return Ok(Some(File {
                name: name.to_owned(),
                bytes,
                revision: revision.to_owned(),
                immutable: true,
            }));
        };
        (dist, name, true)
    } else {
        (active, path, false)
    };
    let name = if dist.files.contains_key(name) {
        name
    } else if immutable || name.starts_with("assets/") {
        return Ok(None);
    } else {
        "index.html"
    };
    // Build metadata is never a public asset.
    if name.starts_with('.') || name == "frontend.json" {
        return Ok(None);
    }
    Ok(Some(File {
        name: name.to_owned(),
        bytes: dist.files[name].clone(),
        revision: dist.manifest.revision.clone(),
        immutable,
    }))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn fixture(instance: &Path, digit: char) -> Manifest {
        let revision = digit.to_string().repeat(40);
        let root = instance.join("frontend-releases").join(&revision);
        std::fs::create_dir_all(root.join("assets")).unwrap();
        let files: BTreeMap<String, String> = [
            ("index.html", format!("<html>{digit}</html>")),
            ("assets/chunk.js", format!("chunk {digit}")),
        ]
        .into_iter()
        .map(|(name, bytes)| {
            std::fs::write(root.join(name), &bytes).unwrap();
            (
                name.to_owned(),
                format!("{:x}", Sha256::digest(bytes.as_bytes())),
            )
        })
        .collect();
        let manifest = json!({"format":1,"revision":revision,"files":files});
        std::fs::write(root.join("frontend.json"), manifest.to_string()).unwrap();
        serde_json::from_value(manifest).unwrap()
    }
    fn select(instance: &Path, manifest: &Manifest) {
        let value = json!({"frontend":{"format":manifest.format,"revision":manifest.revision,"files":manifest.files}});
        std::fs::write(instance.join("next.json"), value.to_string()).unwrap();
        std::fs::rename(instance.join("next.json"), instance.join("current.json")).unwrap();
    }

    #[test]
    fn atomic_selection_retains_old_chunks_and_rolls_back() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().canonicalize().unwrap();
        let a = fixture(&root, 'a');
        let b = fixture(&root, 'b');
        select(&root, &a);
        assert_eq!(
            serve(&root, "index.html").unwrap().unwrap().revision,
            a.revision
        );
        select(&root, &b);
        assert_eq!(
            serve(&root, "workspaces/example")
                .unwrap()
                .unwrap()
                .revision,
            b.revision
        );
        let old = serve(&root, &format!("{PREFIX}{}/assets/chunk.js", a.revision))
            .unwrap()
            .unwrap();
        assert_eq!(old.bytes, b"chunk a");
        assert!(old.immutable);
        assert!(
            serve(&root, &format!("{PREFIX}{}/missing.js", b.revision))
                .unwrap()
                .is_none()
        );
        select(&root, &a);
        assert_eq!(
            serve(&root, "index.html").unwrap().unwrap().revision,
            a.revision
        );
    }

    #[tokio::test]
    async fn versioned_assets_preserve_existing_access_boundary() {
        use axum::{
            Router,
            body::Body,
            http::{Request, StatusCode},
            response::IntoResponse,
            routing::get,
        };
        use tower::ServiceExt;
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().canonicalize().unwrap();
        let a = fixture(&root, 'a');
        select(&root, &a);
        let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
        let access =
            server_access::test_support::access(pool, root.clone(), json!({"keys":[]})).await;
        let cookie = server_access::test_support::web_session(&access).await;
        let app = access.layer(
            Router::new()
                .route("/api/private", get(|| async { "private" }))
                .route("/mcp", get(|| async { "mcp" }))
                .route(
                    "/{*path}",
                    get(
                        move |axum::extract::Path(path): axum::extract::Path<String>| {
                            let root = root.clone();
                            async move {
                                match serve(&root, &path).unwrap() {
                                    Some(file) => file.bytes.into_response(),
                                    None => StatusCode::NOT_FOUND.into_response(),
                                }
                            }
                        },
                    ),
                ),
        );
        let path = format!("/{PREFIX}{}/assets/chunk.js", a.revision);
        // Existing policy makes shell assets public, while enforcing Host/Origin
        // on all routes and credentials on API and MCP. Keep that exact policy.
        for url in [&path, "/index.html"] {
            let response = app
                .clone()
                .oneshot(
                    Request::builder()
                        .uri(url)
                        .header("host", "203.0.113.7:8443")
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::OK);
            let response = app
                .clone()
                .oneshot(
                    Request::builder()
                        .uri(url)
                        .header("host", "203.0.113.7:8443")
                        .header("origin", "https://wrong.example")
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::FORBIDDEN);
        }
        for url in ["/api/private", "/mcp"] {
            let response = app
                .clone()
                .oneshot(
                    Request::builder()
                        .uri(url)
                        .header("host", "203.0.113.7:8443")
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert!(!response.status().is_success());
        }
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/private")
                    .header("host", "203.0.113.7:8443")
                    .header("cookie", cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[test]
    fn retained_chunk_checks_its_hash_without_reading_the_entire_old_dist() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().canonicalize().unwrap();
        let a = fixture(&root, 'a');
        let b = fixture(&root, 'b');
        select(&root, &b);
        let old = root.join("frontend-releases").join(&a.revision);
        std::fs::remove_file(old.join("index.html")).unwrap();
        let url = format!("{PREFIX}{}/assets/chunk.js", a.revision);
        assert_eq!(serve(&root, &url).unwrap().unwrap().bytes, b"chunk a");
        std::fs::write(old.join("assets/chunk.js"), "corrupt").unwrap();
        assert!(serve(&root, &url).is_err());
    }

    #[test]
    fn invalid_payload_and_traversal_fail_closed() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().canonicalize().unwrap();
        let a = fixture(&root, 'a');
        std::fs::write(
            root.join("frontend-releases")
                .join(&a.revision)
                .join("index.html"),
            "broken",
        )
        .unwrap();
        select(&root, &a);
        assert!(serve(&root, "index.html").is_err());
        for path in [
            "../current.json",
            "/index.html",
            "assets/../index.html",
            "assets\\chunk.js",
        ] {
            assert!(serve(&root, path).is_err());
        }
    }
}
