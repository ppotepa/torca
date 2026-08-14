SELECT message_id, conversation_id, actor_id, emoji, active, updated_at_ms
FROM message_reactions
WHERE conversation_id = ?1 AND active = 1
ORDER BY updated_at_ms ASC;
