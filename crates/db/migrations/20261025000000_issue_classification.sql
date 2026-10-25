-- Normalize old clients and direct auto-generation writes without rebuilding the
-- Issue table (and its many foreign keys).
UPDATE local_issues SET priority = 'medium' WHERE priority IS NULL;
CREATE TRIGGER local_issue_priority_insert AFTER INSERT ON local_issues
WHEN NEW.priority IS NULL BEGIN
    UPDATE local_issues SET priority = 'medium' WHERE id = NEW.id;
END;
CREATE TRIGGER local_issue_priority_update AFTER UPDATE OF priority ON local_issues
WHEN NEW.priority IS NULL BEGIN
    UPDATE local_issues SET priority = 'medium' WHERE id = NEW.id;
END;

-- One-time seed. Reads never recreate user-deleted normal tags.
WITH defaults(name, color) AS (VALUES
    ('system', '0 0% 50%'), ('survey', '205 100% 40%'),
    ('bug', '355 65% 53%'), ('feature', '124 82% 30%')
)
INSERT INTO local_tags(id, project_id, name, color)
SELECT randomblob(16), p.id, d.name, d.color FROM projects p CROSS JOIN defaults d
WHERE NOT EXISTS (SELECT 1 FROM local_tags t WHERE t.project_id=p.id AND t.name=d.name);

-- Existing duplicate names are legal. Preserve every definition and select the
-- lowest system ID as the canonical relation target.

-- Historical execution ownership survives release to interactive usage.
INSERT OR IGNORE INTO local_issue_tags(id, issue_id, tag_id)
SELECT randomblob(16), i.id, t.id
FROM local_issues i JOIN local_tags t ON t.id=(
    SELECT id FROM local_tags WHERE project_id=i.project_id AND name='system' ORDER BY id LIMIT 1
)
WHERE EXISTS (
    SELECT 1 FROM local_workspace_links l JOIN workspaces w ON w.id=l.workspace_id
    WHERE l.issue_id=i.id AND l.project_id=i.project_id
      AND json_valid(w.execution_owner)
      AND json_extract(w.execution_owner, '$.kind') IN ('openwiki_bootstrap','openwiki_sync','integration')
) OR EXISTS (
    SELECT 1 FROM integration_runs r
    WHERE r.project_id=i.project_id AND json_extract(r.payload, '$.card_id')=lower(
      substr(hex(i.id),1,8)||'-'||substr(hex(i.id),9,4)||'-'||substr(hex(i.id),13,4)||'-'||substr(hex(i.id),17,4)||'-'||substr(hex(i.id),21,12))
);
