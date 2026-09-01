INSERT INTO message_reactions
    (message_id, conversation_id, actor_id, emoji, active, updated_at_ms)
VALUES (?1, ?2, ?3, ?4, ?5, ?6)
ON CONFLICT(message_id, actor_id, emoji) DO UPDATE SET
    conversation_id = excluded.conversation_id,
    active = excluded.active,
    updated_at_ms = excluded.updated_at_ms
WHERE excluded.updated_at_ms >= message_reactions.updated_at_ms;
