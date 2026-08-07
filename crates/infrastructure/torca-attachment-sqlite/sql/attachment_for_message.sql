SELECT attachment_id, name, media_type, size_bytes, status,
       created_at_ms, updated_at_ms, attempt_count, transfer_offset, content_digest
FROM attachments
WHERE message_id = ?1
ORDER BY created_at_ms, attachment_id;
