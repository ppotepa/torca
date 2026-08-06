CREATE TABLE IF NOT EXISTS contacts (
    contact_id BLOB PRIMARY KEY NOT NULL,
    remote_identity_id BLOB NOT NULL,
    remote_key_id BLOB NOT NULL,
    remote_key_algorithm INTEGER NOT NULL,
    remote_public_key BLOB NOT NULL,
    remote_key_generation INTEGER NOT NULL,
    onion_address TEXT NOT NULL,
    capability_id BLOB NOT NULL,
    status INTEGER NOT NULL,
    created_at_ms INTEGER NOT NULL,
    updated_at_ms INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS conversations (
    conversation_id BLOB PRIMARY KEY NOT NULL,
    contact_id BLOB UNIQUE NOT NULL REFERENCES contacts(contact_id) ON DELETE CASCADE,
    status INTEGER NOT NULL,
    created_at_ms INTEGER NOT NULL,
    updated_at_ms INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS conversations_contact ON conversations(contact_id);
