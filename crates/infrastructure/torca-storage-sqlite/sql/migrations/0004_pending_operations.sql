CREATE TABLE pending_operations (
    operation_id TEXT PRIMARY KEY NOT NULL,
    resource_id TEXT NOT NULL,
    operation_kind TEXT NOT NULL,
    text_payload TEXT,
    binary_payload BLOB,
    attempts INTEGER NOT NULL DEFAULT 0,
    next_attempt_at_ms INTEGER NOT NULL,
    created_at_ms INTEGER NOT NULL,
    last_error TEXT
);

CREATE INDEX pending_operations_due
ON pending_operations(next_attempt_at_ms, created_at_ms, operation_id);
