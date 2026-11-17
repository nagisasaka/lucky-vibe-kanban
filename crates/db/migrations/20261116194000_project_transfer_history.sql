-- Portable display projections have no runtime/session/process foreign keys.
CREATE TABLE project_transfer_history (
    project_id BLOB NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    issue_id BLOB NOT NULL REFERENCES local_issues(id) ON DELETE CASCADE,
    run_id BLOB NOT NULL,
    payload TEXT NOT NULL,
    PRIMARY KEY (project_id, run_id)
);
