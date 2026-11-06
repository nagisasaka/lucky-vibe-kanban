-- Semantic artifacts and filesystem restoration journals remain files.
CREATE TABLE repository_wiki_state (
    repository_id BLOB PRIMARY KEY NOT NULL,
    payload TEXT NOT NULL CHECK(json_valid(payload))
);
CREATE TABLE repository_wiki_receipts (
    repository_id BLOB NOT NULL,
    id BLOB NOT NULL,
    payload TEXT NOT NULL CHECK(json_valid(payload)),
    PRIMARY KEY(repository_id, id)
);
CREATE TABLE repository_wiki_publications (
    repository_id BLOB NOT NULL,
    id BLOB NOT NULL,
    payload TEXT NOT NULL CHECK(json_valid(payload)),
    PRIMARY KEY(repository_id, id)
);
CREATE TABLE repository_integrations (
    repository_id BLOB NOT NULL,
    id BLOB NOT NULL,
    payload TEXT NOT NULL CHECK(json_valid(payload)),
    PRIMARY KEY(repository_id, id)
);
