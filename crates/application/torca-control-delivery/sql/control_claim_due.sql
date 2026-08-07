WITH due AS (
    SELECT job_id
    FROM control_outbox
    WHERE state = 0 AND next_attempt_at_ms <= ?1
    ORDER BY next_attempt_at_ms, job_id
    LIMIT ?2
)
UPDATE control_outbox
SET state = 1,
    attempts = attempts + 1,
    claimed_at_ms = ?1
WHERE state = 0 AND job_id IN (SELECT job_id FROM due)
RETURNING job_id, contact_id, kind, payload, attempts, next_attempt_at_ms;
