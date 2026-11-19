pub mod shared_resources;
mod workspace_manager;

pub use workspace_manager::{
    ManagedWorkspace, RepoWorkspaceInput, RepoWorktree, WorkspaceDeletionContext, WorkspaceError,
    WorkspaceManager, WorktreeContainer, delete_workspace_record_in_transaction,
};
