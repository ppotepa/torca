INSERT INTO contacts(
    contact_id, remote_identity_id, remote_key_id, remote_key_algorithm,
    remote_public_key, remote_key_generation, onion_address, capability_id,
    status, created_at_ms, updated_at_ms
) VALUES (?1, ?2, ?3, 1, ?4, 1, 'example.onion:443', ?5, 1, 1, 1);
