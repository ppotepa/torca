INSERT OR IGNORE INTO control_outbox(
    job_id, contact_id, kind, payload, state, attempts, next_attempt_at_ms, claimed_at_ms
) VALUES (?1, ?2, 1, ?3, 0, 0, ?4, NULL);
