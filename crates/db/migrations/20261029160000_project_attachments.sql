-- Keep existing IDs and junction references while allowing equal bytes with distinct names.
COMMIT;
PRAGMA foreign_keys = OFF;
BEGIN TRANSACTION;
CREATE TABLE attachments_new (
 id BLOB PRIMARY KEY, file_path TEXT NOT NULL, original_name TEXT NOT NULL,
 mime_type TEXT, size_bytes INTEGER, hash TEXT NOT NULL,
 created_at TEXT NOT NULL DEFAULT (datetime('now','subsec')),
 updated_at TEXT NOT NULL DEFAULT (datetime('now','subsec'))
);
INSERT INTO attachments_new SELECT * FROM attachments;
DROP TABLE attachments;
ALTER TABLE attachments_new RENAME TO attachments;
CREATE INDEX idx_attachments_hash ON attachments(hash);
CREATE UNIQUE INDEX idx_attachments_identity ON attachments(hash, original_name, COALESCE(mime_type, ''));
CREATE TABLE project_attachments (
 project_id BLOB NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
 attachment_id BLOB NOT NULL REFERENCES attachments(id) ON DELETE CASCADE,
 created_at TEXT NOT NULL DEFAULT (datetime('now','subsec')),
 PRIMARY KEY(project_id, attachment_id)
);
COMMIT;
PRAGMA foreign_keys = ON;
BEGIN TRANSACTION;
