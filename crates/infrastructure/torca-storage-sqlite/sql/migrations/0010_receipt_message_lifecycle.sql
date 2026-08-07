-- Receipt rows are the durable source of truth for peer-observed delivery/read state.
-- Apply the matching monotonic message transition in the same SQLite statement transaction.
CREATE TRIGGER IF NOT EXISTS receipt_marks_message_delivered
AFTER INSERT ON receipts
WHEN NEW.kind = 0
BEGIN
    UPDATE messages
    SET status = 3,
        updated_at_ms = MAX(updated_at_ms, NEW.received_at_ms)
    WHERE message_id = NEW.message_id
      AND direction = 0
      AND status = 2;
END;

CREATE TRIGGER IF NOT EXISTS receipt_marks_message_read
AFTER INSERT ON receipts
WHEN NEW.kind = 1
BEGIN
    UPDATE messages
    SET status = 4,
        updated_at_ms = MAX(updated_at_ms, NEW.received_at_ms)
    WHERE message_id = NEW.message_id
      AND direction = 0
      AND status IN (2, 3);
END;
