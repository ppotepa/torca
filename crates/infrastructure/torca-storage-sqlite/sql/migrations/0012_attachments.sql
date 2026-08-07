CREATE TABLE attachments (
    attachment_id BLOB PRIMARY KEY NOT NULL,
    message_id BLOB NOT NULL REFERENCES messages(message_id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    media_type TEXT NOT NULL,
    size_bytes INTEGER NOT NULL,
    status INTEGER NOT NULL,
    created_at_ms INTEGER NOT NULL,
    updated_at_ms INTEGER NOT NULL,
    attempt_count INTEGER NOT NULL DEFAULT 0,
    transfer_offset INTEGER NOT NULL DEFAULT 0,
    content_digest BLOB
);

CREATE INDEX attachments_message
ON attachments(message_id, attachment_id);

CREATE INDEX attachments_transfer_state
ON attachments(status, updated_at_ms, attachment_id);
