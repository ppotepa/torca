UPDATE control_outbox
SET state = 0,
    next_attempt_at_ms = ?2,
    claimed_at_ms = NULL
WHERE job_id = ?1 AND state = 1;
