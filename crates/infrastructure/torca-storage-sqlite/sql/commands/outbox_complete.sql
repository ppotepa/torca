UPDATE outbox SET state = 2, claimed_at_ms = NULL WHERE message_id = ?1 AND state = 1;
