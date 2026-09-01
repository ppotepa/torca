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
        MAX(created_at_ms) OVER (PARTITION BY conversation_id) AS content_activity_at_ms,
        SUM(CASE WHEN direction = 1 AND status = 3 THEN 1 ELSE 0 END)
            OVER (PARTITION BY conversation_id) AS unread_count,
        ROW_NUMBER() OVER (
            PARTITION BY conversation_id
            ORDER BY created_at_ms DESC, message_id DESC
        ) AS row_number
    FROM messages
)
SELECT
    c.conversation_id,
    COALESCE(r.unread_count, 0) + COALESCE((
        SELECT COUNT(*)
        FROM message_reactions mr
        JOIN messages reacted_message ON reacted_message.message_id = mr.message_id
        WHERE mr.conversation_id = c.conversation_id
          AND mr.active = 1
          AND reacted_message.direction = 0
          AND mr.updated_at_ms > COALESCE((
              SELECT read_through_ms
              FROM conversation_read_state crs
              WHERE crs.conversation_id = c.conversation_id
          ), 0)
    ), 0),
    COALESCE(r.content_activity_at_ms, c.updated_at_ms),
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
ORDER BY COALESCE(r.content_activity_at_ms, c.updated_at_ms) DESC, c.conversation_id DESC;
