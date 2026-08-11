SELECT
    operation_id,
    resource_id,
    operation_kind,
    text_payload,
    binary_payload,
    attempts,
    next_attempt_at_ms,
    created_at_ms,
    last_error
FROM pending_operations
WHERE next_attempt_at_ms <= ?1
ORDER BY next_attempt_at_ms, created_at_ms, operation_id
LIMIT ?2;
