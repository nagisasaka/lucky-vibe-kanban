-- Import receipts are committed with the new Project. A retry never creates a
-- second Project, including when the same snapshot has a new export UUID.
CREATE TABLE project_transfer_receipts (
    export_id BLOB PRIMARY KEY NOT NULL,
    content_hash TEXT NOT NULL UNIQUE,
    -- Keep the receipt after Project deletion so retries remain detectable,
    -- without interfering with the existing Project deletion lifecycle.
    project_id BLOB NOT NULL UNIQUE,
    report TEXT NOT NULL CHECK(json_valid(report))
);
