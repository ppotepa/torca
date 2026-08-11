SELECT message_id, direction, status, body, reply_to_message_id,
created_at_ms, updated_at_ms, attempt_count, sent_at_ms, delivered_at_ms, read_at_ms
FROM messages
WHERE conversation_id = ?1
ORDER BY created_at_ms, message_id;
