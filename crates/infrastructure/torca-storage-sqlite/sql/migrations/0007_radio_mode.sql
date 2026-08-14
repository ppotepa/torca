CREATE TABLE radio_preferences (
    contact_id BLOB PRIMARY KEY NOT NULL REFERENCES contacts(contact_id) ON DELETE CASCADE,
    local_enabled INTEGER NOT NULL CHECK(local_enabled IN (0, 1)),
    revision INTEGER NOT NULL CHECK(revision >= 0),
    changed_at_ms INTEGER NOT NULL
);

CREATE UNIQUE INDEX radio_single_active_contact
ON radio_preferences(local_enabled)
WHERE local_enabled = 1;

CREATE TABLE conversation_events (
    event_id BLOB PRIMARY KEY NOT NULL,
    conversation_id BLOB NOT NULL REFERENCES conversations(conversation_id) ON DELETE CASCADE,
    kind INTEGER NOT NULL,
    actor INTEGER NOT NULL,
    correlation_id BLOB NOT NULL,
    occurred_at_ms INTEGER NOT NULL
);

CREATE INDEX conversation_events_timeline
ON conversation_events(conversation_id, occurred_at_ms DESC, event_id DESC);
