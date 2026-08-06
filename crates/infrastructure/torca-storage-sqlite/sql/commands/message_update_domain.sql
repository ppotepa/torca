UPDATE messages SET
    conversation_id = ?2,
    direction = ?3,
    status = ?4,
    body = ?5,
    reply_to_message_id = ?6,
    created_at_ms = ?7,
    updated_at_ms = ?8,
    attempt_count = ?9
WHERE message_id = ?1;
