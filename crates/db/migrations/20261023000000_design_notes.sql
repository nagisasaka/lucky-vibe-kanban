-- Intentionally no cascading deletion: design history has no retention policy yet.
CREATE TABLE design_notes (
 id BLOB PRIMARY KEY NOT NULL,
 project_id BLOB NOT NULL REFERENCES projects(id),
 title TEXT NOT NULL,
 body TEXT NOT NULL,
 revision INTEGER NOT NULL,
 editor TEXT NOT NULL,
 change_reason TEXT,
 created_at TEXT NOT NULL,
 updated_at TEXT NOT NULL
);
CREATE INDEX design_notes_project ON design_notes(project_id, updated_at DESC, id);
CREATE TABLE design_note_revisions (
 note_id BLOB NOT NULL REFERENCES design_notes(id),
 revision INTEGER NOT NULL,
 snapshot TEXT NOT NULL,
 PRIMARY KEY(note_id, revision)
);
CREATE TABLE design_note_additions (
 id BLOB PRIMARY KEY NOT NULL,
 note_id BLOB NOT NULL REFERENCES design_notes(id),
 heading TEXT NOT NULL,
 content TEXT NOT NULL,
 kind TEXT NOT NULL CHECK(kind IN ('decision','proposal','open_question')),
 recorder TEXT NOT NULL,
 source TEXT NOT NULL,
 speaker TEXT NOT NULL,
 referenced_body_revision INTEGER NOT NULL,
 short_quote TEXT,
 hidden BOOLEAN NOT NULL DEFAULT 0,
 revision INTEGER NOT NULL,
 editor TEXT NOT NULL,
 created_at TEXT NOT NULL,
 updated_at TEXT NOT NULL,
 FOREIGN KEY(note_id, referenced_body_revision) REFERENCES design_note_revisions(note_id, revision)
);
CREATE TABLE design_note_addition_revisions (
 addition_id BLOB NOT NULL REFERENCES design_note_additions(id),
 revision INTEGER NOT NULL,
 snapshot TEXT NOT NULL,
 PRIMARY KEY(addition_id, revision)
);
CREATE TABLE design_note_requests (
 project_id BLOB NOT NULL REFERENCES projects(id),
 actor TEXT NOT NULL,
 request_id BLOB NOT NULL,
 input TEXT NOT NULL,
 result TEXT,
 PRIMARY KEY(project_id, actor, request_id)
);
