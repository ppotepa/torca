SELECT message_id, conversation_id, direction, status, body, reply_to_message_id,
       created_at_ms, updated_at_ms, attempt_count
FROM messages
ORDER BY created_at_ms, message_id;
