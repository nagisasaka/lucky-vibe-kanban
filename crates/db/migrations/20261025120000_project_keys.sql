-- Existing Projects keep their historical Issue prefix. Issue rows are untouched.
ALTER TABLE projects ADD COLUMN key TEXT NOT NULL DEFAULT 'LOCAL'
    CHECK (length(key) BETWEEN 2 AND 10
        AND key GLOB '[A-Z]*' AND key NOT GLOB '*[^A-Z0-9]*');
CREATE UNIQUE INDEX idx_projects_key ON projects(key);
