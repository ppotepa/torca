SELECT message_id, command_id, attempts, next_attempt_at_ms
FROM outbox
WHERE state = 0 AND next_attempt_at_ms <= ?1
ORDER BY next_attempt_at_ms, message_id
LIMIT ?2;
