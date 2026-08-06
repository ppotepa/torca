SELECT remote_identity_id, remote_key_id, remote_key_algorithm, remote_public_key,
       remote_key_generation, onion_address, capability_id, status, created_at_ms, updated_at_ms
FROM contacts
WHERE contact_id = ?1;
