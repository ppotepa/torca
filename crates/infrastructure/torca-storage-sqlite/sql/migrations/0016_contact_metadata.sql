CREATE TABLE contact_metadata (
    contact_id BLOB PRIMARY KEY NOT NULL REFERENCES contacts(contact_id) ON DELETE CASCADE,
    display_name TEXT NOT NULL,
    updated_at_ms INTEGER NOT NULL,
    CHECK(length(display_name) BETWEEN 1 AND 256)
);
