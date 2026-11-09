-- Freeze existing layouts; never move worktrees or rename shared repository storage.
ALTER TABLE workspace_repos ADD COLUMN worktree_name TEXT;
UPDATE workspace_repos SET worktree_name = (SELECT name FROM repos WHERE repos.id = workspace_repos.repo_id);
