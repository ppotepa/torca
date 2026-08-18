INSERT INTO contact_connectivity_preferences(contact_id, availability_mode, updated_at_ms)
VALUES (?1, ?2, ?3)
ON CONFLICT(contact_id) DO UPDATE SET
    availability_mode = excluded.availability_mode,
    updated_at_ms = excluded.updated_at_ms;
