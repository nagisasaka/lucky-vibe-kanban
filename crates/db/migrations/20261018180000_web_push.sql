-- Registration is explicit per browser, authenticated owner and Project.
-- Endpoint and keys are sealed with the server-access encryption key.
CREATE TABLE web_push_subscriptions (
    id BLOB PRIMARY KEY,
    owner BLOB NOT NULL,
    project_id BLOB NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    endpoint_hash TEXT NOT NULL,
    sealed BLOB NOT NULL,
    created_at INTEGER NOT NULL DEFAULT (unixepoch()),
    UNIQUE(endpoint_hash, project_id)
);
CREATE TABLE web_push_deliveries (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    subscription_id BLOB NOT NULL REFERENCES web_push_subscriptions(id) ON DELETE CASCADE,
    event_key TEXT NOT NULL,
    agent_run_id BLOB NOT NULL REFERENCES agent_runs(id) ON DELETE CASCADE,
    attempt_id BLOB,
    reference_id TEXT,
    kind TEXT NOT NULL,
    tries INTEGER NOT NULL DEFAULT 0,
    next_at INTEGER NOT NULL DEFAULT (unixepoch()),
    created_at INTEGER NOT NULL DEFAULT (unixepoch()),
    finished INTEGER NOT NULL DEFAULT 0,
    UNIQUE(subscription_id, event_key)
);
CREATE INDEX web_push_due ON web_push_deliveries(finished, next_at);
-- These triggers run inside the canonical event/projection transaction. No
-- browser connection, polling historical logs or provider prose creates work.
CREATE TRIGGER web_push_question AFTER INSERT ON agent_events
WHEN json_extract(NEW.event_envelope,'$.payload.type') IN ('input_requested','approval_requested')
BEGIN
    INSERT OR IGNORE INTO web_push_deliveries(subscription_id,event_key,agent_run_id,attempt_id,reference_id,kind)
    SELECT s.id, 'control:' || hex(NEW.run_attempt_id) || ':' || json_extract(NEW.event_envelope,'$.payload.type') || ':' || COALESCE(json_extract(NEW.event_envelope,'$.payload.data.input_id'),json_extract(NEW.event_envelope,'$.payload.data.approval_id')), r.id, NEW.run_attempt_id,
        COALESCE(json_extract(NEW.event_envelope,'$.payload.data.input_id'),json_extract(NEW.event_envelope,'$.payload.data.approval_id')),
        json_extract(NEW.event_envelope,'$.payload.type')
    FROM agent_runs r JOIN local_workspace_links l ON l.workspace_id=r.workspace_id
    JOIN web_push_subscriptions s ON s.project_id=l.project_id
    WHERE r.id=NEW.agent_run_id AND r.provider_id='codex';
END;
CREATE TRIGGER web_push_terminal AFTER UPDATE OF status ON agent_runs
WHEN NEW.provider_id='codex' AND NEW.status != OLD.status
 AND NEW.status IN ('succeeded','failed','cancelled','crashed','audit_failed')
BEGIN
    INSERT OR IGNORE INTO web_push_deliveries(subscription_id,event_key,agent_run_id,attempt_id,kind)
    SELECT s.id, 'terminal:' || hex(NEW.id) || ':' || COALESCE(hex((SELECT id FROM agent_run_attempts WHERE agent_run_id=NEW.id ORDER BY attempt_number DESC LIMIT 1)),''),
        NEW.id, (SELECT id FROM agent_run_attempts WHERE agent_run_id=NEW.id ORDER BY attempt_number DESC LIMIT 1), NEW.status
    FROM local_workspace_links l JOIN web_push_subscriptions s ON s.project_id=l.project_id
    WHERE l.workspace_id=NEW.workspace_id;
END;
