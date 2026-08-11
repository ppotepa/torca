SELECT a.attachment_id, a.message_id, a.name, a.media_type, a.size_bytes, a.status,
       a.created_at_ms, a.updated_at_ms, a.attempt_count, a.transfer_offset,
       m.direction
FROM attachments AS a
JOIN messages AS m ON m.message_id = a.message_id
ORDER BY a.updated_at_ms, a.attachment_id;
