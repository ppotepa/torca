CREATE TABLE IF NOT EXISTS conversation_read_state (
    conversation_id BLOB PRIMARY KEY,
    read_through_ms INTEGER NOT NULL
);
