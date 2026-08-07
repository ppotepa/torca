INSERT INTO control_outbox(
    job_id, contact_id, kind, payload, state, attempts, next_attempt_at_ms, claimed_at_ms
) VALUES (?1, ?2, ?3, ?4, 0, 0, ?5, NULL);
