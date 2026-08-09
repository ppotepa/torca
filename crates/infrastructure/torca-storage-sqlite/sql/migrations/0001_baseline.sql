-- SOURCE: 0001_foundation.sql

CREATE TABLE IF NOT EXISTS schema_metadata (
    singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
    version INTEGER NOT NULL
);
INSERT OR IGNORE INTO schema_metadata(singleton, version) VALUES (1, 0);

CREATE TABLE IF NOT EXISTS processed_commands (
    command_id BLOB PRIMARY KEY NOT NULL,
    completed_at_ms INTEGER NOT NULL,
    result_kind TEXT NOT NULL,
    result_payload BLOB
);
CREATE TABLE IF NOT EXISTS runtime_settings (
    setting_key TEXT PRIMARY KEY NOT NULL,
    bool_value INTEGER NOT NULL CHECK (bool_value IN (0, 1)),
    updated_at_ms INTEGER NOT NULL
);
INSERT OR IGNORE INTO runtime_settings(setting_key, bool_value, updated_at_ms)
VALUES ('notifications_enabled', 1, 0);

CREATE TABLE IF NOT EXISTS pairing_sessions (
    session_id BLOB PRIMARY KEY NOT NULL,
    code TEXT NOT NULL,
    role INTEGER NOT NULL,
    state INTEGER NOT NULL,
    expires_at_ms INTEGER NOT NULL,
    local_approved INTEGER NOT NULL CHECK (local_approved IN (0, 1)),
    remote_approved INTEGER NOT NULL CHECK (remote_approved IN (0, 1)),
    remote_identity_id BLOB,
    remote_key_id BLOB,
    remote_key_algorithm INTEGER,
    remote_public_key BLOB,
    remote_key_generation INTEGER,
    remote_onion_address TEXT,
    remote_capability_id BLOB
);


-- SOURCE: 0002_identity.sql

CREATE TABLE IF NOT EXISTS local_identity (
    singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
    identity_id BLOB NOT NULL,
    key_id BLOB NOT NULL,
    key_algorithm INTEGER NOT NULL,
    public_key BLOB NOT NULL,
    key_generation INTEGER NOT NULL,
    display_name TEXT,
    avatar_reference TEXT,
    created_at_ms INTEGER NOT NULL,
    updated_at_ms INTEGER NOT NULL
);


-- SOURCE: 0003_messaging.sql

CREATE TABLE IF NOT EXISTS messages (
    message_id BLOB PRIMARY KEY NOT NULL,
    conversation_id BLOB NOT NULL,
    direction INTEGER NOT NULL,
    status INTEGER NOT NULL,
    body TEXT NOT NULL,
    reply_to_message_id BLOB,
    created_at_ms INTEGER NOT NULL,
    updated_at_ms INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS messages_conversation_created ON messages(conversation_id, created_at_ms, message_id);

CREATE TABLE IF NOT EXISTS outbox (
    message_id BLOB PRIMARY KEY NOT NULL REFERENCES messages(message_id) ON DELETE CASCADE,
    command_id BLOB UNIQUE NOT NULL,
    state INTEGER NOT NULL,
    attempts INTEGER NOT NULL,
    next_attempt_at_ms INTEGER NOT NULL,
    claimed_at_ms INTEGER
);
CREATE INDEX IF NOT EXISTS outbox_due ON outbox(state, next_attempt_at_ms);

CREATE TABLE IF NOT EXISTS inbound_dedup (
    envelope_id BLOB PRIMARY KEY NOT NULL,
    accepted_at_ms INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS receipts (
    receipt_id BLOB PRIMARY KEY NOT NULL,
    message_id BLOB NOT NULL REFERENCES messages(message_id) ON DELETE CASCADE,
    kind INTEGER NOT NULL,
    received_at_ms INTEGER NOT NULL,
    UNIQUE(message_id, kind)
);


-- SOURCE: 0004_contacts_conversations.sql

CREATE TABLE IF NOT EXISTS contacts (
    contact_id BLOB PRIMARY KEY NOT NULL,
    remote_identity_id BLOB NOT NULL,
    remote_key_id BLOB NOT NULL,
    remote_key_algorithm INTEGER NOT NULL,
    remote_public_key BLOB NOT NULL,
    remote_key_generation INTEGER NOT NULL,
    onion_address TEXT NOT NULL,
    capability_id BLOB NOT NULL,
    status INTEGER NOT NULL,
    created_at_ms INTEGER NOT NULL,
    updated_at_ms INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS conversations (
    conversation_id BLOB PRIMARY KEY NOT NULL,
    contact_id BLOB UNIQUE NOT NULL REFERENCES contacts(contact_id) ON DELETE CASCADE,
    status INTEGER NOT NULL,
    created_at_ms INTEGER NOT NULL,
    updated_at_ms INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS conversations_contact ON conversations(contact_id);


-- SOURCE: 0005_message_attempt_count.sql

ALTER TABLE messages ADD COLUMN attempt_count INTEGER NOT NULL DEFAULT 0;


-- SOURCE: 0006_outbound_message_outbox_invariant.sql

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


-- SOURCE: 0007_stale_delivery_requeue.sql

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


-- SOURCE: 0008_delivery_message_state_lifecycle.sql

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


-- SOURCE: 0009_peer_credentials.sql

CREATE TABLE peer_credentials (
    contact_id BLOB PRIMARY KEY NOT NULL REFERENCES contacts(contact_id) ON DELETE CASCADE,
    local_capability_id BLOB NOT NULL UNIQUE,
    secret_handle BLOB NOT NULL UNIQUE
);


-- SOURCE: 0010_receipt_message_lifecycle.sql

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


-- SOURCE: 0011_control_outbox.sql

CREATE TABLE control_outbox (
    job_id BLOB PRIMARY KEY NOT NULL,
    contact_id BLOB NOT NULL REFERENCES contacts(contact_id) ON DELETE CASCADE,
    kind INTEGER NOT NULL,
    payload BLOB NOT NULL,
    state INTEGER NOT NULL DEFAULT 0,
    attempts INTEGER NOT NULL DEFAULT 0,
    next_attempt_at_ms INTEGER NOT NULL,
    claimed_at_ms INTEGER
);

CREATE INDEX control_outbox_due
ON control_outbox(state, next_attempt_at_ms, job_id);


-- SOURCE: 0012_attachments.sql

CREATE TABLE attachments (
    attachment_id BLOB PRIMARY KEY NOT NULL,
    message_id BLOB NOT NULL REFERENCES messages(message_id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    media_type TEXT NOT NULL,
    size_bytes INTEGER NOT NULL,
    status INTEGER NOT NULL,
    created_at_ms INTEGER NOT NULL,
    updated_at_ms INTEGER NOT NULL,
    attempt_count INTEGER NOT NULL DEFAULT 0,
    transfer_offset INTEGER NOT NULL DEFAULT 0,
    content_digest BLOB
);

CREATE INDEX attachments_message
ON attachments(message_id, attachment_id);

CREATE INDEX attachments_transfer_state
ON attachments(status, updated_at_ms, attachment_id);


-- SOURCE: 0013_delivery_attempt_sync.sql

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


-- SOURCE: 0014_failed_message_dead_letters_outbox.sql

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


-- SOURCE: 0015_retry_message_requeues_outbox.sql

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


-- SOURCE: 0016_contact_metadata.sql

CREATE TABLE contact_metadata (
    contact_id BLOB PRIMARY KEY NOT NULL REFERENCES contacts(contact_id) ON DELETE CASCADE,
    display_name TEXT NOT NULL,
    updated_at_ms INTEGER NOT NULL,
    CHECK(length(display_name) BETWEEN 1 AND 256)
);


-- SOURCE: 0017_unique_remote_identity.sql

CREATE UNIQUE INDEX IF NOT EXISTS contacts_remote_identity_unique
ON contacts(remote_identity_id);


-- SOURCE: 0018_contact_verification.sql

CREATE TABLE contact_verification (
    contact_id BLOB PRIMARY KEY NOT NULL REFERENCES contacts(contact_id) ON DELETE CASCADE,
    remote_identity_id BLOB NOT NULL,
    verified_at_ms INTEGER NOT NULL
);
