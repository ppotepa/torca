UPDATE contacts
SET status = ?2, updated_at_ms = ?3
WHERE contact_id = ?1 AND status = ?4;
