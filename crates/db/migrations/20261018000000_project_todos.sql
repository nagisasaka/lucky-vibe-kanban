CREATE TABLE project_todos (
    id BLOB PRIMARY KEY NOT NULL,
    project_id BLOB NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    body TEXT NOT NULL CHECK (length(trim(body)) > 0),
    category TEXT NOT NULL DEFAULT '',
    completed INTEGER NOT NULL DEFAULT 0 CHECK (completed IN (0, 1)),
    issue_id BLOB REFERENCES local_issues(id) ON DELETE SET NULL,
    revision INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
CREATE INDEX project_todos_listing ON project_todos(project_id, created_at DESC, id);
CREATE TRIGGER project_todos_issue_insert BEFORE INSERT ON project_todos
WHEN NEW.issue_id IS NOT NULL AND NOT EXISTS (
    SELECT 1 FROM local_issues WHERE id = NEW.issue_id AND project_id = NEW.project_id
) BEGIN SELECT RAISE(ABORT, 'TODO card must belong to its Project'); END;
CREATE TRIGGER project_todos_issue_update BEFORE UPDATE OF issue_id, project_id ON project_todos
WHEN NEW.issue_id IS NOT NULL AND NOT EXISTS (
    SELECT 1 FROM local_issues WHERE id = NEW.issue_id AND project_id = NEW.project_id
) BEGIN SELECT RAISE(ABORT, 'TODO card must belong to its Project'); END;
