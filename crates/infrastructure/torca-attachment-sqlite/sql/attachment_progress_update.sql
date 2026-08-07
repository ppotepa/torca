UPDATE attachments
SET transfer_offset = ?2,
    content_digest = COALESCE(?3, content_digest),
    updated_at_ms = ?4
WHERE attachment_id = ?1;
