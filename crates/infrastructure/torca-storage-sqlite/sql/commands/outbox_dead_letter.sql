UPDATE outbox SET state = 3, claimed_at_ms = NULL WHERE message_id = ?1 AND state IN (0, 1);
