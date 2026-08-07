-- Keep the message aggregate's retry count aligned with the durable outbox claim that owns the
-- actual transport attempt. Existing state triggers continue to own Queued/Sending/Sent/Failed.
CREATE TRIGGER IF NOT EXISTS outbox_claim_syncs_message_attempt
AFTER UPDATE OF state ON outbox
WHEN OLD.state = 0 AND NEW.state = 1
BEGIN
    UPDATE messages
    SET attempt_count = NEW.attempts + 1,
        updated_at_ms = COALESCE(NEW.claimed_at_ms, updated_at_ms)
    WHERE message_id = NEW.message_id
      AND direction = 0;
END;

CREATE TRIGGER IF NOT EXISTS outbox_reschedule_syncs_message_time
AFTER UPDATE OF state ON outbox
WHEN OLD.state = 1 AND NEW.state = 0
BEGIN
    UPDATE messages
    SET attempt_count = NEW.attempts,
        updated_at_ms = MAX(updated_at_ms, COALESCE(OLD.claimed_at_ms, updated_at_ms))
    WHERE message_id = NEW.message_id
      AND direction = 0;
END;

CREATE TRIGGER IF NOT EXISTS outbox_complete_syncs_message_attempt
AFTER UPDATE OF state ON outbox
WHEN OLD.state = 1 AND NEW.state = 2
BEGIN
    UPDATE messages
    SET attempt_count = NEW.attempts,
        updated_at_ms = MAX(updated_at_ms, COALESCE(OLD.claimed_at_ms, updated_at_ms))
    WHERE message_id = NEW.message_id
      AND direction = 0;
END;

CREATE TRIGGER IF NOT EXISTS outbox_dead_letter_syncs_message_attempt
AFTER UPDATE OF state ON outbox
WHEN OLD.state IN (0, 1) AND NEW.state = 3
BEGIN
    UPDATE messages
    SET attempt_count = NEW.attempts,
        updated_at_ms = MAX(updated_at_ms, COALESCE(OLD.claimed_at_ms, updated_at_ms))
    WHERE message_id = NEW.message_id
      AND direction = 0;
END;
