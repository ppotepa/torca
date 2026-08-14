INSERT OR IGNORE INTO conversation_events(
    event_id,
    conversation_id,
    kind,
    actor,
    correlation_id,
    occurred_at_ms
)
SELECT ?1, conversation_id, ?3, ?4, ?5, ?6
FROM conversations
WHERE contact_id = ?2;
