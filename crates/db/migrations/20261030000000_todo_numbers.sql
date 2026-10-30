-- UUID byte order breaks ties deterministically without changing existing attributes.
ALTER TABLE project_todos ADD COLUMN number INTEGER NOT NULL DEFAULT 0 CHECK(typeof(number) = 'integer' AND number >= 0);
UPDATE project_todos SET number = ranked.n FROM (
    SELECT id, row_number() OVER (PARTITION BY project_id ORDER BY created_at, id) AS n
    FROM project_todos
) AS ranked WHERE project_todos.id = ranked.id;
CREATE UNIQUE INDEX project_todos_number ON project_todos(project_id, number);
ALTER TABLE project_todos ADD COLUMN display_id TEXT GENERATED ALWAYS AS ('TODO-' || number) VIRTUAL;
CREATE TABLE project_todo_counters (
    project_id BLOB PRIMARY KEY NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    last_number INTEGER NOT NULL CHECK(typeof(last_number) = 'integer' AND last_number > 0)
);
INSERT INTO project_todo_counters SELECT project_id, MAX(number) FROM project_todos GROUP BY project_id;
CREATE TRIGGER project_todos_number_insert BEFORE INSERT ON project_todos
WHEN NEW.number <= 0 OR NEW.number != (SELECT last_number FROM project_todo_counters WHERE project_id=NEW.project_id)
 OR NOT EXISTS(SELECT 1 FROM project_todo_counters WHERE project_id=NEW.project_id)
BEGIN SELECT RAISE(ABORT, 'TODO number must be allocated by Project counter'); END;
CREATE TRIGGER project_todos_number_immutable BEFORE UPDATE OF number, project_id ON project_todos
WHEN NEW.number != OLD.number OR NEW.project_id != OLD.project_id
BEGIN SELECT RAISE(ABORT, 'TODO identity is immutable'); END;
CREATE TRIGGER project_todo_counter_monotonic BEFORE UPDATE ON project_todo_counters
WHEN NEW.project_id != OLD.project_id OR NEW.last_number <= OLD.last_number
BEGIN SELECT RAISE(ABORT, 'TODO counter must increase'); END;
