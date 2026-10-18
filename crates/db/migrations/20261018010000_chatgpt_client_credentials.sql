-- Separate from active login configuration so credentials survive setup/login.
-- The encrypted document binds credentials to issuer, Web client and resource.
CREATE TABLE access_chatgpt_client (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    encrypted BLOB NOT NULL
);
