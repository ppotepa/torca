SELECT c.contact_id, m.message_id
FROM conversations c
JOIN messages m ON m.conversation_id = c.conversation_id
WHERE c.conversation_id = ?1
  AND m.direction = 1
  AND m.status = 3
ORDER BY m.created_at_ms, m.message_id;
