-- Only final semantic records move. No foreign keys: historical events can
-- outlive their Run/Workspace metadata, and must not be silently discarded.
CREATE TABLE repository_change_manifests (
    repository_id BLOB NOT NULL,
    id BLOB NOT NULL,
    workspace_id BLOB NOT NULL,
    payload TEXT NOT NULL CHECK(json_valid(payload)),
    PRIMARY KEY(repository_id, id)
);
CREATE INDEX idx_repository_change_manifests_workspace
    ON repository_change_manifests(repository_id, workspace_id, id);
CREATE TABLE repository_manifest_imports (
    repository_id BLOB PRIMARY KEY NOT NULL,
    imported_count INTEGER NOT NULL CHECK(imported_count >= 0)
);
