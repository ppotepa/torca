INSERT INTO outbox(message_id, command_id, state, attempts, next_attempt_at_ms, claimed_at_ms)
VALUES (?1, ?2, 0, 0, ?3, NULL);
