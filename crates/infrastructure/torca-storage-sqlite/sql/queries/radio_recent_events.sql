SELECT
    recent.event_id,
    recent.contact_id,
    recent.kind,
    recent.actor,
    recent.correlation_id,
    recent.occurred_at_ms
FROM (
    SELECT
        ce.event_id,
        c.contact_id,
        ce.kind,
        ce.actor,
        ce.correlation_id,
        ce.occurred_at_ms
    FROM conversation_events ce
    JOIN conversations c ON c.conversation_id = ce.conversation_id
    ORDER BY ce.occurred_at_ms DESC, ce.event_id DESC
    LIMIT ?1
) recent
ORDER BY recent.occurred_at_ms ASC, recent.event_id ASC;
