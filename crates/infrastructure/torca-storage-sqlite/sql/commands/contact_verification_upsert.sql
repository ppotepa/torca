INSERT OR REPLACE INTO contact_verification(contact_id, remote_identity_id, verified_at_ms)
SELECT contact_id, remote_identity_id, ?2
FROM contacts
WHERE contact_id = ?1;
