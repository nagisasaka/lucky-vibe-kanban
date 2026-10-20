-- Only the unchanged default Todo column is renamed; IDs and issue links survive.
UPDATE local_project_statuses SET name = 'Not Started'
WHERE name = 'Todo' AND color = '210 80% 52%' AND sort_order = 100 AND hidden = 0;

-- Ownership of an automatic transition is invalidated by any subsequent status
-- change. The API also clears it for an explicit same-column assignment.
ALTER TABLE local_issues ADD COLUMN agent_status_owner BLOB;
CREATE TRIGGER local_issue_status_owner_reset
AFTER UPDATE OF status_id ON local_issues
WHEN OLD.status_id IS NOT NEW.status_id
BEGIN
    UPDATE local_issues SET agent_status_owner = NULL WHERE id = NEW.id;
END;
