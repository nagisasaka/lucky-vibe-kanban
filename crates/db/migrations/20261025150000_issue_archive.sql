-- Issue archive is independent of workflow status and Workspace archive.
ALTER TABLE local_issues ADD COLUMN archived INTEGER NOT NULL DEFAULT 0 CHECK (archived IN (0, 1));
ALTER TABLE local_issues ADD COLUMN archive_link_revision INTEGER NOT NULL DEFAULT 0;
CREATE INDEX idx_local_issues_archive ON local_issues(project_id, archived);

-- Enforce the boundary for every writer, including workflow and MCP paths.
CREATE TRIGGER issue_archive_guard_link_insert BEFORE INSERT ON local_workspace_links
WHEN EXISTS (SELECT 1 FROM local_issues WHERE id=NEW.issue_id AND archived=1)
 AND NOT EXISTS (SELECT 1 FROM local_workspace_links WHERE workspace_id=NEW.workspace_id AND issue_id=NEW.issue_id AND project_id=NEW.project_id)
BEGIN SELECT RAISE(ABORT, 'Issue is archived; unarchive it before creating or linking a Workspace'); END;
CREATE TRIGGER issue_archive_guard_link_update BEFORE UPDATE OF issue_id,project_id ON local_workspace_links
WHEN (OLD.issue_id != NEW.issue_id OR OLD.project_id != NEW.project_id)
 AND EXISTS (SELECT 1 FROM local_issues WHERE id=NEW.issue_id AND archived=1)
BEGIN SELECT RAISE(ABORT, 'Issue is archived; unarchive it before creating or linking a Workspace'); END;

-- Detect even unlink/relink ABA between confirmation and execution.
CREATE TRIGGER issue_archive_link_insert AFTER INSERT ON local_workspace_links
BEGIN UPDATE local_issues SET archive_link_revision=archive_link_revision+1 WHERE id=NEW.issue_id; END;
CREATE TRIGGER issue_archive_link_delete AFTER DELETE ON local_workspace_links
BEGIN UPDATE local_issues SET archive_link_revision=archive_link_revision+1 WHERE id=OLD.issue_id; END;
CREATE TRIGGER issue_archive_link_update AFTER UPDATE ON local_workspace_links
BEGIN
 UPDATE local_issues SET archive_link_revision=archive_link_revision+1 WHERE id=OLD.issue_id;
 UPDATE local_issues SET archive_link_revision=archive_link_revision+1 WHERE id=NEW.issue_id AND id!=OLD.issue_id;
END;
