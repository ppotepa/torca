CREATE TABLE control_outbox (
    job_id BLOB PRIMARY KEY NOT NULL,
    contact_id BLOB NOT NULL REFERENCES contacts(contact_id) ON DELETE CASCADE,
    kind INTEGER NOT NULL,
    payload BLOB NOT NULL,
    state INTEGER NOT NULL DEFAULT 0,
    attempts INTEGER NOT NULL DEFAULT 0,
    next_attempt_at_ms INTEGER NOT NULL,
    claimed_at_ms INTEGER
);

CREATE INDEX control_outbox_due
ON control_outbox(state, next_attempt_at_ms, job_id);
