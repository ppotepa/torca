INSERT INTO contact_metadata (contact_id, display_name, updated_at_ms)
SELECT ?1, ?2, ?3
WHERE EXISTS (
    SELECT 1 FROM contacts WHERE contact_id = ?1 AND status != 2
)
ON CONFLICT(contact_id) DO UPDATE SET
    display_name = excluded.display_name,
    updated_at_ms = excluded.updated_at_ms;
