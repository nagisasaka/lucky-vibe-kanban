mod build_metadata;

use std::{fs, path::PathBuf};

fn main() {
    // Load .env from the workspace root
    let workspace_root = PathBuf::from(
        std::env::var_os("CARGO_MANIFEST_DIR")
            .expect("Cargo must supply the build script manifest root"),
    )
    .join("../..");
    // Cargo checksum freshness does not cover build-script input mtimes.
    // Re-evaluate this small script even with explicit build identity, so .env
    // edits/removal and checkout switches cannot reuse another root's output.
    // This sentinel is never created; Cargo owns the normal build-dir lock.
    println!(
        "cargo:rerun-if-changed={}/lvk-build-identity-refresh",
        std::env::var("OUT_DIR").unwrap()
    );
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=build_metadata.rs");
    for name in [
        "LVK_BUILD_REVISION",
        "LVK_BUILD_KIND",
        "LVK_BUILD_RELEASE_TAG",
        "LVK_BUILD_DIRTY",
    ] {
        println!("cargo:rerun-if-env-changed={name}");
    }
    let metadata = build_metadata::resolve(&workspace_root, |key| std::env::var(key).ok());
    for (name, value) in [
        ("REVISION", metadata.revision),
        ("KIND", metadata.kind),
        ("RELEASE_TAG", metadata.release_tag),
        ("DIRTY", metadata.dirty),
    ] {
        println!("cargo:rustc-env=LVK_BUILD_{name}={value}");
    }
    let env_file = workspace_root.join(".env");
    dotenv::from_path(&env_file).ok();

    // Re-run build script when these env vars or .env file change
    println!("cargo:rerun-if-env-changed=POSTHOG_API_KEY");
    println!("cargo:rerun-if-env-changed=POSTHOG_API_ENDPOINT");
    println!("cargo:rerun-if-env-changed=SENTRY_DSN");
    if env_file.exists() {
        println!("cargo:rerun-if-changed={}", env_file.display());
    }

    if let Ok(api_key) = std::env::var("POSTHOG_API_KEY") {
        println!("cargo:rustc-env=POSTHOG_API_KEY={}", api_key);
    }
    if let Ok(api_endpoint) = std::env::var("POSTHOG_API_ENDPOINT") {
        println!("cargo:rustc-env=POSTHOG_API_ENDPOINT={}", api_endpoint);
    }

    // Create packages/local-web/dist directory if it doesn't exist
    let dist_path = workspace_root.join("packages/local-web/dist");
    if !dist_path.exists() {
        println!("cargo:warning=Creating dummy packages/local-web/dist directory for compilation");
        fs::create_dir_all(&dist_path).unwrap();

        // Create a dummy index.html
        let dummy_html = r#"<!DOCTYPE html>
<html><head><title>Build web app first</title></head>
<body><h1>Please build @vibe/local-web first</h1></body></html>"#;

        fs::write(dist_path.join("index.html"), dummy_html).unwrap();
    }
}
