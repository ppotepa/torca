INSERT INTO messages (
    message_id, conversation_id, direction, status, body, reply_to_message_id,
created_at_ms, updated_at_ms, attempt_count, sent_at_ms, delivered_at_ms, read_at_ms
) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12);
