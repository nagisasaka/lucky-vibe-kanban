-- Named, editable specification versions. No automatic edit history is created.
ALTER TABLE local_issues ADD COLUMN current_version_id BLOB REFERENCES issue_versions(id) DEFERRABLE INITIALLY DEFERRED;
ALTER TABLE local_issues ADD COLUMN current_version_revision INTEGER NOT NULL DEFAULT 0;
CREATE TABLE issue_versions (
    id BLOB PRIMARY KEY NOT NULL,
    issue_id BLOB NOT NULL REFERENCES local_issues(id) ON DELETE CASCADE,
    name TEXT NOT NULL CHECK(length(trim(name)) BETWEEN 1 AND 200),
    description TEXT,
    revision INTEGER NOT NULL DEFAULT 0 CHECK(revision >= 0)
);
CREATE INDEX issue_versions_issue ON issue_versions(issue_id);
INSERT INTO issue_versions(id, issue_id, name, description)
    SELECT randomblob(16), id, 'Initial', description FROM local_issues;
UPDATE local_issues SET current_version_id=(SELECT id FROM issue_versions WHERE issue_id=local_issues.id);
-- All insertion paths, including generated integration Issues, get exactly one version.
CREATE TRIGGER issue_initial_version AFTER INSERT ON local_issues
BEGIN
    INSERT INTO issue_versions(id,issue_id,name,description) VALUES(randomblob(16),NEW.id,'Initial',NEW.description);
    UPDATE local_issues SET current_version_id=(SELECT id FROM issue_versions WHERE issue_id=NEW.id) WHERE id=NEW.id;
END;
CREATE TRIGGER issue_current_version_owner BEFORE UPDATE OF current_version_id ON local_issues
WHEN NEW.current_version_id IS NULL OR NOT EXISTS(SELECT 1 FROM issue_versions WHERE id=NEW.current_version_id AND issue_id=NEW.id)
BEGIN SELECT RAISE(ABORT,'Current version must belong to this Issue'); END;
