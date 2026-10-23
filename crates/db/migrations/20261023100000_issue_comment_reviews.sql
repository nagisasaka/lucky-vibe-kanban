-- Reviews remain ordinary comments; their provenance is not writable by the UI.
ALTER TABLE local_issue_comments ADD COLUMN reviewer_label TEXT;
ALTER TABLE local_issue_comments ADD COLUMN target_revision TEXT;
ALTER TABLE local_issue_comments ADD COLUMN recorded_principal TEXT;
CREATE INDEX idx_local_issue_comments_page ON local_issue_comments(issue_id, created_at, id);
