CREATE TABLE IF NOT EXISTS message_reactions (
    message_id BLOB NOT NULL,
    conversation_id BLOB NOT NULL,
    actor_id BLOB NOT NULL,
    emoji TEXT NOT NULL,
    active INTEGER NOT NULL,
    updated_at_ms INTEGER NOT NULL,
    PRIMARY KEY (message_id, actor_id, emoji)
);
CREATE INDEX IF NOT EXISTS idx_message_reactions_conversation
    ON message_reactions(conversation_id, updated_at_ms);
