CREATE TRIGGER IF NOT EXISTS message_retry_requeues_outbox
AFTER UPDATE OF status ON messages
WHEN OLD.status = 5 AND NEW.status = 0
BEGIN
    UPDATE outbox
    SET state = 0,
        next_attempt_at_ms = NEW.updated_at_ms,
        claimed_at_ms = NULL
    WHERE message_id = NEW.message_id
      AND state = 3;
END;
