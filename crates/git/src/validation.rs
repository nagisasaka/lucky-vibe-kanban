pub fn is_valid_branch_prefix(prefix: &str) -> bool {
    if prefix.is_empty() {
        return true;
    }

    if prefix.contains('/') {
        return false;
    }

    git2::Branch::name_is_valid(&format!("{prefix}/x")).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn direct_folder_boundary_checks_ancestors_but_not_descendants() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = tmp.path().join("repo");
        git2::Repository::init(&repo).unwrap();
        let sub = repo.join("src");
        std::fs::create_dir(&sub).unwrap();
        let git = crate::GitService::new();
        assert!(!git.is_inside_repository(tmp.path()));
        assert!(git.is_inside_repository(&repo));
        assert!(git.is_inside_repository(&sub));
    }

    #[test]
    fn test_valid_prefixes() {
        assert!(is_valid_branch_prefix(""));
        assert!(is_valid_branch_prefix("vk"));
        assert!(is_valid_branch_prefix("feature"));
        assert!(is_valid_branch_prefix("hotfix-123"));
        assert!(is_valid_branch_prefix("foo.bar"));
        assert!(is_valid_branch_prefix("foo_bar"));
        assert!(is_valid_branch_prefix("FOO-Bar"));
    }

    #[test]
    fn test_invalid_prefixes() {
        assert!(!is_valid_branch_prefix("foo/bar"));
        assert!(!is_valid_branch_prefix("foo..bar"));
        assert!(!is_valid_branch_prefix("foo@{"));
        assert!(!is_valid_branch_prefix("foo.lock"));
        // Note: git2 allows trailing dots in some contexts, but we enforce stricter rules
        // for prefixes by checking the full branch name format
        assert!(!is_valid_branch_prefix("foo bar"));
        assert!(!is_valid_branch_prefix("foo?"));
        assert!(!is_valid_branch_prefix("foo*"));
        assert!(!is_valid_branch_prefix("foo~"));
        assert!(!is_valid_branch_prefix("foo^"));
        assert!(!is_valid_branch_prefix("foo:"));
        assert!(!is_valid_branch_prefix("foo["));
        assert!(!is_valid_branch_prefix("/foo"));
        assert!(!is_valid_branch_prefix("foo/"));
        assert!(!is_valid_branch_prefix(".foo"));
    }
}
