-- Verified cumulative counters only. Existing usage is deliberately not backfilled.
CREATE TABLE workspace_token_usage (
    workspace_id BLOB NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    provider_id TEXT NOT NULL,
    provider_session_id TEXT NOT NULL,
    total_tokens INTEGER NOT NULL CHECK(total_tokens >= 0),
    input_tokens INTEGER NOT NULL CHECK(input_tokens >= 0),
    output_tokens INTEGER NOT NULL CHECK(output_tokens >= 0),
    cached_input_tokens INTEGER,
    reasoning_tokens INTEGER,
    first_observed_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    PRIMARY KEY(workspace_id, provider_id, provider_session_id)
);
