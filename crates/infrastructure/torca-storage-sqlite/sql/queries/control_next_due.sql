SELECT MIN(next_attempt_at_ms)
FROM control_outbox
WHERE state = 0;
