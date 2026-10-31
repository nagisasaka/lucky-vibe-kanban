-- Preserve all previously delivered event classes for existing and new consent.
ALTER TABLE web_push_subscriptions ADD COLUMN event_mask INTEGER NOT NULL DEFAULT 31 CHECK(event_mask BETWEEN 0 AND 31);
DROP TRIGGER web_push_question;
DROP TRIGGER web_push_terminal;
CREATE TRIGGER web_push_question AFTER INSERT ON agent_events
WHEN json_extract(NEW.event_envelope,'$.payload.type') IN ('input_requested','approval_requested')
BEGIN
    INSERT OR IGNORE INTO web_push_deliveries(subscription_id,event_key,agent_run_id,attempt_id,reference_id,kind)
    SELECT s.id, 'control:' || hex(NEW.run_attempt_id) || ':' || json_extract(NEW.event_envelope,'$.payload.type') || ':' || COALESCE(json_extract(NEW.event_envelope,'$.payload.data.input_id'),json_extract(NEW.event_envelope,'$.payload.data.approval_id')), r.id, NEW.run_attempt_id,
        COALESCE(json_extract(NEW.event_envelope,'$.payload.data.input_id'),json_extract(NEW.event_envelope,'$.payload.data.approval_id')),
        json_extract(NEW.event_envelope,'$.payload.type')
    FROM agent_runs r JOIN local_workspace_links l ON l.workspace_id=r.workspace_id
    JOIN web_push_subscriptions s ON s.project_id=l.project_id
    WHERE r.id=NEW.agent_run_id AND r.provider_id='codex'
      AND (s.event_mask & CASE json_extract(NEW.event_envelope,'$.payload.type') WHEN 'input_requested' THEN 1 ELSE 2 END) != 0;
END;
CREATE TRIGGER web_push_terminal AFTER UPDATE OF status ON agent_runs
WHEN NEW.provider_id='codex' AND NEW.status != OLD.status
 AND NEW.status IN ('succeeded','failed','cancelled','crashed','audit_failed')
BEGIN
    INSERT OR IGNORE INTO web_push_deliveries(subscription_id,event_key,agent_run_id,attempt_id,kind)
    SELECT s.id, 'terminal:' || hex(NEW.id) || ':' || COALESCE(hex((SELECT id FROM agent_run_attempts WHERE agent_run_id=NEW.id ORDER BY attempt_number DESC LIMIT 1)),''),
        NEW.id, (SELECT id FROM agent_run_attempts WHERE agent_run_id=NEW.id ORDER BY attempt_number DESC LIMIT 1), NEW.status
    FROM local_workspace_links l JOIN web_push_subscriptions s ON s.project_id=l.project_id
    WHERE l.workspace_id=NEW.workspace_id
      AND (s.event_mask & CASE NEW.status WHEN 'succeeded' THEN 4 WHEN 'cancelled' THEN 16 ELSE 8 END) != 0;
END;
