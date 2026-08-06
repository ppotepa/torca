UPDATE outbox SET state = 0, claimed_at_ms = NULL WHERE state = 1 AND claimed_at_ms <= ?1;
