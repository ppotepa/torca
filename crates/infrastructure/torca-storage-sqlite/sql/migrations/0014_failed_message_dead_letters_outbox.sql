-- A terminal outbound message failure must release durable outbox ownership as one database-side
-- invariant. This also covers attachment preparation failures that occur before any network claim.
CREATE TRIGGER IF NOT EXISTS outbound_message_failure_dead_letters_outbox
AFTER UPDATE OF status ON messages
WHEN NEW.direction = 0 AND NEW.status = 5 AND OLD.status <> 5
BEGIN
    UPDATE outbox
    SET state = 3,
        claimed_at_ms = NULL
    WHERE message_id = NEW.message_id
      AND state IN (0, 1);
END;
