use std::path::Path;

use git::{GitService, GitServiceError};

// Fixtures must choose an identity before creating even their initial commit.
pub trait FixtureIdentity {
    fn initialize_fixture(&self, path: &Path) -> Result<(), GitServiceError>;
}
impl FixtureIdentity for GitService {
    fn initialize_fixture(&self, path: &Path) -> Result<(), GitServiceError> {
        let repo = git2::Repository::init(path)?;
        let mut config = repo.config()?;
        config.set_str("user.name", "Test User")?;
        config.set_str("user.email", "test@example.com")?;
        self.initialize_repo_with_main_branch(path)
    }
}
