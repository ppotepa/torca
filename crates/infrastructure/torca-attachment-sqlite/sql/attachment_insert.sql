INSERT INTO attachments(
    attachment_id, message_id, name, media_type, size_bytes, status,
    created_at_ms, updated_at_ms, attempt_count, transfer_offset, content_digest, last_error_code
) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12);
