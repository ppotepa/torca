SELECT message_id, direction, status, body, reply_to_message_id,
       created_at_ms, updated_at_ms, attempt_count
FROM messages
WHERE conversation_id = ?1
  AND (
    ?2 IS NULL
    OR created_at_ms < ?2
    OR (created_at_ms = ?2 AND message_id < ?3)
  )
ORDER BY created_at_ms DESC, message_id DESC
LIMIT ?4;
