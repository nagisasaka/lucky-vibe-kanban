//! Build-time source identity. Never inspect the launch directory at runtime.
use std::{path::Path, process::Command};

#[derive(Debug, Default, PartialEq)]
pub struct Metadata {
    pub revision: String,
    pub kind: String,
    pub release_tag: String,
    pub dirty: String,
}

fn git(root: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

pub fn resolve(root: &Path, env: impl Fn(&str) -> Option<String>) -> Metadata {
    // An explicit (even empty) revision is authoritative. Never mix injected
    // artifact metadata with the builder's unrelated checkout.
    let injected = env("LVK_BUILD_REVISION");
    let (revision, kind, release_tag, dirty) = if let Some(revision) = injected {
        (
            revision,
            env("LVK_BUILD_KIND").unwrap_or_default(),
            env("LVK_BUILD_RELEASE_TAG").unwrap_or_default(),
            env("LVK_BUILD_DIRTY").unwrap_or_default(),
        )
    } else {
        // A source archive nested in another repository is not that repository.
        let top = git(root, &["rev-parse", "--show-toplevel"]);
        if top
            .as_deref()
            .and_then(|p| Path::new(p).canonicalize().ok())
            != root.canonicalize().ok()
            || top.is_none()
        {
            return Metadata::default();
        }
        (
            git(root, &["rev-parse", "HEAD"]).unwrap_or_default(),
            "development".into(),
            String::new(),
            git(root, &["status", "--porcelain", "--untracked-files=normal"])
                .map(|status| (!status.is_empty()).to_string())
                .unwrap_or_default(),
        )
    };
    if !matches!(revision.len(), 40 | 64) || !revision.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Metadata::default();
    }
    let release_tag = if release_tag.starts_with('v')
        && release_tag
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b".-".contains(&b))
    {
        release_tag
    } else {
        String::new()
    };
    let kind = match kind.as_str() {
        "development" => kind,
        "release" if !release_tag.is_empty() => kind,
        _ => String::new(),
    };
    Metadata {
        revision: revision.to_ascii_lowercase(),
        release_tag: if kind == "release" {
            release_tag
        } else {
            String::new()
        },
        kind,
        dirty: if matches!(dirty.as_str(), "true" | "false") {
            dirty
        } else {
            String::new()
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const SHA: &str = "0123456789abcdef0123456789abcdef01234567";
    fn injected(values: &[(&str, &str)]) -> Metadata {
        resolve(Path::new("/not-a-checkout"), |key| {
            values
                .iter()
                .find(|(k, _)| *k == key)
                .map(|(_, v)| v.to_string())
        })
    }
    #[test]
    fn release_and_dirty_are_explicit() {
        let value = injected(&[
            ("LVK_BUILD_REVISION", SHA),
            ("LVK_BUILD_KIND", "release"),
            ("LVK_BUILD_RELEASE_TAG", "v0.1.50-lvk.1-server.9"),
            ("LVK_BUILD_DIRTY", "false"),
        ]);
        assert_eq!(value.revision, SHA);
        assert_eq!(value.kind, "release");
        assert_eq!(value.release_tag, "v0.1.50-lvk.1-server.9");
        assert_eq!(value.dirty, "false");
    }
    #[test]
    fn unknown_fields_are_not_guessed() {
        let value = injected(&[("LVK_BUILD_REVISION", SHA)]);
        assert_eq!(value.kind, "");
        assert_eq!(value.dirty, "");
        assert_eq!(value.release_tag, "");
        assert_eq!(injected(&[("LVK_BUILD_REVISION", "")]), Metadata::default());
        assert_eq!(
            injected(&[("LVK_BUILD_REVISION", "0.1.44")]),
            Metadata::default()
        );
        assert_eq!(
            resolve(Path::new("/not-a-checkout"), |_| None),
            Metadata::default()
        );
    }
    #[test]
    fn invalid_release_and_dirty_are_unknown() {
        let value = injected(&[
            ("LVK_BUILD_REVISION", SHA),
            ("LVK_BUILD_KIND", "release"),
            ("LVK_BUILD_RELEASE_TAG", "bad\nvalue"),
            ("LVK_BUILD_DIRTY", "maybe"),
        ]);
        assert_eq!(value.kind, "");
        assert_eq!(value.release_tag, "");
        assert_eq!(value.dirty, "");
    }
    #[test]
    fn dirty_development_is_not_a_release() {
        let value = injected(&[
            ("LVK_BUILD_REVISION", SHA),
            ("LVK_BUILD_KIND", "development"),
            ("LVK_BUILD_RELEASE_TAG", "v1.2.3"),
            ("LVK_BUILD_DIRTY", "true"),
        ]);
        assert_eq!(value.kind, "development");
        assert_eq!(value.dirty, "true");
        assert_eq!(value.release_tag, "");
    }
    #[test]
    fn local_git_tracks_commits_and_untracked_dirty_state() {
        let root = std::env::temp_dir().join(format!("lvk-build-identity-{}", std::process::id()));
        std::fs::create_dir(&root).unwrap();
        struct Cleanup(std::path::PathBuf);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
        let _cleanup = Cleanup(root.clone());
        let run = |args: &[&str]| git(&root, args).expect("fixture git command");
        run(&["init", "--quiet"]);
        run(&[
            "-c",
            "user.name=Test",
            "-c",
            "user.email=test@example.invalid",
            "commit",
            "--allow-empty",
            "-m",
            "fixture",
        ]);
        let clean = resolve(&root, |_| None);
        assert_eq!(clean.revision, run(&["rev-parse", "HEAD"]));
        assert_eq!(clean.kind, "development");
        assert_eq!(clean.dirty, "false");
        std::fs::write(root.join("untracked"), "dirty").unwrap();
        assert_eq!(resolve(&root, |_| None).dirty, "true");
        std::fs::remove_file(root.join("untracked")).unwrap();
        assert_eq!(resolve(&root, |_| None).dirty, "false");
        // Empty injection must not fall back even in a valid checkout.
        assert_eq!(
            resolve(&root, |key| (key == "LVK_BUILD_REVISION").then(String::new)),
            Metadata::default()
        );
    }
}
