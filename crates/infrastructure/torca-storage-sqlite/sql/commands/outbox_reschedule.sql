UPDATE outbox SET state = 0, attempts = ?2, next_attempt_at_ms = ?3, claimed_at_ms = NULL
WHERE message_id = ?1 AND state = 1;
