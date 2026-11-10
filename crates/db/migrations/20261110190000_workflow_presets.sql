-- Fixed preset parameters and receipts; existing graph snapshots/history stay intact.
CREATE TABLE workflow_preset_defaults (
    project_id BLOB NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    preset TEXT NOT NULL,
    additions TEXT NOT NULL DEFAULT '',
    revision INTEGER NOT NULL DEFAULT 1,
    PRIMARY KEY(project_id, preset)
);
CREATE TABLE workflow_preset_launches (
    request_id BLOB PRIMARY KEY NOT NULL,
    project_id BLOB NOT NULL,
    issue_id BLOB NOT NULL,
    request_json TEXT NOT NULL,
    workflow_id BLOB NOT NULL,
    run_id BLOB
);
ALTER TABLE node_executions ADD COLUMN source_snapshot TEXT;
ALTER TABLE node_executions ADD COLUMN source_base TEXT;
