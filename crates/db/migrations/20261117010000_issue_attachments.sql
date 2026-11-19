-- Add Issue ownership without moving existing Workspace/Project references.
CREATE TABLE issue_attachments (
 issue_id BLOB NOT NULL REFERENCES local_issues(id) ON DELETE CASCADE,
 attachment_id BLOB NOT NULL REFERENCES attachments(id) ON DELETE CASCADE,
 created_at TEXT NOT NULL DEFAULT (datetime('now','subsec')),
 PRIMARY KEY(issue_id, attachment_id)
);
CREATE INDEX idx_issue_attachments_attachment ON issue_attachments(attachment_id);
