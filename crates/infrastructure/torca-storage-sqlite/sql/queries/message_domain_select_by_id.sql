SELECT conversation_id, direction, status, body, reply_to_message_id,
       created_at_ms, updated_at_ms, attempt_count
FROM messages
WHERE message_id = ?1;
