-- When a claimed delivery job is returned to Pending (stale recovery or explicit reschedule),
-- a process crash may have left the domain message in Sending. Restore only that transient state
-- to Queued so a recovered outbox record can be claimed and sent again.
CREATE TRIGGER IF NOT EXISTS outbox_requeue_sending_message
AFTER UPDATE OF state ON outbox
WHEN OLD.state = 1 AND NEW.state = 0
BEGIN
    UPDATE messages
    SET status = 0
    WHERE message_id = NEW.message_id
      AND direction = 0
      AND status = 1;
END;
