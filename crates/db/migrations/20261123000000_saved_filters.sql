CREATE TABLE saved_filters (
    id BLOB PRIMARY KEY NOT NULL,
    project_id BLOB NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    definition TEXT NOT NULL CHECK(json_valid(definition)),
    revision INTEGER NOT NULL DEFAULT 1 CHECK(revision > 0),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE INDEX saved_filters_project ON saved_filters(project_id, created_at, id);
-- Atomic replay receipts survive definition deletion; never depend on a live filter FK.
CREATE TABLE saved_filter_requests (
    project_id BLOB NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    actor TEXT NOT NULL,
    request_id BLOB NOT NULL,
    input TEXT NOT NULL,
    result TEXT,
    PRIMARY KEY(project_id, actor, request_id)
);
-- Opaque semantic-module payload. Definition revisions and evidence hashes are
-- owned by that module, not inferred from updated_at or the candidate IDs.
CREATE TABLE saved_filter_semantic_cache (
    filter_id BLOB PRIMARY KEY NOT NULL REFERENCES saved_filters(id) ON DELETE CASCADE,
    generation INTEGER NOT NULL,
    state_json TEXT NOT NULL CHECK(json_valid(state_json))
);
