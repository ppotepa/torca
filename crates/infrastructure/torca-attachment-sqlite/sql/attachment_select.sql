SELECT message_id, name, media_type, size_bytes, status,
       created_at_ms, updated_at_ms, attempt_count, transfer_offset, content_digest
FROM attachments
WHERE attachment_id = ?1;
