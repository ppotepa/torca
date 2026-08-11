UPDATE pending_operations
SET
    attempts = ?2,
    next_attempt_at_ms = ?3,
    last_error = ?4
WHERE operation_id = ?1;
