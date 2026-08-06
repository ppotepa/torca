INSERT INTO messages (
    message_id, conversation_id, direction, status, body, reply_to_message_id,
    created_at_ms, updated_at_ms, attempt_count
) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9);
