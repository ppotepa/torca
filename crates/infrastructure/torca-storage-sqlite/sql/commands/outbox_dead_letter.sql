UPDATE outbox
SET state = 3,
    attempts = attempts + CASE WHEN state = 1 THEN 1 ELSE 0 END,
    claimed_at_ms = NULL
WHERE message_id = ?1 AND state IN (0, 1);
