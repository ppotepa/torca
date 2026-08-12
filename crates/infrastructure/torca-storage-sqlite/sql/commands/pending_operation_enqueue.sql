INSERT INTO pending_operations (
    operation_id,
    resource_id,
    operation_kind,
    text_payload,
    binary_payload,
    attempts,
    next_attempt_at_ms,
    created_at_ms,
    last_error
) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
ON CONFLICT(operation_id) DO UPDATE SET
    text_payload = excluded.text_payload,
    binary_payload = excluded.binary_payload,
    attempts = 0,
    next_attempt_at_ms = excluded.next_attempt_at_ms,
    last_error = NULL;
