WITH due AS (
    SELECT message_id
    FROM outbox
    WHERE state = 0 AND next_attempt_at_ms <= ?1
    ORDER BY next_attempt_at_ms, message_id
    LIMIT ?2
)
UPDATE outbox
SET state = 1, claimed_at_ms = ?1
WHERE state = 0 AND message_id IN (SELECT message_id FROM due)
RETURNING message_id, command_id, attempts, next_attempt_at_ms, claimed_at_ms;
