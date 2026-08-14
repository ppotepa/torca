INSERT INTO radio_preferences(contact_id, local_enabled, revision, changed_at_ms)
VALUES (?1, ?2, ?3, ?4)
ON CONFLICT(contact_id) DO UPDATE SET
    local_enabled = excluded.local_enabled,
    revision = excluded.revision,
    changed_at_ms = excluded.changed_at_ms;
