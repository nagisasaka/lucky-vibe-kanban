-- A reservation that provably never acquired a host is not an exited process.
-- Preserve every existing row/identity unchanged. In particular, do not repair
-- historical terminal reservations at upgrade: those need explicit verification.
CREATE TABLE agent_process_registry_next (
    id                    BLOB PRIMARY KEY,
    run_attempt_id        BLOB NOT NULL UNIQUE,
    registry_status       TEXT NOT NULL DEFAULT 'reserved' CHECK (registry_status IN (
                              'reserved',
                              'spawned',
                              'running',
                              'exited',
                              'unreachable',
                              'not_started'
                          )),
    host_endpoint         TEXT,
    host_token            TEXT,
    host_instance_id      TEXT,
    host_pid              INTEGER,
    last_host_event_sequence INTEGER NOT NULL DEFAULT 0,
    supervisor_instance_id TEXT,
    pid                   INTEGER,
    process_group_id      INTEGER,
    process_started_at    TEXT,
    executable            TEXT,
    command_fingerprint   TEXT,
    exit_code             INTEGER,
    observed_exited_at    TEXT,
    lease_owner           TEXT,
    lease_expires_at      TEXT,
    created_at            TEXT NOT NULL DEFAULT (datetime('now', 'subsec')),
    updated_at            TEXT NOT NULL DEFAULT (datetime('now', 'subsec')),
    host_protocol_version INTEGER,
    host_start_identity TEXT,
    launch_requested INTEGER NOT NULL DEFAULT 0 CHECK (launch_requested IN (0, 1)),
    starting_write_attempts INTEGER NOT NULL DEFAULT 0 CHECK (starting_write_attempts >= 0),
    starting_failure_pending INTEGER NOT NULL DEFAULT 0 CHECK (starting_failure_pending IN (0, 1)),
    FOREIGN KEY (run_attempt_id) REFERENCES agent_run_attempts(id) ON DELETE CASCADE,
    CHECK (pid IS NULL OR pid > 0),
    CHECK (host_pid IS NULL OR host_pid > 0),
    CHECK ((host_endpoint IS NULL) = (host_token IS NULL)),
    CHECK (last_host_event_sequence >= 0),
    CHECK (process_group_id IS NULL OR process_group_id > 0),
    CHECK ((lease_owner IS NULL) = (lease_expires_at IS NULL)),
    CHECK (
        (registry_status = 'reserved'
            AND pid IS NULL
            AND process_started_at IS NULL
            AND observed_exited_at IS NULL
            AND exit_code IS NULL)
        OR
        (registry_status IN ('spawned', 'running', 'unreachable')
            AND pid IS NOT NULL
            AND process_started_at IS NOT NULL
            AND observed_exited_at IS NULL
            AND exit_code IS NULL)
        OR
        (registry_status = 'not_started'
            AND launch_requested = 0
            AND host_endpoint IS NULL AND host_token IS NULL
            AND host_instance_id IS NULL AND host_pid IS NULL
            AND host_protocol_version IS NULL AND host_start_identity IS NULL
            AND pid IS NULL AND process_group_id IS NULL
            AND process_started_at IS NULL AND observed_exited_at IS NULL
            AND exit_code IS NULL AND last_host_event_sequence = 0
            AND executable IS NULL AND command_fingerprint IS NULL
            AND supervisor_instance_id IS NULL AND lease_owner IS NULL)
        OR
        (registry_status = 'exited'
            AND pid IS NOT NULL
            AND process_started_at IS NOT NULL
            AND observed_exited_at IS NOT NULL)
    )
);

INSERT INTO agent_process_registry_next (id,run_attempt_id,registry_status,host_endpoint,host_token,host_instance_id,host_pid,last_host_event_sequence,supervisor_instance_id,pid,process_group_id,process_started_at,executable,command_fingerprint,exit_code,observed_exited_at,lease_owner,lease_expires_at,created_at,updated_at,host_protocol_version,host_start_identity,launch_requested,starting_write_attempts,starting_failure_pending)
    SELECT id,run_attempt_id,registry_status,host_endpoint,host_token,host_instance_id,host_pid,last_host_event_sequence,supervisor_instance_id,pid,process_group_id,process_started_at,executable,command_fingerprint,exit_code,observed_exited_at,lease_owner,lease_expires_at,created_at,updated_at,host_protocol_version,host_start_identity,launch_requested,starting_write_attempts,starting_failure_pending FROM agent_process_registry;
DROP TABLE agent_process_registry;
ALTER TABLE agent_process_registry_next RENAME TO agent_process_registry;

CREATE UNIQUE INDEX idx_agent_process_registry_process_identity
    ON agent_process_registry(pid, process_started_at)
    WHERE pid IS NOT NULL AND process_started_at IS NOT NULL;
CREATE INDEX idx_agent_process_registry_status
    ON agent_process_registry(registry_status);

CREATE INDEX idx_agent_process_registry_prelaunch_recovery
    ON agent_process_registry(run_attempt_id)
    WHERE launch_requested = 1 AND registry_status = 'reserved' AND host_endpoint IS NULL;

-- Canonical terminal projection and reservation release commit or roll back
-- together. A concurrent host reservation wins first or remains fenced by the
-- terminal projection; NULL provider PID alone is never sufficient evidence.
CREATE TRIGGER release_unstarted_agent_reservation
AFTER UPDATE OF status ON agent_run_attempts
WHEN OLD.status NOT IN ('succeeded','failed','cancelled','crashed','audit_failed')
 AND NEW.status IN ('failed','cancelled','crashed','audit_failed')
BEGIN
    UPDATE agent_process_registry
    SET registry_status='not_started', launch_requested=0,
        updated_at=NEW.updated_at
    WHERE run_attempt_id=NEW.id AND registry_status='reserved'
      AND host_endpoint IS NULL AND host_token IS NULL
      AND host_instance_id IS NULL AND host_pid IS NULL
      AND host_protocol_version IS NULL AND host_start_identity IS NULL
      AND pid IS NULL AND process_group_id IS NULL
      AND process_started_at IS NULL AND observed_exited_at IS NULL
      AND exit_code IS NULL AND last_host_event_sequence=0
      AND executable IS NULL AND command_fingerprint IS NULL
      AND supervisor_instance_id IS NULL AND lease_owner IS NULL
      AND NOT EXISTS(SELECT 1 FROM native_audit_streams n WHERE n.run_attempt_id=NEW.id);
END;
