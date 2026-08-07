CREATE TABLE peer_credentials (
    contact_id BLOB PRIMARY KEY NOT NULL REFERENCES contacts(contact_id) ON DELETE CASCADE,
    local_capability_id BLOB NOT NULL UNIQUE,
    secret_handle BLOB NOT NULL UNIQUE
);
