CREATE TABLE access_preview_grants (
 hash TEXT PRIMARY KEY, browser_hash TEXT NOT NULL, origin TEXT NOT NULL,
 session_hash TEXT NOT NULL REFERENCES access_sessions(hash) ON DELETE CASCADE,
 expires INTEGER NOT NULL
);
CREATE TABLE access_preview_sessions (
 hash TEXT PRIMARY KEY, origin TEXT NOT NULL,
 session_hash TEXT NOT NULL REFERENCES access_sessions(hash) ON DELETE CASCADE,
 expires INTEGER NOT NULL
);
