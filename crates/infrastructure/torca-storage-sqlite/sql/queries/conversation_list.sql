SELECT conversation_id, contact_id, status, created_at_ms, updated_at_ms
FROM conversations
ORDER BY created_at_ms, conversation_id;
