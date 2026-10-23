SELECT coalesce(substr(CAST(c.message AS BLOB), ?, ?), zeroblob(0)) AS chunk
FROM local_issue_comments c JOIN local_issues i ON i.id = c.issue_id
WHERE c.id = ? AND c.issue_id = ? AND i.project_id = ?
