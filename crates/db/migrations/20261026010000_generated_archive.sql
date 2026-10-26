-- Provenance is recorded only at dedicated Issue creation; no historical sweep.
CREATE TABLE generated_issue_workspaces (
 workspace_id BLOB PRIMARY KEY REFERENCES workspaces(id) ON DELETE CASCADE,
 issue_id BLOB NOT NULL REFERENCES local_issues(id) ON DELETE CASCADE,
 project_id BLOB NOT NULL,
 owner_kind TEXT NOT NULL,
 owner_run_id TEXT,
 repository_id BLOB NOT NULL,
 eligibility_revision INTEGER NOT NULL DEFAULT 0,
 attempt_result TEXT
);
-- An operator archive/unarchive consumes automatic cleanup eligibility as well.
CREATE TRIGGER generated_archive_issue_manual AFTER UPDATE OF archived ON local_issues
BEGIN
 UPDATE generated_issue_workspaces SET eligibility_revision=eligibility_revision+1,attempt_result=COALESCE(attempt_result,'operator archive state changed') WHERE issue_id=NEW.id;
END;
CREATE TRIGGER generated_archive_workspace_manual AFTER UPDATE OF archived ON workspaces
BEGIN
 UPDATE generated_issue_workspaces SET eligibility_revision=eligibility_revision+1,attempt_result=COALESCE(attempt_result,'operator archive state changed') WHERE workspace_id=NEW.id;
END;
-- A changed link consumes eligibility, including unlink/relink ABA.
CREATE TRIGGER generated_archive_link_delete AFTER DELETE ON local_workspace_links
BEGIN UPDATE generated_issue_workspaces SET eligibility_revision=eligibility_revision+1,attempt_result=CASE WHEN workspace_id=OLD.workspace_id THEN COALESCE(attempt_result,'generated link changed') ELSE attempt_result END WHERE workspace_id=OLD.workspace_id OR issue_id=OLD.issue_id; END;
CREATE TRIGGER generated_archive_link_update AFTER UPDATE ON local_workspace_links
BEGIN UPDATE generated_issue_workspaces SET eligibility_revision=eligibility_revision+1,attempt_result=CASE WHEN workspace_id=OLD.workspace_id OR workspace_id=NEW.workspace_id THEN COALESCE(attempt_result,'generated link changed') ELSE attempt_result END WHERE workspace_id=OLD.workspace_id OR workspace_id=NEW.workspace_id OR issue_id=OLD.issue_id OR issue_id=NEW.issue_id; END;

-- Capture the first internal bind; a later replacement cannot adopt eligibility.
CREATE TRIGGER generated_archive_owner_bind AFTER UPDATE OF execution_owner ON workspaces
WHEN json_extract(NEW.execution_owner,'$.run_id') IS NOT NULL
BEGIN
 UPDATE generated_issue_workspaces SET owner_run_id=json_extract(NEW.execution_owner,'$.run_id')
 WHERE workspace_id=NEW.id AND owner_run_id IS NULL AND owner_kind=json_extract(NEW.execution_owner,'$.kind');
END;

CREATE TRIGGER generated_archive_link_insert AFTER INSERT ON local_workspace_links
BEGIN UPDATE generated_issue_workspaces SET eligibility_revision=eligibility_revision+1,attempt_result=CASE WHEN workspace_id=NEW.workspace_id THEN COALESCE(attempt_result,'generated link changed') ELSE attempt_result END WHERE workspace_id=NEW.workspace_id OR issue_id=NEW.issue_id; END;
-- Detect owner/path ABA even if the final values are restored before cleanup.
CREATE TRIGGER generated_archive_owner_changed AFTER UPDATE OF execution_owner,container_ref,branch ON workspaces
BEGIN UPDATE generated_issue_workspaces SET eligibility_revision=eligibility_revision+1 WHERE workspace_id=NEW.id; END;
