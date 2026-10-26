-- Immutable compatibility variant for version 20261025120000. The original
-- migration remains unchanged for databases that already applied its checksum.
ALTER TABLE projects ADD COLUMN key TEXT NOT NULL DEFAULT 'LOCAL'
    CHECK (length(key) BETWEEN 2 AND 10
        AND key GLOB '[A-Z]*' AND key NOT GLOB '*[^A-Z0-9]*');

-- Preserve the single-Project prefix and every historical Issue identifier.
-- UUID order makes assignment independent of Project names and query order.
-- Above 99999 Projects the column CHECK fails and SQLx rolls back this entire
-- migration, including the new column; never truncate or reuse a generated key.
WITH ranked AS (
    SELECT rowid AS project_rowid,
           row_number() OVER (ORDER BY hex(id), rowid) AS ordinal
    FROM projects
)
UPDATE projects SET key = (
    SELECT CASE WHEN ordinal = 1 THEN 'LOCAL'
                ELSE 'LOCAL' || ordinal END
    FROM ranked WHERE project_rowid = projects.rowid
);
CREATE UNIQUE INDEX idx_projects_key ON projects(key);
