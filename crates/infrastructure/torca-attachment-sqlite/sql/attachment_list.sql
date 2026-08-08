SELECT attachment_id, message_id, name, media_type, size_bytes, status,
       created_at_ms, updated_at_ms, attempt_count, transfer_offset
FROM attachments
ORDER BY updated_at_ms, attachment_id;
