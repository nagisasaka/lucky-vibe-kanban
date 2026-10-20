-- An explicit saved value (including disabled) supersedes legacy file configuration.
CREATE TABLE runtime_access_settings (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    revision INTEGER NOT NULL,
    settings TEXT NOT NULL
);
