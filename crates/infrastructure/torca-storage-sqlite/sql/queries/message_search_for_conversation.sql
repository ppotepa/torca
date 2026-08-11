SELECT message_id, direction, status, body, reply_to_message_id,
created_at_ms, updated_at_ms, attempt_count, sent_at_ms, delivered_at_ms, read_at_ms
FROM messages
WHERE conversation_id = ?1
  AND instr(lower(body), lower(?2)) > 0
ORDER BY created_at_ms DESC, message_id DESC
LIMIT ?3;
