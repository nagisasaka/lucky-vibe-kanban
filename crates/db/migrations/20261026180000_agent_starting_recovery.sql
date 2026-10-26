-- Launch eligibility is explicit: a setup-only reservation is not failed by recovery.
-- Legacy reservations deliberately remain ineligible; no historical Run is relaunched.
-- The counter is committed BEFORE each Starting write, so restart cannot reset the bound.
ALTER TABLE agent_process_registry ADD COLUMN launch_requested INTEGER NOT NULL DEFAULT 0 CHECK (launch_requested IN (0, 1));
ALTER TABLE agent_process_registry ADD COLUMN starting_write_attempts INTEGER NOT NULL DEFAULT 0 CHECK (starting_write_attempts >= 0);

-- A durable failure fence prevents a concurrent late Starting/host reservation.
ALTER TABLE agent_process_registry ADD COLUMN starting_failure_pending INTEGER NOT NULL DEFAULT 0 CHECK (starting_failure_pending IN (0, 1));

CREATE INDEX idx_agent_process_registry_prelaunch_recovery
    ON agent_process_registry(run_attempt_id)
    WHERE launch_requested = 1 AND registry_status = 'reserved' AND host_endpoint IS NULL;
