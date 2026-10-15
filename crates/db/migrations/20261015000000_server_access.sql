-- Separate server membership from historical local/cloud identities. No backfill
-- attributes old actions to a newly authenticated person.
CREATE TABLE access_settings (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    revision INTEGER NOT NULL,
    encrypted BLOB NOT NULL
);
CREATE TABLE access_members (
    id TEXT PRIMARY KEY,
    issuer TEXT NOT NULL,
    subject TEXT NOT NULL,
    display_name TEXT NOT NULL,
    state TEXT NOT NULL CHECK (state IN ('pending', 'active', 'disabled')),
    role TEXT NOT NULL CHECK (role IN ('admin', 'member')),
    created_at INTEGER NOT NULL,
    UNIQUE (issuer, subject)
);
CREATE TABLE access_sessions (
    hash TEXT PRIMARY KEY,
    user_id TEXT REFERENCES access_members(id),
    kind TEXT NOT NULL CHECK (kind IN ('web', 'setup', 'recovery')),
    csrf TEXT NOT NULL,
    expires INTEGER NOT NULL,
    touched INTEGER NOT NULL,
    revision INTEGER NOT NULL
);
CREATE INDEX access_sessions_user ON access_sessions(user_id);
CREATE TABLE access_keys (
    hash TEXT PRIMARY KEY,
    kind TEXT NOT NULL CHECK (kind IN ('setup', 'recovery')),
    expires INTEGER NOT NULL
);
CREATE TABLE access_transactions (
    state_hash TEXT PRIMARY KEY,
    browser_hash TEXT NOT NULL,
    encrypted BLOB NOT NULL,
    expires INTEGER NOT NULL
);
CREATE TABLE access_confirmations (
    session_hash TEXT PRIMARY KEY REFERENCES access_sessions(hash) ON DELETE CASCADE,
    encrypted BLOB NOT NULL,
    expires INTEGER NOT NULL
);
CREATE TABLE access_audit (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    at INTEGER NOT NULL,
    actor TEXT,
    channel TEXT NOT NULL,
    action TEXT NOT NULL,
    target TEXT,
    outcome TEXT NOT NULL
);
