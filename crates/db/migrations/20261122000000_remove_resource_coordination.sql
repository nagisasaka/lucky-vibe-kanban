-- Retire the prerelease resource coordinator and execution bridge. Historical
-- migration files remain intact; only their feature-specific data is removed.
DROP TRIGGER resource_workspace_change;
DROP TRIGGER resource_workspace_delete;
DROP TRIGGER resource_session_delete;
DROP TRIGGER resource_process_delete;

CREATE TEMP TABLE retired_mediation_workspaces (id BLOB PRIMARY KEY);
INSERT OR IGNORE INTO retired_mediation_workspaces SELECT id FROM workspaces WHERE json_extract(execution_owner, '$.kind') = 'resource_mediation';
-- Preparation could have stopped after creating the reserved direct folder but
-- before binding its owner. Match only the coordinator's exact instance path.
INSERT OR IGNORE INTO retired_mediation_workspaces
SELECT w.id FROM workspaces w JOIN resource_mediations m
ON w.container_ref = (
    SELECT substr(file, 1, length(file) - length('db.v2.sqlite'))
    FROM pragma_database_list WHERE name='main' AND file LIKE '%/db.v2.sqlite'
) || 'resource-mediation/' || substr(lower(hex(m.id)),1,8) || '-' ||
    substr(lower(hex(m.id)),9,4) || '-' || substr(lower(hex(m.id)),13,4) || '-' ||
    substr(lower(hex(m.id)),17,4) || '-' || substr(lower(hex(m.id)),21,12)
WHERE w.workspace_kind='direct_folder' AND w.execution_owner IS NULL;
DELETE FROM orchestration_runs WHERE product_kind = 'resource_mediation';
DELETE FROM execution_processes WHERE run_reason = 'resourcecommand';
DELETE FROM sessions WHERE workspace_id IN (SELECT id FROM retired_mediation_workspaces);
DELETE FROM workspaces WHERE id IN (SELECT id FROM retired_mediation_workspaces);
DROP TABLE retired_mediation_workspaces;

DROP TABLE bridge_settlements;
DROP TABLE bridge_commands;
DROP TABLE bridge_sources;
DROP TABLE bridge_runners;
DROP TABLE resource_mediations;
DROP TABLE resource_events;
DROP TABLE resource_holders;
DROP TABLE resource_operations;
DROP TABLE shared_resources;

ALTER TABLE orchestration_runs ADD COLUMN product_kind_new TEXT NOT NULL
    DEFAULT 'workflow' CHECK(product_kind_new IN ('workflow','arena','integration'));
UPDATE orchestration_runs SET product_kind_new=product_kind;
DROP INDEX idx_orchestration_runs_source_definition;
ALTER TABLE orchestration_runs DROP COLUMN product_kind;
ALTER TABLE orchestration_runs RENAME COLUMN product_kind_new TO product_kind;
CREATE INDEX idx_orchestration_runs_source_definition ON orchestration_runs(product_kind,source_definition_id);

DROP TRIGGER integration_script_admission;
ALTER TABLE execution_processes ADD COLUMN run_reason_new TEXT NOT NULL DEFAULT 'setupscript'
    CHECK(run_reason_new IN ('setupscript','cleanupscript','archivescript','devserver','integrationvalidation'));
UPDATE execution_processes SET run_reason_new=run_reason;
DROP INDEX idx_execution_processes_run_reason;
DROP INDEX idx_execution_processes_session_status_run_reason;
DROP INDEX idx_execution_processes_session_run_reason_created;
ALTER TABLE execution_processes DROP COLUMN run_reason;
ALTER TABLE execution_processes RENAME COLUMN run_reason_new TO run_reason;
CREATE INDEX idx_execution_processes_run_reason ON execution_processes(run_reason);
CREATE INDEX idx_execution_processes_session_status_run_reason ON execution_processes(session_id,status,run_reason);
CREATE INDEX idx_execution_processes_session_run_reason_created ON execution_processes(session_id,run_reason,created_at DESC);
CREATE TRIGGER integration_script_admission BEFORE INSERT ON execution_processes
WHEN EXISTS(SELECT 1 FROM integration_reservations r JOIN sessions s ON r.resource_key=lower(hex(s.workspace_id))
 WHERE r.resource_kind='workspace' AND s.id=NEW.session_id
 AND NOT EXISTS(SELECT 1 FROM integration_runs i WHERE i.id=r.run_id AND i.workspace_id=s.workspace_id
 AND i.session_id=s.id AND i.status='validating' AND NEW.run_reason='integrationvalidation'))
BEGIN SELECT RAISE(ABORT,'Workspace reserved by formal Integration'); END;
