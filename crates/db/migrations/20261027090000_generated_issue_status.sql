-- Enrol future registrations and still-active owners, never released history.
-- This migration does not update any Issue; live transitions apply later.
CREATE TABLE generated_issue_status (
 workspace_id BLOB PRIMARY KEY REFERENCES generated_issue_workspaces(workspace_id) ON DELETE CASCADE,
 projected_status TEXT CHECK(projected_status IN ('in progress','in review','done'))
);

-- Read current product identity rather than the status carried by a notification.
-- Agent success alone never proves publication. Bootstrap's successful End node
-- follows publish_repository; Sync retains its publication result on the owner.
CREATE VIEW generated_issue_status_projection AS
SELECT g.workspace_id, g.issue_id, g.project_id,
 CASE
  WHEN g.owner_kind='integration' THEN
   CASE WHEN json_extract(i.payload,'$.published')=1 THEN 'done'
        WHEN i.status IN ('blocked','failed','cancelled','recovery_required','succeeded') THEN 'in review'
        WHEN i.status='integrating'
         AND a.status IN ('awaiting_input','awaiting_approval','failed','cancelled','crashed','audit_failed') THEN 'in review'
        ELSE 'in progress' END
  WHEN g.owner_kind='openwiki_sync' AND a.id IS NOT NULL
       AND json_extract(w.execution_owner,'$.result.status')='succeeded' THEN 'done'
  WHEN json_extract(w.execution_owner,'$.result.status') IN ('failed','cancelled') THEN 'in review'
  WHEN g.owner_kind='openwiki_bootstrap' THEN
   CASE WHEN f.status='succeeded' THEN 'done'
        WHEN f.status IN ('awaiting_human','awaiting_arena','failed','canceled') THEN 'in review'
        WHEN EXISTS(SELECT 1 FROM node_executions n JOIN agent_runs child ON child.id=n.agent_run_id
                    WHERE n.run_id=f.id AND n.status IN ('running','awaiting_human')
                      AND child.workspace_id=w.id
                      AND child.status IN ('awaiting_input','awaiting_approval')) THEN 'in review'
        ELSE 'in progress' END
  WHEN a.status IN ('awaiting_input','awaiting_approval','failed','cancelled','crashed','audit_failed') THEN 'in review'
  ELSE 'in progress'
 END AS desired_status
FROM generated_issue_workspaces g
JOIN workspaces w ON w.id=g.workspace_id
JOIN local_workspace_links l ON l.workspace_id=w.id AND l.issue_id=g.issue_id AND l.project_id=g.project_id
JOIN local_issues issue ON issue.id=g.issue_id AND issue.project_id=g.project_id
JOIN local_project_statuses current_status ON current_status.id=issue.status_id
LEFT JOIN integration_runs i ON lower(hex(i.id))=replace(g.owner_run_id,'-','')
 AND i.workspace_id=w.id AND i.repository_id=g.repository_id AND i.project_id=g.project_id
 AND replace(json_extract(i.payload,'$.card_id'),'-','')=lower(hex(g.issue_id))
LEFT JOIN workflow_runs f ON lower(hex(f.id))=replace(g.owner_run_id,'-','')
 AND f.workspace_id=w.id AND f.repository_id=g.repository_id AND f.trigger_source='openwiki_bootstrap'
LEFT JOIN agent_runs a ON a.workspace_id=w.id AND
 ((g.owner_kind='openwiki_sync' AND lower(hex(a.id))=replace(g.owner_run_id,'-',''))
  OR (g.owner_kind='integration' AND a.id=i.agent_run_id))
WHERE g.owner_kind=json_extract(w.execution_owner,'$.kind')
 AND replace(json_extract(w.execution_owner,'$.repository_id'),'-','')=lower(hex(g.repository_id))
 AND json_extract(w.execution_owner,'$.run_id') IS g.owner_run_id
 AND g.owner_kind IN ('integration','openwiki_bootstrap','openwiki_sync')
 AND (g.owner_kind!='integration' OR i.id IS NOT NULL)
 -- An explicit whole-Issue cancellation belongs to the operator.
 AND lower(current_status.name) NOT IN ('cancelled','canceled');

-- Carry active reservations across an upgrade without repairing their Cards.
-- An already published owner gets a receipt only: replay must preserve any
-- manual reopening. Released history is deliberately not enrolled.
INSERT INTO generated_issue_status(workspace_id,projected_status)
SELECT p.workspace_id,CASE WHEN p.desired_status='done' THEN 'done' ELSE NULL END
FROM generated_issue_status_projection p
JOIN generated_issue_workspaces g ON g.workspace_id=p.workspace_id
JOIN workspaces w ON w.id=g.workspace_id
WHERE w.usage='execution_only' AND (
 (g.owner_kind='integration' AND EXISTS(
  SELECT 1 FROM integration_runs i WHERE lower(hex(i.id))=replace(g.owner_run_id,'-','')
   AND i.status NOT IN ('failed','cancelled')
   AND (i.status!='succeeded' OR json_extract(i.payload,'$.published')=1)))
 OR (g.owner_kind='openwiki_bootstrap' AND json_extract(w.execution_owner,'$.result') IS NULL
  AND NOT EXISTS(SELECT 1 FROM workflow_runs f WHERE lower(hex(f.id))=replace(g.owner_run_id,'-','')
   AND f.status IN ('failed','canceled')))
 -- Agent terminal is not Sync completion: the host may still publish/clean up.
 OR (g.owner_kind='openwiki_sync' AND json_extract(w.execution_owner,'$.result') IS NULL)
);

CREATE TRIGGER generated_issue_status_apply AFTER UPDATE OF projected_status ON generated_issue_status
WHEN NEW.projected_status IS NOT NULL AND OLD.projected_status IS NOT NEW.projected_status
BEGIN
 UPDATE local_issues SET
  status_id=(SELECT s.id FROM local_project_statuses s WHERE s.project_id=local_issues.project_id AND s.hidden=0 AND lower(s.name)=NEW.projected_status ORDER BY s.sort_order,s.id LIMIT 1),
  completed_at=CASE WHEN NEW.projected_status='done' THEN datetime('now','subsec') ELSE NULL END,
  updated_at=datetime('now','subsec')
 WHERE id=(SELECT issue_id FROM generated_issue_workspaces WHERE workspace_id=NEW.workspace_id);
END;

CREATE TRIGGER generated_issue_status_register AFTER INSERT ON generated_issue_workspaces
BEGIN
 INSERT INTO generated_issue_status(workspace_id) VALUES(NEW.workspace_id);
 UPDATE generated_issue_status SET projected_status=(SELECT desired_status FROM generated_issue_status_projection p WHERE p.workspace_id=generated_issue_status.workspace_id)
 WHERE workspace_id=NEW.workspace_id AND projected_status IS NOT 'done'
 AND EXISTS(SELECT 1 FROM generated_issue_status_projection p JOIN local_project_statuses s
            ON s.project_id=p.project_id AND s.hidden=0 AND lower(s.name)=p.desired_status
            WHERE p.workspace_id=generated_issue_status.workspace_id AND p.desired_status IS NOT generated_issue_status.projected_status);
END;

CREATE TRIGGER generated_issue_status_owner AFTER UPDATE OF execution_owner ON workspaces
BEGIN
 UPDATE generated_issue_status SET projected_status=(SELECT desired_status FROM generated_issue_status_projection p WHERE p.workspace_id=generated_issue_status.workspace_id)
 WHERE workspace_id=NEW.id AND projected_status IS NOT 'done'
 AND EXISTS(SELECT 1 FROM generated_issue_status_projection p JOIN local_project_statuses s
            ON s.project_id=p.project_id AND s.hidden=0 AND lower(s.name)=p.desired_status
            WHERE p.workspace_id=generated_issue_status.workspace_id AND p.desired_status IS NOT generated_issue_status.projected_status);
END;

CREATE TRIGGER generated_issue_status_bind AFTER UPDATE OF owner_run_id ON generated_issue_workspaces
BEGIN
 UPDATE generated_issue_status SET projected_status=(SELECT desired_status FROM generated_issue_status_projection p WHERE p.workspace_id=generated_issue_status.workspace_id)
 WHERE workspace_id=NEW.workspace_id AND projected_status IS NOT 'done'
 AND EXISTS(SELECT 1 FROM generated_issue_status_projection p JOIN local_project_statuses s
            ON s.project_id=p.project_id AND s.hidden=0 AND lower(s.name)=p.desired_status
            WHERE p.workspace_id=generated_issue_status.workspace_id AND p.desired_status IS NOT generated_issue_status.projected_status);
END;

CREATE TRIGGER generated_issue_status_integration_insert AFTER INSERT ON integration_runs
BEGIN
 UPDATE generated_issue_status SET projected_status=(SELECT desired_status FROM generated_issue_status_projection p WHERE p.workspace_id=generated_issue_status.workspace_id)
 WHERE workspace_id=NEW.workspace_id AND projected_status IS NOT 'done'
 AND EXISTS(SELECT 1 FROM generated_issue_status_projection p JOIN local_project_statuses s
            ON s.project_id=p.project_id AND s.hidden=0 AND lower(s.name)=p.desired_status
            WHERE p.workspace_id=generated_issue_status.workspace_id AND p.desired_status IS NOT generated_issue_status.projected_status);
END;

CREATE TRIGGER generated_issue_status_integration AFTER UPDATE OF status,payload,agent_run_id ON integration_runs
BEGIN
 UPDATE generated_issue_status SET projected_status=(SELECT desired_status FROM generated_issue_status_projection p WHERE p.workspace_id=generated_issue_status.workspace_id)
 WHERE workspace_id=NEW.workspace_id AND projected_status IS NOT 'done'
 AND EXISTS(SELECT 1 FROM generated_issue_status_projection p JOIN local_project_statuses s
            ON s.project_id=p.project_id AND s.hidden=0 AND lower(s.name)=p.desired_status
            WHERE p.workspace_id=generated_issue_status.workspace_id AND p.desired_status IS NOT generated_issue_status.projected_status);
END;

CREATE TRIGGER generated_issue_status_workflow_insert AFTER INSERT ON workflow_runs
BEGIN
 UPDATE generated_issue_status SET projected_status=(SELECT desired_status FROM generated_issue_status_projection p WHERE p.workspace_id=generated_issue_status.workspace_id)
 WHERE workspace_id=NEW.workspace_id AND projected_status IS NOT 'done'
 AND EXISTS(SELECT 1 FROM generated_issue_status_projection p JOIN local_project_statuses s
            ON s.project_id=p.project_id AND s.hidden=0 AND lower(s.name)=p.desired_status
            WHERE p.workspace_id=generated_issue_status.workspace_id AND p.desired_status IS NOT generated_issue_status.projected_status);
END;

CREATE TRIGGER generated_issue_status_workflow AFTER UPDATE OF status ON workflow_runs
BEGIN
 UPDATE generated_issue_status SET projected_status=(SELECT desired_status FROM generated_issue_status_projection p WHERE p.workspace_id=generated_issue_status.workspace_id)
 WHERE workspace_id=NEW.workspace_id AND projected_status IS NOT 'done'
 AND EXISTS(SELECT 1 FROM generated_issue_status_projection p JOIN local_project_statuses s
            ON s.project_id=p.project_id AND s.hidden=0 AND lower(s.name)=p.desired_status
            WHERE p.workspace_id=generated_issue_status.workspace_id AND p.desired_status IS NOT generated_issue_status.projected_status);
END;

CREATE TRIGGER generated_issue_status_agent_insert AFTER INSERT ON agent_runs
BEGIN
 UPDATE generated_issue_status SET projected_status=(SELECT desired_status FROM generated_issue_status_projection p WHERE p.workspace_id=generated_issue_status.workspace_id)
 WHERE workspace_id=NEW.workspace_id AND projected_status IS NOT 'done'
 AND EXISTS(SELECT 1 FROM generated_issue_status_projection p JOIN local_project_statuses s
            ON s.project_id=p.project_id AND s.hidden=0 AND lower(s.name)=p.desired_status
            WHERE p.workspace_id=generated_issue_status.workspace_id AND p.desired_status IS NOT generated_issue_status.projected_status);
END;

CREATE TRIGGER generated_issue_status_agent AFTER UPDATE OF status ON agent_runs
BEGIN
 UPDATE generated_issue_status SET projected_status=(SELECT desired_status FROM generated_issue_status_projection p WHERE p.workspace_id=generated_issue_status.workspace_id)
 WHERE workspace_id=NEW.workspace_id AND projected_status IS NOT 'done'
 AND EXISTS(SELECT 1 FROM generated_issue_status_projection p JOIN local_project_statuses s
            ON s.project_id=p.project_id AND s.hidden=0 AND lower(s.name)=p.desired_status
            WHERE p.workspace_id=generated_issue_status.workspace_id AND p.desired_status IS NOT generated_issue_status.projected_status);
END;

CREATE TRIGGER generated_issue_status_node AFTER UPDATE OF status,agent_run_id ON node_executions
BEGIN
 UPDATE generated_issue_status SET projected_status=(SELECT desired_status FROM generated_issue_status_projection p WHERE p.workspace_id=generated_issue_status.workspace_id)
 WHERE workspace_id=(SELECT workspace_id FROM workflow_runs WHERE id=NEW.run_id) AND projected_status IS NOT 'done'
 AND EXISTS(SELECT 1 FROM generated_issue_status_projection p JOIN local_project_statuses s
            ON s.project_id=p.project_id AND s.hidden=0 AND lower(s.name)=p.desired_status
            WHERE p.workspace_id=generated_issue_status.workspace_id AND p.desired_status IS NOT generated_issue_status.projected_status);
END;

CREATE TRIGGER generated_issue_status_node_insert AFTER INSERT ON node_executions
BEGIN
 UPDATE generated_issue_status SET projected_status=(SELECT desired_status FROM generated_issue_status_projection p WHERE p.workspace_id=generated_issue_status.workspace_id)
 WHERE workspace_id=(SELECT workspace_id FROM workflow_runs WHERE id=NEW.run_id) AND projected_status IS NOT 'done'
 AND EXISTS(SELECT 1 FROM generated_issue_status_projection p JOIN local_project_statuses s
            ON s.project_id=p.project_id AND s.hidden=0 AND lower(s.name)=p.desired_status
            WHERE p.workspace_id=generated_issue_status.workspace_id AND p.desired_status IS NOT generated_issue_status.projected_status);
END;
