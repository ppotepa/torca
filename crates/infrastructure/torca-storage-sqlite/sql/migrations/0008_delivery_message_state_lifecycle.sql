CREATE TRIGGER IF NOT EXISTS outbox_claim_marks_message_sending
AFTER UPDATE OF state ON outbox
WHEN OLD.state = 0 AND NEW.state = 1
BEGIN
    UPDATE messages
    SET status = 1,
        updated_at_ms = COALESCE(NEW.claimed_at_ms, updated_at_ms)
    WHERE message_id = NEW.message_id
      AND direction = 0
      AND status = 0;
END;

CREATE TRIGGER IF NOT EXISTS outbox_complete_marks_message_sent
AFTER UPDATE OF state ON outbox
WHEN OLD.state = 1 AND NEW.state = 2
BEGIN
    UPDATE messages
    SET status = 2
    WHERE message_id = NEW.message_id
      AND direction = 0
      AND status = 1;
END;

CREATE TRIGGER IF NOT EXISTS outbox_terminal_marks_message_failed
AFTER UPDATE OF state ON outbox
WHEN NEW.state = 3 AND OLD.state IN (0, 1)
BEGIN
    UPDATE messages
    SET status = 5
    WHERE message_id = NEW.message_id
      AND direction = 0
      AND status IN (0, 1);
END;
