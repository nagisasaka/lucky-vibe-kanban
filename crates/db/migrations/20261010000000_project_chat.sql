-- Conversations are owned separately from shared project state. No synthetic Issue.
CREATE TABLE project_conversations (
    id BLOB PRIMARY KEY NOT NULL,
    project_id BLOB NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    owner_user_id BLOB NOT NULL,
    name TEXT NOT NULL DEFAULT 'Astra',
    event_cursor INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (datetime('now', 'subsec')),
    UNIQUE(project_id, owner_user_id, name)
);
CREATE TABLE project_chat_messages (
    sequence INTEGER PRIMARY KEY AUTOINCREMENT,
    id BLOB NOT NULL UNIQUE,
    conversation_id BLOB NOT NULL REFERENCES project_conversations(id) ON DELETE CASCADE,
    role TEXT NOT NULL CHECK(role IN ('user','assistant','event','tool')),
    content TEXT NOT NULL,
    intent TEXT NOT NULL DEFAULT 'discuss',
    status TEXT NOT NULL DEFAULT 'pending' CHECK(status IN ('pending','running','done')),
    checkpoint TEXT,
    retry_at INTEGER NOT NULL DEFAULT 0,
    attempts INTEGER NOT NULL DEFAULT 0,
    error TEXT,
    source_key TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now', 'subsec')),
    UNIQUE(conversation_id, source_key)
);
CREATE INDEX project_chat_pending ON project_chat_messages(conversation_id, status, sequence);
-- A write-ahead receipt for tool side effects, not an execution scheduler.
CREATE TABLE project_chat_operations (
    id BLOB PRIMARY KEY NOT NULL,
    message_id BLOB NOT NULL REFERENCES project_chat_messages(id) ON DELETE CASCADE,
    call_id TEXT NOT NULL,
    name TEXT NOT NULL,
    arguments TEXT NOT NULL,
    result TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now', 'subsec')),
    UNIQUE(message_id, call_id)
);
-- Small durable notification outbox. Triggers participate in the original write
-- transaction, including writes made directly from a Workspace. Token/tool noise
-- never enters this feed. AUTOINCREMENT prevents cursor reuse after deletions.
CREATE TABLE project_chat_events (
    sequence INTEGER PRIMARY KEY AUTOINCREMENT,
    project_id BLOB NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    source_key TEXT NOT NULL UNIQUE,
    payload TEXT NOT NULL
);
CREATE INDEX project_chat_events_project ON project_chat_events(project_id, sequence);
CREATE TRIGGER project_chat_issue_created AFTER INSERT ON local_issues
BEGIN
    INSERT OR IGNORE INTO project_chat_events(project_id,source_key,payload)
    VALUES(NEW.project_id,'issue-created:' || hex(NEW.id),
      json_object('kind','issue_created','issue_id',lower(hex(NEW.id)),
        'title',NEW.title,'creator_user_id',lower(hex(NEW.creator_user_id))));
END;
CREATE TRIGGER project_chat_workspace_link AFTER INSERT ON local_workspace_links
BEGIN
    INSERT INTO project_chat_events(project_id,source_key,payload)
    SELECT NEW.project_id,'workspace-link:' || lower(hex(randomblob(16))),
      json_object('kind','workspace_linked','workspace_id',lower(hex(NEW.workspace_id)),
        'issue_id',lower(hex(NEW.issue_id)))
    WHERE EXISTS(SELECT 1 FROM local_issues i WHERE i.id=NEW.issue_id AND i.project_id=NEW.project_id);
END;
CREATE TRIGGER project_chat_direct_merge AFTER INSERT ON merges
BEGIN
    INSERT OR IGNORE INTO project_chat_events(project_id,source_key,payload)
    SELECT l.project_id,'merge:' || hex(NEW.id),
      json_object('kind','merge_recorded','workspace_id',lower(hex(NEW.workspace_id)),
        'issue_id',lower(hex(l.issue_id)),'repo_id',lower(hex(NEW.repo_id)),
        'merge_commit',NEW.merge_commit,'target_branch',NEW.target_branch_name,
        'wiki_status','unverified')
    FROM local_workspace_links l JOIN local_issues i ON i.id=l.issue_id AND i.project_id=l.project_id WHERE l.workspace_id=NEW.workspace_id;
END;
CREATE TRIGGER project_chat_run_requested AFTER INSERT ON agent_runs
BEGIN
    INSERT OR IGNORE INTO project_chat_events(project_id,source_key,payload)
    SELECT l.project_id,'run-request:' || hex(NEW.id),
      json_object('kind','run_requested','workspace_id',lower(hex(NEW.workspace_id)),
        'issue_id',lower(hex(l.issue_id)),'agent_run_id',lower(hex(NEW.id)),
        'session_id',lower(hex(NEW.session_id)),'request',json(NEW.request_envelope))
    FROM local_workspace_links l JOIN local_issues i ON i.id=l.issue_id AND i.project_id=l.project_id
    WHERE l.workspace_id=NEW.workspace_id;
END;
CREATE TRIGGER project_chat_agent_event AFTER INSERT ON agent_events
WHEN json_extract(NEW.event_envelope, '$.payload.type') IN
 ('input_requested','input_resolved','approval_requested','approval_resolved','goal_updated','goal_cleared','error')
 OR (json_extract(NEW.event_envelope,'$.payload.type')='lifecycle_changed' AND json_extract(NEW.event_envelope,'$.payload.data.status') IN ('awaiting_input','awaiting_approval','succeeded','failed','cancelled','crashed','audit_failed'))
 OR (json_extract(NEW.event_envelope,'$.payload.type')='provider_extension' AND json_extract(NEW.event_envelope,'$.payload.data.provider_namespace')='codex' AND json_extract(NEW.event_envelope,'$.payload.data.provider_event')='async_questions')
 OR (json_extract(NEW.event_envelope, '$.payload.type') = 'message' AND
    (json_extract(NEW.event_envelope, '$.payload.data.final_output') = 1 OR
     json_extract(NEW.event_envelope, '$.payload.data.message.role') = 'user'))
BEGIN
    INSERT OR IGNORE INTO project_chat_events(project_id, source_key, payload)
    SELECT l.project_id, 'agent:' || hex(NEW.event_id),
      json_object('kind','agent','workspace_id',lower(hex(l.workspace_id)),
        'issue_id',lower(hex(l.issue_id)), 'event',json(NEW.event_envelope))
    FROM agent_runs r JOIN local_workspace_links l ON l.workspace_id=r.workspace_id
    JOIN local_issues i ON i.id=l.issue_id AND i.project_id=l.project_id
    WHERE r.id=NEW.agent_run_id;
END;
CREATE TRIGGER project_chat_issue_update AFTER UPDATE ON local_issues
WHEN OLD.title IS NOT NEW.title OR OLD.description IS NOT NEW.description OR OLD.status_id IS NOT NEW.status_id
BEGIN
    INSERT INTO project_chat_events(project_id, source_key, payload)
    VALUES(NEW.project_id, 'issue:' || lower(hex(randomblob(16))),
      json_object('kind','issue_changed','issue_id',lower(hex(NEW.id)),
        'title',NEW.title,'status_id',lower(hex(NEW.status_id))));
END;
CREATE TRIGGER project_chat_direct_command AFTER INSERT ON agent_run_commands
BEGIN
    INSERT OR IGNORE INTO project_chat_events(project_id, source_key, payload)
    SELECT l.project_id, 'command:' || hex(NEW.command_id),
      json_object('kind','command','workspace_id',lower(hex(l.workspace_id)),
        'issue_id',lower(hex(l.issue_id)), 'command',json(NEW.command_envelope))
    FROM agent_runs r JOIN local_workspace_links l ON l.workspace_id=r.workspace_id
    JOIN local_issues i ON i.id=l.issue_id AND i.project_id=l.project_id
    WHERE r.id=NEW.agent_run_id;
END;

-- The check belongs to the common command admission path, so a direct UI
-- answer racing an Astra answer cannot be delivered twice. Replays of the
-- original idempotency key still use the existing command receipt.
CREATE TRIGGER agent_input_still_pending BEFORE INSERT ON agent_run_commands
WHEN json_extract(NEW.command_envelope,'$.command.type')='submit_input'
 AND NOT EXISTS(SELECT 1 FROM agent_run_commands c WHERE c.idempotency_key=NEW.idempotency_key)
 AND NOT EXISTS(
   SELECT 1 FROM agent_events e JOIN agent_runs r ON r.id=e.agent_run_id
   JOIN agent_run_state s ON s.agent_run_id=r.id
   WHERE e.agent_run_id=NEW.agent_run_id
     AND r.status NOT IN ('succeeded','failed','cancelled','crashed','audit_failed')
     AND e.run_attempt_number=s.last_run_attempt_number
     AND json_extract(e.event_envelope,'$.payload.type')='input_requested'
     AND json_extract(e.event_envelope,'$.payload.data.input_id')=json_extract(NEW.command_envelope,'$.command.data.input_id')
     AND NOT EXISTS(SELECT 1 FROM agent_events x WHERE x.agent_run_id=e.agent_run_id
       AND x.run_attempt_number=e.run_attempt_number AND x.sequence>e.sequence
       AND json_extract(x.event_envelope,'$.payload.type')='input_resolved'
       AND json_extract(x.event_envelope,'$.payload.data.input_id')=json_extract(e.event_envelope,'$.payload.data.input_id'))
     AND NOT EXISTS(SELECT 1 FROM agent_run_commands c WHERE c.agent_run_id=e.agent_run_id
       AND c.created_at>=e.created_at
       AND json_extract(c.command_envelope,'$.command.type')='submit_input'
       AND json_extract(c.command_envelope,'$.command.data.input_id')=json_extract(e.event_envelope,'$.payload.data.input_id'))
 )
BEGIN
  SELECT RAISE(ABORT,'Question is no longer pending or an answer has already been submitted');
END;

-- Shared by the Workspace question UI and Astra. Reservations survive an
-- ambiguous delivery; another request must inspect rather than send twice.
CREATE TABLE agent_question_answers (
    question_id TEXT PRIMARY KEY NOT NULL,
    session_id BLOB NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    request_id BLOB NOT NULL UNIQUE,
    actor TEXT NOT NULL,
    answer TEXT NOT NULL,
    result TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now','subsec'))
);
CREATE TRIGGER project_chat_question_answer AFTER INSERT ON agent_question_answers
BEGIN
    INSERT OR IGNORE INTO project_chat_events(project_id,source_key,payload)
    SELECT l.project_id,'question-answer:' || NEW.question_id,
      json_object('kind','question_answer_submitted','question_id',NEW.question_id,
      'actor',NEW.actor,'workspace_id',lower(hex(l.workspace_id)),'answer',NEW.answer)
    FROM sessions s JOIN local_workspace_links l ON l.workspace_id=s.workspace_id
    JOIN local_issues i ON i.id=l.issue_id AND i.project_id=l.project_id
    WHERE s.id=NEW.session_id;
END;
