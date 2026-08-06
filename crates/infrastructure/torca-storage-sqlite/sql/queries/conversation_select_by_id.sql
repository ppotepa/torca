SELECT contact_id, status, created_at_ms, updated_at_ms
FROM conversations
WHERE conversation_id = ?1;
