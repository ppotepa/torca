-- Every newly inserted outbound queued message must become durable work in the same SQLite transaction.
-- direction=0 => outbound; status=0 => queued. command_id intentionally reuses the stable message_id
-- so retries and repeated application commands cannot create a second logical delivery job.
CREATE TRIGGER IF NOT EXISTS messages_enqueue_outbound
AFTER INSERT ON messages
WHEN NEW.direction = 0 AND NEW.status = 0
BEGIN
    INSERT OR IGNORE INTO outbox(
        message_id,
        command_id,
        state,
        attempts,
        next_attempt_at_ms,
        claimed_at_ms
    )
    VALUES (
        NEW.message_id,
        NEW.message_id,
        0,
        0,
        NEW.created_at_ms,
        NULL
    );
END;
