CREATE TABLE IF NOT EXISTS notification_outbox (
    cursor INTEGER PRIMARY KEY AUTOINCREMENT,
    event_id TEXT NOT NULL UNIQUE,
    payload TEXT NOT NULL,
    created_at_ms INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS notification_outbox_created_at
    ON notification_outbox(created_at_ms);
