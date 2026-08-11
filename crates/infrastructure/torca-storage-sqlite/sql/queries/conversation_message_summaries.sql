WITH ranked AS (
    SELECT
        conversation_id,
        message_id,
        direction,
        status,
        body,
        reply_to_message_id,
        created_at_ms,
        updated_at_ms,
attempt_count,
sent_at_ms,
delivered_at_ms,
read_at_ms,
        SUM(CASE WHEN direction = 1 AND status = 3 THEN 1 ELSE 0 END)
            OVER (PARTITION BY conversation_id) AS unread_count,
        ROW_NUMBER() OVER (
            PARTITION BY conversation_id
            ORDER BY updated_at_ms DESC, created_at_ms DESC, message_id DESC
        ) AS row_number
    FROM messages
)
SELECT
    c.conversation_id,
    COALESCE(r.unread_count, 0),
    COALESCE(r.updated_at_ms, c.updated_at_ms),
    r.message_id,
    r.direction,
    r.status,
    r.body,
    r.reply_to_message_id,
    r.created_at_ms,
    r.updated_at_ms,
    r.attempt_count,
    r.sent_at_ms,
    r.delivered_at_ms,
    r.read_at_ms
FROM conversations c
LEFT JOIN ranked r
    ON r.conversation_id = c.conversation_id
   AND r.row_number = 1
ORDER BY COALESCE(r.updated_at_ms, c.updated_at_ms) DESC, c.conversation_id DESC;
