-- Fetch only enough UTF-8 bytes for a 256-scalar preview, never the full body.
SELECT id, author_id, parent_id, reviewer_label, recorded_principal,
       target_revision, created_at, updated_at,
       length(CAST(message AS BLOB)) AS total_bytes,
       coalesce(substr(CAST(message AS BLOB), 1, 1024), zeroblob(0)) AS preview_bytes
FROM local_issue_comments
WHERE issue_id = ? AND (? IS NULL OR (created_at, id) > (?, ?))
ORDER BY created_at, id LIMIT ?
