CREATE TABLE IF NOT EXISTS messages (
    message_id BLOB PRIMARY KEY NOT NULL,
    conversation_id BLOB NOT NULL,
    direction INTEGER NOT NULL,
    status INTEGER NOT NULL,
    body TEXT NOT NULL,
    reply_to_message_id BLOB,
    created_at_ms INTEGER NOT NULL,
    updated_at_ms INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS messages_conversation_created ON messages(conversation_id, created_at_ms, message_id);

CREATE TABLE IF NOT EXISTS outbox (
    message_id BLOB PRIMARY KEY NOT NULL REFERENCES messages(message_id) ON DELETE CASCADE,
    command_id BLOB UNIQUE NOT NULL,
    state INTEGER NOT NULL,
    attempts INTEGER NOT NULL,
    next_attempt_at_ms INTEGER NOT NULL,
    claimed_at_ms INTEGER
);
CREATE INDEX IF NOT EXISTS outbox_due ON outbox(state, next_attempt_at_ms);

CREATE TABLE IF NOT EXISTS inbound_dedup (
    envelope_id BLOB PRIMARY KEY NOT NULL,
    accepted_at_ms INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS receipts (
    receipt_id BLOB PRIMARY KEY NOT NULL,
    message_id BLOB NOT NULL REFERENCES messages(message_id) ON DELETE CASCADE,
    kind INTEGER NOT NULL,
    received_at_ms INTEGER NOT NULL,
    UNIQUE(message_id, kind)
);
