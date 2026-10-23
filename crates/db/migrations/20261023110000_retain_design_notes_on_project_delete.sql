-- Retain design history when its Project is deleted. Access after deletion is
-- deliberately closed; retrieval/export/retention policy belongs to LOCAL-99.
-- No FK to projects: this identity outlives the live Project and its metadata.
CREATE TABLE design_note_projects (
 id BLOB PRIMARY KEY NOT NULL,
 name TEXT NOT NULL,
 organization_id BLOB,
 deleted_at TEXT
);
INSERT INTO design_note_projects(id,name,organization_id)
SELECT p.id,p.name,m.organization_id FROM projects p
LEFT JOIN local_project_metadata m ON m.project_id=p.id
WHERE EXISTS(SELECT 1 FROM design_notes n WHERE n.project_id=p.id)
   OR EXISTS(SELECT 1 FROM design_note_requests r WHERE r.project_id=p.id);

-- Rebuild the FK graph in dependency order, without disabling foreign keys.
-- Snapshots and retry evidence are copied verbatim within the migration transaction.
CREATE TEMP TABLE retain_design_notes AS SELECT * FROM design_notes;
CREATE TEMP TABLE retain_design_note_revisions AS SELECT * FROM design_note_revisions;
CREATE TEMP TABLE retain_design_note_additions AS SELECT * FROM design_note_additions;
CREATE TEMP TABLE retain_design_note_addition_revisions AS SELECT * FROM design_note_addition_revisions;
CREATE TEMP TABLE retain_design_note_requests AS SELECT * FROM design_note_requests;
DROP TABLE design_note_addition_revisions;
DROP TABLE design_note_additions;
DROP TABLE design_note_revisions;
DROP TABLE design_note_requests;
DROP TABLE design_notes;

CREATE TABLE design_notes (
 id BLOB PRIMARY KEY NOT NULL,
 project_id BLOB NOT NULL REFERENCES design_note_projects(id),
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
 project_id BLOB NOT NULL REFERENCES design_note_projects(id),
 actor TEXT NOT NULL,
 request_id BLOB NOT NULL,
 input TEXT NOT NULL,
 result TEXT,
 PRIMARY KEY(project_id, actor, request_id)
);

INSERT INTO design_notes SELECT * FROM retain_design_notes;
DROP TABLE retain_design_notes;
INSERT INTO design_note_revisions SELECT * FROM retain_design_note_revisions;
DROP TABLE retain_design_note_revisions;
INSERT INTO design_note_additions SELECT * FROM retain_design_note_additions;
DROP TABLE retain_design_note_additions;
INSERT INTO design_note_addition_revisions SELECT * FROM retain_design_note_addition_revisions;
DROP TABLE retain_design_note_addition_revisions;
INSERT INTO design_note_requests SELECT * FROM retain_design_note_requests;
DROP TABLE retain_design_note_requests;

CREATE TRIGGER design_notes_retain_project BEFORE INSERT ON design_notes
BEGIN
 INSERT OR IGNORE INTO design_note_projects(id,name,organization_id)
 SELECT p.id,p.name,m.organization_id FROM projects p
 LEFT JOIN local_project_metadata m ON m.project_id=p.id WHERE p.id=NEW.project_id;
END;

CREATE TRIGGER design_note_requests_retain_project BEFORE INSERT ON design_note_requests
BEGIN
 INSERT OR IGNORE INTO design_note_projects(id,name,organization_id)
 SELECT p.id,p.name,m.organization_id FROM projects p
 LEFT JOIN local_project_metadata m ON m.project_id=p.id WHERE p.id=NEW.project_id;
END;

-- Runs before metadata CASCADE; a later FK rejection rolls this back as well.
CREATE TRIGGER design_notes_project_deleted BEFORE DELETE ON projects
BEGIN
 UPDATE design_note_projects SET name=OLD.name,
 organization_id=(SELECT organization_id FROM local_project_metadata WHERE project_id=OLD.id),
 deleted_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id=OLD.id;
END;
-- Never allow a new Project to inherit retained notes or the former access scope.
CREATE TRIGGER design_notes_prevent_project_reuse BEFORE INSERT ON projects
WHEN EXISTS(SELECT 1 FROM design_note_projects WHERE id=NEW.id AND deleted_at IS NOT NULL)
BEGIN
 SELECT RAISE(ABORT, 'Deleted Project identity is reserved for retained design history');
END;
