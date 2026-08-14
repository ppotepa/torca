UPDATE conversations
SET status = ?3,
    updated_at_ms = ?4
WHERE conversation_id = ?1
  AND contact_id = ?2;
