-- A Project owns its numbering lifetime, independently of its display key.
-- Historical receipts store Issue UUIDs, not allocated numbers. Deleted rows
-- cannot be reconstructed from them; this baseline covers surviving rows only.
ALTER TABLE projects ADD COLUMN issue_number_high_water INTEGER NOT NULL DEFAULT 0
    CHECK (typeof(issue_number_high_water) = 'integer' AND issue_number_high_water >= 0);

UPDATE projects SET issue_number_high_water = MAX(0, COALESCE(
    (SELECT MAX(issue_number) FROM local_issues WHERE project_id = projects.id), 0));

CREATE TRIGGER project_issue_number_never_decreases
BEFORE UPDATE OF issue_number_high_water ON projects
WHEN NEW.issue_number_high_water < OLD.issue_number_high_water
BEGIN SELECT RAISE(ABORT, 'Issue number high water cannot decrease'); END;

-- Preserve high water for explicit inserts as well (imports/test fixtures).
-- Normal creation reserves the number in the same BEGIN IMMEDIATE transaction.
CREATE TRIGGER issue_number_record_insert AFTER INSERT ON local_issues
BEGIN
    UPDATE projects SET issue_number_high_water = MAX(issue_number_high_water, NEW.issue_number)
    WHERE id = NEW.project_id;
END;
CREATE TRIGGER issue_number_record_update AFTER UPDATE OF issue_number, project_id ON local_issues
BEGIN
    UPDATE projects SET issue_number_high_water = MAX(issue_number_high_water, NEW.issue_number)
    WHERE id = NEW.project_id;
END;
