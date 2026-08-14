SELECT MIN(next_attempt_at_ms)
FROM outbox
WHERE state = 0;
