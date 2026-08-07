UPDATE messages
SET status = 4,
    updated_at_ms = ?2
WHERE message_id = ?1
  AND direction = 1
  AND status = 3;
