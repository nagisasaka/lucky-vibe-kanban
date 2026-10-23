ALTER TABLE project_todos ADD COLUMN creation_source TEXT NOT NULL DEFAULT 'unknown'
    CHECK (creation_source IN ('ui', 'mcp', 'unknown'));

CREATE TRIGGER project_todos_creation_source_immutable
BEFORE UPDATE OF creation_source ON project_todos
WHEN NEW.creation_source IS NOT OLD.creation_source
BEGIN
    SELECT RAISE(ABORT, 'TODO creation source is immutable');
END;
