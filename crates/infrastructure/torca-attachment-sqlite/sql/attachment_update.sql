UPDATE attachments
SET status = ?2,
    updated_at_ms = ?3,
    attempt_count = ?4,
    transfer_offset = ?5,
    content_digest = ?6,
    last_error_code = ?7
WHERE attachment_id = ?1;
