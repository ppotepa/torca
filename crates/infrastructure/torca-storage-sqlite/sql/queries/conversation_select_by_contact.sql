SELECT conversation_id, status, created_at_ms, updated_at_ms
FROM conversations
WHERE contact_id = ?1;
