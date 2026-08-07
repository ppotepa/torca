CREATE TABLE contact_verification (
    contact_id BLOB PRIMARY KEY NOT NULL REFERENCES contacts(contact_id) ON DELETE CASCADE,
    remote_identity_id BLOB NOT NULL,
    verified_at_ms INTEGER NOT NULL
);
