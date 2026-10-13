-- Receipts for remote requests, not another task/job queue. Work remains owned
-- by existing Issues, Workspaces, AgentRuns and Integration runs.
CREATE TABLE remote_mcp_operations (
    id BLOB PRIMARY KEY NOT NULL,
    principal TEXT NOT NULL,
    request_id BLOB NOT NULL,
    project_id BLOB NOT NULL REFERENCES projects(id),
    operation TEXT NOT NULL,
    input_hash TEXT NOT NULL,
    targets TEXT NOT NULL,
    process_instance BLOB NOT NULL,
    status TEXT NOT NULL DEFAULT 'accepted'
        CHECK (status IN ('accepted','running','succeeded','failed','uncertain')),
    result TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    UNIQUE(principal, request_id)
);
CREATE INDEX remote_mcp_operations_project ON remote_mcp_operations(project_id, created_at, id);
