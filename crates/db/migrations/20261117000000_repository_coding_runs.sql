-- Run identity and completion history are read after recovery artifacts expire.
-- No import, rewrite or deletion of legacy filesystem/DB records.
CREATE TABLE repository_coding_runs (
    repository_id BLOB NOT NULL,
    id BLOB NOT NULL,
    payload TEXT NOT NULL CHECK(json_valid(payload)),
    no_source_change INTEGER NOT NULL DEFAULT 0 CHECK(no_source_change IN (0,1)),
    completion_error TEXT,
    PRIMARY KEY(repository_id, id)
);
